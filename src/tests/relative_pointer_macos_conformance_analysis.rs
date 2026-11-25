use serde_json::json;

use super::{CaptureProgress, NumericEdgeTracker, analyze, analyze_engine, detail, kind};

fn record(kind: &str, detail: serde_json::Value) -> serde_json::Value {
    json!({"kind": kind, "detail": detail})
}

fn engine_record(frame: u64, kind: &str, detail: serde_json::Value) -> serde_json::Value {
    json!({"frame": frame, "kind": kind, "detail": detail})
}

fn engine_active(records: &mut Vec<serde_json::Value>, frame: u64) {
    records.push(engine_record(
        frame,
        "engine_platform",
        json!({"operation": "acquire", "result": "Ok(())"}),
    ));
    records.push(engine_record(
        frame,
        "engine_platform",
        json!({"operation": "hide", "result": "Ok(())"}),
    ));
    records.push(engine_record(frame, "gesture_commit", json!({})));
    records.push(engine_record(
        frame,
        "engine_state",
        json!({"trigger": "route", "active_lease": true, "capture": true, "session": true}),
    ));
}

fn engine_clear(
    records: &mut Vec<serde_json::Value>,
    frame: u64,
    trigger: &str,
    restore: bool,
    terminal: Option<(&str, Option<&str>)>,
) {
    if restore {
        records.push(engine_record(
            frame,
            "engine_platform",
            json!({"operation": "restore", "result": "Ok(())"}),
        ));
    }
    records.push(engine_record(
        frame,
        "engine_platform",
        json!({"operation": "release", "result": "Ok(())"}),
    ));
    records.push(engine_record(
        frame,
        "engine_platform",
        json!({"operation": "show", "result": "Ok(())"}),
    ));
    if let Some((outcome, reason)) = terminal {
        records.push(engine_record(
            frame,
            "engine_terminal",
            json!({"terminal": outcome, "reason": reason}),
        ));
    }
    records.push(engine_record(
        frame,
        "engine_state",
        json!({"trigger": trigger, "active_lease": false, "capture": false, "session": false}),
    ));
}

fn complete_engine_capture() -> Vec<serde_json::Value> {
    let mut records = vec![
        engine_record(
            0,
            "engine_start",
            json!({"diagnostic_support_only": true, "production_capability_enabled": false}),
        ),
        engine_record(
            1,
            "engine_diagnostic_support",
            json!({
                "native_support": "NormalizationUnavailable",
                "diagnostic_support": "Supported { normalization_epoch: 1, restore_down_anchor: true }",
                "production_capability_enabled": false,
            }),
        ),
    ];
    engine_active(&mut records, 1);
    records.push(engine_record(1, "engine_device", json!({"lease_id": 1})));
    records.push(engine_record(1, "relative_motion", json!({"lease_id": 1})));
    records.push(engine_record(1, "up", json!({})));
    records.push(engine_record(1, "semantic_complete", json!({})));
    engine_clear(
        &mut records,
        1,
        "relative_route",
        true,
        Some(("completed", None)),
    );

    engine_active(&mut records, 2);
    records.push(engine_record(
        2,
        "engine_trigger",
        json!({"trigger": "owner_unmount"}),
    ));
    records.push(engine_record(2, "screen_build", json!({"mounted": false})));
    engine_clear(&mut records, 2, "owner_unmount", false, None);

    engine_active(&mut records, 3);
    records.push(engine_record(
        3,
        "engine_trigger",
        json!({"trigger": "semantic_cancel"}),
    ));
    records.push(engine_record(3, "semantic_cancel_intent", json!({})));
    engine_clear(
        &mut records,
        3,
        "semantic_cancel",
        true,
        Some(("cancelled", Some("ExplicitSemanticCancel"))),
    );

    engine_active(&mut records, 4);
    records.push(engine_record(
        4,
        "engine_trigger",
        json!({"trigger": "focus_loss"}),
    ));
    engine_clear(
        &mut records,
        4,
        "focus_loss",
        false,
        Some(("cancelled", Some("FocusLost"))),
    );

    engine_active(&mut records, 5);
    records.push(engine_record(
        5,
        "engine_trigger",
        json!({"trigger": "application_shutdown"}),
    ));
    engine_clear(
        &mut records,
        5,
        "application_shutdown",
        false,
        Some(("cancelled", Some("ApplicationShutdown"))),
    );
    records.push(engine_record(5, "engine_run_return", json!({})));
    records
}

#[test]
fn real_engine_capture_reports_lifecycle_only_and_keeps_support_unproven() {
    let verdicts = analyze_engine(&complete_engine_capture());
    assert_eq!(verdicts.len(), 7);
    assert!(
        verdicts[..6].iter().all(|result| result.status == "PASS"),
        "{verdicts:?}"
    );
    assert_eq!(verdicts[6].status, "UNPROVEN");
}

