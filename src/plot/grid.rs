// SPEC-HAL-296: Grid — Tick Calculation and Grid Line Geometry
//
// Calculates "nice" tick positions for axes and builds overlay
// vertices for grid lines. Drawn behind data series.

use crate::plot::AxisKind;
use crate::plot::series::candle::Vertex;
use crate::plot::transform::Transform;

/// Calculate nice tick positions for a range.
///
/// Returns evenly-spaced values at "nice" intervals (1, 2, 5, 10, 20, 50, etc.).
/// Ported from `scale.rs` in the original chart renderer.
pub fn calculate_ticks(min: f64, max: f64, target_count: usize) -> Vec<f64> {
    if max <= min || target_count == 0 {
        return vec![];
    }

    let range = max - min;
    let rough_step = range / target_count as f64;

    // Find nice step size
    let magnitude = 10_f64.powf(rough_step.log10().floor());
    let normalized = rough_step / magnitude;

    let nice_step = if normalized < 1.5 {
        magnitude
    } else if normalized < 3.5 {
        2.0 * magnitude
    } else if normalized < 7.5 {
        5.0 * magnitude
    } else {
        10.0 * magnitude
    };

    let first_tick = (min / nice_step).ceil() * nice_step;
    let mut ticks = Vec::new();
    let mut tick = first_tick;

    while tick <= max {
        ticks.push(tick);
        tick += nice_step;
    }

    ticks
}

/// Calculate tick positions for an axis kind.
pub fn calculate_axis_ticks(kind: AxisKind, min: f64, max: f64, target_count: usize) -> Vec<f64> {
    match kind {
        AxisKind::Time | AxisKind::Linear => calculate_ticks(min, max, target_count),
        AxisKind::Log => calculate_log_ticks(min, max, target_count),
    }
}

/// Calculate logarithmic tick positions for a positive range.
pub fn calculate_log_ticks(min: f64, max: f64, target_count: usize) -> Vec<f64> {
    if min <= 0.0 || max <= min || target_count == 0 || !min.is_finite() || !max.is_finite() {
        return Vec::new();
    }

    let start_exp = min.log10().floor() as i32;
    let end_exp = max.log10().ceil() as i32;
    let decade_count = (end_exp - start_exp + 1).max(1) as usize;
    let mantissas: &[f64] = if decade_count <= target_count.saturating_div(2).max(1) {
        &[1.0, 2.0, 5.0]
    } else {
        &[1.0]
    };

    let mut ticks = Vec::new();
    for exp in start_exp..=end_exp {
        let decade = 10_f64.powi(exp);
        for &mantissa in mantissas {
            let tick = mantissa * decade;
            if tick >= min && tick <= max {
                ticks.push(tick);
            }
        }
    }

    ticks.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    ticks.dedup_by(|a, b| (*a - *b).abs() <= f64::EPSILON * a.abs().max(b.abs()).max(1.0));
    ticks
}

/// Build grid line vertices for the current viewport.
///
/// Returns overlay vertices ready for the crosshair pipeline.
/// Horizontal lines at Y tick positions, vertical lines at X tick positions.
/// Color is subtle dark grey to stay behind data.
pub fn build_grid_vertices(
    transform: &Transform,
    width_px: f32,
    height_px: f32,
    x_target_ticks: usize,
    y_target_ticks: usize,
    grid_color: [f32; 4],
) -> Vec<Vertex> {
    let color = grid_color;
    let line_width = 1.0_f32;
    let half_w = line_width * 0.5;

    let mut verts = Vec::new();

    // Horizontal grid lines (at Y tick positions)
    let y_ticks = calculate_axis_ticks(
        transform.y_kind,
        transform.y_min,
        transform.y_max,
        y_target_ticks,
    );
    for &tick_y in &y_ticks {
        let Some(py) = transform.y_to_px_checked(tick_y).map(f32::round) else {
            continue;
        };
        if py >= 0.0 && py <= height_px {
            verts.push(Vertex {
                position: [0.0, py - half_w],
                color,
            });
            verts.push(Vertex {
                position: [width_px, py - half_w],
                color,
            });
            verts.push(Vertex {
                position: [0.0, py + half_w],
                color,
            });
            verts.push(Vertex {
                position: [0.0, py + half_w],
                color,
            });
            verts.push(Vertex {
                position: [width_px, py - half_w],
                color,
            });
            verts.push(Vertex {
                position: [width_px, py + half_w],
                color,
            });
        }
    }

    // Vertical grid lines (at X tick positions)
    let x_ticks = calculate_axis_ticks(
        transform.x_kind,
        transform.x_min,
        transform.x_max,
        x_target_ticks,
    );
    for &tick_x in &x_ticks {
        let Some(px) = transform.x_to_px_checked(tick_x).map(f32::round) else {
            continue;
        };
        if px >= 0.0 && px <= width_px {
            verts.push(Vertex {
                position: [px - half_w, 0.0],
                color,
            });
            verts.push(Vertex {
                position: [px + half_w, 0.0],
                color,
            });
            verts.push(Vertex {
                position: [px - half_w, height_px],
                color,
            });
            verts.push(Vertex {
                position: [px - half_w, height_px],
                color,
            });
            verts.push(Vertex {
                position: [px + half_w, 0.0],
                color,
            });
            verts.push(Vertex {
                position: [px + half_w, height_px],
                color,
            });
        }
    }

    verts
}

/// Format a numeric value with adaptive precision.
///
/// Uses magnitude to choose decimal places. Axis-agnostic.
pub fn format_value(value: f64) -> String {
    let abs = value.abs();
    if abs < 0.000001 {
        format!("{:.8}", value)
    } else if abs < 0.01 {
        format!("{:.6}", value)
    } else if abs < 1.0 {
        format!("{:.4}", value)
    } else if abs < 100.0 {
        format!("{:.2}", value)
    } else if abs < 10000.0 {
        format!("{:.1}", value)
    } else {
        format!("{:.0}", value)
    }
}

/// Format an axis tick with enough precision to distinguish adjacent ticks.
pub fn format_value_for_interval(value: f64, tick_interval: f64) -> String {
    if !value.is_finite() || !tick_interval.is_finite() || tick_interval <= 0.0 {
        return format_value(value);
    }
    let decimals = if tick_interval >= 1.0 {
        0
    } else {
        (-tick_interval.log10()).ceil().max(0.0) as usize
    }
    .min(12);
    let zero_threshold = tick_interval * 1e-9;
    let value = if value.abs() < zero_threshold {
        0.0
    } else {
        value
    };
    format!("{value:.decimals$}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_ticks_emit_decade_ticks_for_wide_ranges() {
        let ticks = calculate_log_ticks(1.0, 1000.0, 4);
        assert_eq!(ticks, vec![1.0, 10.0, 100.0, 1000.0]);
    }

    #[test]
    fn log_ticks_reject_non_positive_ranges() {
        assert!(calculate_log_ticks(0.0, 100.0, 4).is_empty());
        assert!(calculate_axis_ticks(AxisKind::Log, -10.0, 100.0, 4).is_empty());
    }
}
