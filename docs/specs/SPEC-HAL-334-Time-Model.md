# SPEC-HAL-334: **Time Model — Timestamp and Civil Types**

### *One Representation of Time Across the Framework*

**Author:** JC
**Status:** Draft (Binding upon adoption)
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-000 (Glossary & Naming Principles)

**Consumed by:** SPEC-HAL-296 (Plot), SPEC-HAL-335 (TemporalSubstrate), SPEC-HAL-238 (DatePicker), SPEC-HAL-273 (TimePicker)

---

# 1. Purpose

Define the single framework-wide representation of time. Every subsystem that stores, maps, or displays a point in time uses these types. No subsystem invents its own.

# 2. Problem Statement

Time is currently fragmented:

- `DateValue` (civil date) lives in `src/model/date_value.rs` — correct location, house type.
- `TimeValue` (civil time-of-day) lives inside `src/widgets/time_picker.rs` — a model type trapped in a widget file.
- SPEC-HAL-296 commits Plot's time axis to raw `i64` microseconds since UNIX epoch — correct representation, no named type.
- SPEC-HAL-282 referenced an opaque `DateTime` and `Date` that exist nowhere.

Four subsystems, four answers. Any composition that places a schedule next to a time-series view converts at every seam. This SPEC ends that.

# 3. Types

All types live in `src/model/`. All are house types. No external crate type crosses this module's public boundary (chrono remains available for serialization/interop at application edges only).

## 3.1 Timestamp — absolute instant

```rust
/// Absolute instant: microseconds since UNIX epoch, UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Timestamp(pub i64);
```

**INV-334-1:** `Timestamp` is the only absolute-instant type in the framework. Microsecond resolution, i64 range (±~292,000 years). Identical in meaning to the values SPEC-HAL-296 already mandates for `AxisKind::Time` — Plot is conformant by construction and adopts the named type at its next touch.

The name `Instant` is forbidden for this type (collision with `std::time::Instant`, which is monotonic and unrelated).

## 3.2 Civil types

- `DateValue` — exists, unchanged, authoritative civil date.
- `TimeValue` — relocates verbatim from `src/widgets/time_picker.rs` to `src/model/time_value.rs`. `TimePicker` imports it from the model. No semantic change.

**INV-334-2:** Civil types carry no frame information. A `DateValue`/`TimeValue` pair is meaningless as an instant until projected through a `TimeFrame` (§4).

## 3.3 TimeRange

```rust
/// Half-open absolute range [start, end).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimeRange {
    pub start: Timestamp,
    pub end: Timestamp,
}
```

**INV-334-3:** `start <= end` always. Construction enforces it. All range mathematics in the framework (intersection, containment, culling) operates on half-open ranges — no off-by-one ambiguity at boundaries.

# 4. TimeFrame — civil projection law

```rust
/// Civil projection frame: fixed offset from UTC, in minutes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TimeFrame {
    pub utc_offset_minutes: i32,
}
```

Conversion functions (free functions in `src/model/time.rs`):

```rust
pub fn to_civil(ts: Timestamp, frame: TimeFrame) -> (DateValue, TimeValue);
pub fn from_civil(date: DateValue, time: TimeValue, frame: TimeFrame) -> Timestamp;
pub fn day_of_week(date: DateValue) -> u8; // 0 = Monday
```

**INV-334-4 (Fixed-Offset Law):** Under a given `TimeFrame`, `to_civil` and `from_civil` are total, mutually inverse, and monotonic. This holds precisely because the offset is fixed.

**INV-334-5 (Scope honesty):** Timezone databases, DST transitions, and leap seconds are explicitly out of scope. Applications requiring local-time display supply the offset; applications spanning DST transitions accept fixed-offset semantics or convert at their own edge. This is a stated boundary, not an omission. Lifting it is a future SPEC, not a reinterpretation of this one.

# 5. Boundary Laws

**FORBID-334-1:** No new subsystem defines its own instant, date, time-of-day, or range type. Discovery of a private reimplementation is an architectural alarm per standing doctrine.

**FORBID-334-2:** No chrono (or other external) time type appears in any public signature of `src/model/`, `src/core/`, `src/plot/`, or any widget.

**FORBID-334-3:** No API mixes units. Microseconds everywhere `Timestamp` appears; conversion helpers (`Timestamp::from_millis`, `as_secs_f64`, etc.) are the only sanctioned unit boundary.

# 6. Adoption Map

- **Plot (296):** already i64 µs; adopts the `Timestamp` newtype opportunistically.
- **TemporalSubstrate (335):** consumes `Timestamp`, `TimeRange`, `TimeFrame`, civil types — exclusively.
- **DatePicker / TimePicker:** continue consuming civil types; `TimeValue` import path changes on relocation.

# 7. Non-Goals

- Timezone database, DST arithmetic (INV-334-5)
- Calendar systems other than proleptic Gregorian
- Duration arithmetic library beyond what consumers need (added on demand, in this module)
