use std::collections::{HashMap, HashSet};

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize)]
pub(super) struct Verdict {
    pub requirement: &'static str,
    pub status: &'static str,
    pub detail: String,
}

fn verdict(requirement: &'static str, status: &'static str, detail: impl Into<String>) -> Verdict {
    Verdict {
        requirement,
        status,
        detail: detail.into(),
    }
}

fn kind(record: &Value) -> Option<&str> {
    record.get("kind")?.as_str()
}

fn detail(record: &Value) -> &Value {
    &record["detail"]
}

fn pair(value: &Value) -> Option<[f64; 2]> {
    let values = value.as_array()?;
    if values.len() != 2 {
        return None;
    }
    Some([values[0].as_f64()?, values[1].as_f64()?])
}

fn scale(value: &Value) -> Option<usize> {
    known_scale(value.as_f64()?)
}

pub(super) fn known_scale(value: f64) -> Option<usize> {
    if (value - 1.0).abs() < 0.000_001 {
        Some(0)
    } else if (value - 2.0).abs() < 0.000_001 {
        Some(1)
    } else {
        None
    }
}

fn directions(delta: [f64; 2]) -> u8 {
    u8::from(delta[0] > 0.0)
        | (u8::from(delta[0] < 0.0) << 1)
        | (u8::from(delta[1] > 0.0) << 2)
        | (u8::from(delta[1] < 0.0) << 3)
}

fn is_ok(value: &Value) -> bool {
    value.as_str() == Some("Ok(())")
}

fn cleanup_sequence(record: &Value) -> bool {
    let d = detail(record);
    let Some(operations) = d["operations"].as_array() else {
        return false;
    };
    let names: Vec<_> = operations
        .iter()
        .filter_map(|operation| operation["operation"].as_str())
        .collect();
    let expected = if d["restore_safe"] == true {
        vec!["restore", "release", "show"]
    } else {
        vec!["release", "show"]
    };
    names == expected
        && operations
            .iter()
            .all(|operation| is_ok(&operation["result"]))
}

fn unsafe_no_warp(records: &[Value], reason: &str) -> (&'static str, usize) {
    let cleanups: Vec<_> = records
        .iter()
        .filter(|record| {
            kind(record) == Some("cleanup")
                && detail(record)["was_active"] == true
                && detail(record)["reason"] == reason
        })
        .collect();
    if cleanups.is_empty() {
        ("UNPROVEN", 0)
    } else if cleanups
        .iter()
        .any(|record| detail(record)["restore_safe"] != false || !cleanup_sequence(record))
    {
        ("FAIL", cleanups.len())
    } else {
        ("PASS", cleanups.len())
    }
}

fn canonical_locked_delta(d: &Value) -> Option<[f64; 2]> {
    let raw = pair(&d["native_delta"])?;
    let logical = pair(&d["normalized_logical_delta"])?;
    let fused = d["fused"]["relative_deltas"]
        .as_array()
        .filter(|events| events.len() == 1)
        .and_then(|events| pair(&events[0]))?;
    if (0..2).all(|axis| {
        raw[axis].is_finite()
            && logical[axis].is_finite()
            && fused[axis].is_finite()
            && (raw[axis] as f32).to_bits() == (logical[axis] as f32).to_bits()
            && (logical[axis] as f32).to_bits() == (fused[axis] as f32).to_bits()
    }) {
        Some(logical)
    } else {
        None
    }
}

const EDGE_BITS: [u8; 4] = [1, 2, 4, 8];
const EDGE_NAMES: [&str; 4] = ["left", "right", "top", "bottom"];

struct EdgePath {
    lease: u64,
    scale: usize,
    scale_factor: f64,
    size_physical: [f64; 2],
    position: [f64; 2],
    extent: [f64; 2],
    continuation: [u8; 4],
}

impl EdgePath {
    fn outside(&self, position: [f64; 2], edge: usize) -> bool {
        match edge {
            0 => position[0] < 0.0,
            1 => position[0] >= self.extent[0],
            2 => position[1] < 0.0,
            _ => position[1] >= self.extent[1],
        }
    }

    fn observe(&mut self, delta: [f64; 2]) -> u8 {
        let next = [self.position[0] + delta[0], self.position[1] + delta[1]];
        if !next.iter().all(|value| value.is_finite()) {
            return 0;
        }
        let mut proven = 0;
        for edge in 0..4 {
            let axis = usize::from(edge >= 2);
            let outward = if edge % 2 == 0 {
                delta[axis] < 0.0
            } else {
                delta[axis] > 0.0
            };
            if !self.outside(next, edge) {
                self.continuation[edge] = 0;
            } else if !self.outside(self.position, edge) && outward {
                self.continuation[edge] = 1;
            } else if self.continuation[edge] > 0 && outward {
                self.continuation[edge] = self.continuation[edge].saturating_add(1).min(4);
            }
            if self.continuation[edge] == 4 {
                proven |= EDGE_BITS[edge];
            }
        }
        self.position = next;
        proven
    }
}

