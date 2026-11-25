//! SPEC-HAL-346 deterministic interaction generator registry.

use super::list_view_resize::{
    LIST_VIEW_RESIZE_COL_INDEX_WIDTH, LIST_VIEW_RESIZE_COL_NAME_WIDTH,
    LIST_VIEW_RESIZE_LV2_INITIAL_HORIZONTAL_SCROLL,
};
use super::registry::{
    InteractionFixtureDefinition, InteractionFixtureKind, LIST_VIEW_RESIZE_LV1_FIXTURE_ID,
    LIST_VIEW_RESIZE_LV2_FIXTURE_ID, TREE_VIEW_PROJECTION_TV1_FIXTURE_ID,
};
use super::tree_view_projection::TREE_VIEW_PROJECTION_TREE_COLUMN_WIDTH;
use crate::core::event::{KeyCode, Modifiers};
use crate::core::interaction_fuzz::{
    CoverageKey, EndpointTeleportationMode, IntentCommandClass, InteractionDriver,
    InteractionDriverResult, InteractionIntentCommand, InteractionIntentTarget,
    KeyboardNavigationPolicy, PointerApproach, PointerCaptureTransitionClass,
    PointerMoveExposureClass, PointerPathPolicy, PointerPathPolicyClass,
    PointerPathSampleCountClass, WheelDeltaMode,
};
use crate::core::math::Point;

pub const GEN_346_RSZ1_ID: &str = "GEN-346-RSZ1";
pub const GEN_346_RSZ2_ID: &str = "GEN-346-RSZ2";
pub const GEN_346_RSZ3_ID: &str = "GEN-346-RSZ3";
pub const GEN_346_RSZ4_ID: &str = "GEN-346-RSZ4";
pub const GEN_346_SCR1_ID: &str = "GEN-346-SCR1";
pub const GEN_346_FOC1_ID: &str = "GEN-346-FOC1";
pub const GEN_346_PTH1_ID: &str = "GEN-346-PTH1";
pub const GEN_346_PTH2_ID: &str = "GEN-346-PTH2";
pub const GEN_346_PTH3_ID: &str = "GEN-346-PTH3";
pub const GEN_346_KBD1_ID: &str = "GEN-346-KBD1";

const RSZ1_GENERATOR_SEED: u64 = 0x3460_0001;
const RSZ2_GENERATOR_SEED: u64 = 0x3460_0002;
const RSZ3_GENERATOR_SEED: u64 = 0x3460_0003;
const RSZ4_GENERATOR_SEED: u64 = 0x3460_0004;
const SCR1_GENERATOR_SEED: u64 = 0x3460_0101;
const FOC1_GENERATOR_SEED: u64 = 0x3460_0201;
const PTH1_GENERATOR_SEED: u64 = 0x3460_1001;
const PTH2_GENERATOR_SEED: u64 = 0x3460_1002;
const PTH3_GENERATOR_SEED: u64 = 0x3460_1003;
const KBD1_GENERATOR_SEED: u64 = 0x3460_2001;
const HEADER_SEPARATOR_Y: f32 = 12.0;
const BODY_CELL_Y: f32 = 48.0;
const OUTSIDE_LIST_VIEW_X: f32 = -24.0;
const OUTSIDE_LIST_VIEW_Y: f32 = 20.0;