#[test]
fn real_engine_capture_accepts_advertised_production_support_without_an_override() {
    let mut records = complete_engine_capture();
    records[0]["detail"] = json!({"conformance_instrumented": true});
    records[1]["detail"] = json!({
        "native_support": "Supported { normalization_epoch: 1, restore_down_anchor: true }",
        "diagnostic_support": "Supported { normalization_epoch: 1, restore_down_anchor: true }",
        "production_capability_enabled": true,
        "diagnostic_support_override": false,
    });

    let verdicts = analyze_engine(&records);
    assert!(verdicts[..6].iter().all(|result| result.status == "PASS"));
    assert_eq!(verdicts[6].status, "UNPROVEN");
}

#[test]
fn real_engine_capture_rejects_owner_warp_and_late_focus_cleanup() {
    let mut records = complete_engine_capture();
    let unmount = records
        .iter()
        .position(|record| {
            kind(record) == Some("engine_trigger") && detail(record)["trigger"] == "owner_unmount"
        })
        .unwrap();
    records.insert(
        unmount + 2,
        engine_record(
            2,
            "engine_platform",
            json!({"operation": "restore", "result": "Ok(())"}),
        ),
    );
    let semantic = records
        .iter()
        .position(|record| {
            kind(record) == Some("engine_trigger") && detail(record)["trigger"] == "semantic_cancel"
        })
        .unwrap();
    let terminal = records[semantic + 1..]
        .iter()
        .position(|record| kind(record) == Some("engine_terminal"))
        .unwrap()
        + semantic
        + 1;
    let show = records[semantic + 1..]
        .iter()
        .position(|record| {
            kind(record) == Some("engine_platform") && detail(record)["operation"] == "show"
        })
        .unwrap()
        + semantic
        + 1;
    records.swap(terminal, show);
    let focus_state = records
        .iter()
        .position(|record| {
            kind(record) == Some("engine_state") && detail(record)["trigger"] == "focus_loss"
        })
        .unwrap();
    records[focus_state]["frame"] = json!(5);
    let verdicts = analyze_engine(&records);
    assert_eq!(verdicts[2].status, "FAIL");
    assert_eq!(verdicts[3].status, "FAIL");
    assert_eq!(verdicts[4].status, "FAIL");
    assert_eq!(verdicts[6].status, "UNPROVEN");
}

fn locked_motion(scale: f64, lease: u64, delta: [f64; 2]) -> serde_json::Value {
    record(
        "device_mouse_motion",
        json!({
            "locked": true, "focused": true, "window_scale": scale, "lease_id": lease,
            "window_size_physical": [100.0 * scale, 100.0 * scale],
            "native_delta": delta, "normalized_logical_delta": delta,
            "fused": {"relative_deltas": [delta]}, "quartz_location": [50.0, 50.0]
        }),
    )
}

fn scale_motion(scale: f64, lease: u64) -> Vec<serde_json::Value> {
    let mut records = Vec::new();
    let directions = [[1.0, 0.0], [-1.0, 0.0], [0.0, 1.0], [0.0, -1.0]];
    for delta in directions.into_iter().cycle().take(8) {
        records.push(record(
            "device_mouse_motion",
            json!({
                "locked": false, "focused": true, "window_scale": scale,
                "native_delta": delta
            }),
        ));
        records.push(record(
            "cursor_moved",
            json!({
                "locked": false, "focused": true, "window_scale": scale,
                "logical_delta": delta
            }),
        ));
    }
    records.push(record(
        "lock_active",
        json!({
            "lease_id": lease, "window_scale": scale,
            "anchor_logical": [50.0, 50.0],
            "window_size_physical": [100.0 * scale, 100.0 * scale],
            "quartz_location": [50.0, 50.0]
        }),
    ));
    for delta in directions.into_iter().cycle().take(8) {
        records.push(locked_motion(scale, lease, delta));
    }
    for (cross, outward, return_inside) in [
        ([-60.0, 0.0], [-1.0, 0.0], [63.0, 0.0]),
        ([60.0, 0.0], [1.0, 0.0], [-63.0, 0.0]),
        ([0.0, -60.0], [0.0, -1.0], [0.0, 63.0]),
        ([0.0, 60.0], [0.0, 1.0], [0.0, -63.0]),
    ] {
        records.push(locked_motion(scale, lease, cross));
        for _ in 0..3 {
            records.push(locked_motion(scale, lease, outward));
        }
        records.push(locked_motion(scale, lease, return_inside));
    }
    records.push(record(
        "cursor_moved",
        json!({
            "locked": true, "focused": true, "window_scale": scale,
            "fused": {"absolute_move_count": 0}
        }),
    ));
    records
}

