use crate::core::event::KeyCode;
// SPEC-HAL-011: Button (Interactive Control)
// SPEC-HAL-021: Hover animation support

use std::cell::Cell;

use crate::core::animation::{Animation, AnimationSample, Easing, TransitionHandle};
use crate::core::appearance::{
    AppearanceConfigurationId, AppearanceConfigurationOwner, AppearanceFallbackPolicy,
    AppearancePrimitive, AppearanceProblem, AppearanceResolutionInput, AppearanceRole,
    AppearanceSelector, AppearanceSurface, AppearanceValueResolution,
    ButtonAppearanceConfiguration, ButtonAppearanceContract, ButtonAppearanceDefinition,
    ButtonAppearanceFacts, ButtonAppearanceMetrics, ButtonInstanceAppearance, ButtonMaterialPolicy,
    FocusCueModality, PreparedButtonStyle, ProductSelectionFact, ResolvedButtonAppearance,
    UnresolvedAppearance, UnresolvedButtonStyle,
};
use crate::core::event::{Event, EventPhase, PointerButton, PointerEventKind};
use crate::core::layout::{
    Borders, Constraints, LayoutResult, Margins, Padding, compute_box_model, shrink_constraints,
};
use crate::core::math::{RectF, Size};
use crate::core::paint::{Color, LAYER_CONTENT, LAYER_FOCUS, RoundedCorners, SurfaceFill, ZIndex};
use crate::core::paragraph::TextAlign;
use crate::core::scene_identity::SceneNodeKey;
use crate::core::text_engine::{
    compute_control_line_height, compute_control_text_baseline_y, draw_text_debug_overlay,
};
use crate::core::theme::Theme;
use crate::core::widget::WidgetDowncast;
use crate::core::widget::{EventContext, LayoutContext, PaintContext, UpdateContext, Widget};
use crate::core::widget_tree::NodeId;
use crate::model::form_binder::{BindingKey, ChangeWriter};
use crate::renderer::image_binding::{
    ImageBindingFailure, ImageBindingIdentity, ImageBindingSampling,
};
use crate::renderer::sdf_material_status::{
    CHAMPAGNE_V4_RADIUS, ChampagneV4Basis, SdfMaterialRevision, SdfMaterialSource,
};
use crate::utils::id::WidgetInternalId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    Normal,
    Hovered,
    Pressed,
}

#[derive(Default)]
struct ButtonPointerActivation {
    eligible: bool,
    pointer: Option<crate::core::event::PointerId>,
}

impl ButtonPointerActivation {
    fn begin(&mut self, pointer: crate::core::event::PointerId) -> bool {
        if self.pointer.is_some_and(|owner| owner != pointer) {
            return false;
        }
        self.pointer = Some(pointer);
        self.eligible = true;
        true
    }

    fn owns(&self, pointer: crate::core::event::PointerId) -> bool {
        self.pointer == Some(pointer)
    }

    fn cancel(&mut self) {
        self.eligible = false;
    }

    fn take(&mut self) -> bool {
        self.pointer = None;
        std::mem::take(&mut self.eligible)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ButtonAppearanceError {
    UnresolvedStyle(UnresolvedButtonStyle),
    UnresolvedRequest(UnresolvedAppearance),
    InvalidInput(AppearanceProblem),
    PendingCapabilityAdmission {
        surface: AppearanceSurface,
    },
    ImageResourceUnavailable {
        identity: crate::renderer::image_binding::ImageBindingIdentity,
        failure: crate::renderer::image_binding::ImageBindingFailure,
    },
    MissingFrameworkPublication,
}

/// The consumer inherits its paint owner's surface, canonical clips and
/// main/overlay scope; appearance cannot select or escape them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonAppearanceScope {
    InheritedWidget,
}

/// Supported Button lowering, not evidence of Compile, submission or presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonAppearancePrimitiveProfile {
    SolidRoundedRect,
    GradientRoundedRect,
    BorderRoundedRect,
    ImageRect {
        identity: ImageBindingIdentity,
        sampling: ImageBindingSampling,
        /// CPU resource observation taken when inspection is requested. Failure
        /// does not change the published values or select a fallback.
        availability: Result<(), ImageBindingFailure>,
    },
    /// SPEC-R-019 material lowered to ordinary Image cells, without cell identity.
    NinePatchImageCells {
        identity: ImageBindingIdentity,
        sampling: ImageBindingSampling,
        availability: Result<(), ImageBindingFailure>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonAppearanceLayerInspection {
    pub role: AppearanceRole,
    pub z_index: ZIndex,
    pub profile: ButtonAppearancePrimitiveProfile,
}

/// An on-demand copy of the coherent last Update publication and its consumer
/// profile. It does not resolve a pending replacement or refresh framework facts.
/// Image availability is a separate observation, not execution evidence or a lease.
#[derive(Debug, Clone, PartialEq)]
pub struct ButtonAppearanceInspection {
    pub resolution: Result<ResolvedButtonAppearance, ButtonAppearanceError>,
    pub scope: ButtonAppearanceScope,
    pub content_z_index: ZIndex,
    pub layers: Vec<ButtonAppearanceLayerInspection>,
}

const BUTTON_CONTENT_Z_INDEX: ZIndex = LAYER_CONTENT + 2;

fn appearance_layer_z_index(role: AppearanceRole) -> ZIndex {
    match role {
        AppearanceRole::Underlay => LAYER_CONTENT - 1,
        AppearanceRole::Surface => LAYER_CONTENT,
        AppearanceRole::Border => LAYER_CONTENT + 1,
        AppearanceRole::Overlay => LAYER_FOCUS,
        AppearanceRole::FocusCue => LAYER_FOCUS + 1,
    }
}

/// Explicit SDF appearance selected through [`Button::request_sdf_material`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonSdfRestingProfile {
    Light,
    Dark,
    LightV2,
    DarkV2,
    LightV3,
    DarkV3,
    /// Rounded gold face with four enabled interaction cells and a static disabled face.
    /// Supports scales from 0.5× through 4× for faces at least 16 logical units on each axis.
    ChampagneV4,
}

impl ButtonSdfRestingProfile {
    fn is_v3(self) -> bool {
        matches!(self, Self::LightV3 | Self::DarkV3)
    }

    fn uses_basis_revision(self) -> bool {
        self.is_v3() || self == Self::ChampagneV4
    }

    fn follows_press(self) -> bool {
        !matches!(self, Self::Light | Self::Dark)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonSdfRequestError {
    RevisionOverflow,
}

impl std::fmt::Display for ButtonSdfRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Button SDF material revision exhausted")
    }
}

impl std::error::Error for ButtonSdfRequestError {}

/// Prospective owner input for later engine K admission, never committed paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ButtonSdfRequestSnapshot {
    pub(crate) profile: ButtonSdfRestingProfile,
    pub(crate) pressed: bool,
    pub(crate) hovered: bool,
    pub(crate) revision: SdfMaterialRevision,
    pub(crate) basis_revision: SdfMaterialRevision,
    pub(crate) source: SdfMaterialSource,
}

#[derive(Default)]
struct ButtonSdfRequestState {
    prospective: Option<ButtonSdfRestingProfile>,
    published: Option<ButtonSdfRestingProfile>,
    published_basis: Option<ChampagneV4Basis>,
    published_pressed: bool,
    published_hovered: bool,
    revision: Option<SdfMaterialRevision>,
    basis_revision: Option<SdfMaterialRevision>,
    pending: bool,
    suppressed: bool,
}

impl ButtonSdfRequestState {
    fn request(
        &mut self,
        profile: Option<ButtonSdfRestingProfile>,
    ) -> Result<(), ButtonSdfRequestError> {
        if profile == self.prospective {
            return Ok(());
        }
        if profile != self.published {
            if let Some(revision) = self.revision {
                // Pure preflight only. Revision publication remains in Update.
                revision
                    .checked_increment()
                    .map_err(|_| ButtonSdfRequestError::RevisionOverflow)?;
            }
            if profile.is_some_and(ButtonSdfRestingProfile::uses_basis_revision)
                || self.published.is_some_and(ButtonSdfRestingProfile::uses_basis_revision)
            {
                if let Some(basis) = self.basis_revision {
                    basis
                        .checked_increment()
                        .map_err(|_| ButtonSdfRequestError::RevisionOverflow)?;
                }
            }
        }
        self.prospective = profile;
        self.pending = true;
        Ok(())
    }

