// SPEC-HAL-297: Plot Tool System — trait, types, built-in tools.

use crate::core::event::Modifiers;
use crate::core::paint::Color;
use crate::plot::PlotColorScheme;
use crate::plot::series::candle::Vertex as OverlayVertex;
use crate::plot::transform::Transform;

/// Phase of a pointer gesture within a tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GesturePhase {
    Down,
    Move,
    Up,
    Cancel,
}

/// Result of tool gesture handling.
pub enum GestureResult {
    /// Tool consumed the event.
    Handled,
    /// Tool consumed the event and produced output.
    Output(ToolOutput),
    /// Tool did not handle — fall through to default.
    Passthrough,
}

/// Output produced by a completed tool gesture.
pub enum ToolOutput {
    Measurement(MeasureResult),
    Annotation(AnnotationData),
    Custom(Box<dyn std::any::Any + Send>),
}

/// Result of a completed measurement gesture.
#[derive(Debug, Clone)]
pub struct MeasureResult {
    pub start_x: f64,
    pub start_y: f64,
    pub end_x: f64,
    pub end_y: f64,
    pub y_distance: f64,
    pub y_percent: f64,
    pub x_distance: f64,
}

/// Geometry describing an annotation produced by a tool.
pub enum AnnotationData {
    HorizontalLine {
        y: f64,
    },
    VerticalLine {
        x: f64,
    },
    Rectangle {
        x_min: f64,
        x_max: f64,
        y_min: f64,
        y_max: f64,
    },
    TrendLine {
        start: (f64, f64),
        end: (f64, f64),
    },
    FibonacciRetracement {
        top: f64,
        bottom: f64,
        x_start: f64,
        x_end: f64,
    },
    Text {
        position: (f64, f64),
        content: String,
    },
    Custom(Box<dyn std::any::Any + Send>),
}

/// INV-297-1: Tools do not mutate ViewState, Series, or selection.
/// They produce ToolOutput only.
pub trait PlotTool: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;

    fn handle_gesture(
        &mut self,
        phase: GesturePhase,
        screen_x: f32,
        screen_y: f32,
        transform: &Transform,
        modifiers: Modifiers,
    ) -> GestureResult;

    fn deactivate(&mut self) {}

    /// Resolve colors for built-in theme-managed tool affordances.
    ///
    /// Custom tools keep ownership of their own overlay colors unless they
    /// explicitly implement this hook.
    fn apply_color_scheme(&mut self, _scheme: &PlotColorScheme) {}

    fn overlay_vertices(
        &self,
        transform: &Transform,
        viewport_w: f32,
        viewport_h: f32,
    ) -> Vec<OverlayVertex> {
        let _ = (transform, viewport_w, viewport_h);
        Vec::new()
    }

    /// Text label to render during gesture.
    /// Returns (text, screen_x, screen_y) or None.
    fn gesture_label(&self) -> Option<(String, f32, f32)> {
        None
    }
}

/// Default tool — all events pass through to Plot's default handler.
pub struct PointerTool;

impl PlotTool for PointerTool {
    fn id(&self) -> &str {
        "pointer"
    }
    fn name(&self) -> &str {
        "Pointer"
    }

    fn handle_gesture(
        &mut self,
        _phase: GesturePhase,
        _sx: f32,
        _sy: f32,
        _transform: &Transform,
        _modifiers: Modifiers,
    ) -> GestureResult {
        GestureResult::Passthrough
    }
}

/// Drag between two points to measure distance and percentage.
pub struct MeasureTool {
    start: Option<(f64, f64)>,
    end: Option<(f64, f64)>,
    start_px: Option<(f32, f32)>,
    end_px: Option<(f32, f32)>,
    /// Custom label formatter. Receives a MeasureResult, returns the label string.
    formatter: Option<Box<dyn Fn(&MeasureResult) -> String + Send + Sync>>,
    line_color: Color,
    guide_color: Color,
}

impl MeasureTool {
    pub fn new() -> Self {
        Self {
            start: None,
            end: None,
            start_px: None,
            end_px: None,
            formatter: None,
            line_color: Color::from_linear(1.0, 1.0, 1.0, 0.8),
            guide_color: Color::from_linear(0.5, 0.5, 0.5, 0.4),
        }
    }

