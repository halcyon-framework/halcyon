// SPEC-HAL-096A: Style Primitives & Decoration Stack
//
// Formalizes existing visual decoration patterns:
// - BorderStyle: 2-rect inset technique
// - FocusStyle: expanded rect at LAYER_FOCUS
// - HoverStyle: premultiplied alpha overlay
// - DecorationStack: ordered application of style primitives

use crate::core::math::RectF;
use crate::core::paint::{Color, LAYER_FOCUS, PaintSink, RoundedCorners, ShadowMode};

/// SPEC-R-015-v2: Default focus ring parameters for ShadowRing variant.
pub const FOCUS_RING_SPREAD: f32 = 1.0;
pub const FOCUS_RING_SOFTNESS: f32 = 0.8;
pub const FOCUS_RING_DEFAULT_Z: u32 = 1;

// ============================================================================
// BorderStyle: 2-rect inset border technique
// ============================================================================

/// Border decoration using the established 2-rect inset pattern.
///
/// Rendering: outer rect (border color) + inner rect inset (fill color).
/// Inner radius = max(0, outer_radius - inset_radius_delta).
#[derive(Debug, Clone, Copy)]
pub struct BorderStyle {
    /// Border thickness in logical pixels
    pub thickness: f32,
    /// Border color
    pub color: Color,
    /// Corner radius for outer rect
    pub radius: f32,
    /// Radius reduction for inner rect (usually equals thickness)
    pub inset_radius_delta: f32,
}

impl BorderStyle {
    /// Create a border style with uniform radius delta
    pub fn new(thickness: f32, color: Color, radius: f32) -> Self {
        Self {
            thickness,
            color,
            radius,
            inset_radius_delta: thickness,
        }
    }

    /// Render border using 2-rect inset technique.
    ///
    /// Draws outer rect with `border_color`, inner rect with `fill_color`.
    pub fn paint(&self, sink: &mut dyn PaintSink, rect: RectF, fill_color: Color, z_index: u32) {
        // Outer rect (border)
        sink.push_rounded_rect(
            rect,
            self.color,
            RoundedCorners::uniform(self.radius),
            z_index,
            None,
        );

        // Inner rect (fill) - inset by thickness
        let inner_rect = RectF::new(
            rect.x + self.thickness,
            rect.y + self.thickness,
            (rect.width - 2.0 * self.thickness).max(0.0),
            (rect.height - 2.0 * self.thickness).max(0.0),
        );
        let inner_radius = (self.radius - self.inset_radius_delta).max(0.0);
        sink.push_rounded_rect(
            inner_rect,
            fill_color,
            RoundedCorners::uniform(inner_radius),
            z_index,
            None,
        );
    }
}

// ============================================================================
// FocusStyle: Keyboard focus indicator (enum-backed for multiple techniques)
// ============================================================================

/// Focus ring decoration supporting multiple rendering techniques.
///
/// Two variants exist to cover existing widget patterns:
/// - `StrokeRect`: stroke-only border (Button pattern, renders at layer)
/// - `FillExpandedRect`: filled expanded rect (Checkbox pattern, renders at z)
#[derive(Debug, Clone, Copy)]
pub enum FocusStyle {
    /// Stroke-only border rect (Button pattern).
    /// Renders at specified layer (default: LAYER_FOCUS).
    StrokeRect {
        thickness: f32,
        color: Color,
        radius: f32,
        layer: u32,
    },
    /// Filled expanded rect (Checkbox pattern).
    /// Expands rect by `expansion` pixels, renders filled rect at specified z.
    FillExpandedRect {
        expansion: f32,
        color: Color,
        radius: f32,
        z: u32,
    },
    /// SPEC-R-015-v2: Gaussian-soft ring via shadow pipeline (ShadowMode::Ring).
    /// Renders as a smooth glow band around the shape boundary.
    ShadowRing {
        /// Distance from shape boundary to ring center (logical pixels)
        spread: f32,
        /// Gaussian sigma controlling ring softness (logical pixels)
        softness: f32,
        /// Ring color (typically accent color, semi-transparent)
        color: Color,
        /// Corner radius of the shape being ringed
        radius: f32,
        /// Z-index for the ring
        z: u32,
    },
}

impl FocusStyle {
    /// Create a stroke-rect focus style (Button default).
    pub fn stroke_rect(thickness: f32, color: Color, radius: f32) -> Self {
        Self::StrokeRect {
            thickness,
            color,
            radius,
            layer: LAYER_FOCUS,
        }
    }

    /// Create a fill-expanded-rect focus style (Checkbox default).
    pub fn fill_expanded(expansion: f32, color: Color, radius: f32, z: u32) -> Self {
        Self::FillExpandedRect {
            expansion,
            color,
            radius,
            z,
        }
    }

    /// Create a shadow-ring focus style (smooth Gaussian glow).
    pub fn shadow_ring(spread: f32, softness: f32, color: Color, radius: f32) -> Self {
        Self::ShadowRing {
            spread,
            softness,
            color,
            radius,
            z: 1, // below widget content
        }
    }