fn two_scale_motion() -> Vec<serde_json::Value> {
    let mut records = scale_motion(1.0, 1);
    records.extend(scale_motion(2.0, 2));
    records
}

#[test]
fn sparse_capture_never_passes_required_evidence() {
    let verdicts = analyze(&[]);
    assert!(verdicts.iter().all(|verdict| verdict.status == "UNPROVEN"));
}

#[test]
fn one_real_scale_proves_point_unit_identity_without_claiming_multidisplay() {
    let verdicts = analyze(&scale_motion(2.0, 2));
    for index in [0, 1, 3, 4, 5] {
        assert_eq!(verdicts[index].status, "PASS", "{verdicts:?}");
    }
    assert_eq!(
        verdicts
            .iter()
            .find(|verdict| verdict.requirement
                == "12.2.7 multi-display no-stranding and unsafe no-warp")
            .unwrap()
            .status,
        "UNPROVEN"
    );
    assert_eq!(verdicts.last().unwrap().status, "UNPROVEN");
}

#[test]
fn complete_two_scale_mapping_and_suppression_can_pass() {
    let verdicts = analyze(&two_scale_motion());
    assert_eq!(verdicts[0].status, "PASS");
    assert_eq!(verdicts[1].status, "PASS");
    assert_eq!(verdicts[3].status, "PASS");
    assert_eq!(verdicts[4].status, "PASS");
    assert_eq!(verdicts[5].status, "PASS");
    assert_eq!(verdicts.last().unwrap().status, "UNPROVEN");
}

#[test]
fn guided_capture_requires_coverage_before_shutdown() {
    let mut progress = CaptureProgress::default();
    assert!(!progress.ready_for_shutdown());
    assert!(progress.next_action(false, Some(1)).contains("On 2x"));
    for record in scale_motion(2.0, 2) {
        progress.observe(&record);
    }
    assert!(!progress.ready_for_shutdown());
    assert!(progress.next_action(false, Some(1)).contains("press R"));
    for reason in [
        "down_anchor_restore",
        "explicit_release",
        "simulated_owner_unmount",
        "window_destroy",
        "focus_loss_or_app_switch",
    ] {
        let mut operations = Vec::new();
        let restored = reason == "down_anchor_restore";
        if restored {
            operations.push(json!({"operation": "restore", "result": "Ok(())"}));
        }
        operations.push(json!({"operation": "release", "result": "Ok(())"}));
        operations.push(json!({"operation": "show", "result": "Ok(())"}));
        progress.observe(&record(
            "cleanup",
            json!({
                "was_active": true, "reason": reason,
                "restore_safe": restored, "operations": operations
            }),
        ));
        if reason == "window_destroy" {
            progress.observe(&record("window_destroy", json!({})));
        }
    }
    assert!(progress.ready_for_shutdown());
    assert!(!progress.complete());
    progress.observe(&record(
        "cleanup",
        json!({
            "was_active": true, "reason": "shutdown", "restore_safe": false,
            "operations": [
                {"operation": "release", "result": "Ok(())"},
                {"operation": "show", "result": "Ok(())"}
            ]
        }),
    ));
    progress.observe(&record("exit", json!({})));
    assert!(progress.complete());
}

#[test]
fn edge_proof_requires_numeric_crossing_and_three_later_samples_from_one_lease() {
    let mut edges = NumericEdgeTracker::default();
    edges.observe(&record(
        "lock_active",
        json!({
            "lease_id": 1, "window_scale": 1.0,
            "anchor_logical": [50.0, 50.0], "window_size_physical": [100.0, 100.0]
        }),
    ));
    for _ in 0..3 {
        edges.observe(&locked_motion(1.0, 1, [-1.0, 0.0]));
    }
    assert_eq!(edges.masks[0], 0);
    edges.observe(&locked_motion(1.0, 1, [-48.0, 0.0]));
    for _ in 0..2 {
        edges.observe(&locked_motion(1.0, 1, [-1.0, 0.0]));
    }
    edges.observe(&locked_motion(1.0, 2, [-1.0, 0.0]));
    assert_eq!(edges.masks[0], 0);
    edges.observe(&locked_motion(1.0, 1, [-1.0, 0.0]));
    assert_eq!(edges.masks[0], 1);
    edges.observe(&record("cleanup", json!({"lease_id": 1})));
    edges.observe(&locked_motion(1.0, 1, [100.0, 0.0]));
    assert_eq!(edges.masks[0], 1);

    let mut changed_extent = NumericEdgeTracker::default();
    changed_extent.observe(&record(
        "lock_active",
        json!({
            "lease_id": 3, "window_scale": 1.0,
            "anchor_logical": [50.0, 50.0], "window_size_physical": [100.0, 100.0]
        }),
    ));
    let mut resized_motion = locked_motion(1.0, 3, [-60.0, 0.0]);
    resized_motion["detail"]["window_size_physical"] = json!([120.0, 100.0]);
    changed_extent.observe(&resized_motion);
    assert!(changed_extent.active.is_none());
    assert_eq!(changed_extent.masks[0], 0);
}

