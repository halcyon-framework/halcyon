use super::*;

impl AppRoot {
    /// Create a new AppRoot with the given context and initial root node.
    ///
    /// SPEC-HAL-020 Stage 3: Automatically registers built-in actions.
    pub fn new(mut context: AppContext, root_node: NodeId) -> Self {
        // SPEC-HAL-020 Stage 3: Register built-in actions at startup
        crate::runtime::register_builtin_actions(&mut context.registry);

        Self {
            context,
            screens: ScreenStack::new(),
            modals: ModalStack::new(),
            shortcuts: GlobalShortcuts::new(),
            overlay_root: OverlayRoot::new(),
            root_node,
            base_screen_node: None,
            pending_widget_teardowns: Vec::new(),
            modal_dismiss_pointer_sequence_active: false,
            modal_dismiss_touch_sequences: std::collections::HashSet::new(),
            toast_widget_node: None,
            menu_host: None,
            menu_bar_node: None,
        }
    }

    /// Set the base/root node used for rendering.
    ///
    /// Future stages will let screens/modals update this.
    pub fn set_root_node(&mut self, node: NodeId) {
        self.root_node = node;
    }

    /// Get the currently active root node for rendering.
    ///
    /// Stage 3: just returns the base root. Modal/screen composition comes later.
    pub fn active_root_node(&self) -> NodeId {
        self.root_node
    }

    /// SPEC-HAL-137 INV-OVL-INPUT-1: the active modal subtree is the
    /// only widget-tree input root while a modal is mounted.
    pub fn active_modal_input_root_node(&self) -> Option<NodeId> {
        self.modals.top_entry().and_then(|entry| entry.mounted_node)
    }

    /// SPEC-HAL-137 INV-OVL-INPUT-1: base content is the input root only
    /// when no mounted modal exists.
    pub fn active_input_root_node(&self) -> NodeId {
        self.active_modal_input_root_node()
            .unwrap_or_else(|| self.active_root_node())
    }

    #[cfg(any(target_arch = "wasm32", test))]
    pub(super) fn browser_paste_root(&self) -> Option<NodeId> {
        if self.context.exit_requested()
            || !self.pending_widget_teardowns.is_empty()
            || self.screens.stack.last().is_some_and(|entry| entry.needs_rebuild)
            || self.modals.stack.last().is_some_and(|entry| entry.needs_rebuild)
        {
            return None;
        }
        let root = self.active_input_root_node();
        if self.overlay_root.top().is_some_and(|entry| entry.mounted_node != Some(root)) {
            return None;
        }
        Some(root)
    }

    // ========================================================================
    // SPEC-HAL-018 Stage 6: Screen Stack Operations (with lifecycle)
    // ========================================================================

    /// Push a new screen onto the stack.
    ///
    /// Stage 6: Calls on_leave on old top (if any), then on_enter on new screen.
    pub fn push_screen(&mut self, screen: Box<dyn Screen>) {
        // 1. Pop old top temporarily to call on_leave (avoids double borrow)
        if let Some(mut old_top) = self.screens.pop() {
            {
                let mut cx = ScreenContext {
                    composite_editors: None,
                    app: &mut self.context,
                    screens: &mut self.screens,
                    modals: &mut self.modals,
                    tree: None,
                };
                old_top.on_leave(&mut cx);
            }
            // Put it back
            self.screens.push(old_top);
        }

        // 2. Push new screen
        self.screens.push(screen);

        // 3. Pop new screen temporarily to call on_enter
        if let Some(mut new_top) = self.screens.pop() {
            {
                let mut cx = ScreenContext {
                    composite_editors: None,
                    app: &mut self.context,
                    screens: &mut self.screens,
                    modals: &mut self.modals,
                    tree: None,
                };
                new_top.on_enter(&mut cx);
            }
            // Put it back
            self.screens.push(new_top);
        }
    }

    /// Pop the top screen from the stack.
    ///
    /// Stage 6: Calls on_leave on popped screen, then on_enter on new top (if any).
    /// Stage 12: Marks the new top for rebuild (so it will mount its widget).
    pub fn pop_screen(&mut self) -> Option<Box<dyn Screen>> {
        // 1. Pop and call on_leave
        let mut popped = self.screens.pop()?;
        {
            let mut cx = ScreenContext {
                composite_editors: None,
                app: &mut self.context,
                screens: &mut self.screens,
                modals: &mut self.modals,
                tree: None,
            };
            popped.on_leave(&mut cx);
        }

        // 2. Pop new top temporarily to call on_enter (if any)
        if let Some(mut new_top) = self.screens.pop() {
            {
                let mut cx = ScreenContext {
                    composite_editors: None,
                    app: &mut self.context,
                    screens: &mut self.screens,
                    modals: &mut self.modals,
                    tree: None,
                };
                new_top.on_enter(&mut cx);
            }
            // Put it back
            self.screens.push(new_top);
        }

        // Stage 12: Mark the new top for rebuild
        // When a screen is popped, the screen underneath needs to rebuild its widget
        self.screens.mark_top_rebuild();

        Some(popped)
    }