#[derive(Default)]
struct NumericEdgeTracker {
    masks: [u8; 2],
    active: Option<EdgePath>,
}

impl NumericEdgeTracker {
    fn observe(&mut self, record: &Value) {
        let d = detail(record);
        match kind(record) {
            Some("lock_active") => {
                let (Some(scale), Some(scale_factor), Some(lease), Some(anchor), Some(size)) = (
                    scale(&d["window_scale"]),
                    d["window_scale"].as_f64(),
                    d["lease_id"].as_u64(),
                    pair(&d["anchor_logical"]),
                    pair(&d["window_size_physical"]),
                ) else {
                    self.active = None;
                    return;
                };
                let extent = [size[0] / scale_factor, size[1] / scale_factor];
                self.active = (0..2)
                    .all(|axis| {
                        extent[axis].is_finite()
                            && extent[axis] > 0.0
                            && anchor[axis].is_finite()
                            && anchor[axis] >= 0.0
                            && anchor[axis] < extent[axis]
                    })
                    .then_some(EdgePath {
                        lease,
                        scale,
                        scale_factor,
                        size_physical: size,
                        position: anchor,
                        extent,
                        continuation: [0; 4],
                    });
            }
            Some("device_mouse_motion") if d["locked"] == true && d["focused"] == true => {
                if self.active.as_ref().is_some_and(|active| {
                    d["window_scale"].as_f64() != Some(active.scale_factor)
                        || pair(&d["window_size_physical"]) != Some(active.size_physical)
                }) {
                    self.active = None;
                    return;
                }
                if let Some(active) = self.active.as_mut()
                    && d["lease_id"].as_u64() == Some(active.lease)
                    && scale(&d["window_scale"]) == Some(active.scale)
                    && let Some(delta) = canonical_locked_delta(d)
                {
                    self.masks[active.scale] |= active.observe(delta);
                }
            }
            Some("cleanup" | "window_destroy") => self.active = None,
            _ => {}
        }
    }

    fn remaining(&self, scale: usize, edge: usize) -> Option<u8> {
        let active = self
            .active
            .as_ref()
            .filter(|active| active.scale == scale)?;
        active
            .outside(active.position, edge)
            .then_some(3_u8.saturating_sub(active.continuation[edge].saturating_sub(1)))
    }
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
struct ScaleCoverage {
    unlocked_native: u8,
    unlocked_absolute: u8,
    locked_native: u8,
    locked_absolute: u8,
    unlocked_native_signs: u8,
    unlocked_absolute_signs: u8,
    locked_signs: u8,
}

impl ScaleCoverage {
    fn unlocked_ready(self) -> bool {
        self.unlocked_native >= 8
            && self.unlocked_absolute >= 8
            && self.unlocked_native_signs == 15
            && self.unlocked_absolute_signs == 15
    }

    fn locked_ready(self) -> bool {
        self.locked_native >= 8 && self.locked_signs == 15
    }
}

#[derive(Default)]
pub(super) struct CaptureProgress {
    scales: [ScaleCoverage; 2],
    edges: NumericEdgeTracker,
    restored: bool,
    released: bool,
    unmounted: bool,
    destroyed: bool,
    focus_lost: bool,
    shutdown: bool,
    pending_destroy: bool,
    pending_shutdown: bool,
}

impl CaptureProgress {
    fn scale_ready(&self, index: usize) -> bool {
        let scale = self.scales[index];
        scale.unlocked_ready()
            && scale.locked_ready()
            && scale.locked_absolute > 0
            && self.edges.masks[index] == 15
    }