    /// Create a shadow-ring with explicit z-index.
    pub fn shadow_ring_z(spread: f32, softness: f32, color: Color, radius: f32, z: u32) -> Self {
        Self::ShadowRing {
            spread,
            softness,
            color,
            radius,
            z,
        }
    }

    /// Render focus ring with alpha modulation.
    ///
    /// `alpha_mul` scales the focus ring color's alpha (1.0 = full, 0.0 = invisible).
    /// StrokeRect: uses `push_border_rounded_rect` at `layer` with rounding.
    /// FillExpandedRect: expands rect by `expansion`, uses `push_rounded_rect` at `z`.
    pub fn paint(&self, sink: &mut dyn PaintSink, rect: RectF, alpha_mul: f32) {
        match self {
            Self::StrokeRect {
                thickness,
                color,
                radius,
                layer,
            } => {
                let modulated = color.with_alpha(color.a * alpha_mul);
                sink.push_border_rounded_rect(
                    rect,
                    modulated,
                    *thickness,
                    RoundedCorners::uniform(*radius),
                    *layer,
                    None,
                );
            }
            Self::FillExpandedRect {
                expansion,
                color,
                radius,
                z,
            } => {
                let expanded = RectF::new(
                    rect.x - expansion,
                    rect.y - expansion,
                    rect.width + expansion + expansion,
                    rect.height + expansion + expansion,
                );
                let modulated = color.with_alpha(color.a * alpha_mul);
                sink.push_rounded_rect(
                    expanded,
                    modulated,
                    RoundedCorners::uniform(*radius),
                    *z,
                    None,
                );
            }
            Self::ShadowRing {
                spread,
                softness,
                color,
                radius,
                z,
            } => {
                let modulated = color.with_alpha(color.a * alpha_mul);
                sink.push_shadow(
                    rect,
                    *radius,
                    ShadowMode::Ring,
                    (0.0, 0.0),
                    *spread,
                    *softness,
                    modulated,
                    *z,
                    None,
                );
            }
        }
    }
}

// ============================================================================
// HoverStyle: Animated hover overlay
// ============================================================================

/// Hover overlay decoration using premultiplied alpha.
///
/// Rendering: white overlay with alpha derived from animation value.
/// Uses quadratic ramp: `alpha = t² × max_alpha`.
#[derive(Debug, Clone, Copy)]
pub struct HoverStyle {
    /// Maximum alpha at full hover (0.0-1.0)
    pub max_alpha: f32,
    /// Corner radius (matches widget radius)
    pub radius: f32,
}

impl HoverStyle {
    pub fn new(max_alpha: f32, radius: f32) -> Self {
        Self { max_alpha, radius }
    }

    /// Render hover overlay with given animation value.
    ///
    /// `hover_t`: Animation value [0.0, 1.0] from hover animation.
    /// Uses quadratic ramp and straight-alpha white (shader premultiplies).
    pub fn paint(&self, sink: &mut dyn PaintSink, rect: RectF, hover_t: f32, z_index: u32) {
        if hover_t <= 0.001 {
            return;
        }

        // Quadratic ramp for faster visual response
        // Alpha contract: pass STRAIGHT alpha to GPU, shader premultiplies
        let alpha = hover_t * hover_t * self.max_alpha;
        let overlay_color = Color::from_srgb(1.0, 1.0, 1.0, alpha); // White overlay (straight)

        sink.push_rounded_rect(
            rect,
            overlay_color,
            RoundedCorners::uniform(self.radius),
            z_index,
            None,
        );
    }
}

// ============================================================================
// DecorationStack: Ordered application of style primitives
// ============================================================================

/// Ordered decoration application.
///
/// Fixed order (widgets may omit but not reorder):
/// 1. Background
/// 2. BorderStyle
/// 3. HoverStyle
/// 4. FocusStyle
///
/// This struct holds optional style references; widgets call `paint()` with
/// runtime state flags to apply decorations in correct order.
#[derive(Debug, Clone, Copy)]
pub struct DecorationStack {
    /// Background color
    pub background: Color,
    /// Background corner radius
    pub radius: f32,
    /// Optional border style
    pub border: Option<BorderStyle>,
    /// Optional focus style
    pub focus: Option<FocusStyle>,
    /// Optional hover style
    pub hover: Option<HoverStyle>,
}

/// Runtime state flags for decoration painting
#[derive(Debug, Clone, Copy, Default)]
pub struct DecorationState {
    /// Whether widget has keyboard focus (legacy boolean, use focus_t for animated)
    pub focused: bool,
    /// Focus animation value [0.0, 1.0] — modulates focus ring alpha
    pub focus_t: f32,
    /// Hover animation value [0.0, 1.0]
    pub hover_t: f32,
    /// Whether hover overlay should be drawn (false when pressed)
    pub draw_hover: bool,
}

impl DecorationStack {
    /// Create a minimal decoration stack with just background
    pub fn background(color: Color, radius: f32) -> Self {
        Self {
            background: color,
            radius,
            border: None,
            focus: None,
            hover: None,
        }
    }

    /// Add border style
    pub fn with_border(mut self, border: BorderStyle) -> Self {
        self.border = Some(border);
        self
    }

