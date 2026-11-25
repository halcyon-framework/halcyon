//! Pure appearance values for SPEC-HAL-012 and the paired SPEC-HAL-328 amendment.
//!
//! Button resolution and checked per-instance input surfaces are pure values.
//! Semantic resolution and batch
//! preparation do not publish a configuration or admit renderer/resource use.
//! No value here creates a widget, member, input target, geometry or lifecycle.

use crate::core::event::{
    InteractionDomainId, InteractionDomains, KeyboardAuthorityMode, WindowId,
};
use crate::core::focus_authority::FocusDomainId;
use crate::core::layout::Padding;
use crate::core::paint::{Color, SurfaceGradient};
use crate::core::widget_tree::WidgetTree;
use crate::utils::id::TreeWidgetId;

/// Material selected by the Theme for Buttons that inherit its policy.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ButtonMaterialPolicy {
    #[default]
    Standard,
    SdfV3,
}

/// Immutable product-selected input colors and corner geometry (HAL012).
///
/// Standalone `TextField` and `ComboBox` consume the surface, border, inner edge,
/// radius and optional interaction surfaces. `ComboBox` also consumes the affordance
/// and popup roles.
/// Framework Focus/Capture and qualified cell/composite presentation retain
/// their existing authority. No value changes layout, hit regions or editing.
#[derive(Debug, Clone, PartialEq)]
pub struct InputSurfaceAppearance {
    surface: SurfaceGradient,
    border: Color,
    inner_edge: Color,
    affordance: Color,
    popup_surface: Color,
    popup_highlight: Color,
    radius: f32,
    focus_colors: Option<(Color, Color)>,
    state_surfaces: Option<(SurfaceGradient, SurfaceGradient)>,
    open_surface: Option<SurfaceGradient>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputSurfaceAppearanceError {
    InvalidColor,
    InvalidRadius,
    InvalidFocusColors,
}

impl std::fmt::Display for InputSurfaceAppearanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidColor => write!(
                f,
                "input surface colors must be finite linear RGBA in 0..=1"
            ),
            Self::InvalidRadius => write!(f, "input surface radius must be finite and in 0..=64"),
            Self::InvalidFocusColors => write!(
                f,
                "focus and capture borders must be finite colors, distinct when both are visible"
            ),
        }
    }
}

impl std::error::Error for InputSurfaceAppearanceError {}

impl InputSurfaceAppearance {
    pub fn try_new(
        surface: SurfaceGradient,
        border: Color,
        inner_edge: Color,
        affordance: Color,
        popup_surface: Color,
        popup_highlight: Color,
        radius: f32,
    ) -> Result<Self, InputSurfaceAppearanceError> {
        if ![
            border,
            inner_edge,
            affordance,
            popup_surface,
            popup_highlight,
        ]
        .into_iter()
        .all(valid_color)
        {
            return Err(InputSurfaceAppearanceError::InvalidColor);
        }
        if !radius.is_finite() || !(0.0..=64.0).contains(&radius) {
            return Err(InputSurfaceAppearanceError::InvalidRadius);
        }
        Ok(Self {
            surface,
            border,
            inner_edge,
            affordance,
            popup_surface,
            popup_highlight,
            radius,
            focus_colors: None,
            state_surfaces: None,
            open_surface: None,
        })
    }

    /// Select the visual projection of actual framework Focus and Capture.
    /// Visible colors must differ. An explicit zero-alpha color omits that
    /// ComboBox perimeter; its existing caret then distinguishes Focus/Capture.
    /// TextField retains its required border fallback for an absent perimeter.
    pub fn try_with_focus_colors(
        mut self,
        focused: Color,
        captured: Color,
    ) -> Result<Self, InputSurfaceAppearanceError> {
        if !valid_color(focused)
            || !valid_color(captured)
            || (focused.a > 0.0 && captured.a > 0.0 && focused == captured)
        {
            return Err(InputSurfaceAppearanceError::InvalidFocusColors);
        }
        self.focus_colors = Some((focused, captured));
        Ok(self)
    }

    pub fn focus_colors(&self) -> Option<(Color, Color)> {
        self.focus_colors
    }

    pub(crate) fn focus_border(
        &self,
        state: crate::core::theme::ControlChromeState,
    ) -> Option<Color> {
        let (focused, captured) = self.focus_colors?;
        let color = match state {
            crate::core::theme::ControlChromeState::Focused => Some(focused),
            crate::core::theme::ControlChromeState::Captured => Some(captured),
            _ => None,
        };
        color.filter(|color| color.a > 0.0)
    }

    pub fn surface(&self) -> &SurfaceGradient {
        &self.surface
    }

    /// Select Hover and Active face materials for standalone inputs.
    /// A visible Focus/Capture cue selects Active ahead of Hover. The base
    /// `surface()` is used for Idle, Disabled and any omitted state material.
    /// A ComboBox popup also uses Active while open or revealing unless
    /// `with_open_surface()` supplies a distinct trigger material.
    pub fn with_state_surfaces(mut self, hover: SurfaceGradient, active: SurfaceGradient) -> Self {
        self.state_surfaces = Some((hover, active));
        self
    }

    /// Select the ComboBox trigger material while its popup is open or revealing.
    /// This takes precedence over Hover/Active until the closing reveal settles;
    /// then the current closed interaction material returns. The popup body still
    /// uses `popup_surface()`. Omission preserves the existing Active fallback.
    pub fn with_open_surface(mut self, surface: SurfaceGradient) -> Self {
        self.open_surface = Some(surface);
        self
    }

    pub(crate) fn has_state_surfaces(&self) -> bool {
        self.state_surfaces.is_some()
    }