    pub fn observe(&mut self, record: &Value) {
        self.edges.observe(record);
        let d = detail(record);
        match kind(record) {
            Some("device_mouse_motion") if d["focused"] == true => {
                let (Some(scale), Some(delta)) =
                    (scale(&d["window_scale"]), pair(&d["native_delta"]))
                else {
                    return;
                };
                let coverage = &mut self.scales[scale];
                if d["locked"] == true {
                    coverage.locked_native = coverage.locked_native.saturating_add(1).min(8);
                    coverage.locked_signs |= directions(delta);
                } else {
                    coverage.unlocked_native = coverage.unlocked_native.saturating_add(1).min(8);
                    coverage.unlocked_native_signs |= directions(delta);
                }
            }
            Some("cursor_moved") if d["focused"] == true => {
                if let Some(scale) = scale(&d["window_scale"]) {
                    let coverage = &mut self.scales[scale];
                    if d["locked"] == true {
                        coverage.locked_absolute =
                            coverage.locked_absolute.saturating_add(1).min(1);
                    } else if let Some(delta) = pair(&d["logical_delta"]) {
                        coverage.unlocked_absolute =
                            coverage.unlocked_absolute.saturating_add(1).min(8);
                        coverage.unlocked_absolute_signs |= directions(delta);
                    }
                }
            }
            Some("cleanup") => {
                if d["was_active"] != true || !cleanup_sequence(record) {
                    return;
                }
                match d["reason"].as_str() {
                    Some("down_anchor_restore") if d["restore_safe"] == true => {
                        self.restored = true;
                    }
                    Some("explicit_release") => self.released = true,
                    Some("simulated_owner_unmount") => self.unmounted = true,
                    Some("window_destroy") => self.pending_destroy = true,
                    Some("focus_loss_or_app_switch" | "suspended") => self.focus_lost = true,
                    Some("shutdown") => self.pending_shutdown = true,
                    _ => {}
                }
            }
            Some("window_destroy") if self.pending_destroy => {
                self.destroyed = true;
                self.pending_destroy = false;
            }
            Some("exit") if self.pending_shutdown => {
                self.shutdown = true;
                self.pending_shutdown = false;
            }
            _ => {}
        }
    }

    pub fn ready_for_shutdown(&self) -> bool {
        (0..self.scales.len()).any(|index| self.scale_ready(index))
            && self.restored
            && self.released
            && self.unmounted
            && self.destroyed
            && self.focus_lost
    }

    pub fn complete(&self) -> bool {
        self.ready_for_shutdown() && self.shutdown
    }

    pub fn summary(&self) -> String {
        let scale = |i: usize| {
            let c = self.scales[i];
            format!(
                "{}x U:{}/8+{}/4+{}/8+{}/4 L:{}/8+{}/4 abs:{}/1 edges:{}/4",
                i + 1,
                c.unlocked_native,
                c.unlocked_native_signs.count_ones(),
                c.unlocked_absolute,
                c.unlocked_absolute_signs.count_ones(),
                c.locked_native,
                c.locked_signs.count_ones(),
                c.locked_absolute,
                self.edges.masks[i].count_ones()
            )
        };
        format!(
            "{} | {} | R:{} U:{} X:{} D:{} focus:{} Q:{}",
            scale(0),
            scale(1),
            self.restored,
            self.released,
            self.unmounted,
            self.destroyed,
            self.focus_lost,
            self.shutdown
        )
    }