    fn publish(
        &mut self,
        prepared_style_controls: bool,
        facts: ButtonAppearanceFacts,
    ) -> Result<(), ButtonSdfRequestError> {
        let champagne = self.prospective == Some(ButtonSdfRestingProfile::ChampagneV4);
        let basis = champagne.then_some(if facts.enabled {
            ChampagneV4Basis::Enabled
        } else {
            ChampagneV4Basis::Disabled
        });
        let suppressed = prepared_style_controls || (!facts.enabled && !champagne);
        let stable_basis = self
            .prospective
            .is_some_and(ButtonSdfRestingProfile::uses_basis_revision)
            || self
                .published
                .is_some_and(ButtonSdfRestingProfile::uses_basis_revision);
        let basis_changed = stable_basis
            && (self.prospective != self.published
                || suppressed != self.suppressed
                || basis != self.published_basis);
        let pressed = !suppressed && facts.enabled
            && self
                .prospective
                .is_some_and(ButtonSdfRestingProfile::follows_press)
            && facts.pressed;
        let hovered = !suppressed && champagne && facts.enabled && facts.hovered;
        let interaction_changed =
            pressed != self.published_pressed || hovered != self.published_hovered;
        if !self.pending && !interaction_changed && !basis_changed {
            self.suppressed = suppressed;
            return Ok(());
        }
        if self.prospective != self.published || interaction_changed || basis_changed {
            let revision = match self.revision {
                Some(revision) => revision
                    .checked_increment()
                    .map_err(|_| ButtonSdfRequestError::RevisionOverflow),
                None => Ok(SdfMaterialRevision::initial()),
            };
            let revision = match revision {
                Ok(revision) => revision,
                Err(error) => {
                    self.prospective = self.published;
                    self.pending = false;
                    return Err(error);
                }
            };
            let basis_revision = if stable_basis {
                match self.basis_revision {
                    Some(basis) if !basis_changed => basis,
                    Some(basis) => basis
                        .checked_increment()
                        .map_err(|_| ButtonSdfRequestError::RevisionOverflow)?,
                    None => SdfMaterialRevision::initial(),
                }
            } else {
                revision
            };
            self.revision = Some(revision);
            self.basis_revision = Some(basis_revision);
            self.published = self.prospective;
            self.published_basis = basis;
            self.published_pressed = pressed;
            self.published_hovered = hovered;
        }
        self.suppressed = suppressed;
        self.pending = false;
        Ok(())
    }

