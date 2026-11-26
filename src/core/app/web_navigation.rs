//! Browser location is an external projection of the ordinary App's root.
//! HAL-228 owns navigation state and HAL-231 owns candidate authorization.
//! The existing standalone frame binding participates in every commit.

use super::{AppContext, UiWakeHandle};
use crate::compositor::Compositor;
use crate::core::event::InteractionDomainId;
use crate::core::navigation_application::{
    NavigationApplicationCoordinator, StandaloneNavigationSnapshot,
    StandaloneNavigationSnapshotResult,
};
use crate::core::navigation_orchestrator::NavigationSurfaceOrchestrator;
use crate::core::widget_tree::WidgetMountToken;
use crate::model::navigation_host::{
    NavigationAuthorizationResult, NavigationControlProjection, NavigationEntry, NavigationGuard,
    NavigationHost, NavigationIntent, NavigationMechanicsSnapshot, NavigationNodeId,
};
use crate::platform::web::{BrowserHistory, BrowserTraversal};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::rc::{Rc, Weak};
use winit::platform::web::WindowExtWebSys;

const MAX_ROUTES: usize = 64;
const MAX_PATH_BYTES: usize = 256;
const MAX_QUEUED_INTENTS: usize = 32;

/// Failure to declare a finite set of distinct public routes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WebRouteError {
    #[error("invalid public path")]
    InvalidPath,
    #[error("duplicate route or public path")]
    Duplicate,
    #[error("public route capacity exceeded")]
    Capacity,
    #[error("the root route has no public path")]
    MissingRoot,
}

/// A context-menu override must name an already declared public route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WebContextMenuPolicyError {
    #[error("the context-menu policy route has no public path")]
    UnmappedRoute,
}

/// The application's public path mapping for its ordinary browser App.
///
/// Declare only static public routes. Paths are canonical origin-relative paths,
/// without query, fragment, or encoded segments. The Screen projects committed
/// navigation and keeps any private draft in its own application state.
pub struct WebNavigation {
    root: NavigationNodeId,
    routes: BTreeMap<NavigationNodeId, &'static str>,
    context_menu_enabled: BTreeMap<NavigationNodeId, bool>,
    guard: Option<Box<dyn NavigationGuard>>,
}

impl WebNavigation {
    pub fn new(
        root: NavigationNodeId,
        routes: impl IntoIterator<Item = (NavigationNodeId, &'static str)>,
    ) -> Result<Self, WebRouteError> {
        let mut map = BTreeMap::new();
        for (node, path) in routes {
            if map.len() == MAX_ROUTES {
                return Err(WebRouteError::Capacity);
            }
            if !valid_public_path(path) {
                return Err(WebRouteError::InvalidPath);
            }
            if map.contains_key(&node) || map.values().any(|existing| *existing == path) {
                return Err(WebRouteError::Duplicate);
            }
            map.insert(node, path);
        }
        if !map.contains_key(&root) {
            return Err(WebRouteError::MissingRoot);
        }
        Ok(Self {
            root,
            routes: map,
            context_menu_enabled: BTreeMap::new(),
            guard: None,
        })
    }

    /// Evaluate the ordinary NavigationHost guard for app and browser intents.
    /// The guard can read application-owned state without exposing it to Web.
    pub fn with_guard(mut self, guard: impl NavigationGuard + 'static) -> Self {
        self.guard = Some(Box::new(guard));
        self
    }

    /// Select browser context-menu availability for a declared route (HAL-018 §19.1).
    /// Later selections replace earlier ones for the same route. Unselected routes
    /// use `AppConfig::with_web_context_menu_enabled`'s startup value. Only the
    /// committed navigation projection changes the canvas policy.
    pub fn with_context_menu_enabled_for(
        mut self,
        route: NavigationNodeId,
        enabled: bool,
    ) -> Result<Self, WebContextMenuPolicyError> {
        if !self.routes.contains_key(&route) {
            return Err(WebContextMenuPolicyError::UnmappedRoute);
        }
        self.context_menu_enabled.insert(route, enabled);
        Ok(self)
    }

    pub(super) fn make_host(&mut self) -> NavigationHost {
        let mut host = NavigationHost::new(self.root);
        if let Some(guard) = self.guard.take() {
            host.set_guard(guard);
        }
        host
    }
}

fn valid_public_path(path: &str) -> bool {
    path.len() <= MAX_PATH_BYTES
        && (path == "/"
            || path.strip_prefix('/').is_some_and(|rest| {
                rest.split('/').all(|part| {
                    !part.is_empty()
                        && part
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
                })
            }))
}

/// An application intent could not be queued for this binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WebNavigationRequestError {
    #[error("the browser navigation binding is unavailable")]
    Unavailable,
    #[error("the intent contains a destination outside the public route map")]
    Unmapped,
    #[error("the navigation intent queue is full")]
    Busy,
}