    /// Replace the top screen with a new one.
    ///
    /// Stage 6: Calls on_leave on old screen, then on_enter on new screen.
    pub fn replace_screen(&mut self, screen: Box<dyn Screen>) -> Option<Box<dyn Screen>> {
        // 1. Pop old and call on_leave
        let old = if let Some(mut old_screen) = self.screens.pop() {
            {
                let mut cx = ScreenContext {
                    composite_editors: None,
                    app: &mut self.context,
                    screens: &mut self.screens,
                    modals: &mut self.modals,
                    tree: None,
                };
                old_screen.on_leave(&mut cx);
            }
            Some(old_screen)
        } else {
            None
        };

        // 2. Push new screen
        self.screens.push(screen);

        // 3. Pop new screen temporarily to call on_enter
        if let Some(mut new_top) = self.screens.pop() {
            {
                let mut cx = ScreenContext {
                    composite_editors: None,
                    app: &mut self.context,
                    screens: &mut self.screens,
                    modals: &mut self.modals,
                    tree: None,
                };
                new_top.on_enter(&mut cx);
            }
            // Put it back
            self.screens.push(new_top);
        }

        old
    }

    /// Clear all screens.
    ///
    /// Stage 6: Calls on_leave on each screen in reverse order (top-to-bottom).
    pub fn clear_screens(&mut self) {
        for mut screen in self.screens.drain_reverse() {
            let mut cx = ScreenContext {
                composite_editors: None,
                app: &mut self.context,
                screens: &mut self.screens,
                modals: &mut self.modals,
                tree: None,
            };
            screen.on_leave(&mut cx);
        }
    }

    // ========================================================================
    // SPEC-HAL-018 Stage 6: Modal Stack Operations (with lifecycle)
    // ========================================================================

    /// Open a new modal on top of the stack.
    ///
    /// Stage 6: Calls on_enter on the new modal.
    /// S4.INV1: Captures dismiss_policy from modal before adding to OverlayRoot.
    pub fn open_modal(&mut self, mut modal: Box<dyn Screen>) {
        // S4.INV1: Capture dismiss_policy before giving up ownership
        let dismiss_policy = modal.dismiss_policy();

        // 1. Call on_enter before adding to stack
        {
            let mut cx = ScreenContext {
                composite_editors: None,
                app: &mut self.context,
                screens: &mut self.screens,
                modals: &mut self.modals,
                tree: None,
            };
            modal.on_enter(&mut cx);
        }

        // 2. Add to stack
        self.modals.open(modal);

        // 3. S2.INV1: Mirror to OverlayRoot (mounted_node is None until rebuild)
        // S4.INV1: Pass dismiss_policy captured from the modal
        self.overlay_root.push_modal(None, dismiss_policy);

        // S2.INV1: Assert parity after lifecycle operation
        self.overlay_root.assert_in_sync(self.modals.len());
    }

    /// Close the top modal.
    ///
    /// Stage 6: Calls on_leave on the closed modal. Returns true if a modal was closed.
    /// Stage 12: Queues the modal's mounted widget for removal.
    pub fn close_modal(&mut self) -> bool {
        // Stage 12: Use pop_entry to get mounted_node for cleanup
        if let Some(mut entry) = self.modals.pop_entry() {
            // Queue widget for deferred teardown (if mounted)
            if let Some(node) = entry.mounted_node {
                self.pending_widget_teardowns.push(node);
            }

            // Pop from overlay_root immediately (for correct top_id behavior)
            self.overlay_root.pop_top_modal();

            let mut cx = ScreenContext {
                composite_editors: None,
                app: &mut self.context,
                screens: &mut self.screens,
                modals: &mut self.modals,
                tree: None,
            };
            entry.screen.on_leave(&mut cx);

            // S2.INV1: Assert parity after lifecycle operation
            self.overlay_root.assert_in_sync(self.modals.len());

            true
        } else {
            false
        }
    }

    /// Close all modals.
    ///
    /// Stage 6: Calls on_leave on each modal in reverse order (top-to-bottom).
    /// Stage 12: Queues all modal mounted widgets for removal.
    /// S4.2.INV5: Each modal completes full lifecycle before next begins.
    pub fn close_all_modals(&mut self) {
        // Stage 12: Use drain_entries_reverse to get mounted_nodes (top-to-bottom)
        // S4.2.INV5: Each modal's lifecycle (on_leave) completes before next
        for mut entry in self.modals.drain_entries_reverse() {
            // Queue widget for deferred teardown (if mounted)
            if let Some(node) = entry.mounted_node {
                self.pending_widget_teardowns.push(node);
            }

            // Pop from overlay_root immediately (for correct top_id behavior)
            self.overlay_root.pop_top_modal();

            let mut cx = ScreenContext {
                composite_editors: None,
                app: &mut self.context,
                screens: &mut self.screens,
                modals: &mut self.modals,
                tree: None,
            };
            entry.screen.on_leave(&mut cx);
        }

        // S2.INV1: Both stacks should now be empty
        debug_assert!(
            self.modals.is_empty(),
            "S2.INV1.a violation: ModalStack not empty after close_all_modals"
        );
        self.overlay_root.assert_in_sync(self.modals.len());
    }

    /// SPEC-HAL-020 Stage 7: Open the action palette modal.
    ///
    /// Convenience method for opening the action palette as a modal overlay.
    /// The palette allows searching and executing actions via keyboard shortcuts.
    pub fn open_action_palette(&mut self) {
        let modal = Box::new(crate::core::action_palette_modal::ActionPaletteModal::new());
        self.open_modal(modal);
    }

    /// Stage 6: Initialize the first screen's lifecycle.
    ///
    /// Call this after pushing the initial screen (via raw ScreenStack::push)
    /// to fire on_enter exactly once. This should be called before the first frame.
    ///
    /// Note: This is for bootstrapping only. Normal screen operations via
    /// push_screen/pop_screen/replace_screen handle lifecycle automatically.
    pub fn initialize_screens(&mut self) {
        // Pop screen temporarily to call on_enter (avoids double borrow)
        if let Some(mut screen) = self.screens.pop() {
            {
                let mut cx = ScreenContext {
                    composite_editors: None,
                    app: &mut self.context,
                    screens: &mut self.screens,
                    modals: &mut self.modals,
                    tree: None,
                };
                screen.on_enter(&mut cx);
            }
            // Put it back
            self.screens.push(screen);
        }
    }