    /// Set a custom formatter for the measurement label.
    pub fn with_formatter(
        mut self,
        f: impl Fn(&MeasureResult) -> String + Send + Sync + 'static,
    ) -> Self {
        self.formatter = Some(Box::new(f));
        self
    }

    fn clear(&mut self) {
        self.start = None;
        self.end = None;
        self.start_px = None;
        self.end_px = None;
    }
}

impl PlotTool for MeasureTool {
    fn id(&self) -> &str {
        "measure"
    }
    fn name(&self) -> &str {
        "Measure"
    }

    fn handle_gesture(
        &mut self,
        phase: GesturePhase,
        sx: f32,
        sy: f32,
        transform: &Transform,
        _modifiers: Modifiers,
    ) -> GestureResult {
        match phase {
            GesturePhase::Down => {
                let wx = transform.px_to_x(sx);
                let wy = transform.px_to_y(sy);
                self.start = Some((wx, wy));
                self.end = Some((wx, wy));
                self.start_px = Some((sx, sy));
                self.end_px = Some((sx, sy));
                GestureResult::Handled
            }
            GesturePhase::Move => {
                let wx = transform.px_to_x(sx);
                let wy = transform.px_to_y(sy);
                self.end = Some((wx, wy));
                self.end_px = Some((sx, sy));
                GestureResult::Handled
            }
            GesturePhase::Up => {
                if let (Some((sx_w, sy_w)), Some((ex_w, ey_w))) = (self.start, self.end) {
                    let y_dist = ey_w - sy_w;
                    let y_pct = if sy_w.abs() > f64::EPSILON {
                        y_dist / sy_w
                    } else {
                        0.0
                    };
                    let result = MeasureResult {
                        start_x: sx_w,
                        start_y: sy_w,
                        end_x: ex_w,
                        end_y: ey_w,
                        y_distance: y_dist,
                        y_percent: y_pct,
                        x_distance: ex_w - sx_w,
                    };
                    self.clear();
                    GestureResult::Output(ToolOutput::Measurement(result))
                } else {
                    self.clear();
                    GestureResult::Handled
                }
            }
            GesturePhase::Cancel => {
                self.clear();
                GestureResult::Handled
            }
        }
    }

    fn deactivate(&mut self) {
        self.clear();
    }

    fn apply_color_scheme(&mut self, scheme: &PlotColorScheme) {
        self.line_color = scheme.tool_overlay;
        self.guide_color = scheme.tool_guide;
    }

    fn gesture_label(&self) -> Option<(String, f32, f32)> {
        let (sx_w, sy_w) = self.start?;
        let (ex_w, ey_w) = self.end?;
        let (spx, spy) = self.start_px?;
        let (epx, epy) = self.end_px?;

        let y_dist = ey_w - sy_w;
        let y_pct = if sy_w.abs() > f64::EPSILON {
            y_dist / sy_w
        } else {
            0.0
        };

        let result = MeasureResult {
            start_x: sx_w,
            start_y: sy_w,
            end_x: ex_w,
            end_y: ey_w,
            y_distance: y_dist,
            y_percent: y_pct,
            x_distance: ex_w - sx_w,
        };

        let label = if let Some(ref fmt) = self.formatter {
            fmt(&result)
        } else {
            format!(
                "{:+.2} ({:+.1}%) | dx: {:.1}",
                y_dist,
                y_pct * 100.0,
                result.x_distance
            )
        };

        // Top-right corner of the measure rectangle (same offset style as axis labels)
        let label_x = spx.max(epx) + 6.0;
        let label_y = spy.min(epy) - 16.0;

        Some((label, label_x, label_y))
    }

