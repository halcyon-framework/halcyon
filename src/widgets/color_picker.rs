// SPEC-HAL-272: ColorPicker Widget
// Interactive HSV color picker composing the SV area (custom shader pipeline)
// with a hue strip (gradient pipeline) plus click/drag interaction.

use crate::core::event::{Event, EventPhase, PointerButton, PointerEventKind};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::{Color, ColorStop, LAYER_CONTENT, RoundedCorners};
use crate::core::scene::linear_to_srgb;
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;

// ============================================================================
// HSV ↔ RGB Conversion
// ============================================================================

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let c = v * s;
    let hp = h * 6.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = if hp < 1.0 {
        (c, x, 0.0)
    } else if hp < 2.0 {
        (x, c, 0.0)
    } else if hp < 3.0 {
        (0.0, c, x)
    } else if hp < 4.0 {
        (0.0, x, c)
    } else if hp < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    (r + m, g + m, b + m)
}

fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let v = max;
    let s = if max > 0.0 { delta / max } else { 0.0 };

    let h = if delta < f32::EPSILON {
        0.0
    } else if (max - r).abs() < f32::EPSILON {
        ((g - b) / delta).rem_euclid(6.0) / 6.0
    } else if (max - g).abs() < f32::EPSILON {
        ((b - r) / delta + 2.0) / 6.0
    } else {
        ((r - g) / delta + 4.0) / 6.0
    };

    (h, s, v)
}

// ============================================================================
// State
// ============================================================================

#[derive(Copy, Clone, Debug, PartialEq)]
enum DragTarget {
    SvArea,
    HueStrip,
}

/// SPEC-HAL-272: Interactive HSV color picker.
///
/// Composes the SV area (custom GPU shader, `PipelineId::ColorPickerSv`) with
/// a hue strip (gradient pipeline) and click/drag interaction for both regions.
pub struct ColorPicker {
    id: WidgetInternalId,
    hue: f32,
    saturation: f32,
    value: f32,
    drag_target: Option<DragTarget>,
    /// Cached layout size for hit-testing in handle_event.
    layout_size: Size,
    /// When true, skip outer border/background in paint (overlay container provides those).
    embedded: bool,
}

impl ColorPicker {
    pub fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
            hue: 0.0,
            saturation: 1.0,
            value: 1.0,
            drag_target: None,
            layout_size: Size::zero(),
            embedded: false,
        }
    }

    /// Get the current color as an sRGB Color.
    pub fn color(&self) -> Color {
        let (r, g, b) = hsv_to_rgb(self.hue, self.saturation, self.value);
        Color::from_srgb(r, g, b, 1.0)
    }

    /// Set from an external Color (converts RGB → HSV).
    pub fn set_color(&mut self, color: Color) {
        // Color stores linear RGB internally; convert back to sRGB for HSV conversion.
        let r = linear_to_srgb(color.r);
        let g = linear_to_srgb(color.g);
        let b = linear_to_srgb(color.b);
        let (h, s, v) = rgb_to_hsv(r, g, b);
        self.hue = h;
        self.saturation = s;
        self.value = v;
    }

    /// Get HSV components.
    pub fn hsv(&self) -> (f32, f32, f32) {
        (self.hue, self.saturation, self.value)
    }

    /// Base z-layer for paint calls. When embedded in an overlay popup,
    /// content must be above the background blocker at LAYER_OVERLAY + 1.
    fn base_layer(&self) -> u32 {
        if self.embedded {
            crate::core::paint::LAYER_OVERLAY + 2
        } else {
            LAYER_CONTENT
        }
    }

    pub fn with_embedded(mut self, embedded: bool) -> Self {
        self.embedded = embedded;
        self
    }

    /// Get hex string representation of the current color.
    pub fn hex_string(&self) -> String {
        let (r, g, b) = hsv_to_rgb(self.hue, self.saturation, self.value);
        let ri = (r * 255.0).round() as u8;
        let gi = (g * 255.0).round() as u8;
        let bi = (b * 255.0).round() as u8;
        format!("#{:02X}{:02X}{:02X}", ri, gi, bi)
    }
}

impl Default for ColorPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl WidgetDowncast for ColorPicker {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ColorPicker {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let geom = &cx.theme.geometry.color_picker;
        let width = geom.sv_size;
        let height = geom.sv_size + geom.strip_gap + geom.hue_strip_height;

