// SPEC-HAL-335 §4 invariant tests: TimeGeometryEngine.

use crate::engines::temporal::time_geometry::{iso_week, iso_weeks_in_year};
use crate::engines::temporal::{TimeGeometryEngine, TimeScale, WorkingCalendar};
use crate::model::date_value::DateValue;
use crate::model::time::{MICROS_PER_DAY, MICROS_PER_HOUR, TimeFrame, Timestamp, from_civil};
use crate::model::time_value::TimeValue;

fn d(y: i32, m: u8, day: u8) -> DateValue {
    DateValue::new(y, m, day).unwrap()
}

fn mid() -> TimeValue {
    TimeValue::new(0, 0, None)
}

fn ts(date: DateValue) -> Timestamp {
    from_civil(date, mid(), TimeFrame::default())
}

// 2026-06-08 is a Monday. 40 px/day, 400 px viewport → Jun 8 .. Jun 18.
fn engine_days() -> TimeGeometryEngine {
    TimeGeometryEngine::new(ts(d(2026, 6, 8)), TimeScale::Days, 40.0, 400.0)
}

#[test]
fn mapping_inverse_within_half_pixel() {
    let e = engine_days();
    for x in [0.0_f32, 1.0, 39.5, 200.0, 399.0, 400.0] {
        let rt = e.x_of(e.instant_at(x));
        assert!((rt - x).abs() <= 0.5, "INV-335-G2: x={} rt={}", x, rt);
    }
    assert!((e.x_of(ts(d(2026, 6, 10))) - 80.0).abs() < 0.001);
}

#[test]
fn zoom_preserves_instant_under_cursor() {
    let mut e = engine_days();
    let x = 120.0_f32;
    let pinned = e.instant_at(x);
    e.zoom_at(x, 2.0);
    let drift = (e.x_of(pinned) - x).abs();
    assert!(drift <= 0.5, "INV-335-G3: drift {}", drift);
    assert!((e.px_per_unit() - 80.0).abs() < 0.001);
    e.zoom_at(x, 1000.0); // clamps at the max limit
    assert!((e.px_per_unit() - 512.0).abs() < 0.001);
}

#[test]
fn scroll_moves_forward_in_time() {
    let mut e = engine_days();
    let before = e.visible_range();
    e.scroll_by_px(40.0); // exactly one day forward
    let after = e.visible_range();
    assert_eq!(after.start.0 - before.start.0, MICROS_PER_DAY);
    e.scroll_to(ts(d(2026, 7, 1)));
    assert_eq!(e.visible_range().start, ts(d(2026, 7, 1)));
}

#[test]
fn set_scale_preserves_mapping() {
    let mut e = engine_days(); // 40 px/day
    let probe = ts(d(2026, 6, 12));
    let x_before = e.x_of(probe);
    e.set_scale(TimeScale::Weeks); // 280 px/week == identical px-per-µs
    assert!((e.x_of(probe) - x_before).abs() < 0.01);
    assert!((e.px_per_unit() - 280.0).abs() < 0.001);
}

#[test]
fn tiers_days_scale() {
    let e = engine_days();
    let r = e.resolve();
    assert_eq!(r.tiers.len(), 2);
    let top = &r.tiers[0];
    assert_eq!(top.cells.len(), 1);
    assert_eq!(top.cells[0].label, "Jun 2026");
    let bottom = &r.tiers[1];
    assert_eq!(bottom.cells.len(), 10);
    assert_eq!(bottom.cells[0].label, "8");
    assert!((bottom.cells[0].x - 0.0).abs() < 0.001);
    assert!((bottom.cells[0].width - 40.0).abs() < 0.001);
    assert_eq!(bottom.cells[9].label, "17");
}

#[test]
fn tiers_cross_month_boundary() {
    let e = TimeGeometryEngine::new(ts(d(2026, 6, 28)), TimeScale::Days, 40.0, 400.0);
    let r = e.resolve(); // Jun 28 .. Jul 8
    let top = &r.tiers[0];
    assert_eq!(top.cells.len(), 2);
    assert_eq!(top.cells[0].label, "Jun 2026");
    assert_eq!(top.cells[1].label, "Jul 2026");
    assert!((top.cells[1].x - e.x_of(ts(d(2026, 7, 1)))).abs() < 0.001);
}

#[test]
fn week_tier_iso_labels() {
    // 2026-06-08 is the Monday of ISO week 24.
    let e = TimeGeometryEngine::new(ts(d(2026, 6, 8)), TimeScale::Weeks, 70.0, 280.0);
    let r = e.resolve(); // 4 weeks
    let bottom = &r.tiers[1];
    assert_eq!(bottom.cells.len(), 4);
    assert_eq!(bottom.cells[0].label, "W24");
    assert_eq!(bottom.cells[1].label, "W25");
}