    fn overlay_vertices(
        &self,
        transform: &Transform,
        _viewport_w: f32,
        _viewport_h: f32,
    ) -> Vec<OverlayVertex> {
        // Convert world coordinates to physical pixels via transform
        let (sw_x, sw_y) = match self.start {
            Some(p) => p,
            None => return Vec::new(),
        };
        let (ew_x, ew_y) = match self.end {
            Some(p) => p,
            None => return Vec::new(),
        };
        let sx = transform.x_to_px(sw_x);
        let sy = transform.y_to_px(sw_y);
        let ex = transform.x_to_px(ew_x);
        let ey = transform.y_to_px(ew_y);

        let mut verts = Vec::new();
        let color = self.line_color.to_array();
        let thickness = 1.0_f32;

        let dx = ex - sx;
        let dy = ey - sy;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1.0 {
            return verts;
        }

        let nx = -dy / len * thickness / 2.0;
        let ny = dx / len * thickness / 2.0;

        // Dashed diagonal line: 6px on, 4px off
        let dash = 6.0_f32;
        let gap = 4.0_f32;
        let segment = dash + gap;
        let num_segments = (len / segment).ceil() as usize;

        for i in 0..num_segments {
            let t_start = (i as f32 * segment) / len;
            let t_end = ((i as f32 * segment + dash) / len).min(1.0);

            let x1 = sx + dx * t_start;
            let y1 = sy + dy * t_start;
            let x2 = sx + dx * t_end;
            let y2 = sy + dy * t_end;

            verts.push(OverlayVertex {
                position: [x1 + nx, y1 + ny],
                color,
            });
            verts.push(OverlayVertex {
                position: [x2 + nx, y2 + ny],
                color,
            });
            verts.push(OverlayVertex {
                position: [x1 - nx, y1 - ny],
                color,
            });
            verts.push(OverlayVertex {
                position: [x1 - nx, y1 - ny],
                color,
            });
            verts.push(OverlayVertex {
                position: [x2 + nx, y2 + ny],
                color,
            });
            verts.push(OverlayVertex {
                position: [x2 - nx, y2 - ny],
                color,
            });
        }

        // Horizontal + vertical guide lines
        let guide_color = self.guide_color.to_array();
        let gt = 1.0_f32;

        // Horizontal guide at start_y
        let (hx1, hx2) = if sx < ex { (sx, ex) } else { (ex, sx) };
        verts.push(OverlayVertex {
            position: [hx1, sy],
            color: guide_color,
        });
        verts.push(OverlayVertex {
            position: [hx2, sy],
            color: guide_color,
        });
        verts.push(OverlayVertex {
            position: [hx1, sy + gt],
            color: guide_color,
        });
        verts.push(OverlayVertex {
            position: [hx1, sy + gt],
            color: guide_color,
        });
        verts.push(OverlayVertex {
            position: [hx2, sy],
            color: guide_color,
        });
        verts.push(OverlayVertex {
            position: [hx2, sy + gt],
            color: guide_color,
        });

        // Vertical guide at end_x
        let (vy1, vy2) = if sy < ey { (sy, ey) } else { (ey, sy) };
        verts.push(OverlayVertex {
            position: [ex, vy1],
            color: guide_color,
        });
        verts.push(OverlayVertex {
            position: [ex + gt, vy1],
            color: guide_color,
        });
        verts.push(OverlayVertex {
            position: [ex, vy2],
            color: guide_color,
        });
        verts.push(OverlayVertex {
            position: [ex, vy2],
            color: guide_color,
        });
        verts.push(OverlayVertex {
            position: [ex + gt, vy1],
            color: guide_color,
        });
        verts.push(OverlayVertex {
            position: [ex + gt, vy2],
            color: guide_color,
        });

        verts
    }
}

/// Click at a Y level to emit a horizontal line annotation.
/// The consumer converts this to a PriceLevel (SPEC-299).
pub struct HorizontalLineTool {
    /// World Y position during gesture (for preview line).
    preview_y: Option<f64>,
    color: Color,
}

impl HorizontalLineTool {
    pub fn new() -> Self {
        Self {
            preview_y: None,
            color: Color::from_linear(0.8, 0.8, 0.8, 0.6),
        }
    }
}

impl PlotTool for HorizontalLineTool {
    fn id(&self) -> &str {
        "hline"
    }
    fn name(&self) -> &str {
        "Horizontal Line"
    }

    fn handle_gesture(
        &mut self,
        phase: GesturePhase,
        _sx: f32,
        sy: f32,
        transform: &Transform,
        _modifiers: Modifiers,
    ) -> GestureResult {
        match phase {
            GesturePhase::Down => {
                self.preview_y = Some(transform.px_to_y(sy));
                GestureResult::Handled
            }
            GesturePhase::Move => {
                self.preview_y = Some(transform.px_to_y(sy));
                GestureResult::Handled
            }
            GesturePhase::Up => {
                let world_y = transform.px_to_y(sy);
                self.preview_y = None;
                GestureResult::Output(ToolOutput::Annotation(AnnotationData::HorizontalLine {
                    y: world_y,
                }))
            }
            GesturePhase::Cancel => {
                self.preview_y = None;
                GestureResult::Handled
            }
        }
    }