        let w = width.min(constraints.max_width);
        let h = height.min(constraints.max_height);
        self.layout_size = Size::new(w, h);
        LayoutResult::fit(self.layout_size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let geom = &cx.theme.geometry.color_picker;
        let radius = geom.radius;
        let border_color = cx.theme.colors.borders.default;
        let base = self.base_layer();

        // SV area rect
        let sv_rect = RectF::new(rect.x, rect.y, geom.sv_size, geom.sv_size);

        // Hue strip rect
        let strip_y = rect.y + geom.sv_size + geom.strip_gap;
        let strip_rect = RectF::new(rect.x, strip_y, geom.sv_size, geom.hue_strip_height);

        // --- SV Area (custom shader) ---
        cx.paint_color_picker_sv(sv_rect, self.hue, radius, base);

        // SV area border (skip when embedded — overlay container provides border)
        if !self.embedded {
            cx.scene.push_border_rounded_rect(
                sv_rect,
                border_color,
                1.0,
                RoundedCorners::uniform(radius),
                base + 1,
                None,
            );
        }

        // SV indicator dot
        let dot_x = sv_rect.x + self.saturation * sv_rect.width;
        let dot_y = sv_rect.y + (1.0 - self.value) * sv_rect.height;
        let dot_r = 5.0;
        let dot_rect = RectF::new(dot_x - dot_r, dot_y - dot_r, dot_r * 2.0, dot_r * 2.0);
        // White fill (rect pipeline for overlay compatibility)
        cx.scene.push_rect(
            dot_rect,
            Color::from_linear(1.0, 1.0, 1.0, 1.0),
            base + 2,
            None,
        );
        // Dark border
        cx.scene.push_border_rect(
            dot_rect,
            Color::from_linear(0.0, 0.0, 0.0, 0.5),
            1.0,
            base + 3,
            None,
        );

        // --- Hue Strip (gradient pipeline) ---
        let hue_stops = [
            ColorStop {
                offset: 0.000,
                color: Color::from_srgb(1.0, 0.0, 0.0, 1.0),
            },
            ColorStop {
                offset: 0.167,
                color: Color::from_srgb(1.0, 1.0, 0.0, 1.0),
            },
            ColorStop {
                offset: 0.333,
                color: Color::from_srgb(0.0, 1.0, 0.0, 1.0),
            },
            ColorStop {
                offset: 0.500,
                color: Color::from_srgb(0.0, 1.0, 1.0, 1.0),
            },
            ColorStop {
                offset: 0.667,
                color: Color::from_srgb(0.0, 0.0, 1.0, 1.0),
            },
            ColorStop {
                offset: 0.833,
                color: Color::from_srgb(1.0, 0.0, 1.0, 1.0),
            },
            ColorStop {
                offset: 1.000,
                color: Color::from_srgb(1.0, 0.0, 0.0, 1.0),
            },
        ];
        cx.paint_linear_gradient(strip_rect, radius, (0.0, 0.5), (1.0, 0.5), &hue_stops, base);

        // Hue strip border (skip when embedded — overlay container provides border)
        if !self.embedded {
            cx.scene.push_border_rounded_rect(
                strip_rect,
                border_color,
                1.0,
                RoundedCorners::uniform(radius),
                base + 1,
                None,
            );
        }

        // Hue indicator line
        let hue_x = strip_rect.x + self.hue * strip_rect.width;
        let indicator_w = 3.0;
        let indicator_rect = RectF::new(
            hue_x - indicator_w * 0.5,
            strip_rect.y,
            indicator_w,
            strip_rect.height,
        );
        // White fill (rect pipeline for overlay compatibility)
        cx.scene.push_rect(
            indicator_rect,
            Color::from_linear(1.0, 1.0, 1.0, 1.0),
            base + 2,
            None,
        );
        // Dark border
        cx.scene.push_border_rect(
            indicator_rect,
            Color::from_linear(0.0, 0.0, 0.0, 0.5),
            1.0,
            base + 3,
            None,
        );
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }

        match event {
            Event::Pointer(pe) => {
                let (local_x, local_y) = pe.local_pos;

                match pe.kind {
                    PointerEventKind::Down
                    | PointerEventKind::DoubleClick
                    | PointerEventKind::TripleClick => {
                        if pe.button != Some(PointerButton::Primary) {
                            return false;
                        }

                        // Determine which region was hit (SV area is a square with side = width)
                        if local_y >= 0.0
                            && local_y < self.layout_size.width
                            && local_x >= 0.0
                            && local_x <= self.layout_size.width
                        {
                            // SV area
                            self.drag_target = Some(DragTarget::SvArea);
                            self.saturation = (local_x / self.layout_size.width).clamp(0.0, 1.0);
                            self.value = (1.0 - local_y / self.layout_size.width).clamp(0.0, 1.0);
                            cx.capture_pointer(PointerButton::Primary);
                            true
                        } else if local_y > self.layout_size.width
                            && local_y <= self.layout_size.height
                        {
                            // Hue strip
                            self.drag_target = Some(DragTarget::HueStrip);
                            self.hue = (local_x / self.layout_size.width).clamp(0.0, 1.0);
                            cx.capture_pointer(PointerButton::Primary);
                            true
                        } else {
                            false
                        }
                    }
                    PointerEventKind::Move => {
                        if let Some(target) = self.drag_target {
                            match target {
                                DragTarget::SvArea => {
                                    self.saturation =
                                        (local_x / self.layout_size.width).clamp(0.0, 1.0);
                                    self.value =
                                        (1.0 - local_y / self.layout_size.width).clamp(0.0, 1.0);
                                }
                                DragTarget::HueStrip => {
                                    self.hue = (local_x / self.layout_size.width).clamp(0.0, 1.0);
                                }
                            }
                            true
                        } else {
                            false
                        }
                    }
                    PointerEventKind::Up => {
                        if pe.button != Some(PointerButton::Primary) {
                            return false;
                        }
                        if self.drag_target.is_some() {
                            self.drag_target = None;
                            true
                        } else {
                            false
                        }
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn debug_type_name(&self) -> &'static str {
        "ColorPicker"
    }
}