const RSZ_CLASSES: &[InteractionGeneratorClass] = &[InteractionGeneratorClass::ResizeGenerator];
const PTH1_CLASSES: &[InteractionGeneratorClass] = &[
    InteractionGeneratorClass::PointerPathGenerator,
    InteractionGeneratorClass::IncidentalInteractionGenerator,
    InteractionGeneratorClass::ResizeGenerator,
];
const SCR1_CLASSES: &[InteractionGeneratorClass] = &[
    InteractionGeneratorClass::ScrollGenerator,
    InteractionGeneratorClass::PointerPathGenerator,
    InteractionGeneratorClass::IncidentalInteractionGenerator,
    InteractionGeneratorClass::ResizeGenerator,
];
const KBD1_CLASSES: &[InteractionGeneratorClass] =
    &[InteractionGeneratorClass::KeyboardNavigationGenerator];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionGeneratorClass {
    PointerPathGenerator,
    ResizeGenerator,
    IncidentalInteractionGenerator,
    KeyboardNavigationGenerator,
    ScrollGenerator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionGeneratorKind {
    SeparatorResize,
    ResizeWhileHorizontallyScrolled,
    FirstDragThenSubsequentDrag,
    InverseDragRestoration,
    WheelScrollThenResize,
    FocusKeyboardNavigationSmoke,
    IncidentalPointerPathToResize,
    OvershootThenCorrectToSeparator,
    HoverPauseBeforeAction,
    KeyboardTraversalBeforeActivation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratedTargetClass {
    OutsideListView,
    RowCell,
    HeaderCell,
    Separator,
    KeyboardTraversalStep,
    KeyboardActivationTarget,
    ResizeDragEndpoint,
}

#[derive(Debug, Clone, Copy)]
pub struct GeneratedInteractionTarget {
    pub class: GeneratedTargetClass,
    pub point: Option<Point>,
}

impl GeneratedInteractionTarget {
    fn spatial(class: GeneratedTargetClass, point: Point) -> Self {
        Self {
            class,
            point: Some(point),
        }
    }

    fn non_spatial(class: GeneratedTargetClass) -> Self {
        Self { class, point: None }
    }

    fn point(&self) -> Point {
        self.point
            .expect("spatial generated interaction target must have a point")
    }

    fn intent_target(
        self,
        fixture: InteractionFixtureDefinition,
        generator_id: &'static str,
        root_seed: u64,
        generator_seed: u64,
        role: &'static str,
    ) -> InteractionIntentTarget {
        generated_intent_target(
            fixture,
            generator_id,
            root_seed,
            generator_seed,
            self.class,
            self.point(),
            role,
        )
    }
}

#[derive(Debug, Clone)]
pub struct GeneratedInteractionScenario {
    pub generator_id: &'static str,
    pub fixture_id: &'static str,
    pub root_seed: u64,
    pub generator_seed: u64,
    pub generator_classes: &'static [InteractionGeneratorClass],
    pub intended_target: GeneratedInteractionTarget,
    pub incidental_targets: Vec<GeneratedInteractionTarget>,
    pub intent_commands: Vec<InteractionIntentCommand>,
    pub expected_coverage: Vec<CoverageKey>,
}

impl GeneratedInteractionScenario {
    pub fn execute(&self, driver: &mut InteractionDriver) -> InteractionDriverResult<()> {
        for command in self.intent_commands.iter().cloned() {
            driver.try_execute_intent(command)?;
        }
        for key in self.path_exposure_coverage() {
            driver.record_coverage_key(key);
        }
        Ok(())
    }

    pub fn path_exposure_coverage(&self) -> Vec<CoverageKey> {
        let mut keys = Vec::new();
        push_target_exposure_coverage(&mut keys, self.intended_target, false);
        for target in self.incidental_targets.iter().copied() {
            push_target_exposure_coverage(&mut keys, target, true);
        }
        keys
    }

    pub fn uses_instant_pointer_path(&self) -> bool {
        self.intent_commands.iter().any(|command| match command {
            InteractionIntentCommand::MovePointerTo { path_policy, .. }
            | InteractionIntentCommand::ClickTarget { path_policy, .. }
            | InteractionIntentCommand::DragPointerTo { path_policy, .. }
            | InteractionIntentCommand::WheelAtTarget { path_policy, .. }
            | InteractionIntentCommand::TextAtTarget { path_policy, .. } => {
                matches!(path_policy, PointerPathPolicy::Instant)
            }
            InteractionIntentCommand::KeyboardNavigation { .. } => false,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct InteractionGeneratorDefinition {
    pub generator_id: &'static str,
    pub fixture_id: &'static str,
    pub kind: InteractionGeneratorKind,
    pub generator_classes: &'static [InteractionGeneratorClass],
    builder: fn(InteractionFixtureDefinition, u64) -> Option<GeneratedInteractionScenario>,
}

impl InteractionGeneratorDefinition {
    pub fn build(
        self,
        fixture: InteractionFixtureDefinition,
        root_seed: u64,
    ) -> Option<GeneratedInteractionScenario> {
        if self.fixture_id != fixture.fixture_id {
            return None;
        }
        (self.builder)(fixture, root_seed)
    }
}

pub const INTERACTION_GENERATOR_REGISTRY: [InteractionGeneratorDefinition; 11] = [
    InteractionGeneratorDefinition {
        generator_id: GEN_346_RSZ1_ID,
        fixture_id: LIST_VIEW_RESIZE_LV1_FIXTURE_ID,
        kind: InteractionGeneratorKind::SeparatorResize,
        generator_classes: RSZ_CLASSES,
        builder: build_rsz1,
    },
    InteractionGeneratorDefinition {
        generator_id: GEN_346_RSZ1_ID,
        fixture_id: TREE_VIEW_PROJECTION_TV1_FIXTURE_ID,
        kind: InteractionGeneratorKind::SeparatorResize,
        generator_classes: RSZ_CLASSES,
        builder: build_rsz1,
    },
    InteractionGeneratorDefinition {
        generator_id: GEN_346_RSZ2_ID,
        fixture_id: LIST_VIEW_RESIZE_LV2_FIXTURE_ID,
        kind: InteractionGeneratorKind::ResizeWhileHorizontallyScrolled,
        generator_classes: RSZ_CLASSES,
        builder: build_rsz2,
    },
    InteractionGeneratorDefinition {
        generator_id: GEN_346_RSZ3_ID,
        fixture_id: LIST_VIEW_RESIZE_LV1_FIXTURE_ID,
        kind: InteractionGeneratorKind::FirstDragThenSubsequentDrag,
        generator_classes: RSZ_CLASSES,
        builder: build_rsz3,
    },
    InteractionGeneratorDefinition {
        generator_id: GEN_346_RSZ4_ID,
        fixture_id: LIST_VIEW_RESIZE_LV1_FIXTURE_ID,
        kind: InteractionGeneratorKind::InverseDragRestoration,
        generator_classes: RSZ_CLASSES,
        builder: build_rsz4,
    },
    InteractionGeneratorDefinition {
        generator_id: GEN_346_SCR1_ID,
        fixture_id: LIST_VIEW_RESIZE_LV1_FIXTURE_ID,
        kind: InteractionGeneratorKind::WheelScrollThenResize,
        generator_classes: SCR1_CLASSES,
        builder: build_scr1,
    },
    InteractionGeneratorDefinition {
        generator_id: GEN_346_FOC1_ID,
        fixture_id: LIST_VIEW_RESIZE_LV1_FIXTURE_ID,
        kind: InteractionGeneratorKind::FocusKeyboardNavigationSmoke,
        generator_classes: KBD1_CLASSES,
        builder: build_foc1,
    },
    InteractionGeneratorDefinition {
        generator_id: GEN_346_PTH1_ID,
        fixture_id: LIST_VIEW_RESIZE_LV1_FIXTURE_ID,
        kind: InteractionGeneratorKind::IncidentalPointerPathToResize,
        generator_classes: PTH1_CLASSES,
        builder: build_pth1,
    },
    InteractionGeneratorDefinition {
        generator_id: GEN_346_PTH2_ID,
        fixture_id: LIST_VIEW_RESIZE_LV1_FIXTURE_ID,
        kind: InteractionGeneratorKind::OvershootThenCorrectToSeparator,
        generator_classes: PTH1_CLASSES,
        builder: build_pth2,
    },
    InteractionGeneratorDefinition {
        generator_id: GEN_346_PTH3_ID,
        fixture_id: LIST_VIEW_RESIZE_LV1_FIXTURE_ID,
        kind: InteractionGeneratorKind::HoverPauseBeforeAction,
        generator_classes: PTH1_CLASSES,
        builder: build_pth3,
    },
    InteractionGeneratorDefinition {
        generator_id: GEN_346_KBD1_ID,
        fixture_id: LIST_VIEW_RESIZE_LV1_FIXTURE_ID,
        kind: InteractionGeneratorKind::KeyboardTraversalBeforeActivation,
        generator_classes: KBD1_CLASSES,
        builder: build_kbd1,
    },
];

pub fn interaction_generator_registry() -> &'static [InteractionGeneratorDefinition] {
    &INTERACTION_GENERATOR_REGISTRY
}

pub fn find_interaction_generator(generator_id: &str) -> Option<InteractionGeneratorDefinition> {
    INTERACTION_GENERATOR_REGISTRY
        .iter()
        .copied()
        .find(|definition| definition.generator_id == generator_id)
}

pub fn generate_scenarios_for_fixture(
    fixture: InteractionFixtureDefinition,
    root_seed: u64,
) -> Vec<GeneratedInteractionScenario> {
    INTERACTION_GENERATOR_REGISTRY
        .iter()
        .filter_map(|definition| definition.build(fixture, root_seed))
        .collect()
}

fn build_rsz1(
    fixture: InteractionFixtureDefinition,
    root_seed: u64,
) -> Option<GeneratedInteractionScenario> {
    if !is_shared_tabular_resize_fixture(fixture.kind) {
        return None;
    }

    let separator = first_separator_target(fixture);
    let drag_dx = resize_delta(root_seed, RSZ1_GENERATOR_SEED);
    let drag_start = separator.intent_target(
        fixture,
        GEN_346_RSZ1_ID,
        root_seed,
        RSZ1_GENERATOR_SEED,
        "drag_start",
    );
    let drag_end = resize_drag_endpoint(
        fixture,
        GEN_346_RSZ1_ID,
        root_seed,
        RSZ1_GENERATOR_SEED,
        Point::new(separator.point().x + drag_dx, separator.point().y),
        "drag_end",
    );
    Some(GeneratedInteractionScenario {
        generator_id: GEN_346_RSZ1_ID,
        fixture_id: fixture.fixture_id,
        root_seed,
        generator_seed: RSZ1_GENERATOR_SEED,
        generator_classes: RSZ_CLASSES,
        intended_target: separator,
        incidental_targets: vec![GeneratedInteractionTarget::spatial(
            GeneratedTargetClass::HeaderCell,
            Point::new(first_column_width(fixture) / 2.0, HEADER_SEPARATOR_Y),
        )],
        intent_commands: vec![InteractionIntentCommand::drag_pointer_to_targets(
            drag_start,
            drag_end,
            PointerPathPolicy::Linear { steps: 6 },
        )],
        expected_coverage: vec![
            CoverageKey::IntentCommandClass(IntentCommandClass::DragPointerTo),
            CoverageKey::PointerPathPolicyClass(PointerPathPolicyClass::Linear),
            CoverageKey::PointerPathSampleCountClass(PointerPathSampleCountClass::MultiSample),
            CoverageKey::CoalescedMoveClass(PointerMoveExposureClass::FrameSeparated),
            CoverageKey::CrossedHoverTargetClass("header_cell".to_string()),
            CoverageKey::CrossedSeparatorClass("header_separator".to_string()),
            CoverageKey::CaptureTransitionClass(PointerCaptureTransitionClass::PointerDown),
            CoverageKey::CaptureTransitionClass(
                PointerCaptureTransitionClass::DragThresholdCrossed,
            ),
            CoverageKey::CaptureTransitionClass(PointerCaptureTransitionClass::PointerUp),
            CoverageKey::IncidentalRouteTargetClass("header_cell".to_string()),
            CoverageKey::EndpointTeleportationMode(EndpointTeleportationMode::SampledPath),
        ],
    })
}

fn build_rsz2(
    fixture: InteractionFixtureDefinition,
    root_seed: u64,
) -> Option<GeneratedInteractionScenario> {
    if fixture.kind != InteractionFixtureKind::ListViewResize
        || fixture.initial_horizontal_scroll <= 0.0
    {
        return None;
    }

    let separator = scrolled_name_separator_target(fixture.initial_horizontal_scroll);
    let drag_dx = resize_delta(root_seed, RSZ2_GENERATOR_SEED);
    let drag_start = separator.intent_target(
        fixture,
        GEN_346_RSZ2_ID,
        root_seed,
        RSZ2_GENERATOR_SEED,
        "drag_start",
    );
    let drag_end = resize_drag_endpoint(
        fixture,
        GEN_346_RSZ2_ID,
        root_seed,
        RSZ2_GENERATOR_SEED,
        Point::new(separator.point().x + drag_dx, separator.point().y),
        "drag_end",
    );
    Some(GeneratedInteractionScenario {
        generator_id: GEN_346_RSZ2_ID,
        fixture_id: fixture.fixture_id,
        root_seed,
        generator_seed: RSZ2_GENERATOR_SEED,
        generator_classes: RSZ_CLASSES,
        intended_target: separator,
        incidental_targets: vec![GeneratedInteractionTarget::spatial(
            GeneratedTargetClass::HeaderCell,
            Point::new(separator.point().x - 24.0, HEADER_SEPARATOR_Y),
        )],
        intent_commands: vec![InteractionIntentCommand::drag_pointer_to_targets(
            drag_start,
            drag_end,
            PointerPathPolicy::Jittered {
                steps: 7,
                amplitude: 1.25,
                seed: root_seed ^ RSZ2_GENERATOR_SEED,
            },
        )],
        expected_coverage: vec![
            CoverageKey::IntentCommandClass(IntentCommandClass::DragPointerTo),
            CoverageKey::PointerPathPolicyClass(PointerPathPolicyClass::Jittered),
            CoverageKey::PointerPathSampleCountClass(PointerPathSampleCountClass::MultiSample),
            CoverageKey::CoalescedMoveClass(PointerMoveExposureClass::FrameSeparated),
            CoverageKey::CrossedHoverTargetClass("header_cell".to_string()),
            CoverageKey::CrossedSeparatorClass("header_separator".to_string()),
            CoverageKey::CaptureTransitionClass(PointerCaptureTransitionClass::PointerDown),
            CoverageKey::CaptureTransitionClass(
                PointerCaptureTransitionClass::DragThresholdCrossed,
            ),
            CoverageKey::CaptureTransitionClass(PointerCaptureTransitionClass::PointerUp),
            CoverageKey::IncidentalRouteTargetClass("header_cell".to_string()),
            CoverageKey::EndpointTeleportationMode(EndpointTeleportationMode::SampledPath),
        ],
    })
}

fn build_rsz3(
    fixture: InteractionFixtureDefinition,
    root_seed: u64,
) -> Option<GeneratedInteractionScenario> {
    if fixture.kind != InteractionFixtureKind::ListViewResize {
        return None;
    }

    let separator = first_separator_target(fixture);
    let first_drag_dx = resize_delta(root_seed, RSZ3_GENERATOR_SEED);
    let subsequent_drag_dx = first_drag_dx * 0.5;
    let drag_start = separator.intent_target(
        fixture,
        GEN_346_RSZ3_ID,
        root_seed,
        RSZ3_GENERATOR_SEED,
        "first_drag_start",
    );
    let first_drag_end_point = Point::new(separator.point().x + first_drag_dx, separator.point().y);
    let first_drag_end = resize_drag_endpoint(
        fixture,
        GEN_346_RSZ3_ID,
        root_seed,
        RSZ3_GENERATOR_SEED,
        first_drag_end_point,
        "first_drag_end",
    );
    let subsequent_drag_end = resize_drag_endpoint(
        fixture,
        GEN_346_RSZ3_ID,
        root_seed,
        RSZ3_GENERATOR_SEED,
        Point::new(
            separator.point().x + first_drag_dx + subsequent_drag_dx,
            separator.point().y,
        ),
        "subsequent_drag_end",
    );
    Some(GeneratedInteractionScenario {
        generator_id: GEN_346_RSZ3_ID,
        fixture_id: fixture.fixture_id,
        root_seed,
        generator_seed: RSZ3_GENERATOR_SEED,
        generator_classes: RSZ_CLASSES,
        intended_target: separator,
        incidental_targets: vec![GeneratedInteractionTarget::spatial(
            GeneratedTargetClass::HeaderCell,
            Point::new(separator.point().x - 18.0, HEADER_SEPARATOR_Y),
        )],
        intent_commands: vec![
            InteractionIntentCommand::drag_pointer_to_targets(
                drag_start,
                first_drag_end.clone(),
                PointerPathPolicy::Linear { steps: 6 },
            ),
            InteractionIntentCommand::drag_pointer_to_targets(
                first_drag_end,
                subsequent_drag_end,
                PointerPathPolicy::Linear { steps: 5 },
            ),
        ],
        expected_coverage: vec![
            CoverageKey::IntentCommandClass(IntentCommandClass::DragPointerTo),
            CoverageKey::PointerPathPolicyClass(PointerPathPolicyClass::Linear),
            CoverageKey::PointerPathSampleCountClass(PointerPathSampleCountClass::MultiSample),
            CoverageKey::CoalescedMoveClass(PointerMoveExposureClass::FrameSeparated),
            CoverageKey::CrossedHoverTargetClass("header_cell".to_string()),
            CoverageKey::CrossedSeparatorClass("header_separator".to_string()),
            CoverageKey::CaptureTransitionClass(PointerCaptureTransitionClass::PointerDown),
            CoverageKey::CaptureTransitionClass(
                PointerCaptureTransitionClass::DragThresholdCrossed,
            ),
            CoverageKey::CaptureTransitionClass(PointerCaptureTransitionClass::PointerUp),
            CoverageKey::IncidentalRouteTargetClass("header_cell".to_string()),
            CoverageKey::EndpointTeleportationMode(EndpointTeleportationMode::SampledPath),
        ],
    })
}

fn build_rsz4(
    fixture: InteractionFixtureDefinition,
    root_seed: u64,
) -> Option<GeneratedInteractionScenario> {
    if fixture.kind != InteractionFixtureKind::ListViewResize {
        return None;
    }

    let separator = first_separator_target(fixture);
    let drag_dx = resize_delta(root_seed, RSZ4_GENERATOR_SEED);
    let drag_start = separator.intent_target(
        fixture,
        GEN_346_RSZ4_ID,
        root_seed,
        RSZ4_GENERATOR_SEED,
        "forward_drag_start",
    );
    let drag_end_point = Point::new(separator.point().x + drag_dx, separator.point().y);
    let drag_end = resize_drag_endpoint(
        fixture,
        GEN_346_RSZ4_ID,
        root_seed,
        RSZ4_GENERATOR_SEED,
        drag_end_point,
        "forward_drag_end",
    );
    let restore_end = resize_drag_endpoint(
        fixture,
        GEN_346_RSZ4_ID,
        root_seed,
        RSZ4_GENERATOR_SEED,
        separator.point(),
        "restore_drag_end",
    );
    Some(GeneratedInteractionScenario {
        generator_id: GEN_346_RSZ4_ID,
        fixture_id: fixture.fixture_id,
        root_seed,
        generator_seed: RSZ4_GENERATOR_SEED,
        generator_classes: RSZ_CLASSES,
        intended_target: separator,
        incidental_targets: vec![GeneratedInteractionTarget::spatial(
            GeneratedTargetClass::HeaderCell,
            Point::new(separator.point().x - 20.0, HEADER_SEPARATOR_Y),
        )],
        intent_commands: vec![
            InteractionIntentCommand::drag_pointer_to_targets(
                drag_start,
                drag_end.clone(),
                PointerPathPolicy::Linear { steps: 6 },
            ),
            InteractionIntentCommand::drag_pointer_to_targets(
                drag_end,
                restore_end,
                PointerPathPolicy::Linear { steps: 6 },
            ),
        ],
        expected_coverage: vec![
            CoverageKey::IntentCommandClass(IntentCommandClass::DragPointerTo),
            CoverageKey::PointerPathPolicyClass(PointerPathPolicyClass::Linear),
            CoverageKey::PointerPathSampleCountClass(PointerPathSampleCountClass::MultiSample),
            CoverageKey::CoalescedMoveClass(PointerMoveExposureClass::FrameSeparated),
            CoverageKey::CrossedHoverTargetClass("header_cell".to_string()),
            CoverageKey::CrossedSeparatorClass("header_separator".to_string()),
            CoverageKey::CaptureTransitionClass(PointerCaptureTransitionClass::PointerDown),
            CoverageKey::CaptureTransitionClass(
                PointerCaptureTransitionClass::DragThresholdCrossed,
            ),
            CoverageKey::CaptureTransitionClass(PointerCaptureTransitionClass::PointerUp),
            CoverageKey::IncidentalRouteTargetClass("header_cell".to_string()),
            CoverageKey::EndpointTeleportationMode(EndpointTeleportationMode::SampledPath),
        ],
    })
}

fn build_scr1(
    fixture: InteractionFixtureDefinition,
    root_seed: u64,
) -> Option<GeneratedInteractionScenario> {
    if fixture.kind != InteractionFixtureKind::ListViewResize {
        return None;
    }

    let row_cell = GeneratedInteractionTarget::spatial(
        GeneratedTargetClass::RowCell,
        Point::new(LIST_VIEW_RESIZE_COL_INDEX_WIDTH + 32.0, BODY_CELL_Y),
    );
    let separator = first_separator_target(fixture);
    let drag_dx = resize_delta(root_seed, SCR1_GENERATOR_SEED);
    let wheel_target = row_cell.intent_target(
        fixture,
        GEN_346_SCR1_ID,
        root_seed,
        SCR1_GENERATOR_SEED,
        "wheel_target",
    );
    let drag_start = separator.intent_target(
        fixture,
        GEN_346_SCR1_ID,
        root_seed,
        SCR1_GENERATOR_SEED,
        "drag_start",
    );
    let drag_end = resize_drag_endpoint(
        fixture,
        GEN_346_SCR1_ID,
        root_seed,
        SCR1_GENERATOR_SEED,
        Point::new(separator.point().x + drag_dx, separator.point().y),
        "drag_end",
    );
    Some(GeneratedInteractionScenario {
        generator_id: GEN_346_SCR1_ID,
        fixture_id: fixture.fixture_id,
        root_seed,
        generator_seed: SCR1_GENERATOR_SEED,
        generator_classes: SCR1_CLASSES,
        intended_target: separator,
        incidental_targets: vec![row_cell],
        intent_commands: vec![
            InteractionIntentCommand::wheel_at_interaction_target(
                wheel_target,
                0.0,
                -64.0,
                WheelDeltaMode::Pixels,
                PointerPathPolicy::EdgeAware {
                    approach: PointerApproach::HorizontalFirst,
                    steps: 4,
                },
            ),
            InteractionIntentCommand::drag_pointer_to_targets(
                drag_start,
                drag_end,
                PointerPathPolicy::Linear { steps: 6 },
            ),
        ],
        expected_coverage: vec![
            CoverageKey::IntentCommandClass(IntentCommandClass::WheelAtTarget),
            CoverageKey::IntentCommandClass(IntentCommandClass::DragPointerTo),
            CoverageKey::PointerPathPolicyClass(PointerPathPolicyClass::EdgeAware),
            CoverageKey::PointerPathPolicyClass(PointerPathPolicyClass::Linear),
            CoverageKey::PointerPathSampleCountClass(PointerPathSampleCountClass::MultiSample),
            CoverageKey::CoalescedMoveClass(PointerMoveExposureClass::FrameSeparated),
            CoverageKey::CrossedHoverTargetClass("row_cell".to_string()),
            CoverageKey::CrossedSeparatorClass("header_separator".to_string()),
            CoverageKey::IncidentalRouteTargetClass("row_cell".to_string()),
            CoverageKey::EndpointTeleportationMode(EndpointTeleportationMode::SampledPath),
        ],
    })
}

fn build_foc1(
    fixture: InteractionFixtureDefinition,
    root_seed: u64,
) -> Option<GeneratedInteractionScenario> {
    if fixture.kind != InteractionFixtureKind::ListViewResize {
        return None;
    }

    let keys = match (root_seed ^ FOC1_GENERATOR_SEED) % 2 {
        0 => vec![KeyCode::Down, KeyCode::Down, KeyCode::PageDown],
        _ => vec![KeyCode::Home, KeyCode::Down, KeyCode::End],
    };
    Some(GeneratedInteractionScenario {
        generator_id: GEN_346_FOC1_ID,
        fixture_id: fixture.fixture_id,
        root_seed,
        generator_seed: FOC1_GENERATOR_SEED,
        generator_classes: KBD1_CLASSES,
        intended_target: GeneratedInteractionTarget::non_spatial(
            GeneratedTargetClass::KeyboardTraversalStep,
        ),
        incidental_targets: vec![GeneratedInteractionTarget::non_spatial(
            GeneratedTargetClass::KeyboardTraversalStep,
        )],
        intent_commands: vec![InteractionIntentCommand::keyboard_navigation(
            keys,
            None,
            Modifiers::default(),
            KeyboardNavigationPolicy::ArrowTraversal,
        )],
        expected_coverage: vec![
            CoverageKey::IntentCommandClass(IntentCommandClass::KeyboardNavigation),
            CoverageKey::KeyboardNavigationPolicy(KeyboardNavigationPolicy::ArrowTraversal),
            CoverageKey::IncidentalRouteTargetClass("keyboard_traversal_step".to_string()),
        ],
    })
}

fn build_pth1(
    fixture: InteractionFixtureDefinition,
    root_seed: u64,
) -> Option<GeneratedInteractionScenario> {
    if fixture.kind != InteractionFixtureKind::ListViewResize {
        return None;
    }

    let outside = GeneratedInteractionTarget::spatial(
        GeneratedTargetClass::OutsideListView,
        Point::new(OUTSIDE_LIST_VIEW_X, OUTSIDE_LIST_VIEW_Y),
    );
    let row_cell = GeneratedInteractionTarget::spatial(
        GeneratedTargetClass::RowCell,
        Point::new(LIST_VIEW_RESIZE_COL_INDEX_WIDTH + 40.0, BODY_CELL_Y),
    );
    let separator = first_separator_target(fixture);
    let drag_dx = resize_delta(root_seed, PTH1_GENERATOR_SEED);
    let outside_target = outside.intent_target(
        fixture,
        GEN_346_PTH1_ID,
        root_seed,
        PTH1_GENERATOR_SEED,
        "outside_move",
    );
    let row_cell_target = row_cell.intent_target(
        fixture,
        GEN_346_PTH1_ID,
        root_seed,
        PTH1_GENERATOR_SEED,
        "row_cell_move",
    );
    let drag_start = separator.intent_target(
        fixture,
        GEN_346_PTH1_ID,
        root_seed,
        PTH1_GENERATOR_SEED,
        "drag_start",
    );
    let drag_end = resize_drag_endpoint(
        fixture,
        GEN_346_PTH1_ID,
        root_seed,
        PTH1_GENERATOR_SEED,
        Point::new(separator.point().x + drag_dx, separator.point().y),
        "drag_end",
    );

    Some(GeneratedInteractionScenario {
        generator_id: GEN_346_PTH1_ID,
        fixture_id: fixture.fixture_id,
        root_seed,
        generator_seed: PTH1_GENERATOR_SEED,
        generator_classes: PTH1_CLASSES,
        intended_target: separator,
        incidental_targets: vec![outside, row_cell],
        intent_commands: vec![
            InteractionIntentCommand::move_pointer_to_target(
                outside_target,
                PointerPathPolicy::EdgeAware {
                    approach: PointerApproach::HorizontalFirst,
                    steps: 4,
                },
            ),
            InteractionIntentCommand::move_pointer_to_target(
                row_cell_target,
                PointerPathPolicy::Jittered {
                    steps: 5,
                    amplitude: 1.5,
                    seed: root_seed ^ PTH1_GENERATOR_SEED,
                },
            ),
            InteractionIntentCommand::drag_pointer_to_targets(
                drag_start,
                drag_end,
                PointerPathPolicy::EdgeAware {
                    approach: PointerApproach::VerticalFirst,
                    steps: 6,
                },
            ),
        ],
        expected_coverage: vec![
            CoverageKey::IntentCommandClass(IntentCommandClass::MovePointerTo),
            CoverageKey::IntentCommandClass(IntentCommandClass::DragPointerTo),
            CoverageKey::PointerPathPolicyClass(PointerPathPolicyClass::EdgeAware),
            CoverageKey::PointerPathPolicyClass(PointerPathPolicyClass::Jittered),
            CoverageKey::PointerPathSampleCountClass(PointerPathSampleCountClass::MultiSample),
            CoverageKey::CoalescedMoveClass(PointerMoveExposureClass::FrameSeparated),
            CoverageKey::CrossedHoverTargetClass("outside_list_view".to_string()),
            CoverageKey::CrossedHoverTargetClass("row_cell".to_string()),
            CoverageKey::CrossedSeparatorClass("header_separator".to_string()),
            CoverageKey::CaptureTransitionClass(PointerCaptureTransitionClass::PointerDown),
            CoverageKey::CaptureTransitionClass(
                PointerCaptureTransitionClass::DragThresholdCrossed,
            ),
            CoverageKey::CaptureTransitionClass(PointerCaptureTransitionClass::PointerUp),
            CoverageKey::IncidentalRouteTargetClass("outside_list_view".to_string()),
            CoverageKey::IncidentalRouteTargetClass("row_cell".to_string()),
            CoverageKey::EndpointTeleportationMode(EndpointTeleportationMode::SampledPath),
        ],
    })
}

fn build_pth2(
    fixture: InteractionFixtureDefinition,
    root_seed: u64,
) -> Option<GeneratedInteractionScenario> {
    if fixture.kind != InteractionFixtureKind::ListViewResize {
        return None;
    }

    let row_cell = GeneratedInteractionTarget::spatial(
        GeneratedTargetClass::RowCell,
        Point::new(LIST_VIEW_RESIZE_COL_INDEX_WIDTH + 44.0, BODY_CELL_Y),
    );
    let separator = first_separator_target(fixture);
    let drag_dx = resize_delta(root_seed, PTH2_GENERATOR_SEED);
    let row_cell_target = row_cell.intent_target(
        fixture,
        GEN_346_PTH2_ID,
        root_seed,
        PTH2_GENERATOR_SEED,
        "row_cell_move",
    );
    let drag_start = separator.intent_target(
        fixture,
        GEN_346_PTH2_ID,
        root_seed,
        PTH2_GENERATOR_SEED,
        "drag_start",
    );
    let drag_end = resize_drag_endpoint(
        fixture,
        GEN_346_PTH2_ID,
        root_seed,
        PTH2_GENERATOR_SEED,
        Point::new(separator.point().x + drag_dx, separator.point().y),
        "drag_end",
    );
    Some(GeneratedInteractionScenario {
        generator_id: GEN_346_PTH2_ID,
        fixture_id: fixture.fixture_id,
        root_seed,
        generator_seed: PTH2_GENERATOR_SEED,
        generator_classes: PTH1_CLASSES,
        intended_target: separator,
        incidental_targets: vec![row_cell],
        intent_commands: vec![
            InteractionIntentCommand::move_pointer_to_target(
                row_cell_target,
                PointerPathPolicy::OvershootCorrect {
                    overshoot: 14.0,
                    correction_steps: 3,
                },
            ),
            InteractionIntentCommand::drag_pointer_to_targets(
                drag_start,
                drag_end,
                PointerPathPolicy::OvershootCorrect {
                    overshoot: 12.0,
                    correction_steps: 4,
                },
            ),
        ],
        expected_coverage: vec![
            CoverageKey::IntentCommandClass(IntentCommandClass::MovePointerTo),
            CoverageKey::IntentCommandClass(IntentCommandClass::DragPointerTo),
            CoverageKey::PointerPathPolicyClass(PointerPathPolicyClass::OvershootCorrect),
            CoverageKey::PointerPathSampleCountClass(PointerPathSampleCountClass::MultiSample),
            CoverageKey::CoalescedMoveClass(PointerMoveExposureClass::FrameSeparated),
            CoverageKey::CrossedHoverTargetClass("row_cell".to_string()),
            CoverageKey::CrossedSeparatorClass("header_separator".to_string()),
            CoverageKey::CaptureTransitionClass(PointerCaptureTransitionClass::PointerDown),
            CoverageKey::CaptureTransitionClass(
                PointerCaptureTransitionClass::DragThresholdCrossed,
            ),
            CoverageKey::CaptureTransitionClass(PointerCaptureTransitionClass::PointerUp),
            CoverageKey::IncidentalRouteTargetClass("row_cell".to_string()),
            CoverageKey::EndpointTeleportationMode(EndpointTeleportationMode::SampledPath),
        ],
    })
}

fn build_pth3(
    fixture: InteractionFixtureDefinition,
    root_seed: u64,
) -> Option<GeneratedInteractionScenario> {
    if fixture.kind != InteractionFixtureKind::ListViewResize {
        return None;
    }

    let separator = first_separator_target(fixture);
    let drag_dx = resize_delta(root_seed, PTH3_GENERATOR_SEED);
    let hover_target = separator.intent_target(
        fixture,
        GEN_346_PTH3_ID,
        root_seed,
        PTH3_GENERATOR_SEED,
        "hover_target",
    );
    let drag_start = separator.intent_target(
        fixture,
        GEN_346_PTH3_ID,
        root_seed,
        PTH3_GENERATOR_SEED,
        "drag_start",
    );
    let drag_end = resize_drag_endpoint(
        fixture,
        GEN_346_PTH3_ID,
        root_seed,
        PTH3_GENERATOR_SEED,
        Point::new(separator.point().x + drag_dx, separator.point().y),
        "drag_end",
    );
    Some(GeneratedInteractionScenario {
        generator_id: GEN_346_PTH3_ID,
        fixture_id: fixture.fixture_id,
        root_seed,
        generator_seed: PTH3_GENERATOR_SEED,
        generator_classes: PTH1_CLASSES,
        intended_target: separator,
        incidental_targets: vec![GeneratedInteractionTarget::spatial(
            GeneratedTargetClass::HeaderCell,
            Point::new(separator.point().x - 16.0, HEADER_SEPARATOR_Y),
        )],
        intent_commands: vec![
            InteractionIntentCommand::move_pointer_to_target(
                hover_target,
                PointerPathPolicy::HoverPause {
                    steps: 5,
                    hover_frames: 3,
                },
            ),
            InteractionIntentCommand::drag_pointer_to_targets(
                drag_start,
                drag_end,
                PointerPathPolicy::HoverPause {
                    steps: 5,
                    hover_frames: 2,
                },
            ),
        ],
        expected_coverage: vec![
            CoverageKey::IntentCommandClass(IntentCommandClass::MovePointerTo),
            CoverageKey::IntentCommandClass(IntentCommandClass::DragPointerTo),
            CoverageKey::PointerPathPolicyClass(PointerPathPolicyClass::HoverPause),
            CoverageKey::PointerPathSampleCountClass(PointerPathSampleCountClass::MultiSample),
            CoverageKey::CoalescedMoveClass(PointerMoveExposureClass::FrameSeparated),
            CoverageKey::CrossedHoverTargetClass("header_cell".to_string()),
            CoverageKey::CrossedSeparatorClass("header_separator".to_string()),
            CoverageKey::CaptureTransitionClass(PointerCaptureTransitionClass::PointerDown),
            CoverageKey::CaptureTransitionClass(
                PointerCaptureTransitionClass::DragThresholdCrossed,
            ),
            CoverageKey::CaptureTransitionClass(PointerCaptureTransitionClass::PointerUp),
            CoverageKey::IncidentalRouteTargetClass("header_cell".to_string()),
            CoverageKey::EndpointTeleportationMode(EndpointTeleportationMode::SampledPath),
        ],
    })
}

fn build_kbd1(
    fixture: InteractionFixtureDefinition,
    root_seed: u64,
) -> Option<GeneratedInteractionScenario> {
    if fixture.kind != InteractionFixtureKind::ListViewResize {
        return None;
    }

    let (keys, activation) = match (root_seed ^ KBD1_GENERATOR_SEED) % 3 {
        0 => (vec![KeyCode::Tab, KeyCode::Down], Some(KeyCode::Enter)),
        1 => (
            vec![KeyCode::Tab, KeyCode::Tab, KeyCode::Up],
            Some(KeyCode::Space),
        ),
        _ => (
            vec![KeyCode::Down, KeyCode::Down, KeyCode::End],
            Some(KeyCode::Enter),
        ),
    };

    Some(GeneratedInteractionScenario {
        generator_id: GEN_346_KBD1_ID,
        fixture_id: fixture.fixture_id,
        root_seed,
        generator_seed: KBD1_GENERATOR_SEED,
        generator_classes: KBD1_CLASSES,
        intended_target: GeneratedInteractionTarget::non_spatial(
            GeneratedTargetClass::KeyboardActivationTarget,
        ),
        incidental_targets: vec![GeneratedInteractionTarget::non_spatial(
            GeneratedTargetClass::KeyboardTraversalStep,
        )],
        intent_commands: vec![InteractionIntentCommand::keyboard_navigation(
            keys,
            activation,
            Modifiers::default(),
            KeyboardNavigationPolicy::TraversalBeforeActivation,
        )],
        expected_coverage: vec![
            CoverageKey::IntentCommandClass(IntentCommandClass::KeyboardNavigation),
            CoverageKey::KeyboardNavigationPolicy(
                KeyboardNavigationPolicy::TraversalBeforeActivation,
            ),
            CoverageKey::IncidentalRouteTargetClass("keyboard_traversal_step".to_string()),
        ],
    })
}

fn is_shared_tabular_resize_fixture(kind: InteractionFixtureKind) -> bool {
    matches!(
        kind,
        InteractionFixtureKind::ListViewResize | InteractionFixtureKind::TreeViewProjection
    )
}

fn first_column_width(fixture: InteractionFixtureDefinition) -> f32 {
    match fixture.kind {
        InteractionFixtureKind::ListViewResize => LIST_VIEW_RESIZE_COL_INDEX_WIDTH,
        InteractionFixtureKind::TreeViewProjection => TREE_VIEW_PROJECTION_TREE_COLUMN_WIDTH,
    }
}

fn first_separator_target(fixture: InteractionFixtureDefinition) -> GeneratedInteractionTarget {
    GeneratedInteractionTarget::spatial(
        GeneratedTargetClass::Separator,
        Point::new(first_column_width(fixture) - 1.0, HEADER_SEPARATOR_Y),
    )
}

fn scrolled_name_separator_target(initial_horizontal_scroll: f32) -> GeneratedInteractionTarget {
    GeneratedInteractionTarget::spatial(
        GeneratedTargetClass::Separator,
        Point::new(
            LIST_VIEW_RESIZE_COL_INDEX_WIDTH + LIST_VIEW_RESIZE_COL_NAME_WIDTH
                - initial_horizontal_scroll
                - 1.0,
            HEADER_SEPARATOR_Y,
        ),
    )
}

fn resize_drag_endpoint(
    fixture: InteractionFixtureDefinition,
    generator_id: &'static str,
    root_seed: u64,
    generator_seed: u64,
    point: Point,
    role: &'static str,
) -> InteractionIntentTarget {
    generated_intent_target(
        fixture,
        generator_id,
        root_seed,
        generator_seed,
        GeneratedTargetClass::ResizeDragEndpoint,
        point,
        role,
    )
}

fn generated_intent_target(
    fixture: InteractionFixtureDefinition,
    generator_id: &'static str,
    root_seed: u64,
    generator_seed: u64,
    class: GeneratedTargetClass,
    point: Point,
    role: &'static str,
) -> InteractionIntentTarget {
    InteractionIntentTarget::resolved_point(
        generated_target_id(fixture, generator_id, root_seed, class, role),
        generated_target_kind(class),
        Some(generated_intent_target_class_token(class).to_string()),
        "logical_window",
        point,
        generated_source_probe_id(fixture.kind),
        root_seed ^ generator_seed,
    )
}

fn generated_source_probe_id(kind: InteractionFixtureKind) -> &'static str {
    match kind {
        InteractionFixtureKind::ListViewResize => "list_view_resize_probe",
        InteractionFixtureKind::TreeViewProjection => "tree_view_projection_probe",
    }
}

fn generated_target_id(
    fixture: InteractionFixtureDefinition,
    generator_id: &'static str,
    root_seed: u64,
    class: GeneratedTargetClass,
    role: &'static str,
) -> String {
    match class {
        GeneratedTargetClass::Separator if fixture.initial_horizontal_scroll > 0.0 => {
            format!("{}.column_separator.name_description", fixture.fixture_id)
        }
        GeneratedTargetClass::Separator => {
            format!("{}.column_separator.index_name", fixture.fixture_id)
        }
        GeneratedTargetClass::HeaderCell => format!("{}.header_cell.index", fixture.fixture_id),
        GeneratedTargetClass::RowCell => format!("{}.body_cell.name.sample", fixture.fixture_id),
        GeneratedTargetClass::OutsideListView => {
            format!("{}.outside_list_view.left_probe", fixture.fixture_id)
        }
        GeneratedTargetClass::ResizeDragEndpoint => format!(
            "{}.resize_drag_endpoint.{}.{}.seed_{:016x}",
            fixture.fixture_id, generator_id, role, root_seed
        ),
        GeneratedTargetClass::KeyboardTraversalStep => {
            format!("{}.keyboard_traversal_step", fixture.fixture_id)
        }
        GeneratedTargetClass::KeyboardActivationTarget => {
            format!("{}.keyboard_activation_target", fixture.fixture_id)
        }
    }
}

fn generated_target_kind(class: GeneratedTargetClass) -> &'static str {
    match class {
        GeneratedTargetClass::OutsideListView => "outside_list_view",
        GeneratedTargetClass::RowCell => "body_cell",
        GeneratedTargetClass::HeaderCell => "header_cell",
        GeneratedTargetClass::Separator => "column_separator",
        GeneratedTargetClass::KeyboardTraversalStep => "keyboard_traversal_step",
        GeneratedTargetClass::KeyboardActivationTarget => "keyboard_activation_target",
        GeneratedTargetClass::ResizeDragEndpoint => "resize_drag_endpoint",
    }
}

fn generated_intent_target_class_token(class: GeneratedTargetClass) -> &'static str {
    match class {
        GeneratedTargetClass::OutsideListView => "outside_list_view",
        GeneratedTargetClass::RowCell => "body_cell",
        GeneratedTargetClass::HeaderCell => "header_cell",
        GeneratedTargetClass::Separator => "header_separator",
        GeneratedTargetClass::KeyboardTraversalStep => "keyboard_traversal_step",
        GeneratedTargetClass::KeyboardActivationTarget => "keyboard_activation_target",
        GeneratedTargetClass::ResizeDragEndpoint => "resize_drag_endpoint",
    }
}

fn push_target_exposure_coverage(
    keys: &mut Vec<CoverageKey>,
    target: GeneratedInteractionTarget,
    incidental: bool,
) {
    let target_class = generated_target_class_token(target.class);
    if target.point.is_some() {
        keys.push(CoverageKey::CrossedHoverTargetClass(
            target_class.to_string(),
        ));
    }
    if target.class == GeneratedTargetClass::Separator {
        keys.push(CoverageKey::CrossedSeparatorClass(
            "header_separator".to_string(),
        ));
    }
    if incidental {
        keys.push(CoverageKey::IncidentalRouteTargetClass(
            target_class.to_string(),
        ));
    }
}

fn generated_target_class_token(class: GeneratedTargetClass) -> &'static str {
    match class {
        GeneratedTargetClass::OutsideListView => "outside_list_view",
        GeneratedTargetClass::RowCell => "row_cell",
        GeneratedTargetClass::HeaderCell => "header_cell",
        GeneratedTargetClass::Separator => "separator",
        GeneratedTargetClass::KeyboardTraversalStep => "keyboard_traversal_step",
        GeneratedTargetClass::KeyboardActivationTarget => "keyboard_activation_target",
        GeneratedTargetClass::ResizeDragEndpoint => "resize_drag_endpoint",
    }
}

fn resize_delta(root_seed: u64, generator_seed: u64) -> f32 {
    match (root_seed ^ generator_seed) % 3 {
        0 => 25.0,
        1 => 33.0,
        _ => 41.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::interaction_fuzz::{PointerPathSegment, PrimitiveCommandClass};
    use crate::tests::interaction_fuzz::registry::find_interaction_fixture;

    #[test]
    fn generator_registry_order_is_deterministic() {
        let generator_bindings: Vec<(&str, &str)> = interaction_generator_registry()
            .iter()
            .map(|definition| (definition.generator_id, definition.fixture_id))
            .collect();

        assert_eq!(
            generator_bindings,
            vec![
                (GEN_346_RSZ1_ID, LIST_VIEW_RESIZE_LV1_FIXTURE_ID),
                (GEN_346_RSZ1_ID, TREE_VIEW_PROJECTION_TV1_FIXTURE_ID),
                (GEN_346_RSZ2_ID, LIST_VIEW_RESIZE_LV2_FIXTURE_ID),
                (GEN_346_RSZ3_ID, LIST_VIEW_RESIZE_LV1_FIXTURE_ID),
                (GEN_346_RSZ4_ID, LIST_VIEW_RESIZE_LV1_FIXTURE_ID),
                (GEN_346_SCR1_ID, LIST_VIEW_RESIZE_LV1_FIXTURE_ID),
                (GEN_346_FOC1_ID, LIST_VIEW_RESIZE_LV1_FIXTURE_ID),
                (GEN_346_PTH1_ID, LIST_VIEW_RESIZE_LV1_FIXTURE_ID),
                (GEN_346_PTH2_ID, LIST_VIEW_RESIZE_LV1_FIXTURE_ID),
                (GEN_346_PTH3_ID, LIST_VIEW_RESIZE_LV1_FIXTURE_ID),
                (GEN_346_KBD1_ID, LIST_VIEW_RESIZE_LV1_FIXTURE_ID)
            ]
        );
    }

    #[test]
    fn resize_generators_bind_to_expected_fixtures() {
        let lv1 = find_interaction_fixture(LIST_VIEW_RESIZE_LV1_FIXTURE_ID)
            .expect("lv1 fixture must be registered");
        let lv2 = find_interaction_fixture(LIST_VIEW_RESIZE_LV2_FIXTURE_ID)
            .expect("lv2 fixture must be registered");
        let tv1 = find_interaction_fixture(TREE_VIEW_PROJECTION_TV1_FIXTURE_ID)
            .expect("tv1 fixture must be registered");

        let rsz1 = find_interaction_generator(GEN_346_RSZ1_ID)
            .expect("rsz1 generator must be registered")
            .build(lv1, 7)
            .expect("rsz1 must build against lv1");
        let tv1_rsz1 = generate_scenarios_for_fixture(tv1, 7)
            .into_iter()
            .find(|scenario| scenario.generator_id == GEN_346_RSZ1_ID)
            .expect("rsz1 must build against tv1");
        let rsz2 = find_interaction_generator(GEN_346_RSZ2_ID)
            .expect("rsz2 generator must be registered")
            .build(lv2, 7)
            .expect("rsz2 must build against lv2");
        let rsz3 = find_interaction_generator(GEN_346_RSZ3_ID)
            .expect("rsz3 generator must be registered")
            .build(lv1, 7)
            .expect("rsz3 must build against lv1");
        let rsz4 = find_interaction_generator(GEN_346_RSZ4_ID)
            .expect("rsz4 generator must be registered")
            .build(lv1, 7)
            .expect("rsz4 must build against lv1");

        assert_eq!(rsz1.fixture_id, LIST_VIEW_RESIZE_LV1_FIXTURE_ID);
        assert_eq!(tv1_rsz1.fixture_id, TREE_VIEW_PROJECTION_TV1_FIXTURE_ID);
        assert_eq!(rsz2.fixture_id, LIST_VIEW_RESIZE_LV2_FIXTURE_ID);
        assert_eq!(rsz3.fixture_id, LIST_VIEW_RESIZE_LV1_FIXTURE_ID);
        assert_eq!(rsz4.fixture_id, LIST_VIEW_RESIZE_LV1_FIXTURE_ID);
        assert_eq!(rsz1.generator_classes, RSZ_CLASSES);
        assert_eq!(tv1_rsz1.generator_classes, RSZ_CLASSES);
        assert_eq!(rsz2.generator_classes, RSZ_CLASSES);
        assert_eq!(rsz3.generator_classes, RSZ_CLASSES);
        assert_eq!(rsz4.generator_classes, RSZ_CLASSES);
        assert!(!rsz1.uses_instant_pointer_path());
        assert!(!tv1_rsz1.uses_instant_pointer_path());
        assert!(!rsz2.uses_instant_pointer_path());
        assert!(!rsz3.uses_instant_pointer_path());
        assert!(!rsz4.uses_instant_pointer_path());
        assert_eq!(tv1_rsz1.intent_commands.len(), 1);
        assert_eq!(
            tv1_rsz1.intended_target.point().x,
            TREE_VIEW_PROJECTION_TREE_COLUMN_WIDTH - 1.0
        );
        assert_eq!(rsz3.intent_commands.len(), 2);
        assert_eq!(rsz4.intent_commands.len(), 2);
        assert_eq!(rsz2.intended_target.class, GeneratedTargetClass::Separator);
        assert_eq!(
            rsz2.intended_target.point().x,
            LIST_VIEW_RESIZE_COL_INDEX_WIDTH + LIST_VIEW_RESIZE_COL_NAME_WIDTH
                - LIST_VIEW_RESIZE_LV2_INITIAL_HORIZONTAL_SCROLL
                - 1.0
        );

        assert!(
            find_interaction_generator(GEN_346_RSZ2_ID)
                .expect("rsz2 generator must be registered")
                .build(lv1, 7)
                .is_none()
        );
    }

    #[test]
    fn scr1_generator_wheels_at_cell_before_resize() {
        let lv1 = find_interaction_fixture(LIST_VIEW_RESIZE_LV1_FIXTURE_ID)
            .expect("lv1 fixture must be registered");
        let scr1 = find_interaction_generator(GEN_346_SCR1_ID)
            .expect("scr1 generator must be registered")
            .build(lv1, 13)
            .expect("scr1 must build against lv1");

        assert_eq!(scr1.generator_classes, SCR1_CLASSES);
        assert_eq!(scr1.intended_target.class, GeneratedTargetClass::Separator);
        assert_eq!(scr1.incidental_targets.len(), 1);
        assert_eq!(
            scr1.incidental_targets[0].class,
            GeneratedTargetClass::RowCell
        );
        assert!(!scr1.uses_instant_pointer_path());
        assert!(matches!(
            scr1.intent_commands[0],
            InteractionIntentCommand::WheelAtTarget { .. }
        ));

        let mut driver = InteractionDriver::new();
        scr1.execute(&mut driver)
            .expect("scr1 must execute through the interaction driver");
        let metrics = driver.trace_metrics();

        assert_eq!(metrics.intent_count, 2);
        assert!(
            metrics
                .primitive_steps
                .iter()
                .any(|step| step.command_class == PrimitiveCommandClass::WheelPixels)
        );
        assert!(
            driver
                .intent_trace()
                .iter()
                .flat_map(|intent| intent.expansion_steps.iter())
                .any(|step| step.segment == PointerPathSegment::Wheel)
        );
        for key in scr1.expected_coverage.iter() {
            assert!(driver.coverage().contains(key));
        }
    }

    #[test]
    fn foc1_generator_emits_focus_traversal_without_activation() {
        let lv1 = find_interaction_fixture(LIST_VIEW_RESIZE_LV1_FIXTURE_ID)
            .expect("lv1 fixture must be registered");
        let foc1 = find_interaction_generator(GEN_346_FOC1_ID)
            .expect("foc1 generator must be registered")
            .build(lv1, 17)
            .expect("foc1 must build against lv1");

        assert_eq!(foc1.generator_classes, KBD1_CLASSES);
        assert_eq!(
            foc1.intended_target.class,
            GeneratedTargetClass::KeyboardTraversalStep
        );
        assert_eq!(foc1.intended_target.point, None);
        assert_eq!(foc1.incidental_targets.len(), 1);
        assert_eq!(
            foc1.incidental_targets[0].class,
            GeneratedTargetClass::KeyboardTraversalStep
        );

        let mut driver = InteractionDriver::new();
        foc1.execute(&mut driver)
            .expect("foc1 must execute through the interaction driver");
        let metrics = driver.trace_metrics();

        assert_eq!(metrics.intent_count, 1);
        assert_eq!(
            metrics.intent_steps[0].keyboard_navigation_policy,
            Some(KeyboardNavigationPolicy::ArrowTraversal)
        );
        assert_eq!(metrics.pointer_sample_count, 0);
        assert_eq!(
            driver
                .intent_trace()
                .iter()
                .flat_map(|intent| intent.expansion_steps.iter())
                .filter(|step| step.segment == PointerPathSegment::KeyboardActivation)
                .count(),
            0
        );
        for key in foc1.expected_coverage.iter() {
            assert!(driver.coverage().contains(key));
        }
    }

    #[test]
    fn pth1_generator_uses_non_instant_incidental_path() {
        let lv1 = find_interaction_fixture(LIST_VIEW_RESIZE_LV1_FIXTURE_ID)
            .expect("lv1 fixture must be registered");
        let pth1 = find_interaction_generator(GEN_346_PTH1_ID)
            .expect("pth1 generator must be registered")
            .build(lv1, 11)
            .expect("pth1 must build against lv1");

        assert_eq!(pth1.generator_classes, PTH1_CLASSES);
        assert_eq!(pth1.intended_target.class, GeneratedTargetClass::Separator);
        assert_eq!(pth1.incidental_targets.len(), 2);
        assert!(
            pth1.incidental_targets
                .iter()
                .any(|target| target.class == GeneratedTargetClass::OutsideListView)
        );
        assert!(
            pth1.incidental_targets
                .iter()
                .any(|target| target.class == GeneratedTargetClass::RowCell)
        );
        assert!(!pth1.uses_instant_pointer_path());

        let mut driver = InteractionDriver::new();
        pth1.execute(&mut driver)
            .expect("pth1 must execute through the interaction driver");
        let metrics = driver.trace_metrics();

        assert_eq!(metrics.intent_count, 3);
        assert!(
            metrics
                .intent_steps
                .iter()
                .any(|intent| intent.path_policy_class == Some(PointerPathPolicyClass::Jittered))
        );
        assert!(
            metrics
                .intent_steps
                .iter()
                .any(|intent| intent.path_policy_class == Some(PointerPathPolicyClass::EdgeAware))
        );
        assert!(
            metrics
                .primitive_steps
                .iter()
                .any(|step| step.command_class == PrimitiveCommandClass::PointerDown)
        );
        assert!(
            driver
                .intent_trace()
                .iter()
                .flat_map(|intent| intent.expansion_steps.iter())
                .any(|step| step.segment == PointerPathSegment::ThresholdCrossing)
        );
        for key in pth1.expected_coverage.iter() {
            assert!(driver.coverage().contains(key));
        }
    }

    #[test]
    fn pth2_generator_uses_overshoot_correction_path() {
        let lv1 = find_interaction_fixture(LIST_VIEW_RESIZE_LV1_FIXTURE_ID)
            .expect("lv1 fixture must be registered");
        let pth2 = find_interaction_generator(GEN_346_PTH2_ID)
            .expect("pth2 generator must be registered")
            .build(lv1, 29)
            .expect("pth2 must build against lv1");

        assert_eq!(pth2.generator_classes, PTH1_CLASSES);
        assert_eq!(pth2.intended_target.class, GeneratedTargetClass::Separator);
        assert_eq!(pth2.incidental_targets.len(), 1);
        assert_eq!(
            pth2.incidental_targets[0].class,
            GeneratedTargetClass::RowCell
        );
        assert!(!pth2.uses_instant_pointer_path());

        let mut driver = InteractionDriver::new();
        pth2.execute(&mut driver)
            .expect("pth2 must execute through the interaction driver");
        let metrics = driver.trace_metrics();

        assert_eq!(metrics.intent_count, 2);
        assert!(metrics.intent_steps.iter().all(|intent| {
            intent.path_policy_class == Some(PointerPathPolicyClass::OvershootCorrect)
        }));
        for key in pth2.expected_coverage.iter() {
            assert!(driver.coverage().contains(key));
        }
    }

    #[test]
    fn pth3_generator_uses_hover_pause_path() {
        let lv1 = find_interaction_fixture(LIST_VIEW_RESIZE_LV1_FIXTURE_ID)
            .expect("lv1 fixture must be registered");
        let pth3 = find_interaction_generator(GEN_346_PTH3_ID)
            .expect("pth3 generator must be registered")
            .build(lv1, 31)
            .expect("pth3 must build against lv1");

        assert_eq!(pth3.generator_classes, PTH1_CLASSES);
        assert_eq!(pth3.intended_target.class, GeneratedTargetClass::Separator);
        assert_eq!(pth3.incidental_targets.len(), 1);
        assert_eq!(
            pth3.incidental_targets[0].class,
            GeneratedTargetClass::HeaderCell
        );
        assert!(!pth3.uses_instant_pointer_path());

        let mut driver = InteractionDriver::new();
        pth3.execute(&mut driver)
            .expect("pth3 must execute through the interaction driver");
        let metrics = driver.trace_metrics();

        assert_eq!(metrics.intent_count, 2);
        assert!(
            metrics
                .intent_steps
                .iter()
                .all(|intent| intent.path_policy_class == Some(PointerPathPolicyClass::HoverPause))
        );
        for key in pth3.expected_coverage.iter() {
            assert!(driver.coverage().contains(key));
        }
    }

    #[test]
    fn kbd1_generator_emits_traversal_before_activation() {
        let lv1 = find_interaction_fixture(LIST_VIEW_RESIZE_LV1_FIXTURE_ID)
            .expect("lv1 fixture must be registered");
        let kbd1 = find_interaction_generator(GEN_346_KBD1_ID)
            .expect("kbd1 generator must be registered")
            .build(lv1, 23)
            .expect("kbd1 must build against lv1");

        assert_eq!(kbd1.generator_classes, KBD1_CLASSES);
        assert_eq!(
            kbd1.intended_target.class,
            GeneratedTargetClass::KeyboardActivationTarget
        );
        assert_eq!(kbd1.intended_target.point, None);
        assert_eq!(kbd1.incidental_targets.len(), 1);
        assert_eq!(
            kbd1.incidental_targets[0].class,
            GeneratedTargetClass::KeyboardTraversalStep
        );
        assert_eq!(kbd1.incidental_targets[0].point, None);
        assert!(!kbd1.uses_instant_pointer_path());

        let mut driver = InteractionDriver::new();
        kbd1.execute(&mut driver)
            .expect("kbd1 must execute through the interaction driver");
        let metrics = driver.trace_metrics();

        assert_eq!(metrics.intent_count, 1);
        assert_eq!(
            metrics.intent_steps[0].command_class,
            IntentCommandClass::KeyboardNavigation
        );
        assert_eq!(metrics.intent_steps[0].path_policy_class, None);
        assert_eq!(
            metrics.intent_steps[0].keyboard_navigation_policy,
            Some(KeyboardNavigationPolicy::TraversalBeforeActivation)
        );
        assert!(metrics.primitive_step_count >= 6);
        assert_eq!(metrics.pointer_sample_count, 0);
        for key in kbd1.expected_coverage.iter() {
            assert!(driver.coverage().contains(key));
        }
    }

    #[test]
    fn generator_fixture_query_is_deterministic_and_fixture_scoped() {
        let lv1 = find_interaction_fixture(LIST_VIEW_RESIZE_LV1_FIXTURE_ID)
            .expect("lv1 fixture must be registered");
        let lv2 = find_interaction_fixture(LIST_VIEW_RESIZE_LV2_FIXTURE_ID)
            .expect("lv2 fixture must be registered");

        let lv1_scenarios = generate_scenarios_for_fixture(lv1, 19);
        let lv2_scenarios = generate_scenarios_for_fixture(lv2, 19);

        assert_eq!(lv1_scenarios.len(), 9);
        assert_eq!(lv2_scenarios.len(), 1);
        assert_eq!(lv1_scenarios[0].generator_id, GEN_346_RSZ1_ID);
        assert_eq!(lv1_scenarios[1].generator_id, GEN_346_RSZ3_ID);
        assert_eq!(lv1_scenarios[2].generator_id, GEN_346_RSZ4_ID);
        assert_eq!(lv1_scenarios[3].generator_id, GEN_346_SCR1_ID);
        assert_eq!(lv1_scenarios[4].generator_id, GEN_346_FOC1_ID);
        assert_eq!(lv1_scenarios[5].generator_id, GEN_346_PTH1_ID);
        assert_eq!(lv1_scenarios[6].generator_id, GEN_346_PTH2_ID);
        assert_eq!(lv1_scenarios[7].generator_id, GEN_346_PTH3_ID);
        assert_eq!(lv1_scenarios[8].generator_id, GEN_346_KBD1_ID);
        assert_eq!(lv2_scenarios[0].generator_id, GEN_346_RSZ2_ID);
        assert!(
            lv1_scenarios
                .iter()
                .chain(lv2_scenarios.iter())
                .all(|scenario| !scenario.uses_instant_pointer_path())
        );
    }

    #[test]
    fn generated_spatial_intents_use_probe_derived_targets() {
        let lv1 = find_interaction_fixture(LIST_VIEW_RESIZE_LV1_FIXTURE_ID)
            .expect("lv1 fixture must be registered");
        let scenarios = generate_scenarios_for_fixture(lv1, 19);

        for scenario in scenarios {
            for command in scenario.intent_commands.iter() {
                for target in spatial_intent_targets(command) {
                    assert_eq!(target.source_probe_id, "list_view_resize_probe");
                    assert_ne!(
                        target.source_probe_id,
                        "interaction_driver.synthetic_point_target"
                    );
                    assert_eq!(target.coordinate_space, "logical_window");
                    assert!(target.target_id.starts_with(scenario.fixture_id));
                    assert_ne!(target.probe_epoch, 0);
                }
            }
        }
    }

    #[test]
    fn generated_probe_targets_survive_replay_artifact_serialisation() {
        let lv2 = find_interaction_fixture(LIST_VIEW_RESIZE_LV2_FIXTURE_ID)
            .expect("lv2 fixture must be registered");
        let rsz2 = find_interaction_generator(GEN_346_RSZ2_ID)
            .expect("rsz2 generator must be registered")
            .build(lv2, 7)
            .expect("rsz2 must build against lv2");

        let InteractionIntentCommand::DragPointerTo { from, to, .. } = &rsz2.intent_commands[0]
        else {
            panic!("rsz2 must emit a drag intent");
        };
        assert_eq!(from.target_id, "lv2.column_separator.name_description");
        assert_eq!(from.target_kind, "column_separator");
        assert_eq!(from.target_class, Some("header_separator".to_string()));
        assert_eq!(to.target_kind, "resize_drag_endpoint");
        assert_eq!(to.target_class, Some("resize_drag_endpoint".to_string()));
        assert_eq!(from.source_probe_id, "list_view_resize_probe");
        assert_eq!(to.source_probe_id, "list_view_resize_probe");

        let mut driver = InteractionDriver::new();
        rsz2.execute(&mut driver)
            .expect("rsz2 must execute through the interaction driver");
        let artifact = crate::core::interaction_fuzz::InteractionReplayArtifact::from_driver(
            crate::core::interaction_fuzz::ReplayMode::ExactIntent,
            rsz2.fixture_id,
            rsz2.root_seed,
            None,
            &driver,
        );
        let value = serde_json::to_value(&artifact).expect("artifact should convert to JSON value");
        let from_target = &value["intent_trace"][0]["intent_command"]["from"];
        assert_eq!(
            from_target["target_id"],
            "lv2.column_separator.name_description"
        );
        assert_eq!(from_target["source_probe_id"], "list_view_resize_probe");
        assert!(from_target.get("x").is_none());
        assert_eq!(from_target["resolved_point"]["x"], from.resolved_point.x);
        assert_eq!(from_target["resolved_point"]["y"], from.resolved_point.y);
    }

    fn spatial_intent_targets(command: &InteractionIntentCommand) -> Vec<&InteractionIntentTarget> {
        match command {
            InteractionIntentCommand::MovePointerTo { target, .. }
            | InteractionIntentCommand::ClickTarget { target, .. }
            | InteractionIntentCommand::WheelAtTarget { target, .. }
            | InteractionIntentCommand::TextAtTarget { target, .. } => vec![target],
            InteractionIntentCommand::DragPointerTo { from, to, .. } => vec![from, to],
            InteractionIntentCommand::KeyboardNavigation { .. } => Vec::new(),
        }
    }
}