    fn deactivate(&mut self) {
        self.preview_y = None;
    }

    fn apply_color_scheme(&mut self, scheme: &PlotColorScheme) {
        self.color = scheme.tool_horizontal_line;
    }

    fn overlay_vertices(
        &self,
        transform: &Transform,
        viewport_w: f32,
        _viewport_h: f32,
    ) -> Vec<OverlayVertex> {
        let world_y = match self.preview_y {
            Some(y) => y,
            None => return Vec::new(),
        };

        let y = transform.y_to_px(world_y);
        let mut verts = Vec::new();
        let color = self.color.to_array();
        let thickness = 1.0_f32;

        let dash = 8.0_f32;
        let gap = 4.0_f32;
        let mut x = 0.0_f32;
        while x < viewport_w {
            let end = (x + dash).min(viewport_w);
            let y1 = y - thickness / 2.0;
            let y2 = y + thickness / 2.0;
            verts.push(OverlayVertex {
                position: [x, y1],
                color,
            });
            verts.push(OverlayVertex {
                position: [end, y1],
                color,
            });
            verts.push(OverlayVertex {
                position: [x, y2],
                color,
            });
            verts.push(OverlayVertex {
                position: [x, y2],
                color,
            });
            verts.push(OverlayVertex {
                position: [end, y1],
                color,
            });
            verts.push(OverlayVertex {
                position: [end, y2],
                color,
            });
            x += dash + gap;
        }
        verts
    }
}

/// Drag to define a rectangle in world coordinates.
/// Emits AnnotationData::Rectangle. Consumer creates a PlotRegion.
pub struct DrawRectTool {
    start: Option<(f64, f64)>,
    end: Option<(f64, f64)>,
    /// Logical pixel positions for minimum-size check.
    start_px: Option<(f32, f32)>,
    end_px: Option<(f32, f32)>,
    fill_color: Color,
    border_color: Color,
}

impl DrawRectTool {
    pub fn new() -> Self {
        Self {
            start: None,
            end: None,
            start_px: None,
            end_px: None,
            fill_color: Color::from_linear(0.3, 0.5, 1.0, 0.1),
            border_color: Color::from_linear(0.3, 0.5, 1.0, 0.6),
        }
    }

    fn clear(&mut self) {
        self.start = None;
        self.end = None;
        self.start_px = None;
        self.end_px = None;
    }
}

impl PlotTool for DrawRectTool {
    fn id(&self) -> &str {
        "rect"
    }
    fn name(&self) -> &str {
        "Rectangle"
    }

    fn handle_gesture(
        &mut self,
        phase: GesturePhase,
        sx: f32,
        sy: f32,
        transform: &Transform,
        _modifiers: Modifiers,
    ) -> GestureResult {
        match phase {
            GesturePhase::Down => {
                let wx = transform.px_to_x(sx);
                let wy = transform.px_to_y(sy);
                self.start = Some((wx, wy));
                self.end = Some((wx, wy));
                self.start_px = Some((sx, sy));
                self.end_px = Some((sx, sy));
                GestureResult::Handled
            }
            GesturePhase::Move => {
                let wx = transform.px_to_x(sx);
                let wy = transform.px_to_y(sy);
                self.end = Some((wx, wy));
                self.end_px = Some((sx, sy));
                GestureResult::Handled
            }
            GesturePhase::Up => {
                if let (Some((sx_px, sy_px)), Some((ex_px, ey_px))) = (self.start_px, self.end_px) {
                    if (ex_px - sx_px).abs() >= 5.0 && (ey_px - sy_px).abs() >= 5.0 {
                        if let (Some((sw_x, sw_y)), Some((ew_x, ew_y))) = (self.start, self.end) {
                            self.clear();
                            return GestureResult::Output(ToolOutput::Annotation(
                                AnnotationData::Rectangle {
                                    x_min: sw_x.min(ew_x),
                                    x_max: sw_x.max(ew_x),
                                    y_min: sw_y.min(ew_y),
                                    y_max: sw_y.max(ew_y),
                                },
                            ));
                        }
                    }
                }
                self.clear();
                GestureResult::Handled
            }
            GesturePhase::Cancel => {
                self.clear();
                GestureResult::Handled
            }
        }
    }