    /// Add focus style
    pub fn with_focus(mut self, focus: FocusStyle) -> Self {
        self.focus = Some(focus);
        self
    }

    /// Add hover style
    pub fn with_hover(mut self, hover: HoverStyle) -> Self {
        self.hover = Some(hover);
        self
    }

    /// Paint all decorations in correct order.
    ///
    /// Order: Background (z) → Border (z) → Hover (z+1) → Focus (style-determined z/layer)
    ///
    /// INVARIANT: This ordering is FIXED. Widgets may omit layers but CANNOT
    /// reorder them. Focus style determines its own z/layer target.
    pub fn paint(
        &self,
        sink: &mut dyn PaintSink,
        rect: RectF,
        state: DecorationState,
        base_z: u32,
    ) {
        // ORDERING INVARIANT: These steps MUST execute in this exact sequence.
        // Do NOT add conditional reordering logic.

        // 1. Background
        if let Some(border) = &self.border {
            // Use border's 2-rect technique
            border.paint(sink, rect, self.background, base_z);
        } else {
            // Simple background rect
            sink.push_rounded_rect(
                rect,
                self.background,
                RoundedCorners::uniform(self.radius),
                base_z,
                None,
            );
        }

        // 2. Hover overlay (above background, below focus)
        if state.draw_hover {
            if let Some(hover) = &self.hover {
                hover.paint(sink, rect, state.hover_t, base_z + 1);
            }
        }

        // 3. Focus ring (z/layer determined by FocusStyle variant)
        // Use focus_t for animated fade; fall back to boolean focused for non-animated widgets
        let effective_focus_t = if state.focus_t > 0.001 {
            state.focus_t
        } else if state.focused {
            1.0
        } else {
            0.0
        };
        if effective_focus_t > 0.001 {
            if let Some(focus) = &self.focus {
                focus.paint(sink, rect, effective_focus_t);
            }
        }
    }
}

// ============================================================================
// StyleSpec: Theme-level style defaults
// ============================================================================

/// Theme-level style defaults.
///
/// Holds default BorderStyle, FocusStyle, HoverStyle values derived from
/// current hardcoded widget values. Widgets can use these directly or
/// override per-instance.
///
/// INTENTIONALLY INCOMPLETE (SPEC-HAL-096A scope lock):
/// - No shadows
/// - No gradients
/// - No strokes beyond rect primitives
/// - Depth is color-only, per SPEC
///
/// Do NOT add new knobs (elevation, shadow, multi-layer borders) until
/// this system proves itself on Button → Checkbox → TextField → List.
#[derive(Debug, Clone, Copy)]
pub struct StyleSpec {
    /// Focus style defaults for Button (StrokeRect pattern)
    pub focus_button: FocusButtonDefaults,
    /// Focus style defaults for Checkbox (FillExpandedRect pattern)
    pub focus_checkbox: FocusCheckboxDefaults,
    /// Default hover overlay style (extracted from Button: 0.20 max alpha)
    pub hover: HoverStyleDefaults,
    /// Default pressed overlay style (extracted from Button: 0.18 max alpha)
    pub pressed: PressedStyleDefaults,
}

/// Focus style defaults for Button (StrokeRect pattern).
/// Color comes from theme.colors.borders.focus at runtime.
#[derive(Debug, Clone, Copy)]
pub struct FocusButtonDefaults {
    /// Focus ring thickness (Button: 1.5)
    pub thickness: f32,
    /// Layer for focus ring (default: LAYER_FOCUS)
    pub layer: u32,
}

/// Focus style defaults for Checkbox (FillExpandedRect pattern).
/// Color comes from theme.colors.borders.focus at runtime.
#[derive(Debug, Clone, Copy)]
pub struct FocusCheckboxDefaults {
    /// Expansion amount in pixels (Checkbox: focus_ring_padding)
    pub expansion: f32,
    /// Z-index for focus rect (Checkbox: 1, behind border)
    pub z: u32,
}

/// Hover style defaults
#[derive(Debug, Clone, Copy)]
pub struct HoverStyleDefaults {
    /// Maximum hover overlay alpha (Button: 0.20)
    pub max_alpha: f32,
}

/// Pressed style defaults
#[derive(Debug, Clone, Copy)]
pub struct PressedStyleDefaults {
    /// Maximum pressed overlay alpha (Button: 0.18)
    pub max_alpha: f32,
}

impl Default for StyleSpec {
    fn default() -> Self {
        Self {
            focus_button: FocusButtonDefaults {
                thickness: 1.5, // From Button paint()
                layer: LAYER_FOCUS,
            },
            focus_checkbox: FocusCheckboxDefaults {
                expansion: 2.0, // From theme.geometry.checkbox.focus_ring_padding
                z: 1,           // Checkbox focus renders behind border (z=2)
            },
            hover: HoverStyleDefaults {
                max_alpha: 0.20, // From Button paint(): hover_t² × 0.20
            },
            pressed: PressedStyleDefaults {
                max_alpha: 0.18, // From Button paint(): pressed_t² × 0.18
            },
        }
    }
}