    fn snapshot(&self) -> Option<ButtonSdfRequestSnapshot> {
        if self.suppressed {
            return None;
        }
        let profile = self.published?;
        let source = match profile {
            ButtonSdfRestingProfile::Light | ButtonSdfRestingProfile::Dark => SdfMaterialSource::V1 {
                dark: profile == ButtonSdfRestingProfile::Dark,
            },
            ButtonSdfRestingProfile::LightV2 | ButtonSdfRestingProfile::DarkV2 => SdfMaterialSource::V2 {
                dark: profile == ButtonSdfRestingProfile::DarkV2,
            },
            ButtonSdfRestingProfile::LightV3 | ButtonSdfRestingProfile::DarkV3 => SdfMaterialSource::V3 {
                dark: profile == ButtonSdfRestingProfile::DarkV3,
            },
            ButtonSdfRestingProfile::ChampagneV4 => SdfMaterialSource::ChampagneV4(
                self.published_basis.expect("published ChampagneV4 basis"),
            ),
        };
        Some(ButtonSdfRequestSnapshot {
            profile,
            pressed: self.published_pressed,
            hovered: self.published_hovered,
            revision: self.revision?,
            basis_revision: self.basis_revision?,
            source,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ButtonMaterialSelection {
    Theme,
    Explicit,
}

/// Interactive button widget with state machine
pub struct Button {
    id: WidgetInternalId,
    enabled: bool,
    keyboard_activation_retired: bool,
    /// Child Label node (canonical pattern). `None` when constructed
    /// standalone via `Button::with_text` for SPEC-HAL-319 composition.
    pub child: Option<NodeId>,
    /// SPEC-HAL-319: Direct label text for standalone construction
    /// (no child node). Used by the agent widget factory which has no
    /// access to a tree to mount a Label child.
    pub text: Option<String>,
    typography: Option<crate::core::paragraph::TextStyle>,
    text_offset_y: f32,
    caption_alignment: TextAlign,
    /// Explicit padding override. None = use theme geometry tokens.
    /// Set via `.with_padding()` for per-instance customization.
    pub padding: Option<Padding>,
    /// Legacy visual projection; activation eligibility is stored separately.
    pub state: ButtonState,
    pointer_activation: ButtonPointerActivation,
    pub on_click: Option<Box<dyn FnMut()>>,
    /// SPEC-HAL-141: Layout-computed bounds for hit-testing.
    /// NOT authoritative geometry - WidgetNode owns that.
    /// Written: layout phase only. Read: event handlers.
    pub layout_bounds: Size,
    /// Button-owned hover lifecycle fact; pressing does not erase hover.
    hovered: bool,
    /// SPEC-HAL-021: Cached hover animation value [0.0, 1.0]
    /// Sampled during update; never semantic truth for appearance resolution.
    /// Exposed for testing.
    pub hover_t: Cell<f32>,
    /// SPEC-HAL-021: Handle to active hover animation (if any)
    /// Completed handles are cleared during update.
    /// Exposed for testing.
    pub hover_anim: Cell<Option<TransitionHandle>>,
    /// SPEC-HAL-021-A: Cached pressed animation value [0.0, 1.0]
    /// Visual-only, no semantic coupling. Exposed for testing.
    pub pressed_t: Cell<f32>,
    /// SPEC-HAL-021-A: Handle to active pressed animation (if any)
    /// Exposed for testing.
    pub pressed_anim: Cell<Option<TransitionHandle>>,
    /// Existing keyboard press lifetime: key release activates, blur cancels.
    /// Appearance reads this Button-owned fact; animation never clears it.
    pub keyboard_pulse_active: Cell<bool>,
    // Preserve the event's existing animation admission; Timeline writes occur
    // only in update. A false admission publishes the instant terminal value.
    pending_hover_animation: Option<(Animation<f32>, bool)>,
    pending_pressed_animation: Option<(Animation<f32>, bool)>,
    requested_appearance: Option<(PreparedButtonStyle, Option<ProductSelectionFact>)>,
    focus_cue_modality: FocusCueModality,
    material_selection: ButtonMaterialSelection,
    sdf_request: ButtonSdfRequestState,
    appearance_pending: bool,
    /// Padding published before layout, with no interaction facts.
    appearance_metrics: Option<ButtonAppearanceMetrics>,
    /// Immutable last update result; paint cannot publish or mutate it.
    resolved_appearance: Result<ResolvedButtonAppearance, ButtonAppearanceError>,
    /// Ordinary chrome retained while a prospective material has no commitment.
    pending_ordinary_appearance:
        Option<Box<Result<ResolvedButtonAppearance, ButtonAppearanceError>>>,
    /// Geometry qualification published with the resolved values.
    default_relief_published: bool,
    /// SPEC-HAL-207: Binding for click emission via ChangeWriter.
    binding: Option<(BindingKey, ChangeWriter)>,
}

impl Button {
    /// Adds an explicit caption placement offset in logical pixels.
    /// Negative moves text up; zero preserves the theme baseline. Layout and hit bounds are unchanged.
    pub fn with_text_offset_y(
        mut self,
        offset: f32,
    ) -> Result<Self, crate::core::text_engine::TextInputError> {
        if !offset.is_finite() {
            return Err(crate::core::text_engine::TextInputError::PositionOverflow);
        }
        self.text_offset_y = offset;
        Ok(self)
    }

    /// Replaces caption placement during update without invalidating layout.
    pub fn set_text_offset_y(
        &mut self,
        offset: f32,
        cx: &mut UpdateContext,
    ) -> Result<bool, crate::core::text_engine::TextInputError> {
        if !offset.is_finite() {
            return Err(crate::core::text_engine::TextInputError::PositionOverflow);
        }
        if self.text_offset_y == offset {
            return Ok(false);
        }
        self.text_offset_y = offset;
        cx.invalidate_paint();
        Ok(true)
    }

    pub fn with_caption_alignment(mut self, alignment: TextAlign) -> Self {
        self.caption_alignment = alignment;
        self
    }

    /// Select how this Button shows its navigation focus cue.
    /// Pointer and keyboard activation still use the normal focus lifecycle.
    pub fn with_focus_cue_modality(mut self, modality: FocusCueModality) -> Self {
        self.focus_cue_modality = modality;
        self
    }

    /// Replaces caption placement during update without changing layout or input geometry.
    pub fn set_caption_alignment(&mut self, alignment: TextAlign, cx: &mut UpdateContext) -> bool {
        if self.caption_alignment == alignment {
            return false;
        }
        self.caption_alignment = alignment;
        cx.invalidate_paint();
        true
    }

    pub fn with_typography(
        mut self,
        style: crate::core::paragraph::TextStyle,
    ) -> Result<Self, crate::core::text_engine::TextInputError> {
        if self.child.is_some() {
            return Err(crate::core::text_engine::TextInputError::UnsupportedTypography);
        }
        style.validate()?;
        self.typography = Some(style);
        Ok(self)
    }

    pub fn set_typography(
        &mut self,
        style: Option<crate::core::paragraph::TextStyle>,
        cx: &mut UpdateContext,
    ) -> Result<bool, crate::core::text_engine::TextInputError> {
        if self.child.is_some() && style.is_some() {
            return Err(crate::core::text_engine::TextInputError::UnsupportedTypography);
        }
        if let Some(style) = &style {
            style.validate()?;
        }
        if self.typography == style {
            return Ok(false);
        }
        self.typography = style;
        cx.invalidate_layout();
        Ok(true)
    }

    pub fn new(child: NodeId) -> Self {
        Self {
            id: WidgetInternalId::new(),
            enabled: true,
            keyboard_activation_retired: false,
            child: Some(child),
            text: None,
            typography: None,
            text_offset_y: 0.0,
            caption_alignment: TextAlign::Center,
            // None = use theme.geometry.button.padding_x/padding_y
            padding: None,
            state: ButtonState::Normal,
            pointer_activation: ButtonPointerActivation::default(),
            on_click: None,
            layout_bounds: Size::zero(),
            hovered: false,
            hover_t: Cell::new(0.0),
            hover_anim: Cell::new(None),
            pressed_t: Cell::new(0.0),
            pressed_anim: Cell::new(None),
            keyboard_pulse_active: Cell::new(false),
            pending_hover_animation: None,
            pending_pressed_animation: None,
            requested_appearance: None,
            focus_cue_modality: FocusCueModality::Default,
            material_selection: ButtonMaterialSelection::Theme,
            sdf_request: ButtonSdfRequestState::default(),
            appearance_pending: true,
            appearance_metrics: None,
            resolved_appearance: Err(ButtonAppearanceError::MissingFrameworkPublication),
            pending_ordinary_appearance: None,
            default_relief_published: false,
            binding: None,
        }
    }

    /// SPEC-HAL-319: Standalone constructor for runtime composition.
    /// No child node — text is rendered directly from the stored String.
    pub fn with_text(text: impl Into<String>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            enabled: true,
            keyboard_activation_retired: false,
            child: None,
            text: Some(text.into()),
            typography: None,
            text_offset_y: 0.0,
            caption_alignment: TextAlign::Center,
            padding: None,
            state: ButtonState::Normal,
            pointer_activation: ButtonPointerActivation::default(),
            on_click: None,
            layout_bounds: Size::zero(),
            hovered: false,
            hover_t: Cell::new(0.0),
            hover_anim: Cell::new(None),
            pressed_t: Cell::new(0.0),
            pressed_anim: Cell::new(None),
            keyboard_pulse_active: Cell::new(false),
            pending_hover_animation: None,
            pending_pressed_animation: None,
            requested_appearance: None,
            focus_cue_modality: FocusCueModality::Default,
            material_selection: ButtonMaterialSelection::Theme,
            sdf_request: ButtonSdfRequestState::default(),
            appearance_pending: true,
            appearance_metrics: None,
            resolved_appearance: Err(ButtonAppearanceError::MissingFrameworkPublication),
            pending_ordinary_appearance: None,
            default_relief_published: false,
            binding: None,
        }
    }

    /// Select initial policy. Mounted changes use WidgetTree's deferred requests.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub(crate) fn apply_availability_policy(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.appearance_pending = true;
        if !enabled {
            self.pointer_activation.cancel();
            self.keyboard_activation_retired |= self.keyboard_pulse_active.get();
            self.keyboard_pulse_active.set(false);
            self.hovered = false;
            self.state = ButtonState::Normal;
            self.pending_hover_animation = None;
            self.pending_pressed_animation = Some((
                Animation {
                    from: self.pressed_t.get(),
                    to: 0.0,
                    duration: 0.0,
                    easing: Easing::EaseIn,
                },
                false,
            ));
        }
    }

    pub(crate) fn observe_availability_key_boundary(
        &mut self,
        event: &crate::core::event::FusedInputEvent,
    ) {
        use crate::core::event::FusedInputEvent;
        if matches!(
            event,
            FusedInputEvent::KeyDown {
                repeat: false,
                key_code: KeyCode::Enter | KeyCode::Space | KeyCode::Character('i'),
                ..
            } | FusedInputEvent::KeyUp {
                key_code: KeyCode::Enter | KeyCode::Space | KeyCode::Character('i'),
                ..
            }
        ) {
            self.keyboard_activation_retired = false;
        }
    }

    pub fn with_padding(mut self, padding: f32) -> Self {
        self.padding = Some(Padding::all(padding));
        self
    }

    pub fn on_click<F>(mut self, callback: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_click = Some(Box::new(callback));
        self
    }

    /// SPEC-HAL-207: Configure binding for click emission.
    /// On each click, emits `true` via ChangeWriter::set_bool.
    pub fn set_binding(&mut self, key: &str, writer: ChangeWriter) -> &mut Self {
        let binding_key = writer.intern(key);
        self.binding = Some((binding_key, writer));
        self
    }

    pub fn with_binding(mut self, key: &str, writer: ChangeWriter) -> Self {
        self.set_binding(key, writer);
        self
    }

    /// Select a complete programmatic style through the existing widget update
    /// path. Rejection leaves the previous selection and publication intact.
    /// Admits rounded Solid/Gradient paths and square Image surfaces with renderer evidence.
    pub fn set_appearance(
        &mut self,
        style: PreparedButtonStyle,
        selection: Option<ProductSelectionFact>,
    ) -> Result<(), ButtonAppearanceError> {
        style
            .validate_selection(selection)
            .map_err(ButtonAppearanceError::InvalidInput)?;
        for paint in style.paint_values() {
            for surface in [Some(&paint.surface), paint.underlay.as_ref()]
                .into_iter()
                .flatten()
            {
                let source = match surface {
                    AppearanceSurface::Image(image) => &image.source,
                    AppearanceSurface::NinePatch(material) => material.source(),
                    _ => continue,
                };
                if let crate::core::appearance::AppearanceImageSource::Admitted(binding) = source {
                    binding.availability().map_err(|failure| {
                        ButtonAppearanceError::ImageResourceUnavailable {
                            identity: binding.identity(),
                            failure,
                        }
                    })?;
                    if paint.radius == 0.0 {
                        continue;
                    }
                }
                return Err(ButtonAppearanceError::PendingCapabilityAdmission {
                    surface: surface.clone(),
                });
            }
        }
        // Equivalent input still requires the resource admission checked above.
        if self
            .requested_appearance
            .as_ref()
            .is_some_and(|(current, current_selection)| {
                current == &style && *current_selection == selection
            })
            && (self.appearance_pending || self.appearance_instance_is_published())
        {
            return Ok(());
        }
        self.requested_appearance = Some((style, selection));
        self.appearance_pending = true;
        Ok(())
    }

    pub fn use_default_appearance(&mut self) {
        if self.requested_appearance.is_none()
            && (self.appearance_pending || self.appearance_instance_is_published())
        {
            return;
        }
        self.requested_appearance = None;
        self.appearance_pending = true;
    }

    /// Select an explicit SDF profile for the next Update. Pass `None` to
    /// withdraw the request and opt out of Theme material. Prepared styles
    /// retain priority.
    pub fn request_sdf_material(
        &mut self,
        profile: Option<ButtonSdfRestingProfile>,
    ) -> Result<(), ButtonSdfRequestError> {
        self.sdf_request.request(profile)?;
        if self.material_selection != ButtonMaterialSelection::Explicit {
            self.material_selection = ButtonMaterialSelection::Explicit;
            self.appearance_pending = true;
        }
        Ok(())
    }

    /// Restore selection from the Theme on the next ordinary Update.
    pub fn use_theme_material(&mut self) {
        if self.material_selection != ButtonMaterialSelection::Theme {
            self.material_selection = ButtonMaterialSelection::Theme;
            self.appearance_pending = true;
        }
    }

    /// Immutable Update result captured by WidgetTree for the canonical App bridge.
    /// App qualifies the live mount and final target before the renderer admits
    /// and publishes the material.
    pub(crate) fn sdf_request_snapshot(&self) -> Option<ButtonSdfRequestSnapshot> {
        self.sdf_request.snapshot()
    }

    fn appearance_instance_is_published(&self) -> bool {
        self.resolved_appearance.as_ref().is_ok_and(|current| {
            current.request().instance
                == ButtonInstanceAppearance {
                    padding: self.padding,
                }
        })
    }

    /// Last immutable semantic result. Its generic admission status remains
    /// pipeline-neutral; this Button admits its Solid/Gradient/border paint paths.
    pub fn resolved_appearance(&self) -> Result<&ResolvedButtonAppearance, &ButtonAppearanceError> {
        self.resolved_appearance.as_ref()
    }

    /// Describes the published request, values, coverage and actual finite Button
    /// paint profile without changing input, layout, retained validity or Timeline.
    /// Each Image generation is observed once per description. Storage is
    /// allocated only by this call, never by ordinary frame processing.
    pub fn inspect_appearance(&self) -> ButtonAppearanceInspection {
        let resolution = self.resolved_appearance.clone();
        let mut layers = Vec::new();
        if let Ok(appearance) = &resolution {
            layers.reserve(appearance.composition().len());
            for layer in appearance.composition() {
                let profile = match &layer.primitive {
                    AppearancePrimitive::Surface(AppearanceSurface::Solid(_)) => {
                        ButtonAppearancePrimitiveProfile::SolidRoundedRect
                    }
                    AppearancePrimitive::Surface(AppearanceSurface::Gradient(_)) => {
                        ButtonAppearancePrimitiveProfile::GradientRoundedRect
                    }
                    AppearancePrimitive::Stroke(_) => {
                        ButtonAppearancePrimitiveProfile::BorderRoundedRect
                    }
                    AppearancePrimitive::Surface(
                        surface @ (AppearanceSurface::Image(_) | AppearanceSurface::NinePatch(_)),
                    ) => {
                        let source = match surface {
                            AppearanceSurface::Image(image) => &image.source,
                            AppearanceSurface::NinePatch(material) => material.source(),
                            _ => unreachable!(),
                        };
                        let crate::core::appearance::AppearanceImageSource::Admitted(binding) =
                            source
                        else {
                            unreachable!("Button rejects unadmitted Image before publication");
                        };
                        let identity = binding.identity();
                        let (sampling, availability) = layers
                            .iter()
                            .find_map(|layer: &ButtonAppearanceLayerInspection| {
                                match layer.profile {
                                    ButtonAppearancePrimitiveProfile::ImageRect {
                                        identity: previous,
                                        sampling,
                                        availability,
                                    }
                                    | ButtonAppearancePrimitiveProfile::NinePatchImageCells {
                                        identity: previous,
                                        sampling,
                                        availability,
                                    } if previous == identity => Some((sampling, availability)),
                                    _ => None,
                                }
                            })
                            .unwrap_or_else(|| (binding.sampling(), binding.availability()));
                        if matches!(surface, AppearanceSurface::NinePatch(_)) {
                            ButtonAppearancePrimitiveProfile::NinePatchImageCells {
                                identity,
                                sampling,
                                availability,
                            }
                        } else {
                            ButtonAppearancePrimitiveProfile::ImageRect {
                                identity,
                                sampling,
                                availability,
                            }
                        }
                    }
                };
                layers.push(ButtonAppearanceLayerInspection {
                    role: layer.role,
                    z_index: appearance_layer_z_index(layer.role),
                    profile,
                });
            }
        }
        ButtonAppearanceInspection {
            resolution,
            scope: ButtonAppearanceScope::InheritedWidget,
            content_z_index: BUTTON_CONTENT_Z_INDEX,
            layers,
        }
    }

    fn appearance_style(
        &self,
        theme: &Theme,
    ) -> Result<PreparedButtonStyle, ButtonAppearanceError> {
        self.appearance_style_for_material(theme, self.sdf_request.published.is_some())
    }

    fn appearance_style_for_material(
        &self,
        theme: &Theme,
        material: bool,
    ) -> Result<PreparedButtonStyle, ButtonAppearanceError> {
        let instance = ButtonInstanceAppearance {
            padding: self.padding,
        };
        if let Some((style, _)) = &self.requested_appearance {
            return style
                .clone()
                .with_instance(instance)
                .map_err(ButtonAppearanceError::InvalidInput);
        }
        let contract = ButtonAppearanceContract::standard();
        let selector = AppearanceSelector::button();
        let configuration = ButtonAppearanceConfiguration::new(
            AppearanceConfigurationId {
                owner: AppearanceConfigurationOwner::Framework,
                generation: 0,
            },
            contract.clone(),
            vec![ButtonAppearanceDefinition {
                selector: selector.clone(),
                values: if material {
                    theme.button_material_appearance_values(&contract)
                } else {
                    theme.button_appearance_values(&contract)
                },
            }],
            Vec::new(),
        )
        .map_err(ButtonAppearanceError::InvalidInput)?;
        configuration
            .prepare_style(selector, AppearanceFallbackPolicy::Reject)
            .map_err(ButtonAppearanceError::UnresolvedStyle)?
            .with_instance(instance)
            .map_err(ButtonAppearanceError::InvalidInput)
    }

    fn appearance_facts(&self) -> ButtonAppearanceFacts {
        ButtonAppearanceFacts {
            enabled: self.enabled,
            hovered: self.hovered,
            pressed: self.pointer_activation.eligible || self.keyboard_pulse_active.get(),
        }
    }

    fn sample_presentation(&mut self, cx: &mut UpdateContext) {
        if !self.hovered {
            self.pending_hover_animation = None;
            if let Some(handle) = self.hover_anim.replace(None) {
                cx.cancel_animation(handle);
            }
            self.hover_t.set(0.0);
        }
        for (pending, handle, value) in [
            (
                &mut self.pending_hover_animation,
                &self.hover_anim,
                &self.hover_t,
            ),
            (
                &mut self.pending_pressed_animation,
                &self.pressed_anim,
                &self.pressed_t,
            ),
        ] {
            if let Some((animation, admitted)) = pending.take() {
                if let Some(previous) = handle.replace(None) {
                    cx.cancel_animation(previous);
                }
                let terminal = animation.to;
                if let Some(started) = admitted.then(|| cx.start_animation(animation)).flatten() {
                    handle.set(Some(started));
                } else {
                    value.set(terminal);
                }
            }
            let Some(active) = handle.get() else { continue };
            match cx.sample_animation::<f32>(active) {
                Some(AnimationSample::Active(sample)) => value.set(sample),
                Some(AnimationSample::Completed(sample)) => {
                    value.set(sample);
                    handle.set(None);
                }
                Some(AnimationSample::Cancelled | AnimationSample::Missing) | None => {
                    handle.set(None);
                }
                Some(AnimationSample::TypeMismatch) => {
                    panic!("Button presentation handle must contain f32");
                }
            }
        }
    }
}

impl WidgetDowncast for Button {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for Button {
    fn update(&mut self, _dt: f32, cx: &mut UpdateContext) {
        let previous_sdf = self.sdf_request.snapshot();
        if self.material_selection == ButtonMaterialSelection::Theme {
            let profile = match cx
                .theme()
                .map(|theme| (theme.button_material_policy, theme.is_dark()))
            {
                Some((ButtonMaterialPolicy::Standard, _)) => None,
                Some((ButtonMaterialPolicy::SdfV3, true)) => Some(ButtonSdfRestingProfile::DarkV3),
                Some((ButtonMaterialPolicy::SdfV3, false)) => Some(ButtonSdfRestingProfile::LightV3),
                // Without Theme input, keep the last published profile. Any
                // pending explicit input belongs to the previous selection.
                None => self.sdf_request.published,
            };
            if let Err(error) = self.sdf_request.request(profile) {
                tracing::error!(
                    ?error,
                    "Button SDF theme request rejected; prior request retained"
                );
            }
        }
        let facts = self.appearance_facts();
        if let Err(error) = self.sdf_request.publish(self.requested_appearance.is_some(), facts) {
            tracing::error!(
                ?error,
                "Button SDF prospective request rejected; prior request retained"
            );
        }
        let next_sdf = self.sdf_request.snapshot();
        if previous_sdf != next_sdf
            && [previous_sdf, next_sdf]
                .into_iter()
                .flatten()
                .any(|input| input.profile.follows_press())
        {
            cx.invalidate_paint();
        }
        self.sample_presentation(cx);
        let Some(theme) = cx.theme() else { return };
        let style = self.appearance_style(theme);
        let ordinary_style = next_sdf.map(|_| self.appearance_style_for_material(theme, false));
        if let Ok(style) = &style {
            let metrics = style.metrics();
            if self.appearance_metrics != Some(metrics) {
                self.appearance_metrics = Some(metrics);
                cx.invalidate_layout();
            }
        }
        let resolve = |style: Result<PreparedButtonStyle, ButtonAppearanceError>| {
            style.and_then(|style| {
                let framework = cx
                    .appearance_facts()
                    .ok_or(ButtonAppearanceError::MissingFrameworkPublication)?;
                let selection = self
                    .requested_appearance
                    .as_ref()
                    .and_then(|(_, selection)| *selection);
                let input =
                    AppearanceResolutionInput::new(framework, self.appearance_facts(), selection)
                        .with_focus_cue_modality(self.focus_cue_modality);
                match style.resolve(input) {
                    AppearanceValueResolution::Resolved(value) => Ok(value),
                    AppearanceValueResolution::Unresolved(failure) => {
                        Err(ButtonAppearanceError::UnresolvedRequest(failure))
                    }
                }
            })
        };
        let next = resolve(style);
        let next_ordinary = ordinary_style.map(resolve);
        let projection_changed =
            |before: &Result<ResolvedButtonAppearance, ButtonAppearanceError>,
             after: &Result<ResolvedButtonAppearance, ButtonAppearanceError>| {
                match (before, after) {
                    (Ok(before), Ok(after)) => {
                        before.radius() != after.radius()
                            || before.content_color() != after.content_color()
                            || before.composition() != after.composition()
                    }
                    (Err(before), Err(after)) => before != after,
                    _ => true,
                }
            };
        let next_default_relief =
            self.requested_appearance.is_none() && self.sdf_request_snapshot().is_none();
        let ordinary_before = self
            .pending_ordinary_appearance
            .as_deref()
            .unwrap_or(&self.resolved_appearance);
        let ordinary_after = next_ordinary.as_ref().unwrap_or(&next);
        let paint_changed = (self.default_relief_published
            || self.pending_ordinary_appearance.is_some())
            != (next_default_relief || next_ordinary.is_some())
            || projection_changed(ordinary_before, ordinary_after)
            || (self.pending_ordinary_appearance.is_some()
                && next_ordinary.is_some()
                && projection_changed(&self.resolved_appearance, &next));
        self.resolved_appearance = next;
        match (&mut self.pending_ordinary_appearance, next_ordinary) {
            (Some(slot), Some(next)) => **slot = next,
            (slot, Some(next)) => *slot = Some(Box::new(next)),
            (slot, None) => *slot = None,
        }
        self.default_relief_published = next_default_relief;
        self.appearance_pending = false;
        if paint_changed {
            cx.invalidate_paint();
        }
    }

    fn needs_update(&self) -> bool {
        self.appearance_pending
            || self.sdf_request.pending
            || self.pending_hover_animation.is_some()
            || self.pending_pressed_animation.is_some()
            || self.hover_anim.get().is_some()
            || self.pressed_anim.get().is_some()
    }

    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        // The frame's update publishes padding before measurement. A standalone
        // first layout may prepare these same pure inputs without interaction facts.
        let padding = self
            .appearance_metrics
            .unwrap_or_else(|| {
                self.appearance_style(cx.theme)
                    .expect("Button layout requires complete style inputs")
                    .metrics()
            })
            .padding;

        // SPEC-HAL-010: Use canonical box model
        let child_constraints =
            shrink_constraints(constraints, Margins::zero(), Borders::none(), padding);

        let child_size = if let Some(child_id) = self.child {
            // SPEC-HAL-026 §8: descend via cx.layout_child. Engine handles
            // dirty-bit clear, pre_layout, deferred_wiring, current_node,
            // cached_size, last_constraints, layout_epoch. Missing-child
            // structural corruption surfaces as the engine's INV-ZERO-2 panic.
            let size = cx.layout_child(child_id, child_constraints);

            // Disable child Label painting — Button handles text rendering
            // with explicit caption placement. Mutates the restored widget through the tree.
            if let Some(node) = cx.tree.node_mut(child_id) {
                node.widget.set_paint_disabled(true);
            }

            // SPEC-HAL-010: Compute box model and position child
            let box_model = compute_box_model(size, Margins::zero(), Borders::none(), padding);
            let new_rect = RectF::new(
                box_model.content_offset.0,
                box_model.content_offset.1,
                size.width,
                size.height,
            );
            cx.set_child_rect(child_id, new_rect);

            let standard_caption = cx.tree.node(child_id).is_some_and(|node| {
                node.widget
                    .as_any()
                    .downcast_ref::<crate::widgets::label::Label>()
                    .is_some_and(|label| label.size.is_none())
                    && node
                        .widget
                        .text_presentation()
                        .is_some_and(|caption| caption.typography.is_none())
            });
            if standard_caption {
                let metrics = cx.text_engine.font_metrics(cx.theme.typography.font_size);
                Size::new(
                    size.width,
                    size.height.max(compute_control_line_height(
                        cx.theme.typography.line_height,
                        &metrics,
                        cx.theme.typography.baseline_offset,
                    )),
                )
            } else {
                size
            }
        } else {
            // SPEC-HAL-319: Standalone button (no child) — size from stored text.
            let font_size = cx.theme.typography.font_size;
            let metrics = cx.text_engine.font_metrics(font_size);
            let line_height = compute_control_line_height(
                cx.theme.typography.line_height,
                &metrics,
                cx.theme.typography.baseline_offset,
            );
            if let Some(ref text) = self.text {
                let run = cx
                    .text_engine
                    .shape_styled(text, self.typography.as_ref(), font_size)
                    .expect("UI-TYPOGRAPHY-INPUT: Button measurement failed");
                if self.typography.is_some() {
                    Size::new(run.width, run.height)
                } else {
                    Size::new(run.width.max(40.0), line_height)
                }
            } else {
                Size::new(40.0, line_height)
            }
        };

        // SPEC-HAL-010: Final size from box model
        let box_model = compute_box_model(child_size, Margins::zero(), Borders::none(), padding);
        self.layout_bounds = constraints.constrain(box_model.size);
        // SPEC-HAL-086 Stage 2: Return LayoutResult (no overflow)
        LayoutResult::fit(self.layout_bounds)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let request = self.sdf_request_snapshot();
        let champagne = request.is_some_and(|r| r.profile == ButtonSdfRestingProfile::ChampagneV4);
        let sdf = request
            .and_then(|request| cx.button_sdf_material(request.revision))
            .cloned();
        let projection = if sdf.is_none() && !champagne {
            self.pending_ordinary_appearance
                .as_deref()
                .unwrap_or(&self.resolved_appearance)
        } else {
            &self.resolved_appearance
        };
        let Ok(appearance) = projection else {
            tracing::error!(error = ?projection.as_ref().err(), "Button appearance is unresolved");
            return;
        };
        assert_eq!(
            cx.appearance_facts(),
            Some(appearance.request().input.framework()),
            "Button paint must consume its update's framework publication"
        );
        let default_relief = !champagne && sdf.is_none()
            && (self.default_relief_published || self.pending_ordinary_appearance.is_some());
        let radius = if default_relief {
            appearance
                .radius()
                .min(rect.width.max(0.0) * 0.5)
                .min(rect.height.max(0.0) * 0.5)
        } else {
            appearance.radius()
        };
        let bevel = if default_relief {
            appearance
                .composition()
                .iter()
                .find_map(|layer| match (&layer.role, &layer.primitive) {
                    (AppearanceRole::Border, AppearancePrimitive::Stroke(stroke)) => {
                        Some(stroke.width * 3.0)
                    }
                    _ => None,
                })
                .unwrap_or(0.0)
                .min(rect.width.max(0.0) * 0.25)
                .min(rect.height.max(0.0) * 0.25)
        } else {
            0.0
        };
        let key = SceneNodeKey::from(self.id.0);
        let outline_radius = if champagne {
            CHAMPAGNE_V4_RADIUS
        } else if sdf.is_some() && request.is_some_and(|r| r.profile.is_v3()) {
            3.0
        } else {
            radius
        };
        let material_image = sdf
            .as_ref()
            .filter(|input| input.key.metrics().final_logical_rect == rect)
            .and_then(|input| input.image.as_ref());
        let content_color = if champagne && material_image.is_some() {
            Color::from_hex(0x0A1117)
        } else {
            appearance.content_color()
        };
        cx.with_scene_key(key, |cx| {
            // All five roles occupy distinct ordinary-paint Z values. Text stays
            // at its existing layer; decoration cannot enter overlay scope.
            for layer in appearance.composition() {
                let z = appearance_layer_z_index(layer.role);
                if layer.role == AppearanceRole::Surface && (champagne || sdf.is_some()) {
                    if let Some(image) = material_image {
                        cx.scene.push_admitted_image(
                            image.coverage,
                            &image.binding,
                            image.uv,
                            Color::from_hex(0xFFFFFF),
                            z,
                            Some(key),
                        );
                    }
                    continue;
                }
                if champagne && layer.role == AppearanceRole::Border {
                    // ChampagneV4's inset edge is part of the SDF face.
                    continue;
                }
                // Only the framework default exposes its underlay as a bevel.
                // Prepared product surfaces and material images retain their full face.
                let (surface_rect, surface_radius) =
                    if layer.role == AppearanceRole::Surface && default_relief {
                        (
                            RectF::new(
                                rect.x + bevel,
                                rect.y + bevel,
                                rect.width - bevel * 2.0,
                                rect.height - bevel * 2.0,
                            ),
                            (radius - bevel).max(0.0),
                        )
                    } else {
                        (rect, radius)
                    };
                match &layer.primitive {
                    AppearancePrimitive::Surface(AppearanceSurface::Solid(color)) => {
                        cx.scene.push_rounded_rect(
                            surface_rect,
                            *color,
                            RoundedCorners::uniform(surface_radius),
                            z,
                            (layer.role == AppearanceRole::Surface).then_some(key),
                        );
                    }
                    AppearancePrimitive::Surface(AppearanceSurface::Gradient(gradient)) => {
                        cx.scene.push_gradient_rect(
                            surface_rect,
                            surface_radius,
                            gradient.definition(),
                            z,
                            (layer.role == AppearanceRole::Surface).then_some(key),
                        );
                    }
                    AppearancePrimitive::Surface(AppearanceSurface::NinePatch(material)) => {
                        let crate::core::appearance::AppearanceImageSource::Admitted(binding) =
                            material.source()
                        else {
                            unreachable!("Button rejects unadmitted NinePatch before publication");
                        };
                        cx.scene.push_admitted_surface_fill(
                            rect,
                            &SurfaceFill::NinePatch(material.with_source(binding.clone())),
                            z,
                            (layer.role == AppearanceRole::Surface).then_some(key),
                        );
                    }
                    AppearancePrimitive::Stroke(stroke) => {
                        let radius = if matches!(
                            layer.role,
                            AppearanceRole::Border | AppearanceRole::FocusCue
                        ) {
                            outline_radius
                        } else {
                            appearance.radius()
                        };
                        cx.scene.push_border_rounded_rect(
                            rect,
                            stroke.color,
                            stroke.width,
                            RoundedCorners::uniform(radius),
                            z,
                            None,
                        );
                    }
                    AppearancePrimitive::Surface(AppearanceSurface::Image(image)) => {
                        let crate::core::appearance::AppearanceImageSource::Admitted(binding) =
                            &image.source
                        else {
                            unreachable!("Button rejects unadmitted Image before publication");
                        };
                        cx.scene.push_admitted_image(
                            rect,
                            binding,
                            image.uv_rect,
                            image.tint,
                            z,
                            (layer.role == AppearanceRole::Surface).then_some(key),
                        );
                    }
                }
            }

            // Existing content paths retain their text and typography owners.
            let label_text: Option<(String, f32, Option<crate::core::paragraph::TextStyle>)> =
                if let Some(ref text) = self.text {
                    Some((
                        text.clone(),
                        cx.theme.typography.font_size,
                        self.typography.clone(),
                    ))
                } else if let Some(child_id) = self.child {
                    cx.tree.node(child_id).and_then(|node| {
                        node.widget.text_presentation().map(|presentation| {
                            (
                                presentation.text.to_string(),
                                presentation.font_size,
                                presentation.typography.cloned(),
                            )
                        })
                    })
                } else {
                    None
                };

            if let Some((text, font_size, typography)) = label_text {
                let run = cx
                    .text_engine
                    .shape_styled(&text, typography.as_ref(), font_size)
                    .expect("UI-TYPOGRAPHY-INPUT: Button paint input failed");
                let label_x = rect.x
                    + match self.caption_alignment {
                        TextAlign::Center => (rect.width - run.width) * 0.5,
                        TextAlign::Left => appearance.metrics().padding.left,
                        TextAlign::Right => {
                            rect.width - appearance.metrics().padding.right - run.width
                        }
                    };
                let metrics = cx
                    .text_engine
                    .metrics_styled(typography.as_ref(), font_size)
                    .expect("UI-TYPOGRAPHY-INPUT: Button metrics failed");
                let baseline_y = rect.y
                    + compute_control_text_baseline_y(
                        rect.height,
                        metrics.ascent,
                        metrics.descent,
                        cx.theme.typography.baseline_offset,
                    )
                    + self.text_offset_y;
                let label_y = baseline_y - metrics.ascent;
                cx.text_engine.draw(
                    &run,
                    label_x,
                    label_y,
                    content_color,
                    BUTTON_CONTENT_Z_INDEX,
                    cx.scene,
                );
                draw_text_debug_overlay(cx.scene, rect, label_y, baseline_y, &metrics, 100);
            }
        });
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        // SPEC-HAL-011 Section 3.6: Full state machine with pointer capture
        // Cancellation semantics: click fires only if pointer is inside on Up

        // Only handle events in Target phase
        if phase != EventPhase::Target {
            return false;
        }

        if self.keyboard_activation_retired
            && matches!(event, Event::Key(key) if !key.is_down && matches!(key.key_code, KeyCode::Enter | KeyCode::Space | KeyCode::Character('i')))
        {
            self.keyboard_activation_retired = false;
            return false;
        }

        if !self.enabled {
            return false;
        }

        match event {
            Event::Pointer(pe) => match pe.kind {
                // Treat DoubleClick/TripleClick same as Down for buttons.
                // Multi-click semantics are only relevant for text selection, not button activation.
                PointerEventKind::Down
                | PointerEventKind::DoubleClick
                | PointerEventKind::TripleClick => {
                    // Only react to primary (left) button
                    if pe.button != Some(PointerButton::Primary) {
                        return false;
                    }
                    if !self.pointer_activation.begin(pe.id) {
                        return false;
                    }

                    // SPEC-HAL-011: On PointerDown inside rect → Pressed + capture
                    self.state = ButtonState::Pressed;
                    if let Some(button) = pe.button {
                        cx.capture_pointer(button);
                    }

                    // SPEC-HAL-021-A: Start pressed-in visual transition (visual only)
                    let anim = Animation {
                        from: self.pressed_t.get(),
                        to: 1.0,
                        duration: 0.035, // ~35ms for snappy tactile response
                        easing: Easing::EaseOut,
                    };
                    // PRE-STAGE-8 FIX: When animation unavailable, set value directly.
                    self.pressed_t.set(anim.to);
                    self.pending_pressed_animation = Some((anim, cx.has_animation()));

                    true
                }
                PointerEventKind::Move => {
                    if !self.pointer_activation.owns(pe.id) {
                        return false;
                    }
                    // SPEC-HAL-038 Stage 7: Hover state now handled by on_hover_enter/leave
                    // During pointer capture: update visual state based on position (for feedback)
                    if self.pointer_activation.eligible {
                        let (local_x, local_y) = pe.local_pos;
                        let inside = local_x >= 0.0
                            && local_x <= self.layout_bounds.width
                            && local_y >= 0.0
                            && local_y <= self.layout_bounds.height;

                        tracing::debug!(
                            "Button {:?}: Move during press, local=({:.1}, {:.1}), inside={}",
                            self.id.0,
                            local_x,
                            local_y,
                            inside
                        );

                        // Visual feedback: show pressed state only when inside
                        if inside {
                            self.state = ButtonState::Pressed;
                        } else {
                            self.pointer_activation.cancel();
                            self.state = ButtonState::Normal;
                            // SPEC-HAL-021-A: Clear pressed visual when leaving bounds
                            let anim = Animation {
                                from: self.pressed_t.get(),
                                to: 0.0,
                                duration: 0.06, // Quick fade, ≤100ms
                                easing: Easing::EaseIn,
                            };
                            // PRE-STAGE-8 FIX: When animation unavailable, set value directly.
                            self.pressed_t.set(anim.to);
                            self.pending_pressed_animation = Some((anim, cx.has_animation()));
                        }

                        return true; // Consume while captured
                    }
                    false // Not captured, let hover lifecycle handle it
                }
                PointerEventKind::Up => {
                    // Only react to primary (left) button
                    if pe.button != Some(PointerButton::Primary) {
                        return false;
                    }
                    if !self.pointer_activation.owns(pe.id) {
                        return false;
                    }

                    // SPEC-HAL-011: On PointerUp → check if inside for click
                    // Router auto-releases capture after Up, so we don't call release_pointer()

                    // Check if pointer is inside bounds at release time
                    let (local_x, local_y) = pe.local_pos;
                    let inside = local_x >= 0.0
                        && local_x <= self.layout_bounds.width
                        && local_y >= 0.0
                        && local_y <= self.layout_bounds.height;

                    let activation_eligible = self.pointer_activation.take();

                    // Only a surviving primary press can activate on release.
                    if activation_eligible && inside {
                        if let Some(callback) = &mut self.on_click {
                            callback();
                        }
                        // SPEC-HAL-207: Emit via FormBinder binding
                        if let Some((key, writer)) = &self.binding {
                            writer.set_bool(*key, true);
                        }
                    }

                    // Always reset to Normal on Up
                    self.state = ButtonState::Normal;

                    // SPEC-HAL-021-A: Start pressed-out visual transition
                    let anim = Animation {
                        from: self.pressed_t.get(),
                        to: 0.0,
                        duration: 0.08, // ≤100ms per spec
                        easing: Easing::EaseIn,
                    };
                    // PRE-STAGE-8 FIX: When animation unavailable, set value directly.
                    self.pressed_t.set(anim.to);
                    self.pending_pressed_animation = Some((anim, cx.has_animation()));

                    true
                }
                _ => false,
            },
            // SPEC-HAL-038 Stage 6 + SPEC-HAL-021-A: Paired keyboard release
            // matching mouse PointerUp. KeyUp clears the press visual,
            // animates pressed_t 1→0 in 80ms (identical to mouse Up), and
            // fires on_click — identical semantics to PointerUp inside.
            Event::Key(ke) if !ke.is_down => {
                use crate::core::event::KeyCode;
                // Match all activation keys: hard Enter/Space plus the
                // Navigation-mode Enter alias 'i'. The framework's
                // KeyDown 'i'→Enter translation in route_input_event
                // does NOT apply to KeyUp (the KeyUp handler dispatches
                // raw key_code), so we accept KeyCode::Character('i')
                // here directly.
                let is_activation_key = matches!(
                    ke.key_code,
                    KeyCode::Enter | KeyCode::Space | KeyCode::Character('i')
                );
                if !is_activation_key {
                    return false;
                }
                // Only release if we engaged the press in on_keyboard_activate.
                // Without this guard, an unrelated KeyUp for the focused
                // widget (e.g., Tab release after focus moved here) could
                // fire on_click spuriously.
                if !self.keyboard_pulse_active.get() {
                    return false;
                }
                self.keyboard_pulse_active.set(false);

                // SPEC-HAL-021-A: Mirror mouse PointerUp press-out animation
                // (80ms EaseIn, pressed_t 1→0).
                let anim = Animation {
                    from: self.pressed_t.get(),
                    to: 0.0,
                    duration: 0.08,
                    easing: Easing::EaseIn,
                };
                self.pressed_t.set(anim.to);
                self.pending_pressed_animation = Some((anim, cx.has_animation()));

                // SPEC-HAL-011: Fire on_click — matches mouse PointerUp inside.
                if let Some(callback) = &mut self.on_click {
                    callback();
                }
                // SPEC-HAL-207: Emit via FormBinder binding.
                if let Some((key, writer)) = &self.binding {
                    writer.set_bool(*key, true);
                }
                true
            }
            _ => false,
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn accepts_touch_pointer_input(&self) -> bool {
        true
    }

    fn has_unfinished_touch_press(&self, pointer: crate::core::event::PointerId) -> bool {
        self.enabled && self.pointer_activation.owns(pointer)
    }

    // ========================================================================
    // SPEC-HAL-038: Focus System
    // ========================================================================

    fn focusable(&self) -> bool {
        self.enabled
    }

    fn paints_own_focus_indicator(&self) -> bool {
        // SPEC-HAL-012/328: the complete appearance already includes the cue.
        true
    }

    fn on_blur(&mut self, cx: &mut EventContext) {
        tracing::debug!("Button {:?}: lost focus", self.id.0);

        // SPEC-HAL-112 INV-H2: Focus loss cancels either activation lifetime.
        let pointer_active = self.pointer_activation.take();
        if pointer_active || self.keyboard_pulse_active.get() {
            self.keyboard_pulse_active.set(false);
            self.state = if self.hovered {
                ButtonState::Hovered
            } else {
                ButtonState::Normal
            };
            let release_anim = Animation {
                from: self.pressed_t.get(),
                to: 0.0,
                duration: 0.08,
                easing: Easing::EaseIn,
            };
            self.pressed_t.set(release_anim.to);
            self.pending_pressed_animation = Some((release_anim, cx.has_animation()));
        }
    }

    fn on_pointer_capture_cancel(
        &mut self,
        cx: &mut EventContext,
        pointer_id: crate::core::event::PointerId,
        _reason: crate::core::event::CancellationReason,
    ) {
        if !self.pointer_activation.owns(pointer_id) || !self.pointer_activation.take() {
            return;
        }
        // INV-H2: Cancel only the pointer lifetime; a held keyboard press is
        // independent and must still activate on its own valid release.
        let keyboard_pressed = self.keyboard_pulse_active.get();
        self.state = if keyboard_pressed {
            ButtonState::Pressed
        } else if self.hovered {
            ButtonState::Hovered
        } else {
            ButtonState::Normal
        };
        let release_anim = Animation {
            from: self.pressed_t.get(),
            to: if keyboard_pressed { 1.0 } else { 0.0 },
            duration: 0.08,
            easing: Easing::EaseIn,
        };
        self.pressed_t.set(release_anim.to);
        self.pending_pressed_animation = Some((release_anim, cx.has_animation()));
    }

    // ========================================================================
    // SPEC-HAL-038 Stage 6: Keyboard Activation
    // ========================================================================

    fn accepts_keyboard_activation(&self) -> bool {
        self.enabled
    }

    fn on_keyboard_activate(&mut self, cx: &mut EventContext) {
        if !self.enabled || self.keyboard_activation_retired {
            return;
        }
        // SPEC-HAL-038 Stage 6 + SPEC-HAL-021-A: Paired keyboard press matching
        // mouse Down/Up. KeyDown engages the press visual (held); the paired
        // KeyUp event in handle_event releases the press and fires on_click.
        // Identical semantics to PointerDown→PointerUp.
        //
        // Autorepeat guard: the framework fires on_keyboard_activate repeatedly
        // while the key is held (KeyDown autorepeat for Enter/Space, TextInput
        // autorepeat for 'i'). Subsequent invocations while the press is
        // already engaged are no-ops — the visual stays held without
        // re-triggering the press-in animation.
        if self.keyboard_pulse_active.get() {
            return;
        }
        self.keyboard_pulse_active.set(true);

        // SPEC-HAL-021-A: Mirror mouse PointerDown press-in animation
        // (35ms EaseOut, pressed_t 0→1). After completion pressed_t stays
        // at 1.0 (held); update sampling cannot clear keyboard_pulse_active.
        let anim = Animation {
            from: self.pressed_t.get(),
            to: 1.0,
            duration: 0.035,
            easing: Easing::EaseOut,
        };
        self.pressed_t.set(anim.to);
        self.pending_pressed_animation = Some((anim, cx.has_animation()));
    }

    // ========================================================================
    // SPEC-HAL-038 Stage 7: Hover Lifecycle
    // SPEC-HAL-021: Hover animation integration
    // ========================================================================

    fn on_hover_enter(&mut self, cx: &mut EventContext) {
        if !self.enabled {
            return;
        }
        self.hovered = true;
        // Only enter hover if not currently pressed (pressed state takes priority)
        if self.state != ButtonState::Pressed {
            tracing::debug!("Button {:?}: hover enter", self.id.0);
            self.state = ButtonState::Hovered;

            // SPEC-HAL-021: Queue the existing hover transition for update.
            let anim = Animation {
                from: self.hover_t.get(),
                to: 1.0,
                duration: 0.12,
                easing: Easing::EaseOut,
            };
            // PRE-STAGE-8 FIX: When animation unavailable, set value directly.
            // Without this, hover_t could stay at 0 forever if capability is None.
            self.hover_t.set(anim.to);
            self.pending_hover_animation = Some((anim, cx.has_animation()));
        }
    }

    fn on_hover_leave(&mut self, _cx: &mut EventContext) {
        self.hovered = false;
        if self.state != ButtonState::Pressed {
            self.state = ButtonState::Normal;
            self.hover_t.set(0.0);
            self.pending_hover_animation = None;
        }
    }

    // ========================================================================
    // SPEC-HAL-021 Stage 7: Mount-Time Animation Reset
    // ANIM-INV-10: All animation-backed visual state MUST be initialized
    // to neutral values at widget mount time.
    // ========================================================================

    fn on_mount(&mut self) {
        self.keyboard_activation_retired = false;
        // Reset all animation caches to neutral (zero) values.
        // This ensures no visual contamination from prior state or
        // uninitialized memory on first paint.
        self.hover_t.set(0.0);
        self.hover_anim.set(None);
        self.pressed_t.set(0.0);
        self.pressed_anim.set(None);
        self.keyboard_pulse_active.set(false);
        self.pointer_activation.cancel();
        self.pending_hover_animation = None;
        self.pending_pressed_animation = None;
        self.hovered = false;
    }

    fn debug_type_name(&self) -> &'static str {
        "Button"
    }
}

#[cfg(test)]
mod sdf_request_tests {
    use super::*;
    use crate::core::appearance::ProductAppearanceOwner;

    #[test]
    fn button_material_policy_defaults_to_standard_and_survives_theme_changes() {
        use crate::core::app::{AppContext, ModalStack, ScreenContext, ScreenStack};
        use crate::core::theme::DensityMode;

        for density in [
            DensityMode::Compact,
            DensityMode::Regular,
            DensityMode::Intermediate30,
            DensityMode::Comfortable,
        ] {
            for mut theme in [
                Theme::light_with_density(density),
                Theme::dark_with_density(density),
            ] {
                assert_eq!(theme.button_material_policy, ButtonMaterialPolicy::Standard);
                let dark = theme.is_dark();
                for policy in [ButtonMaterialPolicy::Standard, ButtonMaterialPolicy::SdfV3] {
                    let mut screen_theme = theme.clone();
                    screen_theme.button_material_policy = policy;
                    let mut app = AppContext::new(screen_theme);
                    let mut screens = ScreenStack::new();
                    let mut modals = ModalStack::new();
                    let mut screen = ScreenContext::new(&mut app, &mut screens, &mut modals, None);

                    assert_eq!(screen.app.theme.is_dark(), dark);
                    assert_eq!(screen.app.theme.density, density);
                    assert_eq!(screen.app.theme.button_material_policy, policy);
                    assert!(!screen.app.take_theme_changed());

                    for expected_dark in [!dark, dark] {
                        screen.toggle_theme();
                        assert_eq!(screen.app.theme.is_dark(), expected_dark);
                        assert_eq!(screen.app.theme.density, density);
                        assert_eq!(screen.app.theme.button_material_policy, policy);
                        assert!(screen.app.take_theme_changed());
                        assert!(!screen.app.take_theme_changed());
                    }
                }

                theme.button_material_policy = ButtonMaterialPolicy::SdfV3;
                theme.toggle();
                assert_eq!(theme.is_dark(), !dark);
                assert_eq!(theme.density, density);
                assert_eq!(theme.button_material_policy, ButtonMaterialPolicy::SdfV3);
                theme.set_density(DensityMode::Regular);
                assert_eq!(theme.button_material_policy, ButtonMaterialPolicy::SdfV3);
            }
        }
        assert_eq!(Theme::default().button_material_policy, ButtonMaterialPolicy::Standard);
        assert_eq!(ButtonMaterialPolicy::default(), ButtonMaterialPolicy::Standard);
    }

    #[test]
    fn button_material_policy_resolves_in_update_and_respects_overrides() {
        use ButtonSdfRestingProfile::{Dark, DarkV3, LightV3};

        let mut theme = Theme::light();
        let mut button = Button::with_text("Apply");
        let mut sibling = Button::new(NodeId(0));
        let mut cx = UpdateContext::with_theme(None, &theme);
        button.update(0.0, &mut cx);
        assert_eq!(button.sdf_request_snapshot(), None);

        theme.button_material_policy = ButtonMaterialPolicy::SdfV3;
        let mut cx = UpdateContext::with_theme(None, &theme);
        for control in [&mut button, &mut sibling] {
            control.update(0.0, &mut cx);
            assert_eq!(control.sdf_request_snapshot().unwrap().profile, LightV3);
        }
        let initial = button.sdf_request_snapshot().unwrap();
        assert!(!button.needs_update());
        button.request_sdf_material(Some(LightV3)).unwrap();
        assert!(button.needs_update());
        button.update(0.0, &mut cx);
        assert_eq!(button.sdf_request_snapshot(), Some(initial));
        assert!(!button.needs_update());

        theme.toggle();
        let mut cx = UpdateContext::with_theme(None, &theme);
        button.update(0.0, &mut cx);
        sibling.update(0.0, &mut cx);
        assert_eq!(button.sdf_request_snapshot(), Some(initial));
        assert_eq!(sibling.sdf_request_snapshot().unwrap().profile, DarkV3);

        button.request_sdf_material(None).unwrap();
        button.update(0.0, &mut cx);
        assert_eq!(button.sdf_request_snapshot(), None);
        button.use_default_appearance();
        button.update(0.0, &mut cx);
        assert_eq!(button.sdf_request_snapshot(), None);
        button.use_theme_material();
        assert!(button.needs_update());
        button.update(0.0, &mut cx);
        let inherited = button.sdf_request_snapshot().unwrap();
        assert_eq!(inherited.profile, DarkV3);

        button.request_sdf_material(Some(DarkV3)).unwrap();
        button.update(0.0, &mut cx);
        assert_eq!(button.sdf_request_snapshot(), Some(inherited));
        button.use_theme_material();
        assert!(button.needs_update());
        button.update(0.0, &mut cx);
        assert_eq!(button.sdf_request_snapshot(), Some(inherited));
        assert!(!button.needs_update());

        // Restoring inheritance before Update cancels an unpublished explicit
        // selection. Without a Theme, the effective profile stays published.
        button.request_sdf_material(Some(Dark)).unwrap();
        button.use_theme_material();
        sibling = Button::with_text("Rollback");
        let mut missing = UpdateContext::new(None);
        button.update(0.0, &mut missing);
        sibling.update(0.0, &mut missing);
        assert_eq!(button.sdf_request_snapshot(), Some(inherited));
        assert_eq!(sibling.sdf_request_snapshot(), None);
        theme.button_material_policy = ButtonMaterialPolicy::Standard;
        let mut cx = UpdateContext::with_theme(None, &theme);
        button.update(0.0, &mut cx);
        assert_eq!(button.sdf_request_snapshot(), None);
    }

    #[test]
    fn button_material_policy_keeps_prepared_style_priority_and_disabled_suppression() {
        let mut theme = Theme::light();
        theme.button_material_policy = ButtonMaterialPolicy::SdfV3;
        let mut button = Button::with_text("Stage");
        let mut cx = UpdateContext::with_theme(None, &theme);
        let style = button.appearance_style(&theme).unwrap();
        button.set_appearance(style.clone(), None).unwrap();
        button.update(0.0, &mut cx);
        assert_eq!(button.sdf_request_snapshot(), None);
        assert_eq!(button.appearance_style(&theme).unwrap(), style);

        button.use_default_appearance();
        button.update(0.0, &mut cx);
        assert_eq!(
            button.sdf_request_snapshot().unwrap().profile,
            ButtonSdfRestingProfile::LightV3
        );
        button.apply_availability_policy(false);
        button.update(0.0, &mut cx);
        assert_eq!(button.sdf_request_snapshot(), None);
    }

    #[test]
    fn button_sdf_request_publishes_only_in_update_and_preserves_prepared_priority() {
        let theme = Theme::light();
        let mut button = Button::with_text("Stage");
        let mut cx = UpdateContext::with_theme(None, &theme);
        button.update(0.0, &mut cx);
        cx.take_widget_invalidations();
        let original_projection = button.resolved_appearance.clone();
        let original_bounds = button.layout_bounds;
        let default_style = button.appearance_style(&theme).unwrap();
        assert_eq!(button.sdf_request_snapshot(), None);
        assert_eq!(button.sdf_request.revision, None);
        assert!(button.pending_ordinary_appearance.is_none());
        assert!(!button.needs_update());

        button
            .request_sdf_material(Some(ButtonSdfRestingProfile::Light))
            .unwrap();
        assert!(button.needs_update());
        assert_eq!(button.sdf_request_snapshot(), None);
        assert_eq!(button.sdf_request.revision, None);
        button.update(0.0, &mut cx);
        let first = button.sdf_request_snapshot().unwrap();
        let material_style = button.appearance_style(&theme).unwrap();
        assert_eq!(first.profile, ButtonSdfRestingProfile::Light);
        assert_eq!(
            first.revision,
            crate::renderer::sdf_material_status::SdfMaterialRevision::initial()
        );
        assert_eq!(cx.take_widget_invalidations(), (false, false, false));
        assert!(!button.needs_update());
        assert!(button.pending_ordinary_appearance.is_some());
        assert_eq!(button.resolved_appearance, original_projection);

        button
            .request_sdf_material(Some(ButtonSdfRestingProfile::Light))
            .unwrap();
        assert!(!button.needs_update());
        button.update(0.0, &mut cx);
        assert_eq!(button.sdf_request_snapshot(), Some(first));
        assert_eq!(cx.take_widget_invalidations(), (false, false, false));
        button
            .request_sdf_material(Some(ButtonSdfRestingProfile::Dark))
            .unwrap();
        button
            .request_sdf_material(Some(ButtonSdfRestingProfile::Light))
            .unwrap();
        button.update(0.0, &mut cx);
        assert_eq!(button.sdf_request_snapshot(), Some(first));
        assert_eq!(cx.take_widget_invalidations(), (false, false, false));
        button
            .request_sdf_material(Some(ButtonSdfRestingProfile::Dark))
            .unwrap();
        assert_eq!(button.sdf_request_snapshot(), Some(first));
        button.update(0.0, &mut cx);
        let dark = button.sdf_request_snapshot().unwrap();
        assert_eq!(dark.profile, ButtonSdfRestingProfile::Dark);
        assert_eq!(dark.revision, first.revision.checked_increment().unwrap());
        assert_eq!(cx.take_widget_invalidations(), (false, false, false));

        let product = ProductAppearanceOwner(7);
        let selector = AppearanceSelector::button();
        let contract =
            ButtonAppearanceContract::new(vec![selector.clone()], Some(product)).unwrap();
        let configuration = ButtonAppearanceConfiguration::new(
            AppearanceConfigurationId {
                owner: AppearanceConfigurationOwner::Product(product),
                generation: 1,
            },
            contract.clone(),
            vec![ButtonAppearanceDefinition {
                selector: selector.clone(),
                values: theme.button_appearance_values(&contract),
            }],
            Vec::new(),
        )
        .unwrap();
        let style = configuration
            .prepare_style(selector, AppearanceFallbackPolicy::Reject)
            .unwrap();
        let selection = Some(ProductSelectionFact {
            owner: product,
            selected: true,
        });
        button.set_appearance(style.clone(), selection).unwrap();
        assert_eq!(button.sdf_request_snapshot(), Some(dark));
        button.update(0.0, &mut cx);
        cx.take_widget_invalidations();
        assert_eq!(button.sdf_request_snapshot(), None);
        assert!(button.pending_ordinary_appearance.is_none());
        assert_eq!(button.appearance_style(&theme).unwrap(), style);
        assert_eq!(button.requested_appearance.as_ref().unwrap().1, selection);
        button
            .request_sdf_material(Some(ButtonSdfRestingProfile::Light))
            .unwrap();
        button.update(0.0, &mut cx);
        assert_eq!(button.sdf_request_snapshot(), None);
        assert_eq!(button.appearance_style(&theme).unwrap(), style);
        assert_eq!(button.requested_appearance.as_ref().unwrap().1, selection);
        assert_eq!(cx.take_widget_invalidations(), (false, false, false));
        assert!(!button.needs_update());

        button.use_default_appearance();
        button.update(0.0, &mut cx);
        cx.take_widget_invalidations();
        let light = button.sdf_request_snapshot().unwrap();
        assert_eq!(light.profile, ButtonSdfRestingProfile::Light);
        assert_eq!(light.revision, dark.revision.checked_increment().unwrap());
        assert_eq!(button.appearance_style(&theme).unwrap(), material_style);
        button.request_sdf_material(None).unwrap();
        assert_eq!(button.sdf_request_snapshot(), Some(light));
        button.update(0.0, &mut cx);
        assert_eq!(button.sdf_request_snapshot(), None);
        assert!(button.pending_ordinary_appearance.is_none());
        assert_eq!(cx.take_widget_invalidations(), (false, false, false));
        assert_eq!(button.appearance_style(&theme).unwrap(), default_style);
        let withdrawn_revision = button.sdf_request.revision.unwrap();
        assert_eq!(
            withdrawn_revision,
            light.revision.checked_increment().unwrap()
        );
        button.request_sdf_material(None).unwrap();
        assert!(!button.needs_update());
        button
            .request_sdf_material(Some(ButtonSdfRestingProfile::Light))
            .unwrap();
        button.update(0.0, &mut cx);
        assert_eq!(
            button.sdf_request_snapshot().unwrap().revision,
            withdrawn_revision.checked_increment().unwrap()
        );
        assert_eq!(button.resolved_appearance, original_projection);
        assert_eq!(button.layout_bounds, original_bounds);
        assert_eq!(cx.take_widget_invalidations(), (false, false, false));
        assert!(!button.needs_update());
        // No framework facts were fabricated: this is local Update request proof,
        // not an admitted resource, App-frame, rendered-Button or Focus proof.
    }
}