    fn deactivate(&mut self) {
        self.clear();
    }

    fn apply_color_scheme(&mut self, scheme: &PlotColorScheme) {
        self.fill_color = scheme.tool_preview_fill;
        self.border_color = scheme.tool_preview_border;
    }

    fn overlay_vertices(
        &self,
        transform: &Transform,
        _viewport_w: f32,
        _viewport_h: f32,
    ) -> Vec<OverlayVertex> {
        let (sw_x, sw_y) = match self.start {
            Some(p) => p,
            None => return Vec::new(),
        };
        let (ew_x, ew_y) = match self.end {
            Some(p) => p,
            None => return Vec::new(),
        };

        // Convert to physical pixels
        let px1 = transform.x_to_px(sw_x);
        let py1 = transform.y_to_px(sw_y);
        let px2 = transform.x_to_px(ew_x);
        let py2 = transform.y_to_px(ew_y);

        let x1 = px1.min(px2);
        let x2 = px1.max(px2);
        let y1 = py1.min(py2);
        let y2 = py1.max(py2);

        let mut verts = Vec::new();
        let fill_color = self.fill_color.to_array();
        let border_color = self.border_color.to_array();
        let t = 1.0_f32;

        // Fill
        verts.push(OverlayVertex {
            position: [x1, y1],
            color: fill_color,
        });
        verts.push(OverlayVertex {
            position: [x2, y1],
            color: fill_color,
        });
        verts.push(OverlayVertex {
            position: [x1, y2],
            color: fill_color,
        });
        verts.push(OverlayVertex {
            position: [x1, y2],
            color: fill_color,
        });
        verts.push(OverlayVertex {
            position: [x2, y1],
            color: fill_color,
        });
        verts.push(OverlayVertex {
            position: [x2, y2],
            color: fill_color,
        });

        // Top
        verts.push(OverlayVertex {
            position: [x1, y1],
            color: border_color,
        });
        verts.push(OverlayVertex {
            position: [x2, y1],
            color: border_color,
        });
        verts.push(OverlayVertex {
            position: [x1, y1 + t],
            color: border_color,
        });
        verts.push(OverlayVertex {
            position: [x1, y1 + t],
            color: border_color,
        });
        verts.push(OverlayVertex {
            position: [x2, y1],
            color: border_color,
        });
        verts.push(OverlayVertex {
            position: [x2, y1 + t],
            color: border_color,
        });
        // Bottom
        verts.push(OverlayVertex {
            position: [x1, y2 - t],
            color: border_color,
        });
        verts.push(OverlayVertex {
            position: [x2, y2 - t],
            color: border_color,
        });
        verts.push(OverlayVertex {
            position: [x1, y2],
            color: border_color,
        });
        verts.push(OverlayVertex {
            position: [x1, y2],
            color: border_color,
        });
        verts.push(OverlayVertex {
            position: [x2, y2 - t],
            color: border_color,
        });
        verts.push(OverlayVertex {
            position: [x2, y2],
            color: border_color,
        });
        // Left
        verts.push(OverlayVertex {
            position: [x1, y1],
            color: border_color,
        });
        verts.push(OverlayVertex {
            position: [x1 + t, y1],
            color: border_color,
        });
        verts.push(OverlayVertex {
            position: [x1, y2],
            color: border_color,
        });
        verts.push(OverlayVertex {
            position: [x1, y2],
            color: border_color,
        });
        verts.push(OverlayVertex {
            position: [x1 + t, y1],
            color: border_color,
        });
        verts.push(OverlayVertex {
            position: [x1 + t, y2],
            color: border_color,
        });
        // Right
        verts.push(OverlayVertex {
            position: [x2 - t, y1],
            color: border_color,
        });
        verts.push(OverlayVertex {
            position: [x2, y1],
            color: border_color,
        });
        verts.push(OverlayVertex {
            position: [x2 - t, y2],
            color: border_color,
        });
        verts.push(OverlayVertex {
            position: [x2 - t, y2],
            color: border_color,
        });
        verts.push(OverlayVertex {
            position: [x2, y1],
            color: border_color,
        });
        verts.push(OverlayVertex {
            position: [x2, y2],
            color: border_color,
        });

        verts
    }
}
