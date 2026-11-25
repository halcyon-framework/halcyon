// SPEC-HAL-334 invariant tests: Timestamp, TimeRange, civil projection.

use crate::model::date_value::DateValue;
use crate::model::time::{
    MICROS_PER_DAY, TimeFrame, TimeRange, Timestamp, day_of_week, from_civil, to_civil,
};
use crate::model::time_value::TimeValue;

fn d(y: i32, m: u8, day: u8) -> DateValue {
    DateValue::new(y, m, day).unwrap()
}

fn t(h: u8, mi: u8, s: u8) -> TimeValue {
    TimeValue::new(h, mi, Some(s))
}

#[test]
fn epoch_is_1970_01_01_thursday_utc() {
    let (date, time) = to_civil(Timestamp(0), TimeFrame::default());
    assert_eq!(date, d(1970, 1, 1));
    assert_eq!((time.hour, time.minute, time.second), (0, 0, Some(0)));
    assert_eq!(day_of_week(date), 3); // 0 = Monday → Thursday = 3
}

#[test]
fn known_dates_round_trip_and_weekday() {
    let frame = TimeFrame::default();
    let ts = from_civil(d(2026, 6, 10), t(15, 4, 30), frame);
    let (date, time) = to_civil(ts, frame);
    assert_eq!(date, d(2026, 6, 10));
    assert_eq!((time.hour, time.minute, time.second), (15, 4, Some(30)));
    assert_eq!(day_of_week(date), 2); // Wednesday
    assert_eq!(day_of_week(d(2000, 1, 1)), 5); // Saturday
}

#[test]
fn pre_epoch_round_trip() {
    let frame = TimeFrame::default();
    let ts = from_civil(d(1969, 12, 31), t(23, 59, 59), frame);
    assert_eq!(ts, Timestamp(-1_000_000));
    let (date, time) = to_civil(ts, frame);
    assert_eq!(date, d(1969, 12, 31));
    assert_eq!((time.hour, time.minute, time.second), (23, 59, Some(59)));
}

#[test]
fn fixed_offset_inverse_both_signs() {
    // INV-334-4: to_civil and from_civil are mutually inverse under a fixed offset.
    let plus = TimeFrame {
        utc_offset_minutes: 330,
    }; // +05:30
    let (date, time) = to_civil(Timestamp(0), plus);
    assert_eq!(date, d(1970, 1, 1));
    assert_eq!((time.hour, time.minute), (5, 30));
    assert_eq!(from_civil(date, time, plus), Timestamp(0));

    let minus = TimeFrame {
        utc_offset_minutes: -480,
    }; // -08:00
    let (date, time) = to_civil(Timestamp(0), minus);
    assert_eq!(date, d(1969, 12, 31));
    assert_eq!((time.hour, time.minute), (16, 0));
    assert_eq!(from_civil(date, time, minus), Timestamp(0));
}

#[test]
fn to_civil_monotonic() {
    // INV-334-4: monotonic across negative values and day boundaries.
    let frame = TimeFrame::default();
    let key = |us: i64| {
        let (date, time) = to_civil(Timestamp(us), frame);
        (date, time.hour, time.minute, time.second)
    };
    let points = [
        -2 * MICROS_PER_DAY,
        -MICROS_PER_DAY - 1,
        -MICROS_PER_DAY,
        -1,
        0,
        1,
        MICROS_PER_DAY - 1,
        MICROS_PER_DAY,
        2 * MICROS_PER_DAY + 12_345,
    ];
    for w in points.windows(2) {
        assert!(
            key(w[0]) <= key(w[1]),
            "INV-334-4: projection must be monotonic"
        );
    }
}

#[test]
fn time_range_half_open() {
    // INV-334-3: [start, end) — end excluded, edge-touching does not intersect.
    let r = TimeRange::new(Timestamp(100), Timestamp(200));
    assert!(r.contains(Timestamp(100)));
    assert!(r.contains(Timestamp(199)));
    assert!(!r.contains(Timestamp(200)));

    let touching = TimeRange::new(Timestamp(200), Timestamp(300));
    assert!(!r.intersects(touching));
    assert_eq!(r.intersect(touching), None);

    let overlap = TimeRange::new(Timestamp(150), Timestamp(250));
    assert_eq!(
        r.intersect(overlap),
        Some(TimeRange::new(Timestamp(150), Timestamp(200)))
    );
    assert!(TimeRange::new(Timestamp(5), Timestamp(5)).is_empty());
}

#[test]
fn leap_day_and_month_boundary() {
    let frame = TimeFrame::default();
    let ts = from_civil(d(2024, 2, 29), t(12, 0, 0), frame);
    let (date, _) = to_civil(ts, frame);
    assert_eq!(date, d(2024, 2, 29));
    let next = to_civil(Timestamp(ts.0 + MICROS_PER_DAY), frame).0;
    assert_eq!(next, d(2024, 3, 1));
}

#[test]
fn second_none_reads_as_zero() {
    let frame = TimeFrame::default();
    let a = from_civil(d(2026, 6, 10), TimeValue::new(9, 0, None), frame);
    let b = from_civil(d(2026, 6, 10), TimeValue::new(9, 0, Some(0)), frame);
    assert_eq!(a, b);
}