    /// SPEC-HAL-018 Stage 4/10/12: Rebuild the subtree under `root_node` to reflect
    /// the current base screen + modals.
    ///
    /// Model A1: stateless host, screens own their internal widget state.
    /// This is called once per frame BEFORE layout/paint.
    ///
    /// # Important
    /// - ScreenFrameContext borrows AppContext + tree, NOT screens/modals
    /// - Navigation must NOT happen inside Screen::build()
    /// - Screen::build() should use cx.mount_child() to add widgets to tree
    ///
    /// # Stage 12 behavior (incremental rebuild)
    /// - Only rebuilds screens/modals that have needs_rebuild = true
    /// - Base screen widget tracked in AppRoot::base_screen_node
    /// - Modal widgets tracked per-entry in ScreenEntry::mounted_node
    /// - When rebuilding, removes only that screen's subtree (not all children)
    /// - Skips build entirely for screens that don't need rebuild
    /// - Order: base screen first, then modals in stack order (bottom-to-top)
    pub fn rebuild_active_view(&mut self, tree: &mut WidgetTree) {
        // Process pending widget teardowns (deferred from close_modal/close_all_modals)
        for node in self.pending_widget_teardowns.drain(..) {
            tree.remove_node(node);
        }

        // Early return: if no screens and no modals, nothing to rebuild.
        if self.screens.is_empty() && self.modals.is_empty() {
            return;
        }

        // Stage 12/15: Build base screen conditionally.
        // Use pop_entry to get the entry with rebuild flag, not just the screen.
        // Stage 15: OR entry.needs_rebuild with screen.take_rebuild_flag() so screens
        // can trigger rebuild purely via their internal flag.
        if let Some(mut entry) = self.screens.pop_entry() {
            let mut needs = entry.needs_rebuild;
            if !needs {
                needs = entry.screen.take_rebuild_flag();
            }

            if needs {
                // INV-REBUILD-1: Consume screen rebuild flag to prevent spurious next-frame rebuild
                let _ = entry.screen.take_rebuild_flag();

                #[cfg(debug_assertions)]
                tracing::debug!(target: "halcyon::core::app", "rebuild_active_view: base screen REBUILDING");

                // Remove old base screen widget if any (tracked in AppRoot)
                if let Some(old_node) = self.base_screen_node.take() {
                    tree.remove_node(old_node);
                }

                // Build and mount new widget.
                let before_children = tree.children(self.root_node);
                {
                    let mut frame_cx = ScreenFrameContext {
                        app: &mut self.context,
                        tree,
                        parent_node: self.root_node,
                    };
                    let _ = entry.screen.build(&mut frame_cx);
                }

                // Store the mounted node in AppRoot (not in entry). This must be
                // derived from the exact insertion delta, not root child position:
                // root overlays such as ToastWidget may already be mounted.
                let after_children = tree.children(self.root_node);
                let mounted_children: Vec<NodeId> = after_children
                    .iter()
                    .copied()
                    .filter(|node| !before_children.contains(node))
                    .collect();
                assert_eq!(
                    mounted_children.len(),
                    1,
                    "INV-REBUILD-ROOT: Screen::build must mount exactly one root widget"
                );
                self.base_screen_node = mounted_children.first().copied();

                // Mark root dirty so layout re-runs for new children
                tree.mark_layout_dirty(self.root_node);

                // Reset rebuild flag
                entry.needs_rebuild = false;
            } else {
                #[cfg(debug_assertions)]
                tracing::debug!(target: "halcyon::core::app", "rebuild_active_view: base screen SKIPPED (no rebuild needed)");
            }
            // Per-frame data update (runs even when tree is not rebuilt)
            entry.screen.update(tree);

            // Put entry back
            self.screens.push_entry(entry);
        }

        // Stage 12: Build modals conditionally (bottom-to-top order).
        // Pop all entries, conditionally build each one, push them all back.
        let modal_count = self.modals.len();
        if modal_count > 0 {
            // Collect all modal entries (pop_entry returns top first)
            let mut temp_entries: Vec<ScreenEntry> = Vec::with_capacity(modal_count);
            for _ in 0..modal_count {
                if let Some(entry) = self.modals.pop_entry() {
                    temp_entries.push(entry);
                }
            }
            // temp_entries is now top-to-bottom; reverse to get bottom-to-top
            temp_entries.reverse();

            // Build each modal in bottom-to-top order (only if needs_rebuild)
            // Stage 15: OR entry.needs_rebuild with screen.take_rebuild_flag()
            for (idx, entry) in temp_entries.iter_mut().enumerate() {
                let mut needs = entry.needs_rebuild;
                if !needs {
                    needs = entry.screen.take_rebuild_flag();
                }

                if needs {
                    // INV-REBUILD-1: Consume screen rebuild flag to prevent spurious next-frame rebuild
                    let _ = entry.screen.take_rebuild_flag();

                    #[cfg(debug_assertions)]
                    tracing::debug!(target: "halcyon::core::app", "rebuild_active_view: modal[{}] REBUILDING", idx);

                    // Remove old mounted widget if any
                    if let Some(old_node) = entry.mounted_node.take() {
                        tree.remove_node(old_node);
                    }

                    // Build and mount new widget.
                    let before_children = tree.children(self.root_node);
                    {
                        let mut frame_cx = ScreenFrameContext {
                            app: &mut self.context,
                            tree,
                            parent_node: self.root_node,
                        };
                        let _ = entry.screen.build(&mut frame_cx);
                    }

                    // Store the mounted node by exact insertion delta, not sibling order.
                    let after_children = tree.children(self.root_node);
                    let mounted_children: Vec<NodeId> = after_children
                        .iter()
                        .copied()
                        .filter(|node| !before_children.contains(node))
                        .collect();
                    assert_eq!(
                        mounted_children.len(),
                        1,
                        "INV-REBUILD-MODAL: modal Screen::build must mount exactly one root widget"
                    );
                    entry.mounted_node = mounted_children.first().copied();

                    // Modal subtrees must be hit-tested BEFORE the base screen.
                    // hit_test_node sorts children by z_order descending, so a higher
                    // z_order ensures the modal is checked first. Use idx so stacked
                    // modals have ascending z_order (topmost = highest).
                    if let Some(modal_node_id) = entry.mounted_node {
                        if let Some(node) = tree.node_mut(modal_node_id) {
                            node.z_order = (100 + idx) as i16;
                        }
                    }

                    // Mark root dirty so layout re-runs for new children
                    tree.mark_layout_dirty(self.root_node);

                    // Reset rebuild flag
                    entry.needs_rebuild = false;
                } else {
                    #[cfg(debug_assertions)]
                    tracing::debug!(target: "halcyon::core::app",
                        "rebuild_active_view: modal[{}] SKIPPED (no rebuild needed)",
                        idx
                    );
                }
            }

            // Push all entries back in order (bottom-to-top)
            for entry in temp_entries {
                self.modals.open_entry(entry);
            }
        }

        self.sync_toast_widget(tree);
    }