struct IntentMailbox {
    mount: WidgetMountToken,
    routes: BTreeMap<NavigationNodeId, &'static str>,
    wake: UiWakeHandle,
    open: Cell<bool>,
    intents: RefCell<VecDeque<NavigationIntent>>,
}

impl IntentMailbox {
    fn public_path(&self, entry: &NavigationEntry) -> Option<&'static str> {
        if entry.restoration_key().is_some() || !entry.parameters().is_empty() {
            return None;
        }
        self.routes.get(&entry.node_id()).copied()
    }

    fn admits(&self, intent: &NavigationIntent) -> bool {
        match intent {
            NavigationIntent::NavigateTo(entry) => self.public_path(entry).is_some(),
            NavigationIntent::NavigatePath(path) => {
                !path.is_empty()
                    && path.len() <= self.routes.len()
                    && path.iter().all(|entry| self.public_path(entry).is_some())
            }
            _ => true,
        }
    }
}

/// A typed capability to queue intents for one mounted App root. Clones carry
/// the same binding and become unavailable when that root retires.
#[derive(Clone)]
pub struct WebNavigationIntentSink(Weak<IntentMailbox>);

impl WebNavigationIntentSink {
    /// Queue an intent through the ordinary App loop. Success reports queueing,
    /// with the committed result exposed through the App's read-only projection.
    pub fn request(&self, intent: NavigationIntent) -> Result<(), WebNavigationRequestError> {
        use WebNavigationRequestError::{Busy, Unavailable, Unmapped};

        let mailbox = self.0.upgrade().ok_or(Unavailable)?;
        if !mailbox.open.get() || !mailbox.mount.is_live() {
            return Err(Unavailable);
        }
        if !mailbox.admits(&intent) {
            return Err(Unmapped);
        }
        let mut intents = mailbox.intents.borrow_mut();
        if intents.len() >= MAX_QUEUED_INTENTS {
            return Err(Busy);
        }
        intents.push_back(intent);
        if mailbox.wake.wake().is_err() {
            intents.pop_back();
            return Err(Unavailable);
        }
        Ok(())
    }
}

pub(super) struct WebNavigationBinding {
    projection: NavigationControlProjection,
    mailbox: Rc<IntentMailbox>,
}

impl AppContext {
    /// The committed route and history availability for this App's Web root.
    pub fn web_navigation_projection(&self) -> Option<&NavigationControlProjection> {
        Some(&self.web_navigation.as_ref()?.projection)
    }

    /// Look up the declared public path of an exact application route entry.
    pub fn web_navigation_path(&self, entry: &NavigationEntry) -> Option<&'static str> {
        self.web_navigation.as_ref()?.mailbox.public_path(entry)
    }

    /// Capture this in application callbacks instead of mutating a host.
    pub fn web_navigation_intents(&self) -> Option<WebNavigationIntentSink> {
        Some(WebNavigationIntentSink(Rc::downgrade(
            &self.web_navigation.as_ref()?.mailbox,
        )))
    }
}

enum PendingTraversal {
    Application {
        target: usize,
        intent: NavigationIntent,
    },
    Correction {
        target: usize,
    },
}

pub(super) struct AppWebNavigation {
    mailbox: Rc<IntentMailbox>,
    context_menu_enabled: BTreeMap<NavigationNodeId, bool>,
    default_context_menu_enabled: bool,
    history: Option<BrowserHistory>,
    pending: Option<PendingTraversal>,
}