    pub(crate) fn surface_for_interaction(
        &self,
        hovered: bool,
        active: bool,
        popup_visible: bool,
    ) -> &SurfaceGradient {
        if popup_visible {
            if let Some(surface) = &self.open_surface {
                return surface;
            }
        }
        match &self.state_surfaces {
            Some((_, surface)) if active || popup_visible => surface,
            Some((surface, _)) if hovered => surface,
            _ => &self.surface,
        }
    }
    pub fn border(&self) -> Color {
        self.border
    }
    pub fn inner_edge(&self) -> Color {
        self.inner_edge
    }
    pub fn affordance(&self) -> Color {
        self.affordance
    }
    pub fn popup_surface(&self) -> Color {
        self.popup_surface
    }
    pub fn popup_highlight(&self) -> Color {
        self.popup_highlight
    }
    pub fn radius(&self) -> f32 {
        self.radius
    }
}

/// HAL391: a qualified embedding role, never an application feature switch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum CellEditorPresentation {
    #[default]
    Standalone,
    CellIntegrated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CellEditorVisualFacts {
    pub disabled: bool,
    pub hovered: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CellEditorPresentationRefusal {
    UnavailableMount,
    WrongConsumer,
    UnsupportedFamily,
}

/// Immutable same-frame evidence; no geometry or editing authority is stored here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CellEdgePresentation {
    pub row: u64,
    pub column: crate::model::tabular::column::ColumnId,
    pub editor: crate::core::widget_tree::WidgetMountToken,
    pub consumer: crate::core::widget_tree::WidgetMountToken,
    pub window: WindowId,
    pub domain: InteractionDomainId,
    pub state: crate::core::theme::ControlChromeState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CellEditorEmbedding {
    pub row: u64,
    pub column: crate::model::tabular::column::ColumnId,
    pub editor: crate::core::widget_tree::WidgetMountToken,
    pub consumer: crate::core::widget_tree::WidgetMountToken,
}

pub const MAX_APPEARANCE_SELECTORS: usize = 64;
pub const MAX_APPEARANCE_QUALIFIERS: usize = 8;
pub const MAX_PREPARED_APPEARANCE_REQUESTS: usize = 256;
pub const MAX_BUTTON_COMPOSITION_LAYERS: usize = 5;

/// A namespace supplied by the product owner, never an allocated runtime owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProductAppearanceOwner(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ComponentAddress {
    Button,
    Product {
        owner: ProductAppearanceOwner,
        key: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VisualPart {
    Body,
    Product {
        owner: ProductAppearanceOwner,
        key: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AppearanceAddress {
    pub component: ComponentAddress,
    pub part: VisualPart,
}

impl AppearanceAddress {
    pub const BUTTON_BODY: Self = Self {
        component: ComponentAddress::Button,
        part: VisualPart::Body,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AppearanceVariant {
    Default,
    Product {
        owner: ProductAppearanceOwner,
        key: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum QualifierValue {
    Flag(bool),
    Choice(u32),
}

/// Meaning belongs to this product's declaration. There is no framework-fact
/// qualifier namespace and no string interpretation of focus or capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AppearanceQualifier {
    pub owner: ProductAppearanceOwner,
    pub key: u32,
    pub value: QualifierValue,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AppearanceSelector {
    address: AppearanceAddress,
    variant: AppearanceVariant,
    qualifiers: Vec<AppearanceQualifier>,
}

impl AppearanceSelector {
    pub fn new(
        address: AppearanceAddress,
        variant: AppearanceVariant,
        mut qualifiers: Vec<AppearanceQualifier>,
    ) -> Result<Self, AppearanceProblem> {
        if qualifiers.len() > MAX_APPEARANCE_QUALIFIERS {
            return Err(AppearanceProblem::DeclarationLimit);
        }
        qualifiers.sort();
        if qualifiers
            .windows(2)
            .any(|pair| pair[0].owner == pair[1].owner && pair[0].key == pair[1].key)
        {
            return Err(AppearanceProblem::DuplicateQualifier);
        }
        Ok(Self {
            address,
            variant,
            qualifiers,
        })
    }

    pub fn button() -> Self {
        Self {
            address: AppearanceAddress::BUTTON_BODY,
            variant: AppearanceVariant::Default,
            qualifiers: Vec::new(),
        }
    }

    pub fn address(&self) -> AppearanceAddress {
        self.address
    }
    pub fn variant(&self) -> AppearanceVariant {
        self.variant
    }
    pub fn qualifiers(&self) -> &[AppearanceQualifier] {
        &self.qualifiers
    }
}

/// Exact admitted selectors, with no wildcard, cascade or implicit qualifier
/// defaults. Optional selection is a product fact, not a new Button behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonAppearanceContract {
    selectors: Vec<AppearanceSelector>,
    selection_owner: Option<ProductAppearanceOwner>,
}

impl ButtonAppearanceContract {
    pub fn new(
        mut selectors: Vec<AppearanceSelector>,
        selection_owner: Option<ProductAppearanceOwner>,
    ) -> Result<Self, AppearanceProblem> {
        if selectors.is_empty() || selectors.len() > MAX_APPEARANCE_SELECTORS {
            return Err(AppearanceProblem::DeclarationLimit);
        }
        if selectors
            .iter()
            .any(|s| s.address != AppearanceAddress::BUTTON_BODY)
        {
            return Err(AppearanceProblem::UnknownAddress);
        }
        selectors.sort();
        if selectors.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(AppearanceProblem::DuplicateSelector);
        }
        Ok(Self {
            selectors,
            selection_owner,
        })
    }

    pub fn standard() -> Self {
        Self {
            selectors: vec![AppearanceSelector::button()],
            selection_owner: None,
        }
    }

    pub fn selectors(&self) -> &[AppearanceSelector] {
        &self.selectors
    }
    pub fn selection_owner(&self) -> Option<ProductAppearanceOwner> {
        self.selection_owner
    }
}

/// A frame projection supplied by existing framework authorities.
/// Private fields prevent product qualifiers minting facts.
/// Booleans describe this consuming owner in the indicated active domains, not
/// whether some other owner in a window holds focus/capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameworkAppearanceFacts {
    pub(crate) window: WindowId,
    pub(crate) interaction_domain: InteractionDomainId,
    /// Mirrors the focus authority's absent active-domain state without a sentinel.
    pub(crate) focus_domain: Option<FocusDomainId>,
    pub(crate) window_active: bool,
    pub(crate) interaction_domain_active: bool,
    pub(crate) focused: bool,
    pub(crate) navigation_cue_eligible: bool,
    keyboard_focus_cue: bool,
    pub(crate) keyboard_captured: bool,
    pub(crate) pointer_captured: bool,
    pub(crate) scroll_captured: bool,
}

impl FrameworkAppearanceFacts {
    pub fn window(&self) -> WindowId {
        self.window
    }
    pub fn interaction_domain(&self) -> InteractionDomainId {
        self.interaction_domain
    }
    pub fn focus_domain(&self) -> Option<FocusDomainId> {
        self.focus_domain
    }
    pub fn window_active(&self) -> bool {
        self.window_active
    }
    pub fn interaction_domain_active(&self) -> bool {
        self.interaction_domain_active
    }
    pub fn focused(&self) -> bool {
        self.focused
    }

    pub fn navigation_cue_eligible(&self) -> bool {
        self.navigation_cue_eligible
    }

    pub fn focus_cue_visible(&self) -> bool {
        self.keyboard_captured || (self.focused && self.navigation_cue_eligible)
    }
    pub fn keyboard_captured(&self) -> bool {
        self.keyboard_captured
    }
    pub fn pointer_captured(&self) -> bool {
        self.pointer_captured
    }
    pub fn scroll_captured(&self) -> bool {
        self.scroll_captured
    }

    /// Synthetic values for pure contract tests; not an authority receipt.
    #[cfg(feature = "test-harness")]
    pub fn for_test(
        window: WindowId,
        interaction_domain: InteractionDomainId,
        focus_domain: Option<FocusDomainId>,
        focused: bool,
        keyboard_captured: bool,
        pointer_captured: bool,
        scroll_captured: bool,
    ) -> Self {
        Self {
            window,
            interaction_domain,
            focus_domain,
            window_active: true,
            interaction_domain_active: true,
            focused,
            navigation_cue_eligible: true,
            keyboard_focus_cue: false,
            keyboard_captured,
            pointer_captured,
            scroll_captured,
        }
    }
}

/// Immutable input-owner publication shared by one window's update and paint.
/// Remembered ownership is kept distinct from active ownership. The previous
/// publication is retained only to invalidate paint, never to route input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FrameworkAppearanceSnapshot {
    window: WindowId,
    interaction_domain: InteractionDomainId,
    active_window: Option<WindowId>,
    active_interaction_domain: Option<InteractionDomainId>,
    focus_domain: Option<FocusDomainId>,
    focused_owner: Option<TreeWidgetId>,
    keyboard_mode: KeyboardAuthorityMode,
    navigation_cue_eligible: bool,
    keyboard_focus_cue: bool,
    pointer_capture_owners: Vec<TreeWidgetId>,
    scroll_capture_owner: Option<TreeWidgetId>,
}

impl FrameworkAppearanceSnapshot {
    pub(crate) fn capture(
        domains: &InteractionDomains,
        window: WindowId,
        tree: &WidgetTree,
    ) -> Option<Self> {
        let interaction_domain = domains.domain_for_window(window)?;
        let state = domains.state(interaction_domain)?;
        let active_window = domains.active_window();
        // Defensive read-side filtering only. Production removal cleanup and
        // focus recovery own teardown; this projection cannot repair authority.
        let live_owner = |owner| tree.find_by_id(owner).is_some();
        let mut pointer_capture_owners: Vec<_> = state
            .pointer_capture_manager
            .active_pointers()
            .into_iter()
            .filter_map(|pointer| state.pointer_capture_manager.current_capture(pointer))
            .filter(|owner| live_owner(*owner))
            .collect();
        pointer_capture_owners.sort_unstable_by_key(|owner| owner.0);
        pointer_capture_owners.dedup();
        Some(Self {
            window,
            interaction_domain,
            active_window,
            active_interaction_domain: active_window
                .and_then(|active| domains.domain_for_window(active)),
            focus_domain: state.focus_manager.active_focus_domain(),
            focused_owner: state
                .focus_manager
                .focused()
                .filter(|owner| live_owner(*owner)),
            keyboard_mode: state.keyboard_mode,
            navigation_cue_eligible: state.ui_interaction_policy()
                == crate::core::event::UiInteractionPolicy::NavigationAndCapture,
            keyboard_focus_cue: state.focus_manager.keyboard_focus_cue(tree),
            pointer_capture_owners,
            scroll_capture_owner: state
                .scroll_capture_owner
                .filter(|owner| live_owner(*owner)),
        })
    }

    pub(crate) fn active_focused_owner(&self) -> Option<TreeWidgetId> {
        self.is_active().then_some(self.focused_owner).flatten()
    }

    pub(crate) fn keyboard_mode(&self) -> KeyboardAuthorityMode {
        self.keyboard_mode
    }

    fn is_active(&self) -> bool {
        self.active_window == Some(self.window)
            && self.active_interaction_domain == Some(self.interaction_domain)
    }

    pub(crate) fn for_owner(&self, owner: TreeWidgetId) -> FrameworkAppearanceFacts {
        let active = self.is_active();
        let focused = active && self.focused_owner == Some(owner);
        FrameworkAppearanceFacts {
            window: self.window,
            interaction_domain: self.interaction_domain,
            focus_domain: self.focus_domain,
            window_active: self.active_window == Some(self.window),
            interaction_domain_active: self.active_interaction_domain
                == Some(self.interaction_domain),
            focused,
            navigation_cue_eligible: self.navigation_cue_eligible,
            keyboard_focus_cue: focused && self.keyboard_focus_cue,
            keyboard_captured: focused && self.keyboard_mode == KeyboardAuthorityMode::Capture,
            pointer_captured: active && self.pointer_capture_owners.contains(&owner),
            scroll_captured: active && self.scroll_capture_owner == Some(owner),
        }
    }

    pub(crate) fn owners(&self) -> impl Iterator<Item = TreeWidgetId> + '_ {
        self.focused_owner
            .into_iter()
            .chain(self.pointer_capture_owners.iter().copied())
            .chain(self.scroll_capture_owner)
    }
}

/// Values projected by the Button owner, independent of framework authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonAppearanceFacts {
    pub enabled: bool,
    pub hovered: bool,
    pub pressed: bool,
}

impl Default for ButtonAppearanceFacts {
    fn default() -> Self {
        Self {
            enabled: true,
            hovered: false,
            pressed: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductSelectionFact {
    pub owner: ProductAppearanceOwner,
    pub selected: bool,
}

/// Per-Button presentation of the navigation focus cue.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FocusCueModality {
    /// Show the cue whenever navigation focus is eligible.
    #[default]
    Default,
    /// Require accepted keyboard navigation or activation for the current focus owner.
    KeyboardOnly,
}

/// Immutable resolution input copied from separate owners for one request.
/// This is neither persistent interaction/presentation state nor a reducer;
/// resolved values retain it only as read-only provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppearanceResolutionInput {
    framework: FrameworkAppearanceFacts,
    button: ButtonAppearanceFacts,
    selection: Option<ProductSelectionFact>,
    focus_cue_modality: FocusCueModality,
}

impl AppearanceResolutionInput {
    pub fn new(
        framework: FrameworkAppearanceFacts,
        button: ButtonAppearanceFacts,
        selection: Option<ProductSelectionFact>,
    ) -> Self {
        Self {
            framework,
            button,
            selection,
            focus_cue_modality: FocusCueModality::Default,
        }
    }

    pub(crate) fn with_focus_cue_modality(mut self, modality: FocusCueModality) -> Self {
        self.focus_cue_modality = modality;
        self
    }

    pub fn framework(&self) -> FrameworkAppearanceFacts {
        self.framework
    }
    pub fn button(&self) -> ButtonAppearanceFacts {
        self.button
    }
    pub fn selection(&self) -> Option<ProductSelectionFact> {
        self.selection
    }
}

/// Configuration provenance includes the framework default without reserving
/// or fabricating a product namespace for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppearanceConfigurationOwner {
    Framework,
    Product(ProductAppearanceOwner),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppearanceConfigurationId {
    pub owner: AppearanceConfigurationOwner,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageResourceReference {
    pub owner: ProductAppearanceOwner,
    pub key: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppearanceImageSource {
    Resource(ImageResourceReference),
    Admitted(crate::renderer::image_binding::ImageBinding),
    /// Existing Image registration-number shape; no lifetime/admission proof.
    CurrentBinding {
        texture_id: u64,
    },
}

pub type AppearanceImage = crate::core::paint::SurfaceImage<AppearanceImageSource>;
pub type AppearanceSurface = crate::core::paint::SurfaceFill<AppearanceImageSource>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppearanceStroke {
    pub color: Color,
    pub width: f32,
}

/// Padding shared by both existing Button content paths. Text measurement stays
/// with the existing content/text owner. No allocated rect or source-image
/// dimensions enter this metric value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ButtonAppearanceMetrics {
    pub padding: Padding,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ButtonInstanceAppearance {
    pub padding: Option<Padding>,
}

/// Fixed, bounded slots around component-owned content. None is an explicitly
/// permitted absence for underlay, border and overlay, never for the surface.
#[derive(Debug, Clone, PartialEq)]
pub struct ButtonPaintValues {
    pub surface: AppearanceSurface,
    pub radius: f32,
    pub content_color: Color,
    pub underlay: Option<AppearanceSurface>,
    pub border: Option<AppearanceStroke>,
    pub overlay: Option<AppearanceStroke>,
}

/// Authoring inputs may be incomplete. Freezing a configuration does not call
/// that configuration complete; resolve/prepare return missing-value outcomes.
#[derive(Debug, Clone, PartialEq)]
pub struct ButtonAppearanceValues {
    pub metrics: Option<ButtonAppearanceMetrics>,
    pub resting: Option<ButtonPaintValues>,
    pub hovered: Option<ButtonPaintValues>,
    pub pressed: Option<ButtonPaintValues>,
    pub disabled: Option<ButtonPaintValues>,
    pub selected: Option<ButtonPaintValues>,
    pub focus: Option<AppearanceStroke>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonPaintCase {
    Resting,
    Hovered,
    Pressed,
    Disabled,
    Selected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingAppearanceValue {
    Definition,
    Metrics,
    Paint(ButtonPaintCase),
    FocusCue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidAppearanceValue {
    Metrics,
    Surface,
    Radius,
    ContentColor,
    Stroke,
    FocusCue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppearanceProblem {
    DeclarationLimit,
    DuplicateQualifier,
    DuplicateSelector,
    DuplicateFallback,
    ConfigurationMismatch,
    UnknownAddress,
    UndeclaredSelector,
    UndeclaredSelection,
    MissingSelectionFact,
    WrongSelectionOwner,
    ButtonCannotCaptureKeyboard,
    Missing(MissingAppearanceValue),
    Invalid(InvalidAppearanceValue),
    FallbackNotDeclared,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AppearanceFallbackId {
    pub owner: ProductAppearanceOwner,
    pub key: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AppearanceFallbackPolicy {
    #[default]
    Reject,
    Approve(AppearanceFallbackId),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ButtonAppearanceDefinition {
    pub selector: AppearanceSelector,
    pub values: ButtonAppearanceValues,
}

/// Applies only to the exact declared selector, after its primary definition
/// fails semantic value resolution. Resource failure is not handled here.
#[derive(Debug, Clone, PartialEq)]
pub struct ButtonAppearanceFallback {
    pub id: AppearanceFallbackId,
    pub selector: AppearanceSelector,
    pub values: ButtonAppearanceValues,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppearanceRequest {
    pub configuration: AppearanceConfigurationId,
    pub selector: AppearanceSelector,
    pub input: AppearanceResolutionInput,
    pub instance: ButtonInstanceAppearance,
    pub fallback: AppearanceFallbackPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppearanceCoverage {
    Explicit,
    Fallback {
        id: AppearanceFallbackId,
        primary_problem: AppearanceProblem,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppearanceAdmission {
    /// Semantic resolution alone does not admit a pipeline, target or resource.
    /// Each consumer must admit its actual material and clipping/order path.
    PendingCapabilityAdmission,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppearanceRole {
    Underlay,
    Surface,
    Border,
    Overlay,
    FocusCue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentSide {
    BeforeContent,
    AfterContent,
}

impl AppearanceRole {
    pub fn content_side(self) -> ContentSide {
        match self {
            Self::Underlay | Self::Surface | Self::Border => ContentSide::BeforeContent,
            Self::Overlay | Self::FocusCue => ContentSide::AfterContent,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppearancePrimitive {
    Surface(AppearanceSurface),
    Stroke(AppearanceStroke),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppearanceLayer {
    pub role: AppearanceRole,
    pub primitive: AppearancePrimitive,
}

/// Complete *semantic values*, not a renderer-ready appearance. Access is
/// read-only and the value owns every dependency needed to describe its result.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedButtonAppearance {
    request: AppearanceRequest,
    metrics: ButtonAppearanceMetrics,
    paint_case: ButtonPaintCase,
    radius: f32,
    content_color: Color,
    composition: Vec<AppearanceLayer>,
    optional_absences: Vec<AppearanceRole>,
    coverage: AppearanceCoverage,
}

impl ResolvedButtonAppearance {
    pub fn request(&self) -> &AppearanceRequest {
        &self.request
    }
    pub fn metrics(&self) -> &ButtonAppearanceMetrics {
        &self.metrics
    }
    pub fn paint_case(&self) -> ButtonPaintCase {
        self.paint_case
    }
    pub fn radius(&self) -> f32 {
        self.radius
    }
    pub fn content_color(&self) -> Color {
        self.content_color
    }
    pub fn composition(&self) -> &[AppearanceLayer] {
        &self.composition
    }
    pub fn optional_absences(&self) -> &[AppearanceRole] {
        &self.optional_absences
    }
    pub fn coverage(&self) -> AppearanceCoverage {
        self.coverage
    }
    pub fn admission(&self) -> AppearanceAdmission {
        AppearanceAdmission::PendingCapabilityAdmission
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnresolvedAppearance {
    pub request: AppearanceRequest,
    pub problem: AppearanceProblem,
    /// Preserves why a specifically approved fallback was attempted.
    pub primary_problem: Option<AppearanceProblem>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppearanceValueResolution {
    Resolved(ResolvedButtonAppearance),
    Unresolved(UnresolvedAppearance),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedAppearanceValues {
    configuration: AppearanceConfigurationId,
    values: Vec<ResolvedButtonAppearance>,
}

impl PreparedAppearanceValues {
    pub fn configuration(&self) -> AppearanceConfigurationId {
        self.configuration
    }
    pub fn values(&self) -> &[ResolvedButtonAppearance] {
        &self.values
    }
    pub fn admission(&self) -> AppearanceAdmission {
        AppearanceAdmission::PendingCapabilityAdmission
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppearancePreparationFailure {
    DeclarationLimit,
    Unresolved {
        request_index: usize,
        failure: UnresolvedAppearance,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnresolvedButtonStyle {
    pub problem: AppearanceProblem,
    pub primary_problem: Option<AppearanceProblem>,
}

/// Complete immutable style inputs, before interaction projection. Layout may
/// consume only `metrics`; this value contains no framework interaction facts.
/// Preparation does not admit a material to a renderer or publish a widget.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedButtonStyle {
    configuration: AppearanceConfigurationId,
    contract: ButtonAppearanceContract,
    selector: AppearanceSelector,
    instance: ButtonInstanceAppearance,
    fallback: AppearanceFallbackPolicy,
    values: ButtonAppearanceValues,
    metrics: ButtonAppearanceMetrics,
    coverage: AppearanceCoverage,
}

impl PreparedButtonStyle {
    pub fn metrics(&self) -> ButtonAppearanceMetrics {
        self.metrics
    }

    pub(crate) fn with_instance(
        mut self,
        instance: ButtonInstanceAppearance,
    ) -> Result<Self, AppearanceProblem> {
        self.metrics = validate_values(&self.contract, &self.values, instance)?;
        self.instance = instance;
        Ok(self)
    }

    /// Every declared case, including cases not currently visible. The Button
    /// paint consumer uses these values for its bounded material admission.
    pub fn paint_values(&self) -> impl Iterator<Item = ButtonPaintValues> + '_ {
        [
            self.values.resting.as_ref(),
            self.values.hovered.as_ref(),
            self.values.pressed.as_ref(),
            self.values.disabled.as_ref(),
            self.values.selected.as_ref(),
        ]
        .into_iter()
        .flatten()
        .cloned()
    }

    pub fn validate_selection(
        &self,
        selection: Option<ProductSelectionFact>,
    ) -> Result<(), AppearanceProblem> {
        validate_selection(&self.contract, selection)
    }

    pub fn resolve(&self, input: AppearanceResolutionInput) -> AppearanceValueResolution {
        let request = AppearanceRequest {
            configuration: self.configuration,
            selector: self.selector.clone(),
            input,
            instance: self.instance,
            fallback: self.fallback,
        };
        let result = self.validate_selection(input.selection).and_then(|()| {
            if input.framework.keyboard_captured {
                return Err(AppearanceProblem::ButtonCannotCaptureKeyboard);
            }
            resolve_values(&self.contract, &self.values, &request, self.coverage)
        });
        match result {
            Ok(value) => AppearanceValueResolution::Resolved(value),
            Err(problem) => AppearanceValueResolution::Unresolved(UnresolvedAppearance {
                request,
                problem,
                primary_problem: None,
            }),
        }
    }
}

/// Owned, immutable programmatic configuration. No registry, lookup callbacks,
/// publication state, resource access or fallback guessing exists here.
#[derive(Debug, Clone, PartialEq)]
pub struct ButtonAppearanceConfiguration {
    id: AppearanceConfigurationId,
    contract: ButtonAppearanceContract,
    definitions: Vec<ButtonAppearanceDefinition>,
    fallbacks: Vec<ButtonAppearanceFallback>,
}

impl ButtonAppearanceConfiguration {
    pub fn new(
        id: AppearanceConfigurationId,
        contract: ButtonAppearanceContract,
        mut definitions: Vec<ButtonAppearanceDefinition>,
        mut fallbacks: Vec<ButtonAppearanceFallback>,
    ) -> Result<Self, AppearanceProblem> {
        if definitions.len() > MAX_APPEARANCE_SELECTORS
            || fallbacks.len() > MAX_APPEARANCE_SELECTORS
        {
            return Err(AppearanceProblem::DeclarationLimit);
        }
        for selector in definitions
            .iter()
            .map(|d| &d.selector)
            .chain(fallbacks.iter().map(|f| &f.selector))
        {
            validate_selector(&contract, selector)?;
        }
        definitions.sort_by(|a, b| a.selector.cmp(&b.selector));
        fallbacks.sort_by(|a, b| (&a.selector, a.id).cmp(&(&b.selector, b.id)));
        if definitions
            .windows(2)
            .any(|pair| pair[0].selector == pair[1].selector)
        {
            return Err(AppearanceProblem::DuplicateSelector);
        }
        if fallbacks
            .windows(2)
            .any(|pair| pair[0].selector == pair[1].selector && pair[0].id == pair[1].id)
        {
            return Err(AppearanceProblem::DuplicateFallback);
        }
        Ok(Self {
            id,
            contract,
            definitions,
            fallbacks,
        })
    }

    pub fn id(&self) -> AppearanceConfigurationId {
        self.id
    }
    pub fn contract(&self) -> &ButtonAppearanceContract {
        &self.contract
    }

    /// Prepare the declared style without widget-instance properties. The
    /// consuming Button applies its own explicit padding at publication.
    pub fn prepare_style(
        &self,
        selector: AppearanceSelector,
        fallback: AppearanceFallbackPolicy,
    ) -> Result<PreparedButtonStyle, UnresolvedButtonStyle> {
        self.prepare_style_with_instance(selector, ButtonInstanceAppearance::default(), fallback)
    }

    fn prepare_style_with_instance(
        &self,
        selector: AppearanceSelector,
        instance: ButtonInstanceAppearance,
        fallback: AppearanceFallbackPolicy,
    ) -> Result<PreparedButtonStyle, UnresolvedButtonStyle> {
        let failure = |problem| UnresolvedButtonStyle {
            problem,
            primary_problem: None,
        };
        validate_selector(&self.contract, &selector).map_err(failure)?;
        if instance
            .padding
            .is_some_and(|padding| !valid_padding(padding))
        {
            return Err(failure(AppearanceProblem::Invalid(
                InvalidAppearanceValue::Metrics,
            )));
        }
        let primary = self
            .definitions
            .iter()
            .find(|definition| definition.selector == selector)
            .ok_or(AppearanceProblem::Missing(
                MissingAppearanceValue::Definition,
            ))
            .and_then(|definition| {
                validate_values(&self.contract, &definition.values, instance)
                    .map(|metrics| (&definition.values, metrics))
            });
        let (values, metrics, coverage) = match primary {
            Ok((values, metrics)) => (values, metrics, AppearanceCoverage::Explicit),
            Err(primary_problem) => {
                let AppearanceFallbackPolicy::Approve(id) = fallback else {
                    return Err(failure(primary_problem));
                };
                let fallback_failure = |problem| UnresolvedButtonStyle {
                    problem,
                    primary_problem: Some(primary_problem),
                };
                let declared = self
                    .fallbacks
                    .iter()
                    .find(|value| value.selector == selector && value.id == id)
                    .ok_or_else(|| fallback_failure(AppearanceProblem::FallbackNotDeclared))?;
                let metrics = validate_values(&self.contract, &declared.values, instance)
                    .map_err(fallback_failure)?;
                (
                    &declared.values,
                    metrics,
                    AppearanceCoverage::Fallback {
                        id,
                        primary_problem,
                    },
                )
            }
        };
        Ok(PreparedButtonStyle {
            configuration: self.id,
            contract: self.contract.clone(),
            selector,
            instance,
            fallback,
            values: values.clone(),
            metrics,
            coverage,
        })
    }

    pub fn resolve(&self, request: &AppearanceRequest) -> AppearanceValueResolution {
        let unresolved = |problem, primary_problem| {
            AppearanceValueResolution::Unresolved(UnresolvedAppearance {
                request: request.clone(),
                problem,
                primary_problem,
            })
        };
        if let Err(problem) = self.validate_request(request) {
            return unresolved(problem, None);
        }
        match self.prepare_style_with_instance(
            request.selector.clone(),
            request.instance,
            request.fallback,
        ) {
            Ok(style) => style.resolve(request.input),
            Err(failure) => unresolved(failure.problem, failure.primary_problem),
        }
    }

    /// Validate an explicit, bounded set of used requests atomically. Failure
    /// returns no partially prepared values. This is not runtime activation.
    pub fn prepare(
        &self,
        requests: &[AppearanceRequest],
    ) -> Result<PreparedAppearanceValues, AppearancePreparationFailure> {
        if requests.is_empty() || requests.len() > MAX_PREPARED_APPEARANCE_REQUESTS {
            return Err(AppearancePreparationFailure::DeclarationLimit);
        }
        let mut values = Vec::with_capacity(requests.len());
        for (request_index, request) in requests.iter().enumerate() {
            match self.resolve(request) {
                AppearanceValueResolution::Resolved(value) => values.push(value),
                AppearanceValueResolution::Unresolved(failure) => {
                    return Err(AppearancePreparationFailure::Unresolved {
                        request_index,
                        failure,
                    });
                }
            }
        }
        Ok(PreparedAppearanceValues {
            configuration: self.id,
            values,
        })
    }

    fn validate_request(&self, request: &AppearanceRequest) -> Result<(), AppearanceProblem> {
        if request.configuration != self.id {
            return Err(AppearanceProblem::ConfigurationMismatch);
        }
        validate_selector(&self.contract, &request.selector)?;
        validate_selection(&self.contract, request.input.selection)?;
        if request.input.framework.keyboard_captured {
            return Err(AppearanceProblem::ButtonCannotCaptureKeyboard);
        }
        if let Some(padding) = request.instance.padding {
            if !valid_padding(padding) {
                return Err(AppearanceProblem::Invalid(InvalidAppearanceValue::Metrics));
            }
        }
        Ok(())
    }
}

fn validate_selection(
    contract: &ButtonAppearanceContract,
    selection: Option<ProductSelectionFact>,
) -> Result<(), AppearanceProblem> {
    match (contract.selection_owner, selection) {
        (None, Some(_)) => Err(AppearanceProblem::UndeclaredSelection),
        (Some(_), None) => Err(AppearanceProblem::MissingSelectionFact),
        (Some(owner), Some(fact)) if owner != fact.owner => {
            Err(AppearanceProblem::WrongSelectionOwner)
        }
        _ => Ok(()),
    }
}

fn validate_selector(
    contract: &ButtonAppearanceContract,
    selector: &AppearanceSelector,
) -> Result<(), AppearanceProblem> {
    if selector.address != AppearanceAddress::BUTTON_BODY {
        return Err(AppearanceProblem::UnknownAddress);
    }
    if contract.selectors.binary_search(selector).is_err() {
        return Err(AppearanceProblem::UndeclaredSelector);
    }
    Ok(())
}

fn validate_values(
    contract: &ButtonAppearanceContract,
    values: &ButtonAppearanceValues,
    instance: ButtonInstanceAppearance,
) -> Result<ButtonAppearanceMetrics, AppearanceProblem> {
    let missing = |value| AppearanceProblem::Missing(value);
    let invalid = |value| AppearanceProblem::Invalid(value);
    let mut metrics = values
        .metrics
        .ok_or(missing(MissingAppearanceValue::Metrics))?;
    if !valid_padding(metrics.padding) {
        return Err(invalid(InvalidAppearanceValue::Metrics));
    }
    let cases = [
        (ButtonPaintCase::Resting, values.resting.as_ref()),
        (ButtonPaintCase::Hovered, values.hovered.as_ref()),
        (ButtonPaintCase::Pressed, values.pressed.as_ref()),
        (ButtonPaintCase::Disabled, values.disabled.as_ref()),
    ];
    for (case, paint) in cases {
        validate_paint(paint.ok_or(missing(MissingAppearanceValue::Paint(case)))?)?;
    }
    match (contract.selection_owner, values.selected.as_ref()) {
        (Some(_), Some(paint)) => validate_paint(paint)?,
        (Some(_), None) => {
            return Err(missing(MissingAppearanceValue::Paint(
                ButtonPaintCase::Selected,
            )));
        }
        (None, Some(_)) => return Err(AppearanceProblem::UndeclaredSelection),
        (None, None) => {}
    }
    let focus = values
        .focus
        .ok_or(missing(MissingAppearanceValue::FocusCue))?;
    if !valid_stroke(focus) || focus.color.a == 0.0 {
        return Err(invalid(InvalidAppearanceValue::FocusCue));
    }
    if let Some(padding) = instance.padding {
        if !valid_padding(padding) {
            return Err(invalid(InvalidAppearanceValue::Metrics));
        }
        metrics.padding = padding;
    }
    Ok(metrics)
}

fn resolve_values(
    contract: &ButtonAppearanceContract,
    values: &ButtonAppearanceValues,
    request: &AppearanceRequest,
    coverage: AppearanceCoverage,
) -> Result<ResolvedButtonAppearance, AppearanceProblem> {
    let metrics = validate_values(contract, values, request.instance)?;
    let missing = |value| AppearanceProblem::Missing(value);
    let focus = values
        .focus
        .ok_or(missing(MissingAppearanceValue::FocusCue))?;

    // Property precedence chooses paint values only; no fact is collapsed or
    // consumed. Framework cue eligibility remains independent of focus truth.
    let (paint_case, paint) = if !request.input.button.enabled {
        (ButtonPaintCase::Disabled, values.disabled.as_ref())
    } else if request.input.button.pressed {
        (ButtonPaintCase::Pressed, values.pressed.as_ref())
    } else if request.input.button.hovered {
        (ButtonPaintCase::Hovered, values.hovered.as_ref())
    } else if request.input.selection.is_some_and(|fact| fact.selected) {
        (ButtonPaintCase::Selected, values.selected.as_ref())
    } else {
        (ButtonPaintCase::Resting, values.resting.as_ref())
    };
    let paint = paint.ok_or(missing(MissingAppearanceValue::Paint(paint_case)))?;
    let mut composition = Vec::with_capacity(MAX_BUTTON_COMPOSITION_LAYERS);
    let mut optional_absences = Vec::with_capacity(3);
    let mut optional = |role, primitive| {
        if let Some(primitive) = primitive {
            composition.push(AppearanceLayer { role, primitive });
        } else {
            optional_absences.push(role);
        }
    };
    optional(
        AppearanceRole::Underlay,
        paint.underlay.clone().map(AppearancePrimitive::Surface),
    );
    composition.push(AppearanceLayer {
        role: AppearanceRole::Surface,
        primitive: AppearancePrimitive::Surface(paint.surface.clone()),
    });
    for (role, stroke) in [
        (AppearanceRole::Border, paint.border),
        (AppearanceRole::Overlay, paint.overlay),
    ] {
        if let Some(stroke) = stroke {
            composition.push(AppearanceLayer {
                role,
                primitive: AppearancePrimitive::Stroke(stroke),
            });
        } else {
            optional_absences.push(role);
        }
    }
    let framework = request.input.framework;
    let modality_allows_cue = request.input.focus_cue_modality == FocusCueModality::Default
        || framework.keyboard_focus_cue
        || framework.keyboard_captured;
    if framework.focus_cue_visible() && modality_allows_cue {
        composition.push(AppearanceLayer {
            role: AppearanceRole::FocusCue,
            primitive: AppearancePrimitive::Stroke(focus),
        });
    }
    Ok(ResolvedButtonAppearance {
        request: request.clone(),
        metrics,
        paint_case,
        radius: paint.radius,
        content_color: paint.content_color,
        composition,
        optional_absences,
        coverage,
    })
}

fn nonnegative(value: f32) -> bool {
    value.is_finite() && value >= 0.0
}
fn positive(value: f32) -> bool {
    value.is_finite() && value > 0.0
}
fn valid_padding(padding: Padding) -> bool {
    [padding.top, padding.right, padding.bottom, padding.left]
        .into_iter()
        .all(nonnegative)
        && padding.horizontal().is_finite()
        && padding.vertical().is_finite()
}
fn valid_color(color: Color) -> bool {
    [color.r, color.g, color.b, color.a]
        .into_iter()
        .all(|c| c.is_finite() && (0.0..=1.0).contains(&c))
}
fn valid_stroke(stroke: AppearanceStroke) -> bool {
    valid_color(stroke.color) && positive(stroke.width)
}
fn valid_surface(surface: &AppearanceSurface) -> bool {
    match surface {
        AppearanceSurface::Solid(color) => valid_color(*color),
        AppearanceSurface::Gradient(_) => true,
        AppearanceSurface::NinePatch(_) => true,
        AppearanceSurface::Image(image) => {
            let uv = image.uv_rect;
            valid_color(image.tint)
                && nonnegative(uv.x)
                && nonnegative(uv.y)
                && positive(uv.width)
                && positive(uv.height)
                && uv.x + uv.width <= 1.0
                && uv.y + uv.height <= 1.0
        }
    }
}
fn validate_paint(paint: &ButtonPaintValues) -> Result<(), AppearanceProblem> {
    let invalid = |value| AppearanceProblem::Invalid(value);
    if !valid_surface(&paint.surface) || paint.underlay.as_ref().is_some_and(|v| !valid_surface(v))
    {
        return Err(invalid(InvalidAppearanceValue::Surface));
    }
    if !nonnegative(paint.radius) {
        return Err(invalid(InvalidAppearanceValue::Radius));
    }
    if !valid_color(paint.content_color) {
        return Err(invalid(InvalidAppearanceValue::ContentColor));
    }
    if [paint.border, paint.overlay]
        .into_iter()
        .flatten()
        .any(|s| !valid_stroke(s))
    {
        return Err(invalid(InvalidAppearanceValue::Stroke));
    }
    Ok(())
}