    pub(super) fn sync_toast_widget(&mut self, tree: &mut WidgetTree) {
        // SPEC-HAL-269: Mount/unmount ToastWidget based on queue state.
        let has_active_toasts = self.context.toast_manager.borrow().has_active();
        if let Some(node) = self.toast_widget_node {
            let node_valid = tree
                .node(node)
                .is_some_and(|stored| stored.parent == Some(self.root_node));
            if !node_valid {
                self.toast_widget_node = None;
            }
        }

        if has_active_toasts {
            if let Some(node) = self.toast_widget_node {
                tree.mark_layout_dirty(node);
            } else {
                let widget = ToastWidget::new(Rc::clone(&self.context.toast_manager));
                let node = tree.add_child(self.root_node, Box::new(widget), None, 200);
                self.toast_widget_node = Some(node);
                tree.mark_layout_dirty(self.root_node);
            }
        } else {
            if let Some(node) = self.toast_widget_node.take() {
                if tree.node(node).is_some() {
                    tree.remove_node(node);
                }
                tree.mark_layout_dirty(self.root_node);
            }
        }
    }

    /// SPEC-HAL-333 Rev 4 INV-G1: install the menu host. Called once by
    /// App::run after window creation and host resolution.
    pub fn set_menu_host(
        &mut self,
        mut host: Box<dyn crate::platform::menu_host::MenuHost>,
        bar_node: Option<NodeId>,
    ) {
        host.initialize_application_roles(self.context.ui_waker());
        self.menu_host = Some(host);
        self.menu_bar_node = bar_node;
    }

    /// INV-333-QUIT-OWNER: terminal control only, after the native callback.
    /// No Screen flush, ordinary action dispatch, tree or rendering access.
    pub(super) fn take_standard_quit_request(&mut self) -> bool {
        let requested = self.menu_host.as_mut().is_some_and(|host| host.take_quit_request());
        if requested {
            self.context.request_exit();
            tracing::info!(target: "halcyon::native_quit", "HAL333_QUIT core_consumed");
        }
        requested
    }

    /// SPEC-HAL-333 Class G (INV-G3): per-frame menu-host duties, in the same
    /// AppRoot phase as action flushing — never layout or paint.
    /// 1. Install the application model once per replacement (INV-G1).
    /// 2. Mirror dirty item state to the host (INV-A5).
    /// 3. Drain activations and dispatch through the Action System (INV-E1/E2).
    /// 4. In-window: apply staged projection and mark the bar layout-dirty
    ///    (SPEC-HAL-158: dirty authority lives here, never in the widget).
    pub(super) fn sync_menu_host(&mut self, tree: &mut WidgetTree) {
        if self.take_standard_quit_request() {
            tracing::info!(target: "halcyon::native_quit", "HAL333_QUIT menu_sync_exit");
            return;
        }
        let Some(host) = self.menu_host.as_mut() else {
            return;
        };

        {
            let context = &mut self.context;
            if let Some(model) = context.menu_model.as_mut() {
                if !context.menu_model_installed {
                    host.install(model, &context.registry);
                    context.menu_model_installed = true;
                }
                for id in model.drain_dirty() {
                    host.sync_item(model, id);
                }
            }
        }

        let activations = host.drain_activations();
        for item_id in activations {
            let Some(action_id) = self
                .context
                .menu_model
                .as_ref()
                .and_then(|m| m.bound_action(item_id))
            else {
                continue;
            };
            // INV-E1: menu items bind app-scope commands. Engine-ranged ids
            // (SPEC-HAL-176) are keyboard-router territory; a menu binding
            // one is an authoring error — rejected, never half-dispatched.
            if action_id.is_engine_action() {
                tracing::debug!(target: "halcyon::core::app",
                    "SPEC-HAL-333: menu item {item_id} bound to engine action {action_id:?}; ignored"
                );
                continue;
            }
            let handler = self
                .context
                .registry
                .actions
                .get(&action_id)
                .map(|a| a.handler);
            if let Some(handler) = handler {
                handler(&mut self.context);
            }
        }

        if let Some(node) = self.menu_bar_node {
            let applied = tree
                .node_mut(node)
                .and_then(|n| {
                    n.widget
                        .as_any_mut()
                        .downcast_mut::<crate::widgets::menu_bar::MenuBar>()
                })
                .map(|bar| bar.apply_mailbox())
                .unwrap_or(false);
            if applied {
                tree.mark_layout_dirty(node);
            }
        }
    }