impl AppWebNavigation {
    pub(super) fn bind(
        config: WebNavigation,
        window: &winit::window::Window,
        default_context_menu_enabled: bool,
        context: &mut AppContext,
        mount: WidgetMountToken,
        coordinator: &NavigationApplicationCoordinator,
        orchestrator: &mut NavigationSurfaceOrchestrator,
        compositor: &mut Compositor,
    ) -> Self {
        let wake = context
            .ui_waker()
            .expect("ordinary App has a UI wake handle");
        let mailbox = Rc::new(IntentMailbox {
            mount,
            routes: config.routes,
            wake: wake.clone(),
            open: Cell::new(false),
            intents: RefCell::new(VecDeque::new()),
        });
        let mut this = Self {
            mailbox,
            context_menu_enabled: config.context_menu_enabled,
            default_context_menu_enabled,
            history: BrowserHistory::install(window, wake),
            pending: None,
        };
        if !this.initialize(coordinator, orchestrator, compositor) {
            this.retire();
            tracing::warn!("public browser navigation could not bind this entry");
        }
        let projection = orchestrator.nav_host().control_projection();
        if this.mailbox.open.get() {
            this.apply_context_menu_policy(window, &projection);
        }
        context.web_navigation = Some(WebNavigationBinding {
            projection,
            mailbox: Rc::clone(&this.mailbox),
        });
        this
    }

    fn initialize(
        &mut self,
        coordinator: &NavigationApplicationCoordinator,
        orchestrator: &mut NavigationSurfaceOrchestrator,
        compositor: &mut Compositor,
    ) -> bool {
        let Some(history) = self.history.as_mut() else {
            return false;
        };
        let Some(path) = history.path() else {
            return false;
        };
        let Some(node) = self
            .mailbox
            .routes
            .iter()
            .find_map(|(node, public)| (*public == path).then_some(*node))
        else {
            return false;
        };
        if !history.can_bind_initial() {
            return false;
        }
        let Some(snapshot) = snapshot(coordinator, orchestrator) else {
            return false;
        };
        let initial = orchestrator
            .nav_host()
            .authorize_initial_route(snapshot.host_snapshot(), node.into());
        let mut projected = false;
        if let NavigationAuthorizationResult::Authorized(authorized) = initial {
            if let Some(path) = self
                .mailbox
                .public_path(authorized.candidate().to().current())
                && authorized
                    .candidate()
                    .to()
                    .path()
                    .iter()
                    .all(|entry| self.mailbox.public_path(entry).is_some())
                && let Ok(prepared) = coordinator.prepare_standalone_navigation(
                    orchestrator,
                    compositor,
                    &snapshot,
                    authorized,
                    None,
                )
            {
                if !history.initialize(path) {
                    return false;
                }
                prepared.commit();
                projected = true;
            }
        }
        if !projected {
            // A refused deep link starts at the application's existing root
            // within this initial browser slot.
            let Some(path) = self
                .mailbox
                .public_path(&orchestrator.nav_host().current_entry())
            else {
                return false;
            };
            if !history.initialize(path) {
                return false;
            }
        }
        self.mailbox.open.set(true);
        true
    }

    pub(super) fn process(
        &mut self,
        window: &winit::window::Window,
        context: &mut AppContext,
        coordinator: &NavigationApplicationCoordinator,
        orchestrator: &mut NavigationSurfaceOrchestrator,
        compositor: &mut Compositor,
    ) -> bool {
        if !self.mailbox.mount.is_live() || context.exit_requested() {
            self.retire();
            return false;
        }
        let committed = self.settle(coordinator, orchestrator, compositor);
        if committed {
            let binding = context
                .web_navigation
                .as_mut()
                .expect("bound Web navigation projection");
            binding.projection = orchestrator.nav_host().control_projection();
            self.apply_context_menu_policy(window, &binding.projection);
        }
        if self.pending.is_none() && !self.mailbox.intents.borrow().is_empty() {
            if self.mailbox.wake.wake().is_err() {
                self.retire();
            }
        }
        committed
    }

    fn apply_context_menu_policy(
        &self,
        window: &winit::window::Window,
        projection: &NavigationControlProjection,
    ) {
        window.set_context_menu_enabled(
            self.context_menu_enabled
                .get(&projection.current.node_id())
                .copied()
                .unwrap_or(self.default_context_menu_enabled),
        );
    }