    pub fn next_action(&self, locked: bool, current_scale: Option<usize>) -> String {
        if !(0..self.scales.len()).any(|index| self.scale_ready(index)) {
            let Some(i) = current_scale else {
                return "Move the window to a supported 1x or 2x display".into();
            };
            let scale = self.scales[i];
            if !scale.unlocked_ready() {
                if locked {
                    return "Press U, then move the visible pointer inside the window".into();
                }
                return format!(
                    "On {}x, move the visible pointer right, left, down and up until unlocked samples and four signs fill",
                    i + 1
                );
            }
            if !scale.locked_ready() {
                if !locked {
                    return "Move pointer inside the window and press L to lock".into();
                }
                return format!(
                    "On {}x while locked, move right, left, down and up until eight samples and four signs fill",
                    i + 1
                );
            }
            if scale.locked_absolute == 0 {
                if !locked {
                    return "Move pointer inside the window and press L to lock".into();
                }
                return format!(
                    "On {}x, hold the primary mouse button and drag while locked until a real CursorMoved callback is recorded",
                    i + 1
                );
            }
            if self.edges.masks[i] != 15 {
                if !locked {
                    return "Move pointer inside the window and press L for numeric edge crossings"
                        .into();
                }
                if self.edges.active.is_none() {
                    return "Press U, move pointer inside the current window, then press L to recapture its extent".into();
                }
                let edge = (0..4)
                    .find(|edge| self.edges.masks[i] & EDGE_BITS[*edge] == 0)
                    .expect("missing edge exists");
                if let Some(remaining) = self.edges.remaining(i, edge) {
                    return format!(
                        "Continue moving beyond the {} logical window edge for {remaining} more outward samples",
                        EDGE_NAMES[edge]
                    );
                }
                return format!(
                    "On {}x, move {} until the virtual position crosses the logical window edge, then continue for three outward samples",
                    i + 1,
                    EDGE_NAMES[edge]
                );
            }
        }
        let trigger = if !self.restored {
            "press R to restore DownAnchor"
        } else if !self.released {
            "press U to release without restore"
        } else if !self.unmounted {
            "press X for simulated owner-unmount cleanup"
        } else if !self.destroyed {
            "press D to destroy and recreate the window"
        } else if !self.focus_lost {
            "use Cmd-Tab to switch apps and return"
        } else {
            "press Q to clean up and finish capture"
        };
        if locked {
            trigger.into()
        } else if self.ready_for_shutdown() {
            "Move pointer inside, press L, then Q to finish with an active lock".into()
        } else {
            format!("Move pointer inside and press L, then {trigger}")
        }
    }
}

pub(super) fn analyze(records: &[Value]) -> Vec<Verdict> {
    let mut verdicts = Vec::new();
    let mut locked_count = [0_usize; 2];
    let mut unlocked_native_count = [0_usize; 2];
    let mut unlocked_absolute_count = [0_usize; 2];
    let mut locked_signs = [0_u8; 2];
    let mut unlocked_native_signs = [0_u8; 2];
    let mut unlocked_absolute_signs = [0_u8; 2];
    let mut mapping_failure = false;
    let mut locked_quartz: HashMap<u64, Vec<[f64; 2]>> = HashMap::new();
    let mut locked_absolute_count = [0_usize; 2];
    let mut absolute_leak = false;
    let mut locked_scales = HashSet::new();

    for record in records {
        let d = detail(record);
        match kind(record) {
            Some("device_mouse_motion") => {
                let Some(raw) = pair(&d["native_delta"]) else {
                    continue;
                };
                let Some(scale) = scale(&d["window_scale"]) else {
                    continue;
                };
                if d["locked"] == true && d["focused"] == true {
                    locked_count[scale] += 1;
                    locked_signs[scale] |= directions(raw);
                    locked_scales.insert(scale);
                    if canonical_locked_delta(d).is_none() {
                        mapping_failure = true;
                    }
                    if let (Some(lease), Some(location)) =
                        (d["lease_id"].as_u64(), pair(&d["quartz_location"]))
                    {
                        locked_quartz.entry(lease).or_default().push(location);
                    }
                } else if d["locked"] == false && d["focused"] == true {
                    unlocked_native_count[scale] += 1;
                    unlocked_native_signs[scale] |= directions(raw);
                }
            }
            Some("cursor_moved") => {
                let Some(scale) = scale(&d["window_scale"]) else {
                    continue;
                };
                if d["locked"] == true && d["focused"] == true {
                    locked_absolute_count[scale] += 1;
                    if d["fused"]["absolute_move_count"].as_u64().unwrap_or(1) != 0 {
                        absolute_leak = true;
                    }
                } else if d["focused"] == true
                    && let Some(delta) = pair(&d["logical_delta"])
                {
                    unlocked_absolute_count[scale] += 1;
                    unlocked_absolute_signs[scale] |= directions(delta);
                }
            }
            _ => {}
        }
    }

    let scale_coverage: [bool; 2] = std::array::from_fn(|i| {
        locked_count[i] >= 8
            && unlocked_native_count[i] >= 8
            && unlocked_absolute_count[i] >= 8
            && locked_signs[i] == 15
            && unlocked_native_signs[i] == 15
            && unlocked_absolute_signs[i] == 15
    });
    let mapping_status = if mapping_failure {
        "FAIL"
    } else if scale_coverage.iter().any(|covered| *covered) {
        "PASS"
    } else {
        "UNPROVEN"
    };
    verdicts.push(verdict(
        "10.1 adapter identity and directional coverage at one real scale",
        mapping_status,
        format!(
            "complete directional coverage at [1x,2x]={scale_coverage:?}; locked={locked_count:?}; unlocked_native={unlocked_native_count:?}; unlocked_absolute={unlocked_absolute_count:?}; direction masks locked={locked_signs:?}, native={unlocked_native_signs:?}, absolute={unlocked_absolute_signs:?}; exact adapter/fusion mismatch={mapping_failure}"
        ),
    ));

    let mut quartz_failure = false;
    let mut stable_cycles = [0_usize; 2];
    for record in records
        .iter()
        .filter(|record| kind(record) == Some("lock_active"))
    {
        let d = detail(record);
        let (Some(lease), Some(scale)) = (d["lease_id"].as_u64(), scale(&d["window_scale"])) else {
            continue;
        };
        let Some(samples) = locked_quartz.get(&lease) else {
            continue;
        };
        if samples.len() < 8 {
            continue;
        }
        let origin = samples[0];
        if samples
            .iter()
            .any(|point| (point[0] - origin[0]).abs() > 0.5 || (point[1] - origin[1]).abs() > 0.5)
        {
            quartz_failure = true;
        } else {
            stable_cycles[scale] += 1;
        }
    }
    let quartz_status = if quartz_failure {
        "FAIL"
    } else if stable_cycles.iter().any(|count| *count > 0) {
        "PASS"
    } else {
        "UNPROVEN"
    };
    verdicts.push(verdict(
        "10.1 constant global cursor location while locked",
        quartz_status,
        format!("stable lock cycles at [1x,2x]={stable_cycles:?}; requires at least eight Quartz samples at one real scale"),
    ));

    let active_cleanups: Vec<_> = records
        .iter()
        .filter(|record| kind(record) == Some("cleanup") && detail(record)["was_active"] == true)
        .collect();
    let cleanup_failure = active_cleanups
        .iter()
        .any(|record| !cleanup_sequence(record));
    let duplicate_denial = records.iter().any(|record| {
        kind(record) == Some("lock_attempt")
            && detail(record)["already_locked"] == true
            && detail(record)["result"] == "Err(Denied)"
    });
    let gate_inactive_denial = records.iter().any(|record| {
        kind(record) == Some("lock_attempt")
            && detail(record)["already_locked"] == false
            && detail(record)["result"] == "Err(Denied)"
    });
    let acquired = records
        .iter()
        .any(|record| kind(record) == Some("lock_active"));
    verdicts.push(verdict(
        "12.2.1 lock acquisition/release including denial",
        if cleanup_failure { "FAIL" } else { "UNPROVEN" },
        format!("acquired={acquired}; clean terminations={}/{}; duplicate-lock denial={duplicate_denial}; gate-inactive lock denial observed={gate_inactive_denial} (native origin requires review, including prior release state); Section 12.2.1 permits audited unforceability plus separate deterministic adapter-to-engine denial proof when genuine denial has no safe trigger; this native report cannot certify that separate proof", active_cleanups.iter().filter(|record| cleanup_sequence(record)).count(), active_cleanups.len()),
    ));

    let mut edges = NumericEdgeTracker::default();
    for record in records {
        edges.observe(record);
    }
    let edge_masks = edges.masks;
    verdicts.push(verdict(
        "12.2.2 continuing motion at every edge",
        if mapping_failure {
            "FAIL"
        } else if mapping_status == "PASS"
            && quartz_status == "PASS"
            && (0..2).any(|i| scale_coverage[i] && stable_cycles[i] > 0 && edge_masks[i] == 15)
        {
            "PASS"
        } else {
            "UNPROVEN"
        },
        format!("numeric virtual crossings with three further outward canonical samples at [1x,2x]={edge_masks:?} (15 covers four edges); one complete mapped and stable scale is required; each path starts at its bounded DownAnchor and uses the live window extent"),
    ));
    verdicts.push(verdict(
        "12.2.3 native source/unit and logical conversion",
        if mapping_failure {
            "FAIL"
        } else if mapping_status == "PASS"
            && quartz_status == "PASS"
            && (0..2).any(|i| scale_coverage[i] && stable_cycles[i] > 0)
        {
            "PASS"
        } else {
            "UNPROVEN"
        },
        format!("pinned source/unit authority=SPEC-HAL-362 section 10.1 (winit 0.30.12 NSEvent.deltaX/Y in AppKit points); runtime identity/sign verdict={mapping_status}; locked Quartz stability={quartz_status}; no backing-scale or empirical conversion"),
    ));
    verdicts.push(verdict(
        "12.2.4 no duplicate absolute semantic motion",
        if absolute_leak {
            "FAIL"
        } else if (0..2).any(|i| locked_absolute_count[i] > 0 && locked_scales.contains(&i))
        {
            "PASS"
        } else {
            "UNPROVEN"
        },
        format!("locked CursorMoved callbacks at [1x,2x]={locked_absolute_count:?}; fused absolute leakage={absolute_leak}; locked scales observed={}", locked_scales.len()),
    ));

    let mut restore_passes = 0;
    let mut restore_failures = 0;
    for (index, record) in records.iter().enumerate() {
        if kind(record) != Some("cleanup")
            || detail(record)["reason"] != "down_anchor_restore"
            || detail(record)["was_active"] != true
        {
            continue;
        }
        let d = detail(record);
        if d["restore_safe"] != true {
            continue;
        }
        if !cleanup_sequence(record) {
            restore_failures += 1;
            continue;
        }
        let Some(lease) = d["lease_id"].as_u64() else {
            continue;
        };
        let prior = records[..index].iter().rev().find(|prior| {
            kind(prior) == Some("lock_active") && detail(prior)["lease_id"].as_u64() == Some(lease)
        });
        let (Some(before), Some(after)) = (
            prior.and_then(|prior| pair(&detail(prior)["quartz_location"])),
            pair(&d["quartz_location_after"]),
        ) else {
            continue;
        };
        let moved_during_lock = records[..index]
            .iter()
            .rev()
            .take_while(|prior| {
                !(kind(prior) == Some("lock_active")
                    && detail(prior)["lease_id"].as_u64() == Some(lease))
            })
            .any(|prior| kind(prior) == Some("window_moved"));
        if !moved_during_lock
            && (before[0] - after[0]).abs() <= 1.0
            && (before[1] - after[1]).abs() <= 1.0
        {
            restore_passes += 1;
        }
    }
    verdicts.push(verdict(
        "12.2.5 safe same-window DownAnchor restoration",
        if restore_failures > 0 {
            "FAIL"
        } else if restore_passes > 0 {
            "PASS"
        } else {
            "UNPROVEN"
        },
        format!("same-screen Quartz return with ordered restore/release/show={restore_passes}; failed requested restores={restore_failures}"),
    ));

    let reasons: HashSet<_> = active_cleanups
        .iter()
        .filter_map(|record| detail(record)["reason"].as_str())
        .collect();
    verdicts.push(verdict(
        "12.2.6 focus/app-switch/unmount/destroy/shutdown cleanup",
        if cleanup_failure { "FAIL" } else { "UNPROVEN" },
        format!("observed clean triggers={reasons:?}; app switching is indistinguishable from focus loss, and X is simulated rather than a real engine unmount"),
    ));

    let mut display_ids = HashSet::new();
    for record in records {
        if kind(record) == Some("window_created") || kind(record) == Some("window_moved") {
            let monitor = &detail(record)["display"]["monitor"];
            if !monitor.is_null() {
                display_ids.insert(monitor.to_string());
            }
        }
    }
    let unsafe_warp = active_cleanups.iter().any(|record| {
        detail(record)["restore_safe"] != true
            && detail(record)["operations"]
                .as_array()
                .is_some_and(|operations| {
                    operations
                        .iter()
                        .any(|operation| operation["operation"] == "restore")
                })
    });
    let (monitor_no_warp, monitor_invalidations) = unsafe_no_warp(records, "monitor_changed");
    let (scale_no_warp, scale_invalidations) = unsafe_no_warp(records, "scale_changed");
    verdicts.push(verdict(
        "12.2.7 unsafe no-warp on monitor invalidation",
        monitor_no_warp,
        format!("active monitor invalidations={monitor_invalidations}; requires release/show without a restore operation"),
    ));
    verdicts.push(verdict(
        "12.2.7 unsafe no-warp on scale invalidation",
        scale_no_warp,
        format!("active scale invalidations={scale_invalidations}; requires release/show without a restore operation"),
    ));
    verdicts.push(verdict(
        "12.2.7 multi-display no-stranding and unsafe no-warp",
        if unsafe_warp || monitor_no_warp == "FAIL" || scale_no_warp == "FAIL" {
            "FAIL"
        } else {
            "UNPROVEN"
        },
        format!("distinct observed display descriptors={}; monitor no-warp={monitor_no_warp}, scale no-warp={scale_no_warp}; pointer visibility and no-stranding require operator observation", display_ids.len()),
    ));
    verdicts.push(verdict(
        "SPEC-HAL-362 production support admission",
        "UNPROVEN",
        "This native JSONL does not certify the separate denial-path proof, real App lifecycle, or human cursor visibility/no-stranding; review the recorded production-admission evidence separately",
    ));
    verdicts
}

fn engine_event(record: &Value, event: &str) -> bool {
    kind(record) == Some(event)
}

fn engine_operation(record: &Value, operation: &str) -> bool {
    engine_event(record, "engine_platform") && detail(record)["operation"] == operation
}

fn engine_state(record: &Value, trigger: &str, active: bool) -> bool {
    engine_event(record, "engine_state")
        && detail(record)["trigger"] == trigger
        && detail(record)["active_lease"] == active
        && detail(record)["capture"] == active
        && detail(record)["session"] == active
}

fn engine_frame(record: &Value) -> Option<u64> {
    record["frame"].as_u64()
}

fn engine_cleanup(records: &[Value], start: usize, end: usize, restore: bool) -> bool {
    let operations: Vec<_> = records[start..end]
        .iter()
        .filter(|record| engine_event(record, "engine_platform"))
        .collect();
    let expected = if restore {
        &["restore", "release", "show"][..]
    } else {
        &["release", "show"][..]
    };
    operations.len() == expected.len()
        && operations.iter().zip(expected).all(|(record, name)| {
            detail(record)["operation"] == *name && is_ok(&detail(record)["result"])
        })
}

fn engine_trigger_verdict(
    records: &[Value],
    trigger: &'static str,
    requirement: &'static str,
    restore: bool,
    terminal: Option<(&str, Option<&str>)>,
) -> Verdict {
    for (trigger_index, record) in records.iter().enumerate() {
        if !engine_event(record, "engine_trigger") || detail(record)["trigger"] != trigger {
            continue;
        }
        let active = records[..trigger_index]
            .iter()
            .rev()
            .find(|record| engine_event(record, "engine_state"));
        if !active.is_some_and(|state| {
            engine_state(state, "route", true) || engine_state(state, "relative_route", true)
        }) {
            continue;
        }
        let Some(clear_offset) = records[trigger_index + 1..]
            .iter()
            .position(|record| engine_state(record, trigger, false))
        else {
            return verdict(
                requirement,
                "FAIL",
                "active trigger has no cleared engine state",
            );
        };
        let clear_index = trigger_index + 1 + clear_offset;
        let same_frame = engine_frame(record).is_some()
            && engine_frame(record) == engine_frame(&records[clear_index]);
        let ordered_cleanup = engine_cleanup(records, trigger_index + 1, clear_index, restore);
        let first_operation = records[trigger_index + 1..clear_index]
            .iter()
            .position(|record| engine_event(record, "engine_platform"))
            .map(|offset| trigger_index + 1 + offset);
        let show_index = records[trigger_index + 1..clear_index]
            .iter()
            .position(|record| engine_operation(record, "show"))
            .map(|offset| trigger_index + 1 + offset);
        let terminal_matches = match terminal {
            Some((outcome, reason)) => {
                let terminals = records[trigger_index + 1..clear_index]
                    .iter()
                    .enumerate()
                    .filter(|(_, record)| engine_event(record, "engine_terminal"))
                    .collect::<Vec<_>>();
                terminals.len() == 1
                    && terminals.first().is_some_and(|(offset, record)| {
                        detail(record)["terminal"] == outcome
                            && detail(record)["reason"].as_str() == reason
                            && show_index.is_some_and(|show| show < trigger_index + 1 + *offset)
                    })
            }
            None => !records[trigger_index + 1..clear_index]
                .iter()
                .any(|record| engine_event(record, "engine_terminal")),
        };
        let structural = if trigger == "owner_unmount" {
            records[trigger_index + 1..first_operation.unwrap_or(clear_index)]
                .iter()
                .any(|record| {
                    engine_event(record, "screen_build") && detail(record)["mounted"] == false
                })
        } else if trigger == "semantic_cancel" {
            records[trigger_index + 1..first_operation.unwrap_or(clear_index)]
                .iter()
                .any(|record| engine_event(record, "semantic_cancel_intent"))
        } else if trigger == "application_shutdown" {
            records[clear_index + 1..]
                .iter()
                .any(|record| engine_event(record, "engine_run_return"))
        } else {
            true
        };
        if same_frame && ordered_cleanup && terminal_matches && structural {
            return verdict(
                requirement,
                "PASS",
                format!("active {trigger}: same-frame cleanup and cleared lease/capture/session"),
            );
        }
        return verdict(
            requirement,
            "FAIL",
            format!(
                "active {trigger}: same_frame={same_frame}, ordered_cleanup={ordered_cleanup}, terminal={terminal_matches}, structural={structural}"
            ),
        );
    }
    verdict(
        requirement,
        "UNPROVEN",
        format!("no active {trigger} trigger"),
    )
}

pub(super) fn analyze_engine(records: &[Value]) -> Vec<Verdict> {
    let start = records.iter().any(|record| {
        engine_event(record, "engine_start")
            && (detail(record)["diagnostic_support_only"] == true
                || detail(record)["conformance_instrumented"] == true)
    });
    let support = records.iter().any(|record| {
        if !engine_event(record, "engine_diagnostic_support") {
            return false;
        }
        let d = detail(record);
        let native = d["native_support"].as_str().unwrap_or_default();
        let admitted = d["diagnostic_support"].as_str().unwrap_or_default();
        (native == "NormalizationUnavailable"
            && d["production_capability_enabled"] == false
            && admitted.starts_with("Supported {"))
            || (native.starts_with("Supported {")
                && d["production_capability_enabled"] == true
                && d["diagnostic_support_override"] == false
                && admitted == native)
    });
    let acquire = records
        .iter()
        .position(|record| engine_operation(record, "acquire") && is_ok(&detail(record)["result"]));
    let hide = acquire.and_then(|index| {
        records[index + 1..]
            .iter()
            .position(|record| engine_operation(record, "hide") && is_ok(&detail(record)["result"]))
            .map(|offset| index + 1 + offset)
    });
    let commit = hide.and_then(|index| {
        records[index + 1..]
            .iter()
            .position(|record| engine_event(record, "gesture_commit"))
            .map(|offset| index + 1 + offset)
    });
    let ingress = commit.and_then(|index| {
        let device = records[index + 1..]
            .iter()
            .position(|record| engine_event(record, "engine_device"))?
            + index
            + 1;
        let motion = records[device + 1..]
            .iter()
            .position(|record| engine_event(record, "relative_motion"))?
            + device
            + 1;
        (detail(&records[device])["lease_id"] == detail(&records[motion])["lease_id"])
            .then_some(motion)
    });
    let active = commit.is_some_and(|index| {
        records[index + 1..].iter().any(|record| {
            engine_state(record, "route", true) || engine_state(record, "relative_route", true)
        })
    });
    let absolute_leak = records
        .iter()
        .any(|record| engine_event(record, "absolute_move_after_commit"));
    let admission = if absolute_leak {
        "FAIL"
    } else if start
        && support
        && acquire.is_some()
        && hide.is_some()
        && commit.is_some()
        && active
        && ingress.is_some()
    {
        "PASS"
    } else {
        "UNPROVEN"
    };
    let mut verdicts = vec![verdict(
        "SPEC-HAL-362 real App admission and relative ingress",
        admission,
        format!(
            "start={start}, admitted_support={support}, acquire={}, hide={}, commit={}, active={active}, matched_device_ingress={}, absolute_leak={absolute_leak}",
            acquire.is_some(),
            hide.is_some(),
            commit.is_some(),
            ingress.is_some()
        ),
    )];

    let completion = records.iter().enumerate().find(|(index, record)| {
        engine_event(record, "up")
            && records[..*index]
                .iter()
                .rev()
                .find(|earlier| engine_event(earlier, "engine_state"))
                .is_some_and(|state| {
                    engine_state(state, "route", true)
                        || engine_state(state, "relative_route", true)
                })
    });
    let completion_status = if let Some((up_index, up)) = completion {
        let clear = records[up_index + 1..]
            .iter()
            .position(|record| engine_state(record, "relative_route", false))
            .map(|offset| up_index + 1 + offset);
        if let Some(clear_index) = clear {
            let events = &records[up_index + 1..clear_index];
            let semantic_first = events
                .first()
                .is_some_and(|record| engine_event(record, "semantic_complete"));
            let show = events
                .iter()
                .position(|record| engine_operation(record, "show"));
            let terminal = events.iter().position(|record| {
                engine_event(record, "engine_terminal") && detail(record)["terminal"] == "completed"
            });
            let one_terminal = events
                .iter()
                .filter(|record| engine_event(record, "engine_terminal"))
                .count()
                == 1;
            let cleanup = engine_cleanup(records, up_index + 2, clear_index, true);
            if semantic_first
                && one_terminal
                && show
                    .zip(terminal)
                    .is_some_and(|(show, terminal)| show < terminal)
                && cleanup
                && engine_frame(up).is_some()
                && engine_frame(up) == engine_frame(&records[clear_index])
            {
                "PASS"
            } else {
                "FAIL"
            }
        } else {
            "FAIL"
        }
    } else {
        "UNPROVEN"
    };
    verdicts.push(verdict(
        "SPEC-HAL-362 real Up completion and restore/release/show order",
        completion_status,
        "requires active Up, semantic completion before ordered native cleanup, Completed terminal, and same-frame cleared state",
    ));
    verdicts.push(engine_trigger_verdict(
        records,
        "owner_unmount",
        "SPEC-HAL-362 real owner-unmount cleanup",
        false,
        None,
    ));
    verdicts.push(engine_trigger_verdict(
        records,
        "semantic_cancel",
        "SPEC-HAL-362 real semantic cancellation",
        true,
        Some(("cancelled", Some("ExplicitSemanticCancel"))),
    ));
    verdicts.push(engine_trigger_verdict(
        records,
        "focus_loss",
        "SPEC-HAL-362 real focus-loss cleanup",
        false,
        Some(("cancelled", Some("FocusLost"))),
    ));
    verdicts.push(engine_trigger_verdict(
        records,
        "application_shutdown",
        "SPEC-HAL-362 real application shutdown cleanup",
        false,
        Some(("cancelled", Some("ApplicationShutdown"))),
    ));
    verdicts.push(verdict(
        "SPEC-HAL-362 production support admission",
        "UNPROVEN",
        "Engine JSONL certifies lifecycle only; native-denial review and human cursor visibility/no-stranding remain separate evidence for production admission",
    ));
    verdicts
}

#[cfg(test)]
#[path = "../tests/relative_pointer_macos_conformance_analysis.rs"]
mod tests;