    /// Bind a global keyboard shortcut on the app root.
    pub fn bind_shortcut<F>(&mut self, shortcut: Shortcut, handler: F)
    where
        F: for<'a> Fn(&mut ScreenContext<'a>) -> bool + 'static,
    {
        self.shortcuts.bind(shortcut, handler);
    }

    // ========================================================================
    // SPEC-HAL-127 STAGE 4.1 INVARIANTS — AppRoot Dismissal Semantics
    // ========================================================================
    //
    // **S4.1.INV1 — One Event → At Most One Modal Close**
    //   A single input event must trigger zero or one close_modal() call.
    //   Never more than one per event.
    //
    // **S4.1.INV2 — ESC Dismissal Is Centralized**
    //   If overlay_root.top().dismiss_policy.on_escape == true:
    //     - ESC closes exactly one modal
    //     - ESC is consumed (returns true)
    //   If false:
    //     - No modal closes
    //     - ESC propagation behavior is explicitly tested
    //
    // **S4.1.INV3 — Outside Click Dismissal Is Centralized**
    //   On PointerDown outside top modal:
    //     - If on_outside_click == true: modal closes once
    //     - If false: modal does not close
    //     - Event is consumed in both cases (shielding)
    //
    // **S4.1.INV4 — No Double Dismissal Paths**
    //   No modal may be closed both by:
    //     - shielding path AND key handling path for the same event.
    //   Only one path fires per event.
    // ========================================================================

    // F9 retires physical Capture before reading the tree and bypasses ordinary routing.
    pub(super) fn capture_blueprint_shortcut(
        &self,
        event: &FusedInputEvent,
        tree: &mut WidgetTree,
        domains: &mut InteractionDomains,
        router: &mut EventRouter,
        text_engine: &mut dyn crate::core::text_engine::TextEngine,
    ) -> Option<TreeSnapshot> {
        use crate::core::event::{
            KeyCode, KeyboardCaptureInterruption, KeyboardCaptureInterruptionReason,
        };

        if !self.context.developer_shortcuts_enabled
            || !matches!(
                event,
                FusedInputEvent::KeyDown {
                    key_code: KeyCode::F9,
                    ..
                }
            )
        {
            return None;
        }
        if domains.should_route_event(event)
            && let Some(state) = domains.resolve_window_mut(event.window_id())
        {
            router.interrupt_physical_keyboard_capture(
                tree,
                state,
                KeyboardCaptureInterruption {
                    reason: KeyboardCaptureInterruptionReason::AppConsumed,
                    key: event.key_event(),
                },
                text_engine,
            );
        }
        eprintln!("[F9] Exporting BlueprintSnapshot...");
        Some(capture_tree_snapshot(tree))
    }

    /// SPEC-HAL-018 Stage 2+5: High-level input routing (shortcuts + ESC).
    ///
    /// Returns true if the event was fully handled at the app layer and
    /// should NOT be routed further into the widget tree.
    ///
    /// Routing order:
    /// 1. Global shortcuts (Stage 2)
    /// 2. ESC closes top modal if any exist (Stage 5)
    /// 3. Otherwise, event falls through
    ///
    /// Note: Stage 7 introduces handle_fused_input_modal_first which wraps this.
    pub(super) fn reserves_key(&mut self, key: &crate::core::event::KeyEvent) -> bool {
        self.shortcuts.contains_key(key)
            || self
                .context
                .registry
                .binding_registry
                .lookup_key(key.key_code, key.physical_key, key.modifiers)
                .is_some_and(|id| {
                    !id.is_engine_action() && self.context.registry.actions.contains_key(&id)
                })
    }

    pub(super) fn handle_fused_input_pre(&mut self, event: &crate::core::event::FusedInputEvent) -> bool {
        use crate::core::event::{FusedInputEvent, KeyCode};

        // Only handle KeyDown events at app level
        if let FusedInputEvent::KeyDown {
            key_code,
            physical_key,
            modifiers,
            ..
        } = event
        {
            // 1. Global shortcuts get first crack
            let mut cx = ScreenContext {
                composite_editors: None,
                app: &mut self.context,
                screens: &mut self.screens,
                modals: &mut self.modals,
                tree: None,
            };

            if self
                .shortcuts
                .try_handle_key(*key_code, *physical_key, *modifiers, &mut cx)
            {
                return true;
            }

            // 2. SPEC-HAL-127 Stage 4: ESC closes top modal (if policy allows)
            // S4.INV1: OverlayRoot owns dismissal policy
            // S4.INV2: Modal code must not decide dismissal
            if *key_code == KeyCode::Escape && !self.overlay_root.is_empty() {
                // S4.INV1: Check dismiss policy from OverlayRoot
                let should_dismiss = self
                    .overlay_root
                    .top()
                    .map(|entry| entry.dismiss_policy.on_escape)
                    .unwrap_or(false);
                if should_dismiss {
                    self.close_modal();
                    return true;
                }
            }
        }

        // Not handled at app level
        false
    }

    pub(super) fn handle_overlay_pointer_shielding(
        &mut self,
        event: &crate::core::event::FusedInputEvent,
    ) -> bool {
        use crate::core::event::FusedInputEvent;

        if let Some(identity) = touch_contact_identity(event) {
            if self.modal_dismiss_touch_sequences.contains(&identity) {
                if matches!(
                    event,
                    FusedInputEvent::TouchUp { .. } | FusedInputEvent::TouchCancel { .. }
                ) {
                    self.modal_dismiss_touch_sequences.remove(&identity);
                }
                return true;
            }
        }

        if self.modal_dismiss_pointer_sequence_active {
            match event {
                FusedInputEvent::PointerMove { .. }
                | FusedInputEvent::PointerUp { .. }
                | FusedInputEvent::DragStart { .. }
                | FusedInputEvent::DragUpdate { .. }
                | FusedInputEvent::DragEnd { .. } => {
                    if matches!(
                        event,
                        FusedInputEvent::PointerUp { .. } | FusedInputEvent::DragEnd { .. }
                    ) {
                        self.modal_dismiss_pointer_sequence_active = false;
                    }
                    return true;
                }
                _ => {}
            }
        }

        if self.overlay_root.is_empty() {
            return false;
        }

        // SPEC-HAL-127 Stage 4: Modal pointer/scroll shielding + outside-click dismissal
        // S3.INV2: OverlayRoot is the authority for modal presence.
        // S3.INV3: No ModalStack mutation during shielding checks.
        // S3.INV4: Same path for pointer and scroll events.
        // S4.INV1: OverlayRoot owns dismissal policy (outside click).
        let (pointer_pos, is_pointer_down): (Option<Point>, bool) =
            match event {
                FusedInputEvent::PointerMove { pos, .. }
                | FusedInputEvent::TouchMove { pos, .. } => (Some(*pos), false),
                FusedInputEvent::PointerDown { pos, .. }
                | FusedInputEvent::TouchDown { pos, .. } => (Some(*pos), true),
                FusedInputEvent::PointerUp { pos, .. }
                | FusedInputEvent::TouchUp { pos, .. }
                | FusedInputEvent::TouchCancel { pos, .. } => (Some(*pos), false),
                FusedInputEvent::DragStart { origin, .. } => (Some(*origin), false),
                FusedInputEvent::DragUpdate { current, .. } => (Some(*current), false),
                FusedInputEvent::DragEnd { current, .. } => (Some(*current), false),
                FusedInputEvent::Scroll { pos, .. } => (Some(*pos), false),
                _ => (None, false),
            };

        let Some(pos) = pointer_pos else {
            return false;
        };

        if let Some(entry) = self.modals.top_entry() {
            if !entry.covers_point(pos) {
                // S4.INV4: One dismissal decision per event.
                if is_pointer_down {
                    let should_dismiss = self
                        .overlay_root
                        .top()
                        .map(|e| e.dismiss_policy.on_outside_click)
                        .unwrap_or(false);
                    if should_dismiss {
                        self.close_modal();
                        if let Some(identity) = touch_contact_identity(event) {
                            self.modal_dismiss_touch_sequences.insert(identity);
                        } else {
                            self.modal_dismiss_pointer_sequence_active = true;
                        }
                    }
                }

                // No click-through: outside pointer/scroll input is consumed here.
                return true;
            }
        }

        false
    }

    /// SPEC-HAL-018 Stage 7+8: Modal-priority input routing with pointer shielding.
    ///
    /// Returns true if the event was fully handled and should NOT be routed
    /// to the widget tree.
    ///
    /// Routing order:
    /// 1. Top modal (if any) via Screen::handle_input
    /// 2. If an overlay is active: AppRoot Escape/global handling
    /// 3. If an overlay is active: overlay pointer/scroll shielding
    /// 4. Base screen only when no overlay is active
    /// 5. AppRoot/action shortcut handling
    /// 6. Otherwise falls through to widget tree
    ///
    /// Stage 44: tree is now passed to ScreenContext for directive polling.
    pub fn handle_fused_input_modal_first(
        &mut self,
        event: &crate::core::event::FusedInputEvent,
        tree: &mut WidgetTree,
    ) -> bool {
        use crate::core::event::FusedInputEvent;

        // 1. Top modal gets first crack (if any)
        //    Use pop-modify-push pattern to avoid borrow conflicts
        //    Stage 12: Use pop_entry/open_entry to preserve mounted_node
        if let Some(mut entry) = self.modals.pop_entry() {
            let handled = {
                let mut cx = ScreenContext {
                    composite_editors: None,
                    app: &mut self.context,
                    screens: &mut self.screens,
                    modals: &mut self.modals,
                    tree: Some(tree),
                };
                entry.screen.handle_input(event, &mut cx)
            };
            // Put it back
            self.modals.open_entry(entry);

            // Apply deferred invalidate (entry is now back on stack)
            if self.context.take_invalidate_requested() {
                self.screens.mark_top_rebuild();
            }

            if handled {
                return true;
            }
        }

        if self.handle_overlay_pointer_shielding(event) {
            return true;
        }

        if !self.overlay_root.is_empty() {
            if self.handle_fused_input_pre(event) {
                return true;
            }
            return false;
        }

        // 2. Base screen gets next shot (if any)
        //    Stage 12: Use pop_entry/push_entry to preserve rebuild flag
        if let Some(mut entry) = self.screens.pop_entry() {
            let handled = {
                let mut cx = ScreenContext {
                    composite_editors: None,
                    app: &mut self.context,
                    screens: &mut self.screens,
                    modals: &mut self.modals,
                    tree: Some(tree),
                };
                entry.screen.handle_input(event, &mut cx)
            };
            // Put it back
            self.screens.push_entry(entry);

            // Apply deferred invalidate (entry is now back on stack)
            if self.context.take_invalidate_requested() {
                self.screens.mark_top_rebuild();
            }

            if handled {
                return true;
            }
        }

        // 3. AppRoot-level handling (global shortcuts, ESC)
        if self.handle_fused_input_pre(event) {
            return true;
        }

        // 4. SPEC-HAL-020 Stage 2: Action system shortcut dispatch
        //    After global shortcuts but before modal pointer shielding.
        //    Only handles KeyDown events.
        if let FusedInputEvent::KeyDown {
            key_code,
            physical_key,
            modifiers,
            ..
        } = event
        {
            // SPEC-HAL-020C: Layered binding lookup
            // Look up action via binding_registry (mutable for lazy resolve), then copy handler.
            // SPEC-HAL-176: Skip engine-level actions (100..199) — they are dispatched by
            // the event router which has full context (tree, domain_state, focus).
            let handler_opt = {
                let action_id = self.context.registry.binding_registry.lookup_key(
                    *key_code,
                    *physical_key,
                    *modifiers,
                );
                action_id
                    .filter(|id| !id.is_engine_action())
                    .and_then(|id| self.context.registry.actions.get(&id))
                    .map(|action| action.handler)
            };
            // If found, call handler with mutable context (no borrow conflict)
            if let Some(handler) = handler_opt {
                handler(&mut self.context);
                return true;
            }
        }

        // 6. Not handled - falls through to widget tree
        false
    }

    /// Flush pending actions from all screens after event routing.
    ///
    /// Called once per frame after all events have been routed through the widget tree
    /// but before build_scene. This allows screens to poll pending actions from widgets
    /// and apply them to models, ensuring same-frame application.
    ///
    /// Order: modal (if any), then base screen.
    pub fn flush_pending_actions_all(&mut self, tree: &mut WidgetTree) {
        self.flush_pending_actions_inner(tree, None);
    }

    pub(super) fn flush_pending_actions_with_composite_editors(
        &mut self,
        tree: &mut WidgetTree,
        router: &mut EventRouter,
        domain: &mut crate::core::event::InteractionDomainState,
        text_engine: &mut dyn crate::core::text_engine::TextEngine,
    ) {
        let frame_time_ms = router.frame_time_ms();
        self.flush_pending_actions_inner(
            tree,
            Some(CompositeEditorScreenResources {
                router,
                domain,
                text_engine,
                frame_time_ms,
            }),
        );
    }

    #[cfg(test)]
    pub(super) fn flush_pending_startup_modal_actions(&mut self, tree: &mut WidgetTree) {
        self.flush_pending_startup_modal_actions_inner(tree, None);
    }

    pub(super) fn flush_pending_startup_modal_actions_with_composite_editors(
        &mut self,
        tree: &mut WidgetTree,
        router: &mut EventRouter,
        domain: &mut crate::core::event::InteractionDomainState,
        text_engine: &mut dyn crate::core::text_engine::TextEngine,
    ) {
        let frame_time_ms = router.frame_time_ms();
        self.flush_pending_startup_modal_actions_inner(
            tree,
            Some(CompositeEditorScreenResources {
                router,
                domain,
                text_engine,
                frame_time_ms,
            }),
        );
    }

    pub(super) fn flush_pending_actions_inner(
        &mut self,
        tree: &mut WidgetTree,
        mut resources: Option<CompositeEditorScreenResources<'_>>,
    ) {
        // 1. Modal flush (if any)
        if let Some(mut entry) = self.modals.pop_entry() {
            {
                let mut cx = ScreenContext {
                    app: &mut self.context,
                    screens: &mut self.screens,
                    modals: &mut self.modals,
                    tree: Some(tree),
                    composite_editors: resources.as_mut().map(|r| r.reborrow()),
                };
                entry.screen.flush_pending_actions(&mut cx);
            }
            self.modals.open_entry(entry);

            // Apply deferred invalidate (entry is now back on stack)
            if self.context.take_invalidate_requested() {
                self.screens.mark_top_rebuild();
            }
        }

        // 2. Base screen flush (if any)
        if let Some(mut entry) = self.screens.pop_entry() {
            {
                let mut cx = ScreenContext {
                    app: &mut self.context,
                    screens: &mut self.screens,
                    modals: &mut self.modals,
                    tree: Some(tree),
                    composite_editors: resources.as_mut().map(|r| r.reborrow()),
                };
                entry.screen.flush_pending_actions(&mut cx);
            }
            self.screens.push_entry(entry);

            // Apply deferred invalidate (entry is now back on stack)
            if self.context.take_invalidate_requested() {
                self.screens.mark_top_rebuild();
            }
        }

        self.sync_toast_widget(tree);
        self.sync_menu_host(tree);
    }

    pub(super) fn flush_pending_startup_modal_actions_inner(
        &mut self,
        tree: &mut WidgetTree,
        mut resources: Option<CompositeEditorScreenResources<'_>>,
    ) {
        for index in (0..self.modals.stack.len().saturating_sub(1)).rev() {
            let mut entry = self.modals.stack.remove(index);
            {
                let mut cx = ScreenContext {
                    app: &mut self.context,
                    screens: &mut self.screens,
                    modals: &mut self.modals,
                    tree: Some(tree),
                    composite_editors: resources.as_mut().map(|r| r.reborrow()),
                };
                entry.screen.flush_pending_actions(&mut cx);
            }
            self.modals.stack.insert(index, entry);
            if self.context.take_invalidate_requested() {
                self.screens.mark_top_rebuild();
            }
        }
    }

    pub(super) fn startup_ready(&self) -> Result<bool, String> {
        let mut ready = true;
        if let Some(entry) = self.screens.stack.last() {
            ready &= entry.screen.startup_ready()?;
        }
        for entry in &self.modals.stack {
            ready &= entry.screen.startup_ready()?;
        }
        Ok(ready)
    }

    /// Deliver a signal to application-level screen owners.
    ///
    /// Order matches input ownership: top modal first, then base screen. Signal
    /// handling remains outside widget-tree routing and before layout.
    pub(super) fn deliver_signal_modal_first(
        &mut self,
        signal: &crate::core::signal_bus::SignalEnvelope,
        tree: &mut WidgetTree,
    ) -> bool {
        if let Some(mut entry) = self.modals.pop_entry() {
            let handled = {
                let mut cx = ScreenContext {
                    composite_editors: None,
                    app: &mut self.context,
                    screens: &mut self.screens,
                    modals: &mut self.modals,
                    tree: Some(tree),
                };
                entry.screen.handle_signal(signal, &mut cx)
            };
            self.modals.open_entry(entry);

            if self.context.take_invalidate_requested() {
                self.screens.mark_top_rebuild();
            }

            if handled {
                return true;
            }
        }

        if let Some(mut entry) = self.screens.pop_entry() {
            let handled = {
                let mut cx = ScreenContext {
                    composite_editors: None,
                    app: &mut self.context,
                    screens: &mut self.screens,
                    modals: &mut self.modals,
                    tree: Some(tree),
                };
                entry.screen.handle_signal(signal, &mut cx)
            };
            self.screens.push_entry(entry);

            if self.context.take_invalidate_requested() {
                self.screens.mark_top_rebuild();
            }

            if handled {
                return true;
            }
        }

        false
    }

    pub(super) fn paint_child_window_scene_modal_first(
        &mut self,
        window: &ChildWindowSceneState,
        theme: &Theme,
        text_engine: &mut dyn TextEngine,
        scene: &mut dyn crate::core::paint::PaintSink,
        action_regions: &mut Vec<ChildWindowActionRegion>,
    ) -> bool {
        if let Some(entry) = self.modals.pop_entry() {
            let handled = {
                let mut cx = ChildWindowPaintContext {
                    theme,
                    text_engine,
                    scene,
                    action_regions,
                };
                entry.screen.paint_child_window_scene(window, &mut cx)
            };
            self.modals.open_entry(entry);
            if handled {
                return true;
            }
            action_regions.clear();
        }

        if let Some(entry) = self.screens.pop_entry() {
            let handled = {
                let mut cx = ChildWindowPaintContext {
                    theme,
                    text_engine,
                    scene,
                    action_regions,
                };
                entry.screen.paint_child_window_scene(window, &mut cx)
            };
            self.screens.push_entry(entry);
            if handled {
                return true;
            }
        }

        false
    }

    /// SPEC-HAL-018 Stage 9: Process navigation commands queued by screens/modals.
    ///
    /// Executes commands using existing lifecycle-aware methods:
    /// push_screen/pop_screen/replace_screen/clear_screens,
    /// open_modal/close_modal/close_all_modals.
    ///
    /// Called once per frame in run_ui_loop, after input handling but before
    /// rebuild_active_view. This ensures navigation requests from screen input
    /// handlers take effect before the next frame is built.
    ///
    /// The queue is always empty initially (no screens use nav_* helpers yet),
    /// so this is a no-op until screens start using the new API.
    pub fn process_nav_queue(&mut self) {
        // Take queued commands from AppContext
        let cmds = self.context.take_nav_queue();

        for cmd in cmds {
            #[cfg(target_arch = "wasm32")]
            if self.context.web_navigation.is_some()
                && matches!(
                    &cmd,
                    NavCommand::PushScreen(_)
                        | NavCommand::PopScreen
                        | NavCommand::ReplaceScreen(_)
                        | NavCommand::ClearScreens
                )
            {
                tracing::warn!("a Web route Screen must submit typed navigation intents");
                continue;
            }
            match cmd {
                NavCommand::PushScreen(screen) => {
                    self.push_screen(screen);
                }
                NavCommand::PopScreen => {
                    self.pop_screen();
                }
                NavCommand::ReplaceScreen(screen) => {
                    self.replace_screen(screen);
                }
                NavCommand::ClearScreens => {
                    self.clear_screens();
                }
                NavCommand::OpenModal(modal) => {
                    self.open_modal(modal);
                }
                NavCommand::CloseModal => {
                    self.close_modal();
                }
                NavCommand::CloseAllModals => {
                    self.close_all_modals();
                }
            }
        }
    }

    // ========================================================================
    // SPEC-HAL-038 Stage 3: Focus Scope Integration
    // ========================================================================

    /// SPEC-HAL-038 Stage 3: Get the mounted nodes of all open modals.
    ///
    /// Returns modal nodes in stack order (bottom-to-top), suitable for
    /// passing to EventRouter::sync_focus_scopes().
    ///
    /// Only returns nodes that have been mounted (have a mounted_node).
    /// Modals that haven't been built yet will not be included.
    pub fn modal_mounted_nodes(&self) -> Vec<NodeId> {
        self.modals.mounted_nodes()
    }
}