#[test]
fn guided_capture_requires_real_locked_absolute_callback_at_the_chosen_scale() {
    let mut progress = CaptureProgress::default();
    for record in scale_motion(2.0, 2) {
        if kind(&record) == Some("cursor_moved")
            && detail(&record)["locked"] == true
            && detail(&record)["window_scale"] == 2.0
        {
            continue;
        }
        progress.observe(&record);
    }
    assert!(progress.next_action(true, Some(1)).contains("CursorMoved"));
    assert!(!progress.ready_for_shutdown());
}

#[test]
fn a_mismatch_at_an_additional_scale_still_fails_identity() {
    let mut records = scale_motion(2.0, 2);
    let mut mismatch = locked_motion(1.0, 3, [2.0, -3.0]);
    mismatch["detail"]["normalized_logical_delta"] = json!([4.0, -3.0]);
    records.push(mismatch);
    assert_eq!(analyze(&records)[0].status, "FAIL");
}

#[test]
fn real_monitor_and_scale_invalidation_can_prove_no_unsafe_warp() {
    let clean = |reason: &str| {
        record(
            "cleanup",
            json!({
                "was_active": true, "reason": reason, "restore_safe": false,
                "operations": [
                    {"operation": "release", "result": "Ok(())"},
                    {"operation": "show", "result": "Ok(())"}
                ]
            }),
        )
    };
    let verdicts = analyze(&[clean("monitor_changed"), clean("scale_changed")]);
    for requirement in [
        "12.2.7 unsafe no-warp on monitor invalidation",
        "12.2.7 unsafe no-warp on scale invalidation",
    ] {
        assert_eq!(
            verdicts
                .iter()
                .find(|v| v.requirement == requirement)
                .unwrap()
                .status,
            "PASS"
        );
    }
    assert_eq!(
        verdicts
            .iter()
            .find(|v| v.requirement == "12.2.7 multi-display no-stranding and unsafe no-warp")
            .unwrap()
            .status,
        "UNPROVEN"
    );
    let failed = analyze(&[record(
        "cleanup",
        json!({
            "was_active": true, "reason": "monitor_changed", "restore_safe": true,
            "operations": [
                {"operation": "restore", "result": "Ok(())"},
                {"operation": "release", "result": "Ok(())"},
                {"operation": "show", "result": "Ok(())"}
            ]
        }),
    )]);
    assert_eq!(
        failed
            .iter()
            .find(|v| v.requirement == "12.2.7 unsafe no-warp on monitor invalidation")
            .unwrap()
            .status,
        "FAIL"
    );
}

#[test]
fn mapped_delta_mismatch_and_absolute_leak_fail() {
    let verdicts = analyze(&[
        record(
            "device_mouse_motion",
            json!({
                "locked": true, "focused": true, "window_scale": 1.0, "lease_id": 1,
                "native_delta": [2.0, -3.0], "normalized_logical_delta": [4.0, -3.0],
                "fused": {"relative_deltas": [[4.0, -3.0]]}, "quartz_location": [50.0, 50.0]
            }),
        ),
        record(
            "cursor_moved",
            json!({
                "locked": true, "focused": true, "window_scale": 1.0,
                "fused": {"absolute_move_count": 1}
            }),
        ),
    ]);
    assert_eq!(verdicts[0].status, "FAIL");
    assert_eq!(verdicts[5].status, "FAIL");
}

#[test]
fn cleanup_order_and_failure_are_reported() {
    let verdicts = analyze(&[record(
        "cleanup",
        json!({
            "was_active": true, "reason": "down_anchor_restore", "restore_safe": true,
            "operations": [
                {"operation": "release", "result": "Ok(())"},
                {"operation": "restore", "result": "Err(FailedPlatformOperation)"},
                {"operation": "show", "result": "Ok(())"}
            ]
        }),
    )]);
    assert_eq!(verdicts[2].status, "FAIL");
    assert_eq!(verdicts[6].status, "FAIL");
    assert_eq!(verdicts[7].status, "FAIL");
}