#[test]
fn iso_week_year_edges() {
    assert_eq!(iso_weeks_in_year(2026), 53); // Jan 1 2026 is a Thursday
    assert_eq!(iso_weeks_in_year(2025), 52);
    assert_eq!(iso_week(d(2026, 1, 1)), 1);
    assert_eq!(iso_week(d(2027, 1, 1)), 53); // belongs to 2026's W53
    assert_eq!(iso_week(d(2024, 12, 30)), 1); // Monday of 2025 W1
}

#[test]
fn non_working_weekend_merged() {
    let e = engine_days(); // default Mon–Fri calendar
    let r = e.resolve();
    // One merged span: Sat Jun 13 00:00 .. Mon Jun 15 00:00.
    assert_eq!(r.non_working.len(), 1);
    let (x0, x1) = r.non_working[0];
    assert!((x0 - e.x_of(ts(d(2026, 6, 13)))).abs() < 0.001);
    assert!((x1 - e.x_of(ts(d(2026, 6, 15)))).abs() < 0.001);
}

#[test]
fn holiday_is_non_working() {
    let mut e = engine_days();
    let mut cal = WorkingCalendar::default();
    cal.holidays.push(d(2026, 6, 10));
    e.set_calendar(cal);
    let r = e.resolve();
    assert_eq!(r.non_working.len(), 2); // Wed holiday + weekend
    let (x0, x1) = r.non_working[0];
    assert!((x0 - 80.0).abs() < 0.001);
    assert!((x1 - 120.0).abs() < 0.001);
}

#[test]
fn off_hours_at_hours_scale() {
    // 24 visible hours of a working Wednesday; 16 px/hour, 384 px viewport.
    let e = TimeGeometryEngine::new(ts(d(2026, 6, 10)), TimeScale::Hours, 16.0, 384.0);
    let r = e.resolve();
    // Default 09:00–17:00 window → two spans: 00–09 and 17–24.
    assert_eq!(r.non_working.len(), 2);
    let (a0, a1) = r.non_working[0];
    let (b0, b1) = r.non_working[1];
    assert!((a0 - 0.0).abs() < 0.001 && (a1 - 9.0 * 16.0).abs() < 0.001);
    assert!((b0 - 17.0 * 16.0).abs() < 0.001 && (b1 - 384.0).abs() < 0.001);
}

#[test]
fn day_scale_emits_no_off_hours() {
    // px_per_hour = 40/24 < 4.0 → off-hours suppressed; days still emitted.
    let e = engine_days();
    let r = e.resolve();
    for &(x0, x1) in &r.non_working {
        assert!(
            x1 - x0 >= 79.0,
            "only whole-day spans expected at Days scale"
        );
    }
}

#[test]
fn gridlines_at_fine_boundaries() {
    let e = engine_days();
    let r = e.resolve();
    assert_eq!(r.gridlines.len(), 10); // day starts at x = 0, 40, …, 360
    assert!((r.gridlines[1] - 40.0).abs() < 0.001);
}

#[test]
fn now_marker_inside_and_outside() {
    let mut e = engine_days();
    e.set_now(Some(ts(d(2026, 6, 10))));
    assert_eq!(e.resolve().now_x.map(|x| x.round() as i32), Some(80));
    e.set_now(Some(ts(d(2026, 7, 10))));
    assert_eq!(e.resolve().now_x, None);
    e.set_now(None);
    assert_eq!(e.resolve().now_x, None);
}

#[test]
fn working_time_between_spans_weekend() {
    let e = engine_days(); // Mon–Fri, 09:00–17:00
    let a = ts(d(2026, 6, 12)); // Fri 00:00
    let b = ts(d(2026, 6, 16)); // Tue 00:00 → Fri + Mon working
    assert_eq!(e.working_time_between(a, b), 2 * 8 * MICROS_PER_HOUR);
    // Clipped start: Fri from 12:00 → 5h Fri + 8h Mon.
    let a2 = from_civil(
        d(2026, 6, 12),
        TimeValue::new(12, 0, None),
        TimeFrame::default(),
    );
    assert_eq!(e.working_time_between(a2, b), 13 * MICROS_PER_HOUR);
    assert_eq!(e.working_time_between(b, a), 0);
}

#[test]
fn frame_offset_shifts_day_boundaries() {
    let mut e = engine_days();
    e.set_frame(TimeFrame {
        utc_offset_minutes: 120,
    }); // +02:00
    // Anchor is 2026-06-08 00:00 UTC == 02:00 local; local Jun 8 began 2h earlier.
    let r = e.resolve();
    let first = &r.tiers[1].cells[0];
    assert_eq!(first.label, "8");
    let expected_x = -((2 * MICROS_PER_HOUR) as f64 * (40.0 / MICROS_PER_DAY as f64)) as f32;
    assert!((first.x - expected_x).abs() < 0.01);
}