    fn settle(
        &mut self,
        coordinator: &NavigationApplicationCoordinator,
        orchestrator: &mut NavigationSurfaceOrchestrator,
        compositor: &mut Compositor,
    ) -> bool {
        let Some(history) = self.history.as_mut() else {
            return false;
        };
        let Some(snapshot) = snapshot(coordinator, orchestrator) else {
            return false;
        };
        let host = snapshot.host_snapshot();
        let current = host.history_cursor();
        match history.take_traversal() {
            BrowserTraversal::Foreign => self.retire(),
            BrowserTraversal::Owned(target) => {
                let Some(path) = history_path(&self.mailbox, host, target) else {
                    self.retire();
                    return false;
                };
                if history.path().as_deref() != Some(path) {
                    self.retire();
                    return false;
                }
                let pending = self.pending.take();
                if let Some(PendingTraversal::Correction { target: expected }) = pending {
                    if expected != target || target != current {
                        // One exact repair attempt at most.
                        self.retire();
                    }
                    return false;
                }
                if current == target {
                    return false;
                }
                let authorized = match pending {
                    Some(PendingTraversal::Application {
                        target: expected,
                        intent,
                    }) if target == expected => coordinator.authorize_standalone_navigation(
                        orchestrator,
                        &snapshot,
                        intent,
                        None,
                        None,
                    ),
                    _ => orchestrator
                        .nav_host()
                        .authorize_history_cursor(host, target),
                };
                if let NavigationAuthorizationResult::Authorized(authorized) = authorized
                    && matches!(
                        authorized.candidate().evaluated_intent(),
                        NavigationIntent::HistoryBack | NavigationIntent::HistoryForward
                    )
                    && authorized.candidate().prospective_cursor() == target
                    && host.history_state(target) == Some(authorized.candidate().to())
                    && let Ok(prepared) = coordinator.prepare_standalone_navigation(
                        orchestrator,
                        compositor,
                        &snapshot,
                        authorized,
                        None,
                    )
                {
                    prepared.commit();
                    return true;
                }
                if history.traverse(target, current) {
                    self.pending = Some(PendingTraversal::Correction { target: current });
                } else {
                    self.retire();
                }
            }
            BrowserTraversal::None if self.pending.is_none() => {
                let intent = self.mailbox.intents.borrow_mut().pop_front();
                let Some(intent) = intent else {
                    return false;
                };
                if history.current_slot() != Some(current)
                    || history.path().as_deref() != history_path(&self.mailbox, host, current)
                {
                    self.retire();
                    return false;
                }
                let NavigationAuthorizationResult::Authorized(authorized) = coordinator
                    .authorize_standalone_navigation(
                        orchestrator,
                        &snapshot,
                        intent.clone(),
                        None,
                        None,
                    )
                else {
                    return false;
                };
                let candidate = authorized.candidate();
                let Some(path) = self.mailbox.public_path(candidate.to().current()) else {
                    return false;
                };
                if !candidate
                    .to()
                    .path()
                    .iter()
                    .all(|entry| self.mailbox.public_path(entry).is_some())
                {
                    return false;
                }
                if matches!(
                    candidate.evaluated_intent(),
                    NavigationIntent::HistoryBack | NavigationIntent::HistoryForward
                ) {
                    let target = candidate.prospective_cursor();
                    if history.traverse(current, target) {
                        self.pending = Some(PendingTraversal::Application { target, intent });
                    } else {
                        self.retire();
                    }
                    return false;
                }
                if let Ok(prepared) = coordinator.prepare_standalone_navigation(
                    orchestrator,
                    compositor,
                    &snapshot,
                    authorized,
                    None,
                ) {
                    // Every fallible App participant is already reserved. The
                    // host only publishes after the browser accepts the entry.
                    if history.append(current, path) {
                        prepared.commit();
                        return true;
                    }
                    drop(prepared);
                    self.retire();
                }
            }
            BrowserTraversal::None => {}
        }
        false
    }

    pub(super) fn retire(&mut self) {
        self.mailbox.open.set(false);
        self.mailbox.intents.borrow_mut().clear();
        self.pending = None;
        self.history = None;
    }
}

impl Drop for AppWebNavigation {
    fn drop(&mut self) {
        self.retire();
    }
}

fn snapshot(
    coordinator: &NavigationApplicationCoordinator,
    orchestrator: &NavigationSurfaceOrchestrator,
) -> Option<StandaloneNavigationSnapshot> {
    match coordinator.standalone_snapshot(orchestrator, InteractionDomainId::PRIMARY) {
        StandaloneNavigationSnapshotResult::Snapshot(snapshot) => Some(snapshot),
        StandaloneNavigationSnapshotResult::Retry => None,
    }
}

fn history_path(
    mailbox: &IntentMailbox,
    snapshot: &NavigationMechanicsSnapshot,
    cursor: usize,
) -> Option<&'static str> {
    mailbox.public_path(snapshot.history_state(cursor)?.current())
}
