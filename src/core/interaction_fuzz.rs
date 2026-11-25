//! SPEC-HAL-346 in-process interaction command driver.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::ops::Range;
use web_time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::core::event::{
    FusedInputEvent, KeyCode, KeyState, Modifiers, PointerButton, PointerDevice, RawButtonState,
    RawInputEvent, WheelDelta, WindowId,
};
use crate::core::input_fusion::InputFusion;
use crate::core::math::Point;

mod replay_f32 {
    use std::fmt;

    use serde::de::{Error, Unexpected, Visitor};
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(value: &f32, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if value.is_finite() {
            serializer.serialize_f32(*value)
        } else if value.is_nan() {
            serializer.serialize_str("NaN")
        } else if value.is_sign_positive() {
            serializer.serialize_str("Infinity")
        } else {
            serializer.serialize_str("-Infinity")
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<f32, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(ReplayF32Visitor)
    }

    struct ReplayF32Visitor;

    impl Visitor<'_> for ReplayF32Visitor {
        type Value = f32;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a finite f32 number or NaN/Infinity string token")
        }

        fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
        where
            E: Error,
        {
            let narrowed = value as f32;
            if value.is_finite() && narrowed.is_finite() {
                Ok(narrowed)
            } else {
                Err(E::invalid_value(Unexpected::Float(value), &self))
            }
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: Error,
        {
            let narrowed = value as f32;
            if narrowed.is_finite() {
                Ok(narrowed)
            } else {
                Err(E::invalid_value(Unexpected::Signed(value), &self))
            }
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: Error,
        {
            let narrowed = value as f32;
            if narrowed.is_finite() {
                Ok(narrowed)
            } else {
                Err(E::invalid_value(Unexpected::Unsigned(value), &self))
            }
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            match value {
                "NaN" => Ok(f32::NAN),
                "Infinity" | "+Infinity" => Ok(f32::INFINITY),
                "-Infinity" => Ok(f32::NEG_INFINITY),
                _ => Err(E::invalid_value(Unexpected::Str(value), &self)),
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct InteractionDriverConfig {
    pub window_id: WindowId,
    pub default_pointer_device: PointerDevice,
    pub tick_step: Duration,
}

impl Default for InteractionDriverConfig {
    fn default() -> Self {
        Self {
            window_id: WindowId::PRIMARY,
            default_pointer_device: PointerDevice::Mouse,
            tick_step: Duration::from_millis(1),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum InteractionCommand {
    PointerMove {
        x: f32,
        y: f32,
    },
    PointerDown {
        x: f32,
        y: f32,
        button: PointerButton,
    },
    PointerUp {
        x: f32,
        y: f32,
        button: PointerButton,
    },
    WheelPixels {
        x: f32,
        y: f32,
        delta_x: f32,
        delta_y: f32,
    },
    WheelLines {
        x: f32,
        y: f32,
        delta_x: f32,
        delta_y: f32,
    },
    KeyDown {
        key_code: KeyCode,
        modifiers: Modifiers,
        repeat: bool,
    },
    KeyUp {
        key_code: KeyCode,
        modifiers: Modifiers,
    },
    TextInput {
        text: String,
        modifiers: Modifiers,
    },
    Frame {
        count: u32,
    },
    Reconcile,
    SetWindowSize {
        width: f32,
        height: f32,
    },
    SetScaleFactor {
        scale_factor: f32,
    },
}

impl InteractionCommand {
    pub fn pointer_move(x: f32, y: f32) -> Self {
        Self::PointerMove { x, y }
    }

    pub fn pointer_down(x: f32, y: f32) -> Self {
        Self::PointerDown {
            x,
            y,
            button: PointerButton::Primary,
        }
    }

    pub fn pointer_up(x: f32, y: f32) -> Self {
        Self::PointerUp {
            x,
            y,
            button: PointerButton::Primary,
        }
    }

    pub fn frame() -> Self {
        Self::Frame { count: 1 }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum InteractionDriverError {
    NonFiniteSinkValue {
        primitive_step_index: usize,
        command: ReplayInteractionCommand,
        field: &'static str,
        value: f32,
    },
}

pub type InteractionDriverResult<T> = Result<T, InteractionDriverError>;

#[derive(Debug, Clone, PartialEq)]
pub struct InteractionIntentTarget {
    pub target_id: String,
    pub target_kind: String,
    pub target_class: Option<String>,
    pub coordinate_space: String,
    pub resolved_point: Point,
    pub source_probe_id: String,
    pub probe_epoch: u64,
}

impl InteractionIntentTarget {
    pub fn resolved_point(
        target_id: impl Into<String>,
        target_kind: impl Into<String>,
        target_class: Option<String>,
        coordinate_space: impl Into<String>,
        resolved_point: Point,
        source_probe_id: impl Into<String>,
        probe_epoch: u64,
    ) -> Self {
        Self {
            target_id: target_id.into(),
            target_kind: target_kind.into(),
            target_class,
            coordinate_space: coordinate_space.into(),
            resolved_point,
            source_probe_id: source_probe_id.into(),
            probe_epoch,
        }
    }

    pub fn synthetic_point(
        target_kind: &'static str,
        target_class: &'static str,
        point: Point,
    ) -> Self {
        Self::resolved_point(
            point_intent_target_id(target_kind, point),
            target_kind,
            Some(target_class.to_string()),
            "logical_window",
            point,
            "interaction_driver.synthetic_point_target",
            0,
        )
    }

    pub fn from_trace(trace: &InteractionIntentTargetTrace) -> Self {
        Self::resolved_point(
            trace.target_id.clone(),
            trace.target_kind.clone(),
            trace.target_class.clone(),
            trace.coordinate_space.clone(),
            trace.resolved_point.into(),
            trace.source_probe_id.clone(),
            trace.probe_epoch,
        )
    }

    pub fn trace(&self) -> InteractionIntentTargetTrace {
        InteractionIntentTargetTrace {
            target_id: self.target_id.clone(),
            target_kind: self.target_kind.clone(),
            target_class: self.target_class.clone(),
            coordinate_space: self.coordinate_space.clone(),
            resolved_point: self.resolved_point.into(),
            source_probe_id: self.source_probe_id.clone(),
            probe_epoch: self.probe_epoch,
        }
    }
}

#[derive(Debug, Clone)]
pub enum InteractionIntentCommand {
    MovePointerTo {
        target: InteractionIntentTarget,
        path_policy: PointerPathPolicy,
    },
    ClickTarget {
        target: InteractionIntentTarget,
        button: PointerButton,
        path_policy: PointerPathPolicy,
    },
    DragPointerTo {
        from: InteractionIntentTarget,
        to: InteractionIntentTarget,
        button: PointerButton,
        path_policy: PointerPathPolicy,
    },
    WheelAtTarget {
        target: InteractionIntentTarget,
        delta_x: f32,
        delta_y: f32,
        delta_mode: WheelDeltaMode,
        path_policy: PointerPathPolicy,
    },
    KeyboardNavigation {
        keys: Vec<KeyCode>,
        activation: Option<KeyCode>,
        modifiers: Modifiers,
        navigation_policy: KeyboardNavigationPolicy,
    },
    TextAtTarget {
        target: InteractionIntentTarget,
        text: String,
        modifiers: Modifiers,
        path_policy: PointerPathPolicy,
    },
}

impl InteractionIntentCommand {
    pub fn move_pointer_to(x: f32, y: f32, path_policy: PointerPathPolicy) -> Self {
        Self::move_pointer_to_target(
            InteractionIntentTarget::synthetic_point(
                "move_target",
                "pointer_target",
                Point::new(x, y),
            ),
            path_policy,
        )
    }

    pub fn move_pointer_to_target(
        target: InteractionIntentTarget,
        path_policy: PointerPathPolicy,
    ) -> Self {
        Self::MovePointerTo {
            target,
            path_policy,
        }
    }

    pub fn drag_pointer_to(
        from_x: f32,
        from_y: f32,
        to_x: f32,
        to_y: f32,
        path_policy: PointerPathPolicy,
    ) -> Self {
        Self::drag_pointer_to_targets(
            InteractionIntentTarget::synthetic_point(
                "drag_from",
                "drag_start_target",
                Point::new(from_x, from_y),
            ),
            InteractionIntentTarget::synthetic_point(
                "drag_to",
                "drag_end_target",
                Point::new(to_x, to_y),
            ),
            path_policy,
        )
    }

    pub fn drag_pointer_to_targets(
        from: InteractionIntentTarget,
        to: InteractionIntentTarget,
        path_policy: PointerPathPolicy,
    ) -> Self {
        Self::DragPointerTo {
            from,
            to,
            button: PointerButton::Primary,
            path_policy,
        }
    }

    pub fn click_target(x: f32, y: f32, path_policy: PointerPathPolicy) -> Self {
        Self::click_interaction_target(
            InteractionIntentTarget::synthetic_point(
                "click_target",
                "click_target",
                Point::new(x, y),
            ),
            path_policy,
        )
    }

    pub fn click_interaction_target(
        target: InteractionIntentTarget,
        path_policy: PointerPathPolicy,
    ) -> Self {
        Self::ClickTarget {
            target,
            button: PointerButton::Primary,
            path_policy,
        }
    }

    pub fn wheel_at_target(
        x: f32,
        y: f32,
        delta_x: f32,
        delta_y: f32,
        delta_mode: WheelDeltaMode,
        path_policy: PointerPathPolicy,
    ) -> Self {
        Self::wheel_at_interaction_target(
            InteractionIntentTarget::synthetic_point(
                "wheel_target",
                "wheel_target",
                Point::new(x, y),
            ),
            delta_x,
            delta_y,
            delta_mode,
            path_policy,
        )
    }

    pub fn wheel_at_interaction_target(
        target: InteractionIntentTarget,
        delta_x: f32,
        delta_y: f32,
        delta_mode: WheelDeltaMode,
        path_policy: PointerPathPolicy,
    ) -> Self {
        Self::WheelAtTarget {
            target,
            delta_x,
            delta_y,
            delta_mode,
            path_policy,
        }
    }

    pub fn keyboard_navigation(
        keys: Vec<KeyCode>,
        activation: Option<KeyCode>,
        modifiers: Modifiers,
        navigation_policy: KeyboardNavigationPolicy,
    ) -> Self {
        Self::KeyboardNavigation {
            keys,
            activation,
            modifiers,
            navigation_policy,
        }
    }

    pub fn text_at_target(
        x: f32,
        y: f32,
        text: impl Into<String>,
        modifiers: Modifiers,
        path_policy: PointerPathPolicy,
    ) -> Self {
        Self::text_at_interaction_target(
            InteractionIntentTarget::synthetic_point(
                "text_target",
                "text_target",
                Point::new(x, y),
            ),
            text,
            modifiers,
            path_policy,
        )
    }

    pub fn text_at_interaction_target(
        target: InteractionIntentTarget,
        text: impl Into<String>,
        modifiers: Modifiers,
        path_policy: PointerPathPolicy,
    ) -> Self {
        Self::TextAtTarget {
            target,
            text: text.into(),
            modifiers,
            path_policy,
        }
    }

    fn pointer_path_policy(&self) -> Option<&PointerPathPolicy> {
        match self {
            Self::MovePointerTo { path_policy, .. }
            | Self::ClickTarget { path_policy, .. }
            | Self::DragPointerTo { path_policy, .. }
            | Self::WheelAtTarget { path_policy, .. }
            | Self::TextAtTarget { path_policy, .. } => Some(path_policy),
            Self::KeyboardNavigation { .. } => None,
        }
    }

    fn command_class(&self) -> IntentCommandClass {
        match self {
            Self::MovePointerTo { .. } => IntentCommandClass::MovePointerTo,
            Self::ClickTarget { .. } => IntentCommandClass::ClickTarget,
            Self::DragPointerTo { .. } => IntentCommandClass::DragPointerTo,
            Self::WheelAtTarget { .. } => IntentCommandClass::WheelAtTarget,
            Self::KeyboardNavigation { .. } => IntentCommandClass::KeyboardNavigation,
            Self::TextAtTarget { .. } => IntentCommandClass::TextAtTarget,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WheelDeltaMode {
    Pixels,
    Lines,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PointerPathPolicy {
    Instant,
    Linear {
        steps: u32,
    },
    Jittered {
        steps: u32,
        amplitude: f32,
        seed: u64,
    },
    EdgeAware {
        approach: PointerApproach,
        steps: u32,
    },
    OvershootCorrect {
        overshoot: f32,
        correction_steps: u32,
    },
    HoverPause {
        steps: u32,
        hover_frames: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyboardNavigationPolicy {
    TabTraversal,
    ReverseTabTraversal,
    ArrowTraversal,
    TraversalBeforeActivation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointerApproach {
    HorizontalFirst,
    VerticalFirst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointerFramePolicy {
    SameFrameCoalesced,
    FrameSeparated,
    ThresholdCrossing,
    HoverSettled { frames: u32 },
    ReconcileSeparated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointerPathSegment {
    Approach,
    Move,
    PreDownHover,
    Press,
    PreThresholdDrag,
    ThresholdCrossing,
    PostThresholdDrag,
    Release,
    Wheel,
    KeyboardTraversal,
    KeyboardActivation,
    TextInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentCommandClass {
    MovePointerTo,
    ClickTarget,
    DragPointerTo,
    WheelAtTarget,
    KeyboardNavigation,
    TextAtTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointerPathPolicyClass {
    Instant,
    Linear,
    Jittered,
    EdgeAware,
    OvershootCorrect,
    HoverPause,
}

impl From<&PointerPathPolicy> for PointerPathPolicyClass {
    fn from(value: &PointerPathPolicy) -> Self {
        match value {
            PointerPathPolicy::Instant => Self::Instant,
            PointerPathPolicy::Linear { .. } => Self::Linear,
            PointerPathPolicy::Jittered { .. } => Self::Jittered,
            PointerPathPolicy::EdgeAware { .. } => Self::EdgeAware,
            PointerPathPolicy::OvershootCorrect { .. } => Self::OvershootCorrect,
            PointerPathPolicy::HoverPause { .. } => Self::HoverPause,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndpointTeleportationMode {
    EndpointTeleportation,
    SampledPath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointerPathSampleCountClass {
    SingleSample,
    MultiSample,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointerMoveExposureClass {
    SameFrameCoalesced,
    FrameSeparated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointerCaptureTransitionClass {
    PointerDown,
    DragThresholdCrossed,
    PointerUp,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum CoverageKey {
    IntentCommandClass(IntentCommandClass),
    PointerPathPolicyClass(PointerPathPolicyClass),
    KeyboardNavigationPolicy(KeyboardNavigationPolicy),
    PointerPathSampleCountClass(PointerPathSampleCountClass),
    PointerFramePolicy(PointerFramePolicy),
    PointerPathSegment(PointerPathSegment),
    CoalescedMoveClass(PointerMoveExposureClass),
    ThresholdCrossing,
    CrossedHoverTargetClass(String),
    CrossedSeparatorClass(String),
    CaptureTransitionClass(PointerCaptureTransitionClass),
    IncidentalRouteTargetClass(String),
    EndpointTeleportationMode(EndpointTeleportationMode),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrimitiveCommandClass {
    PointerMove,
    PointerDown,
    PointerUp,
    WheelPixels,
    WheelLines,
    KeyDown,
    KeyUp,
    TextInput,
    Frame,
    Reconcile,
    SetWindowSize,
    SetScaleFactor,
}

impl From<&InteractionCommand> for PrimitiveCommandClass {
    fn from(value: &InteractionCommand) -> Self {
        match value {
            InteractionCommand::PointerMove { .. } => Self::PointerMove,
            InteractionCommand::PointerDown { .. } => Self::PointerDown,
            InteractionCommand::PointerUp { .. } => Self::PointerUp,
            InteractionCommand::WheelPixels { .. } => Self::WheelPixels,
            InteractionCommand::WheelLines { .. } => Self::WheelLines,
            InteractionCommand::KeyDown { .. } => Self::KeyDown,
            InteractionCommand::KeyUp { .. } => Self::KeyUp,
            InteractionCommand::TextInput { .. } => Self::TextInput,
            InteractionCommand::Frame { .. } => Self::Frame,
            InteractionCommand::Reconcile => Self::Reconcile,
            InteractionCommand::SetWindowSize { .. } => Self::SetWindowSize,
            InteractionCommand::SetScaleFactor { .. } => Self::SetScaleFactor,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InteractionCoverage {
    keys: BTreeSet<CoverageKey>,
}

impl InteractionCoverage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_intent_trace(&mut self, trace: &IntentTraceStep) {
        self.keys.insert(CoverageKey::IntentCommandClass(
            trace.intent_command.command_class(),
        ));

        if let InteractionIntentCommand::KeyboardNavigation {
            navigation_policy, ..
        } = &trace.intent_command
        {
            self.keys
                .insert(CoverageKey::KeyboardNavigationPolicy(*navigation_policy));
            return;
        }

        let Some(path_policy) = trace.path_policy.as_ref() else {
            return;
        };
        self.keys.insert(CoverageKey::PointerPathPolicyClass(
            PointerPathPolicyClass::from(path_policy),
        ));

        let movement_samples = trace
            .expansion_steps
            .iter()
            .filter(|step| matches!(step.command, InteractionCommand::PointerMove { .. }))
            .count();
        self.keys.insert(CoverageKey::PointerPathSampleCountClass(
            if movement_samples <= 1 {
                PointerPathSampleCountClass::SingleSample
            } else {
                PointerPathSampleCountClass::MultiSample
            },
        ));
        let teleportation_mode =
            if movement_samples <= 1 && matches!(path_policy, PointerPathPolicy::Instant) {
                EndpointTeleportationMode::EndpointTeleportation
            } else {
                EndpointTeleportationMode::SampledPath
            };
        self.keys
            .insert(CoverageKey::EndpointTeleportationMode(teleportation_mode));

        for step in trace.expansion_steps.iter() {
            self.keys
                .insert(CoverageKey::PointerFramePolicy(step.frame_policy));
            match step.frame_policy {
                PointerFramePolicy::SameFrameCoalesced => {
                    self.keys.insert(CoverageKey::CoalescedMoveClass(
                        PointerMoveExposureClass::SameFrameCoalesced,
                    ));
                }
                PointerFramePolicy::FrameSeparated
                | PointerFramePolicy::ThresholdCrossing
                | PointerFramePolicy::HoverSettled { .. }
                | PointerFramePolicy::ReconcileSeparated => {
                    self.keys.insert(CoverageKey::CoalescedMoveClass(
                        PointerMoveExposureClass::FrameSeparated,
                    ));
                }
            }
            self.keys
                .insert(CoverageKey::PointerPathSegment(step.segment));
            if step.segment == PointerPathSegment::ThresholdCrossing {
                self.keys.insert(CoverageKey::ThresholdCrossing);
            }
            match step.segment {
                PointerPathSegment::Press => {
                    self.keys.insert(CoverageKey::CaptureTransitionClass(
                        PointerCaptureTransitionClass::PointerDown,
                    ));
                }
                PointerPathSegment::ThresholdCrossing => {
                    self.keys.insert(CoverageKey::CaptureTransitionClass(
                        PointerCaptureTransitionClass::DragThresholdCrossed,
                    ));
                }
                PointerPathSegment::Release => {
                    self.keys.insert(CoverageKey::CaptureTransitionClass(
                        PointerCaptureTransitionClass::PointerUp,
                    ));
                }
                PointerPathSegment::Approach
                | PointerPathSegment::Move
                | PointerPathSegment::PreDownHover
                | PointerPathSegment::PreThresholdDrag
                | PointerPathSegment::PostThresholdDrag
                | PointerPathSegment::Wheel
                | PointerPathSegment::KeyboardTraversal
                | PointerPathSegment::KeyboardActivation
                | PointerPathSegment::TextInput => {}
            }
        }
    }

    pub fn record_key(&mut self, key: CoverageKey) {
        self.keys.insert(key);
    }

    pub fn contains(&self, key: &CoverageKey) -> bool {
        self.keys.contains(key)
    }

    pub fn total_keys(&self) -> usize {
        self.keys.len()
    }

    pub fn keys(&self) -> impl Iterator<Item = &CoverageKey> {
        self.keys.iter()
    }
}

pub const INTERACTION_FUZZ_SCHEMA: &str = "halcyon.interaction_fuzz.v1";
pub const INTERACTION_REPLAY_SCHEMA_VERSION: u32 = 1;
pub const INTERACTION_CONFIDENCE_POSITIVE_CLAIM: &str = "No invariant violations found across the declared fixture/generator/oracle frontier for this run.";
pub const ORACLE_346_U1_ID: &str = "ORACLE-346-U1";
pub const ORACLE_346_U1_NAME: &str = "Finite Sink Geometry";
pub const ORACLE_346_U3_ID: &str = "ORACLE-346-U3";
pub const ORACLE_346_U3_NAME: &str = "Capture Owner Validity";
pub const ORACLE_346_U7_ID: &str = "ORACLE-346-U7";
pub const ORACLE_346_U7_NAME: &str = "Layout Settled";
pub const ORACLE_346_U11_ID: &str = "ORACLE-346-U11";
pub const ORACLE_346_U11_NAME: &str = "Incidental Navigation Integrity";

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ReplayPoint {
    pub x: f32,
    pub y: f32,
}

impl From<Point> for ReplayPoint {
    fn from(value: Point) -> Self {
        Self {
            x: value.x,
            y: value.y,
        }
    }
}

impl From<ReplayPoint> for Point {
    fn from(value: ReplayPoint) -> Self {
        Point::new(value.x, value.y)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionIntentTargetTrace {
    pub target_id: String,
    pub target_kind: String,
    pub target_class: Option<String>,
    pub coordinate_space: String,
    pub resolved_point: ReplayPoint,
    pub source_probe_id: String,
    pub probe_epoch: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayPointerButton {
    Primary,
    Secondary,
    Auxiliary,
}

impl From<PointerButton> for ReplayPointerButton {
    fn from(value: PointerButton) -> Self {
        match value {
            PointerButton::Primary => Self::Primary,
            PointerButton::Secondary => Self::Secondary,
            PointerButton::Auxiliary => Self::Auxiliary,
        }
    }
}

impl From<ReplayPointerButton> for PointerButton {
    fn from(value: ReplayPointerButton) -> Self {
        match value {
            ReplayPointerButton::Primary => Self::Primary,
            ReplayPointerButton::Secondary => Self::Secondary,
            ReplayPointerButton::Auxiliary => Self::Auxiliary,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayModifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

impl From<Modifiers> for ReplayModifiers {
    fn from(value: Modifiers) -> Self {
        Self {
            ctrl: value.ctrl,
            alt: value.alt,
            shift: value.shift,
            meta: value.meta,
        }
    }
}

impl From<ReplayModifiers> for Modifiers {
    fn from(value: ReplayModifiers) -> Self {
        Self {
            ctrl: value.ctrl,
            alt: value.alt,
            shift: value.shift,
            meta: value.meta,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayKeyCode {
    Unknown,
    Character(char),
    Enter,
    Tab,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Escape,
    Space,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    Shift,
    Control,
    Alt,
    Meta,
}

impl From<KeyCode> for ReplayKeyCode {
    fn from(value: KeyCode) -> Self {
        match value {
            KeyCode::Unknown => Self::Unknown,
            KeyCode::Character(value) => Self::Character(value),
            KeyCode::Enter => Self::Enter,
            KeyCode::Tab => Self::Tab,
            KeyCode::Backspace => Self::Backspace,
            KeyCode::Delete => Self::Delete,
            KeyCode::Left => Self::Left,
            KeyCode::Right => Self::Right,
            KeyCode::Up => Self::Up,
            KeyCode::Down => Self::Down,
            KeyCode::Home => Self::Home,
            KeyCode::End => Self::End,
            KeyCode::PageUp => Self::PageUp,
            KeyCode::PageDown => Self::PageDown,
            KeyCode::Escape => Self::Escape,
            KeyCode::Space => Self::Space,
            KeyCode::F1 => Self::F1,
            KeyCode::F2 => Self::F2,
            KeyCode::F3 => Self::F3,
            KeyCode::F4 => Self::F4,
            KeyCode::F5 => Self::F5,
            KeyCode::F6 => Self::F6,
            KeyCode::F7 => Self::F7,
            KeyCode::F8 => Self::F8,
            KeyCode::F9 => Self::F9,
            KeyCode::F10 => Self::F10,
            KeyCode::F11 => Self::F11,
            KeyCode::F12 => Self::F12,
            KeyCode::Shift => Self::Shift,
            KeyCode::Control => Self::Control,
            KeyCode::Alt => Self::Alt,
            KeyCode::Meta => Self::Meta,
        }
    }
}

impl From<ReplayKeyCode> for KeyCode {
    fn from(value: ReplayKeyCode) -> Self {
        match value {
            ReplayKeyCode::Unknown => Self::Unknown,
            ReplayKeyCode::Character(value) => Self::Character(value),
            ReplayKeyCode::Enter => Self::Enter,
            ReplayKeyCode::Tab => Self::Tab,
            ReplayKeyCode::Backspace => Self::Backspace,
            ReplayKeyCode::Delete => Self::Delete,
            ReplayKeyCode::Left => Self::Left,
            ReplayKeyCode::Right => Self::Right,
            ReplayKeyCode::Up => Self::Up,
            ReplayKeyCode::Down => Self::Down,
            ReplayKeyCode::Home => Self::Home,
            ReplayKeyCode::End => Self::End,
            ReplayKeyCode::PageUp => Self::PageUp,
            ReplayKeyCode::PageDown => Self::PageDown,
            ReplayKeyCode::Escape => Self::Escape,
            ReplayKeyCode::Space => Self::Space,
            ReplayKeyCode::F1 => Self::F1,
            ReplayKeyCode::F2 => Self::F2,
            ReplayKeyCode::F3 => Self::F3,
            ReplayKeyCode::F4 => Self::F4,
            ReplayKeyCode::F5 => Self::F5,
            ReplayKeyCode::F6 => Self::F6,
            ReplayKeyCode::F7 => Self::F7,
            ReplayKeyCode::F8 => Self::F8,
            ReplayKeyCode::F9 => Self::F9,
            ReplayKeyCode::F10 => Self::F10,
            ReplayKeyCode::F11 => Self::F11,
            ReplayKeyCode::F12 => Self::F12,
            ReplayKeyCode::Shift => Self::Shift,
            ReplayKeyCode::Control => Self::Control,
            ReplayKeyCode::Alt => Self::Alt,
            ReplayKeyCode::Meta => Self::Meta,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReplayInteractionCommand {
    PointerMove {
        #[serde(with = "replay_f32")]
        x: f32,
        #[serde(with = "replay_f32")]
        y: f32,
    },
    PointerDown {
        #[serde(with = "replay_f32")]
        x: f32,
        #[serde(with = "replay_f32")]
        y: f32,
        button: ReplayPointerButton,
    },
    PointerUp {
        #[serde(with = "replay_f32")]
        x: f32,
        #[serde(with = "replay_f32")]
        y: f32,
        button: ReplayPointerButton,
    },
    WheelPixels {
        #[serde(with = "replay_f32")]
        x: f32,
        #[serde(with = "replay_f32")]
        y: f32,
        #[serde(with = "replay_f32")]
        delta_x: f32,
        #[serde(with = "replay_f32")]
        delta_y: f32,
    },
    WheelLines {
        #[serde(with = "replay_f32")]
        x: f32,
        #[serde(with = "replay_f32")]
        y: f32,
        #[serde(with = "replay_f32")]
        delta_x: f32,
        #[serde(with = "replay_f32")]
        delta_y: f32,
    },
    KeyDown {
        key_code: ReplayKeyCode,
        modifiers: ReplayModifiers,
        repeat: bool,
    },
    KeyUp {
        key_code: ReplayKeyCode,
        modifiers: ReplayModifiers,
    },
    TextInput {
        text: String,
        modifiers: ReplayModifiers,
    },
    Frame {
        count: u32,
    },
    Reconcile,
    SetWindowSize {
        #[serde(with = "replay_f32")]
        width: f32,
        #[serde(with = "replay_f32")]
        height: f32,
    },
    SetScaleFactor {
        #[serde(with = "replay_f32")]
        scale_factor: f32,
    },
}

impl From<&InteractionCommand> for ReplayInteractionCommand {
    fn from(value: &InteractionCommand) -> Self {
        match value {
            InteractionCommand::PointerMove { x, y } => Self::PointerMove { x: *x, y: *y },
            InteractionCommand::PointerDown { x, y, button } => Self::PointerDown {
                x: *x,
                y: *y,
                button: (*button).into(),
            },
            InteractionCommand::PointerUp { x, y, button } => Self::PointerUp {
                x: *x,
                y: *y,
                button: (*button).into(),
            },
            InteractionCommand::WheelPixels {
                x,
                y,
                delta_x,
                delta_y,
            } => Self::WheelPixels {
                x: *x,
                y: *y,
                delta_x: *delta_x,
                delta_y: *delta_y,
            },
            InteractionCommand::WheelLines {
                x,
                y,
                delta_x,
                delta_y,
            } => Self::WheelLines {
                x: *x,
                y: *y,
                delta_x: *delta_x,
                delta_y: *delta_y,
            },
            InteractionCommand::KeyDown {
                key_code,
                modifiers,
                repeat,
            } => Self::KeyDown {
                key_code: (*key_code).into(),
                modifiers: (*modifiers).into(),
                repeat: *repeat,
            },
            InteractionCommand::KeyUp {
                key_code,
                modifiers,
            } => Self::KeyUp {
                key_code: (*key_code).into(),
                modifiers: (*modifiers).into(),
            },
            InteractionCommand::TextInput { text, modifiers } => Self::TextInput {
                text: text.clone(),
                modifiers: (*modifiers).into(),
            },
            InteractionCommand::Frame { count } => Self::Frame { count: *count },
            InteractionCommand::Reconcile => Self::Reconcile,
            InteractionCommand::SetWindowSize { width, height } => Self::SetWindowSize {
                width: *width,
                height: *height,
            },
            InteractionCommand::SetScaleFactor { scale_factor } => Self::SetScaleFactor {
                scale_factor: *scale_factor,
            },
        }
    }
}

impl From<&ReplayInteractionCommand> for InteractionCommand {
    fn from(value: &ReplayInteractionCommand) -> Self {
        match value {
            ReplayInteractionCommand::PointerMove { x, y } => Self::PointerMove { x: *x, y: *y },
            ReplayInteractionCommand::PointerDown { x, y, button } => Self::PointerDown {
                x: *x,
                y: *y,
                button: (*button).into(),
            },
            ReplayInteractionCommand::PointerUp { x, y, button } => Self::PointerUp {
                x: *x,
                y: *y,
                button: (*button).into(),
            },
            ReplayInteractionCommand::WheelPixels {
                x,
                y,
                delta_x,
                delta_y,
            } => Self::WheelPixels {
                x: *x,
                y: *y,
                delta_x: *delta_x,
                delta_y: *delta_y,
            },
            ReplayInteractionCommand::WheelLines {
                x,
                y,
                delta_x,
                delta_y,
            } => Self::WheelLines {
                x: *x,
                y: *y,
                delta_x: *delta_x,
                delta_y: *delta_y,
            },
            ReplayInteractionCommand::KeyDown {
                key_code,
                modifiers,
                repeat,
            } => Self::KeyDown {
                key_code: (*key_code).into(),
                modifiers: (*modifiers).into(),
                repeat: *repeat,
            },
            ReplayInteractionCommand::KeyUp {
                key_code,
                modifiers,
            } => Self::KeyUp {
                key_code: (*key_code).into(),
                modifiers: (*modifiers).into(),
            },
            ReplayInteractionCommand::TextInput { text, modifiers } => Self::TextInput {
                text: text.clone(),
                modifiers: (*modifiers).into(),
            },
            ReplayInteractionCommand::Frame { count } => Self::Frame { count: *count },
            ReplayInteractionCommand::Reconcile => Self::Reconcile,
            ReplayInteractionCommand::SetWindowSize { width, height } => Self::SetWindowSize {
                width: *width,
                height: *height,
            },
            ReplayInteractionCommand::SetScaleFactor { scale_factor } => Self::SetScaleFactor {
                scale_factor: *scale_factor,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReplayInteractionIntentCommand {
    MovePointerTo {
        target: InteractionIntentTargetTrace,
        path_policy: PointerPathPolicy,
    },
    ClickTarget {
        target: InteractionIntentTargetTrace,
        button: ReplayPointerButton,
        path_policy: PointerPathPolicy,
    },
    DragPointerTo {
        from: InteractionIntentTargetTrace,
        to: InteractionIntentTargetTrace,
        button: ReplayPointerButton,
        path_policy: PointerPathPolicy,
    },
    WheelAtTarget {
        target: InteractionIntentTargetTrace,
        #[serde(with = "replay_f32")]
        delta_x: f32,
        #[serde(with = "replay_f32")]
        delta_y: f32,
        delta_mode: WheelDeltaMode,
        path_policy: PointerPathPolicy,
    },
    KeyboardNavigation {
        keys: Vec<ReplayKeyCode>,
        activation: Option<ReplayKeyCode>,
        modifiers: ReplayModifiers,
        navigation_policy: KeyboardNavigationPolicy,
    },
    TextAtTarget {
        target: InteractionIntentTargetTrace,
        text: String,
        modifiers: ReplayModifiers,
        path_policy: PointerPathPolicy,
    },
}

impl From<&InteractionIntentCommand> for ReplayInteractionIntentCommand {
    fn from(value: &InteractionIntentCommand) -> Self {
        match value {
            InteractionIntentCommand::MovePointerTo {
                target,
                path_policy,
            } => Self::MovePointerTo {
                target: target.trace(),
                path_policy: path_policy.clone(),
            },
            InteractionIntentCommand::ClickTarget {
                target,
                button,
                path_policy,
            } => Self::ClickTarget {
                target: target.trace(),
                button: (*button).into(),
                path_policy: path_policy.clone(),
            },
            InteractionIntentCommand::DragPointerTo {
                from,
                to,
                button,
                path_policy,
            } => Self::DragPointerTo {
                from: from.trace(),
                to: to.trace(),
                button: (*button).into(),
                path_policy: path_policy.clone(),
            },
            InteractionIntentCommand::WheelAtTarget {
                target,
                delta_x,
                delta_y,
                delta_mode,
                path_policy,
            } => Self::WheelAtTarget {
                target: target.trace(),
                delta_x: *delta_x,
                delta_y: *delta_y,
                delta_mode: *delta_mode,
                path_policy: path_policy.clone(),
            },
            InteractionIntentCommand::KeyboardNavigation {
                keys,
                activation,
                modifiers,
                navigation_policy,
            } => Self::KeyboardNavigation {
                keys: keys.iter().copied().map(ReplayKeyCode::from).collect(),
                activation: activation.map(ReplayKeyCode::from),
                modifiers: (*modifiers).into(),
                navigation_policy: *navigation_policy,
            },
            InteractionIntentCommand::TextAtTarget {
                target,
                text,
                modifiers,
                path_policy,
            } => Self::TextAtTarget {
                target: target.trace(),
                text: text.clone(),
                modifiers: (*modifiers).into(),
                path_policy: path_policy.clone(),
            },
        }
    }
}

impl From<&ReplayInteractionIntentCommand> for InteractionIntentCommand {
    fn from(value: &ReplayInteractionIntentCommand) -> Self {
        match value {
            ReplayInteractionIntentCommand::MovePointerTo {
                target,
                path_policy,
            } => Self::MovePointerTo {
                target: InteractionIntentTarget::from_trace(target),
                path_policy: path_policy.clone(),
            },
            ReplayInteractionIntentCommand::ClickTarget {
                target,
                button,
                path_policy,
            } => Self::ClickTarget {
                target: InteractionIntentTarget::from_trace(target),
                button: (*button).into(),
                path_policy: path_policy.clone(),
            },
            ReplayInteractionIntentCommand::DragPointerTo {
                from,
                to,
                button,
                path_policy,
            } => Self::DragPointerTo {
                from: InteractionIntentTarget::from_trace(from),
                to: InteractionIntentTarget::from_trace(to),
                button: (*button).into(),
                path_policy: path_policy.clone(),
            },
            ReplayInteractionIntentCommand::WheelAtTarget {
                target,
                delta_x,
                delta_y,
                delta_mode,
                path_policy,
            } => Self::WheelAtTarget {
                target: InteractionIntentTarget::from_trace(target),
                delta_x: *delta_x,
                delta_y: *delta_y,
                delta_mode: *delta_mode,
                path_policy: path_policy.clone(),
            },
            ReplayInteractionIntentCommand::KeyboardNavigation {
                keys,
                activation,
                modifiers,
                navigation_policy,
            } => Self::KeyboardNavigation {
                keys: keys.iter().copied().map(KeyCode::from).collect(),
                activation: activation.map(KeyCode::from),
                modifiers: (*modifiers).into(),
                navigation_policy: *navigation_policy,
            },
            ReplayInteractionIntentCommand::TextAtTarget {
                target,
                text,
                modifiers,
                path_policy,
            } => Self::TextAtTarget {
                target: InteractionIntentTarget::from_trace(target),
                text: text.clone(),
                modifiers: (*modifiers).into(),
                path_policy: path_policy.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayPrimitiveTraceStep {
    pub primitive_step_index: usize,
    pub command: ReplayInteractionCommand,
    pub emitted_event_count: usize,
}

impl From<&PrimitiveTraceStep> for ReplayPrimitiveTraceStep {
    fn from(value: &PrimitiveTraceStep) -> Self {
        Self {
            primitive_step_index: value.primitive_step_index,
            command: ReplayInteractionCommand::from(&value.command),
            emitted_event_count: value.fused_events.len(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayIntentExpansionStep {
    pub primitive_offset: usize,
    pub command: ReplayInteractionCommand,
    pub segment: PointerPathSegment,
    pub frame_policy: PointerFramePolicy,
}

impl From<&IntentExpansionStep> for ReplayIntentExpansionStep {
    fn from(value: &IntentExpansionStep) -> Self {
        Self {
            primitive_offset: value.primitive_offset,
            command: ReplayInteractionCommand::from(&value.command),
            segment: value.segment,
            frame_policy: value.frame_policy,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayIntentTraceStep {
    pub intent_index: usize,
    pub intent_command: ReplayInteractionIntentCommand,
    pub target_trace: Vec<InteractionIntentTargetTrace>,
    pub path_policy: Option<PointerPathPolicy>,
    pub primitive_command_range_start: usize,
    pub primitive_command_range_end: usize,
    pub expansion_steps: Vec<ReplayIntentExpansionStep>,
}

impl From<&IntentTraceStep> for ReplayIntentTraceStep {
    fn from(value: &IntentTraceStep) -> Self {
        Self {
            intent_index: value.intent_index,
            intent_command: ReplayInteractionIntentCommand::from(&value.intent_command),
            target_trace: value.target_trace.clone(),
            path_policy: value.path_policy.clone(),
            primitive_command_range_start: value.primitive_command_range.start,
            primitive_command_range_end: value.primitive_command_range.end,
            expansion_steps: value
                .expansion_steps
                .iter()
                .map(ReplayIntentExpansionStep::from)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayMode {
    ExactIntent,
    ExactPrimitive,
    ExactTrace,
    ShrunkTrace,
    PrimitiveOnlyExpandedIntent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionReplayArtifact {
    pub schema_version: u32,
    pub schema: String,
    pub replay_mode: ReplayMode,
    pub fixture_id: String,
    pub root_seed: u64,
    pub shrink_seed: Option<u64>,
    pub intent_trace: Vec<ReplayIntentTraceStep>,
    pub primitive_trace: Vec<ReplayPrimitiveTraceStep>,
    pub coverage_keys: Vec<CoverageKey>,
}

impl InteractionReplayArtifact {
    pub fn from_driver(
        replay_mode: ReplayMode,
        fixture_id: impl Into<String>,
        root_seed: u64,
        shrink_seed: Option<u64>,
        driver: &InteractionDriver,
    ) -> Self {
        Self {
            schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
            replay_mode,
            fixture_id: fixture_id.into(),
            root_seed,
            shrink_seed,
            intent_trace: driver
                .intent_trace()
                .iter()
                .map(ReplayIntentTraceStep::from)
                .collect(),
            primitive_trace: driver
                .primitive_trace()
                .iter()
                .map(ReplayPrimitiveTraceStep::from)
                .collect(),
            coverage_keys: driver.coverage().keys().cloned().collect(),
            schema: INTERACTION_FUZZ_SCHEMA.to_string(),
        }
    }

    pub fn from_shrunk_driver(
        fixture_id: impl Into<String>,
        root_seed: u64,
        shrink_seed: u64,
        driver: &InteractionDriver,
    ) -> Self {
        Self::from_driver(
            ReplayMode::ShrunkTrace,
            fixture_id,
            root_seed,
            Some(shrink_seed),
            driver,
        )
    }

    pub fn primitive_commands(&self) -> Vec<InteractionCommand> {
        self.primitive_trace
            .iter()
            .map(|step| InteractionCommand::from(&step.command))
            .collect()
    }

    pub fn replay_into(
        &self,
        driver: &mut InteractionDriver,
    ) -> InteractionDriverResult<Vec<FusedInputEvent>> {
        driver.try_execute_all(self.primitive_commands())
    }

    pub fn replay_shrunk_trace_into(
        &self,
        driver: &mut InteractionDriver,
    ) -> InteractionDriverResult<Vec<FusedInputEvent>> {
        debug_assert_eq!(self.replay_mode, ReplayMode::ShrunkTrace);
        self.replay_into(driver)
    }

    pub fn is_supported_schema(&self) -> bool {
        self.schema == INTERACTION_FUZZ_SCHEMA
            && self.schema_version == INTERACTION_REPLAY_SCHEMA_VERSION
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionInjectionPath {
    RawInputFusion,
    IntentExpansion,
    PrimitiveReplay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionSettleBoundary {
    Frame,
    Reconcile,
    Explicit,
}

pub type InteractionFuzzProfileHash = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionFuzzMode {
    ContractSmoke,
    ContractDeep,
    Continuous,
    ChaosSmoke,
    ChaosDeep,
    Replay,
    Shrink,
    NegativeValidation,
    ProfileDiff,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionGeneratorVersionRecord {
    pub generator_id: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionFuzzProfile {
    pub schema_version: u32,
    pub schema: String,
    pub profile_id: String,
    pub mode: InteractionFuzzMode,
    pub artifact_dir: String,
    pub injection_path: InteractionInjectionPath,
    pub settle_boundary: InteractionSettleBoundary,
    pub fixture_ids: Vec<String>,
    pub chaos_surface_ids: Vec<String>,
    pub generator_ids: Vec<String>,
    pub chaos_stream_generator_ids: Vec<String>,
    pub generator_versions: Vec<InteractionGeneratorVersionRecord>,
    pub chaos_stream_generator_versions: Vec<InteractionGeneratorVersionRecord>,
    pub oracle_ids: Vec<String>,
    pub hard_failure_oracle_ids: Vec<String>,
    pub root_seed: u64,
    pub seed_set: Vec<u64>,
    pub rng_algorithm: String,
    pub rng_version: String,
    pub command_budget: usize,
    pub transaction_budget: usize,
    pub coverage_window: usize,
    pub timeout_policy: InteractionTimeoutPolicy,
    pub visual_diagnostics_policy: InteractionVisualDiagnosticsPolicy,
    pub constitution_boundary_profile: InteractionConstitutionBoundaryProfile,
    pub disabled_domains: Vec<String>,
    pub expected_unsupported_domains: Vec<String>,
}

impl InteractionFuzzProfile {
    pub fn profile_hash(&self) -> InteractionFuzzProfileHash {
        let encoded = serde_json::to_vec(self).expect("interaction fuzz profile must serialize");
        let digest = Sha256::digest(encoded);
        let mut out = String::with_capacity("sha256:".len() + 64);
        out.push_str("sha256:");
        for byte in digest {
            write!(&mut out, "{byte:02x}").expect("writing hash to string must succeed");
        }
        out
    }

    pub fn is_complete(&self) -> bool {
        self.schema == INTERACTION_FUZZ_SCHEMA
            && self.schema_version == INTERACTION_REPLAY_SCHEMA_VERSION
            && !self.profile_id.trim().is_empty()
            && !self.artifact_dir.trim().is_empty()
            && !self.rng_algorithm.trim().is_empty()
            && !self.rng_version.trim().is_empty()
            && !self.seed_set.is_empty()
            && self.command_budget > 0
            && self.transaction_budget > 0
            && self.coverage_window > 0
            && self.timeout_policy.is_complete()
            && self.visual_diagnostics_policy.is_complete()
            && self.constitution_boundary_profile.is_complete()
            && match self.mode {
                InteractionFuzzMode::ContractSmoke
                | InteractionFuzzMode::ContractDeep
                | InteractionFuzzMode::Continuous => {
                    !self.fixture_ids.is_empty()
                        && !self.generator_ids.is_empty()
                        && generator_version_records_complete(
                            &self.generator_ids,
                            &self.generator_versions,
                        )
                        && !self.oracle_ids.is_empty()
                }
                InteractionFuzzMode::ChaosSmoke | InteractionFuzzMode::ChaosDeep => {
                    !self.chaos_surface_ids.is_empty()
                        && !self.chaos_stream_generator_ids.is_empty()
                        && generator_version_records_complete(
                            &self.chaos_stream_generator_ids,
                            &self.chaos_stream_generator_versions,
                        )
                        && !self.hard_failure_oracle_ids.is_empty()
                }
                InteractionFuzzMode::Replay
                | InteractionFuzzMode::Shrink
                | InteractionFuzzMode::NegativeValidation
                | InteractionFuzzMode::ProfileDiff => true,
            }
    }
}

fn generator_version_records_complete(
    generator_ids: &[String],
    version_records: &[InteractionGeneratorVersionRecord],
) -> bool {
    if generator_ids.len() != version_records.len() {
        return false;
    }

    let expected_ids = generator_ids.iter().collect::<BTreeSet<_>>();
    let actual_ids = version_records
        .iter()
        .filter(|record| {
            !record.generator_id.trim().is_empty() && !record.version.trim().is_empty()
        })
        .map(|record| &record.generator_id)
        .collect::<BTreeSet<_>>();

    expected_ids == actual_ids
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionTimeoutPolicy {
    pub per_transaction_timeout_ms: u64,
    pub run_timeout_ms: u64,
    pub settle_iteration_budget: u32,
    pub frame_progress_budget: u32,
    pub no_progress_frame_budget: u32,
    pub no_progress_settle_budget: u32,
    pub monotonic_clock_seed: u64,
}

impl InteractionTimeoutPolicy {
    pub fn is_complete(&self) -> bool {
        self.per_transaction_timeout_ms > 0
            && self.run_timeout_ms > 0
            && self.settle_iteration_budget > 0
            && self.frame_progress_budget > 0
            && self.no_progress_frame_budget > 0
            && self.no_progress_settle_budget > 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionProgressMarker {
    pub command_index: usize,
    pub frame_index: u64,
    pub layout_epoch: u64,
    pub scene_epoch: u64,
    pub dirty_state_hash: String,
    pub deferred_mutation_count: usize,
    pub capture_owner_class: String,
    pub focus_owner_class: String,
    pub routed_target_class: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionVisualDiagnosticsPolicy {
    pub capture_enabled: bool,
    pub headless: bool,
    pub rendering_available: bool,
}

impl InteractionVisualDiagnosticsPolicy {
    pub fn is_complete(&self) -> bool {
        (self.capture_enabled && !self.headless && self.rendering_available)
            || self.headless
            || !self.rendering_available
    }

    pub fn status(&self) -> InteractionVisualDiagnosticsStatus {
        if self.capture_enabled && !self.headless && self.rendering_available {
            InteractionVisualDiagnosticsStatus {
                unavailable: false,
                reason: None,
                profile_policy: "capture_enabled".to_string(),
            }
        } else if self.headless {
            InteractionVisualDiagnosticsStatus {
                unavailable: true,
                reason: Some("headless".to_string()),
                profile_policy: "headless".to_string(),
            }
        } else if !self.rendering_available {
            InteractionVisualDiagnosticsStatus {
                unavailable: true,
                reason: Some("rendering_unavailable".to_string()),
                profile_policy: "rendering_unavailable".to_string(),
            }
        } else {
            InteractionVisualDiagnosticsStatus {
                unavailable: true,
                reason: Some("visual_capture_disabled_without_declared_exception".to_string()),
                profile_policy: "invalid_capture_disabled".to_string(),
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionVisualDiagnosticsStatus {
    pub unavailable: bool,
    pub reason: Option<String>,
    pub profile_policy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionVisualTraceArtifact {
    pub artifact_kind: String,
    pub path: String,
    pub detector_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionConstitutionBoundaryProfile {
    pub input_fusion_owner: String,
    pub event_routing_owner: String,
    pub capture_owner: String,
    pub focus_owner: String,
    pub layout_owner: String,
    pub scene_build_owner: String,
    pub rendering_owner: String,
    pub scroll_owner: String,
    pub identity_owner: String,
    pub probe_owner: String,
    pub oracle_owner: String,
    pub shrinking_owner: String,
    pub replay_owner: String,
    pub reporting_owner: String,
    pub visual_diagnostics_owner: String,
}

impl InteractionConstitutionBoundaryProfile {
    pub fn is_complete(&self) -> bool {
        !self.input_fusion_owner.trim().is_empty()
            && !self.event_routing_owner.trim().is_empty()
            && !self.capture_owner.trim().is_empty()
            && !self.focus_owner.trim().is_empty()
            && !self.layout_owner.trim().is_empty()
            && !self.scene_build_owner.trim().is_empty()
            && !self.rendering_owner.trim().is_empty()
            && !self.scroll_owner.trim().is_empty()
            && !self.identity_owner.trim().is_empty()
            && !self.probe_owner.trim().is_empty()
            && !self.oracle_owner.trim().is_empty()
            && !self.shrinking_owner.trim().is_empty()
            && !self.replay_owner.trim().is_empty()
            && !self.reporting_owner.trim().is_empty()
            && !self.visual_diagnostics_owner.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionConstitutionBoundaryCheckStatus {
    pub passed: bool,
    pub failed_checks: Vec<String>,
}

impl InteractionConstitutionBoundaryCheckStatus {
    pub fn from_oracle_evaluations<'a, I>(evaluations: I) -> Self
    where
        I: IntoIterator<Item = &'a InteractionOracleEvaluation>,
    {
        let failed_checks: Vec<String> = evaluations
            .into_iter()
            .filter(|evaluation| !evaluation.passed())
            .map(|evaluation| evaluation.oracle_id.clone())
            .collect();

        Self {
            passed: failed_checks.is_empty(),
            failed_checks,
        }
    }
}

pub const INTERACTION_CHAOS_NO_FAILURE_CLAIM: &str =
    "No hard failures found across the declared chaos budget for this run.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionChaosPrimitiveFamily {
    PointerMove,
    PointerButton,
    WheelPixels,
    WheelLines,
    Keyboard,
    TextInput,
    Frame,
    Reconcile,
    WindowSize,
    ScaleFactor,
}

impl InteractionChaosPrimitiveFamily {
    pub fn from_command(command: &InteractionCommand) -> Self {
        match command {
            InteractionCommand::PointerMove { .. } => Self::PointerMove,
            InteractionCommand::PointerDown { .. } | InteractionCommand::PointerUp { .. } => {
                Self::PointerButton
            }
            InteractionCommand::WheelPixels { .. } => Self::WheelPixels,
            InteractionCommand::WheelLines { .. } => Self::WheelLines,
            InteractionCommand::KeyDown { .. } | InteractionCommand::KeyUp { .. } => Self::Keyboard,
            InteractionCommand::TextInput { .. } => Self::TextInput,
            InteractionCommand::Frame { .. } => Self::Frame,
            InteractionCommand::Reconcile => Self::Reconcile,
            InteractionCommand::SetWindowSize { .. } => Self::WindowSize,
            InteractionCommand::SetScaleFactor { .. } => Self::ScaleFactor,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionChaosPointerPressurePattern {
    RandomWalk,
    VelocityBurst,
    AccelerationBurst,
    Jitter,
    EdgeCornerBias,
    DragThresholdCrossing,
    LongDrag,
    HoverPause,
    PressDragRelease,
    WheelBurstUnderPointer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionChaosPointerMotionPolicy {
    RandomWalkContinuous,
    VelocityBurstContinuous,
    AccelerationBurstContinuous,
    JitterContinuous,
    EdgeCornerSweep,
    ThresholdCrossing,
    LongDragContinuous,
    HoverPauseBoundary,
    PressDragReleasePath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionChaosHaltReason {
    HardFailure,
    CommandBudget,
    TransactionBudget,
    NoNewCoverageWindow,
    CorpusSaturation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionChaosFailureDetector {
    PanicCrashOrUnwind,
    TimeoutOrNoProgress,
    NonFiniteSink,
    CaptureSurvivedReleaseOrTeardown,
    DirtyLayoutAtSettleBoundary,
    EventRouterContradiction,
    AuthorityViolation,
    ReplayNondeterminism,
    UnsupportedCommandOrDomainSkip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionChaosHardFailureClass {
    PanicCrashOrUnwind,
    TimeoutHangOrNoProgress,
    NonFiniteFinalSink,
    PointerCaptureLeak,
    DirtyLayoutAtSettleBoundary,
    EventRouterContradiction,
    AuthorityViolation,
    ReplayNondeterminism,
    UnsupportedDomainSkip,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionChaosStreamPolicy {
    pub direct_injection_submode: Option<String>,
    pub preserve_intermediate_pointer_samples: bool,
    pub record_frame_and_reconcile_boundaries: bool,
}

impl InteractionChaosStreamPolicy {
    pub fn endpoint_teleportation_allowed(&self) -> bool {
        self.direct_injection_submode.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractionChaosSurface {
    pub id: String,
    pub description: String,
    pub root_surface_kind: String,
    pub construction_entrypoint: String,
    pub build_identity: String,
    pub window_size: (f32, f32),
    pub scale_factor: f32,
    pub theme: String,
    pub clock_seed: u64,
    pub initial_focus: Option<String>,
    pub initial_scroll: Option<(f32, f32)>,
    pub initial_selection: Option<String>,
    pub initial_state_snapshot: InteractionStateSnapshot,
    pub allowed_primitive_families: Vec<InteractionChaosPrimitiveFamily>,
    pub enabled_hard_failure_oracles: Vec<String>,
    pub disabled_domains: Vec<String>,
    pub expected_unsupported_domains: Vec<String>,
    pub settle_boundary: InteractionSettleBoundary,
    pub command_budget: usize,
    pub transaction_budget: usize,
    pub coverage_window: usize,
    pub timeout_policy: InteractionTimeoutPolicy,
    pub corpus_policy: String,
    pub visual_diagnostics_policy: InteractionVisualDiagnosticsPolicy,
    pub constitution_boundary_profile: InteractionConstitutionBoundaryProfile,
}

/// Corpus entry emitted by chaos mode when a command stream contributes new pressure coverage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractionChaosCorpusEntry {
    pub corpus_entry_id: String,
    pub schema: String,
    pub fixture_id: String,
    pub chaos_surface_id: String,
    pub generator_id: String,
    pub chaos_stream_generator_id: String,
    pub generator_version: String,
    pub chaos_stream_generator_version: String,
    pub seed: u64,
    pub rng_algorithm: String,
    pub rng_version: String,
    pub primitive_stream_range: InteractionPrimitiveStreamRange,
    pub shrunk_trace: Option<InteractionReplayArtifact>,
    pub shrunk_primitive_trace: Vec<ReplayPrimitiveTraceStep>,
    pub hard_failure_class: Option<InteractionChaosHardFailureClass>,
    pub coverage_keys_gained: Vec<String>,
    pub oracle_set_used: Vec<String>,
    pub hard_failure_oracle_set_used: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractionChaosPressureCoverageSummary {
    pub total_keys: usize,
    pub frontier_saturated: bool,
    pub pressure_keys: Vec<String>,
    pub required_dimensions: Vec<String>,
    pub exercised_dimensions: Vec<String>,
    pub missing_required_dimensions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractionChaosGeneratedCommandRecord {
    pub primitive_step_index: usize,
    pub command: ReplayInteractionCommand,
    pub command_domain: String,
    pub pressure_patterns: Vec<InteractionChaosPointerPressurePattern>,
    pub pointer_motion_policy: Option<InteractionChaosPointerMotionPolicy>,
    pub pointer_sample_ordinal: Option<usize>,
    pub frame_boundary: bool,
    pub reconcile_boundary: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractionChaosReport {
    pub schema_version: u32,
    pub schema: String,
    pub run_id: String,
    pub run_profile_id: String,
    pub run_profile_hash: InteractionFuzzProfileHash,
    pub chaos_mode: InteractionFuzzMode,
    pub surface_set: Vec<String>,
    pub seed_set: Vec<u64>,
    pub rng_algorithm: String,
    pub rng_version: String,
    pub chaos_stream_generator_ids: Vec<String>,
    pub chaos_stream_generator_versions: Vec<InteractionGeneratorVersionRecord>,
    pub stream_policy: InteractionChaosStreamPolicy,
    pub executed_command_budget: usize,
    pub executed_transaction_budget: usize,
    pub generated_command_records: Vec<InteractionChaosGeneratedCommandRecord>,
    pub generated_primitive_commands: Vec<ReplayInteractionCommand>,
    pub primitive_trace: Vec<ReplayPrimitiveTraceStep>,
    pub primitive_family_set: Vec<InteractionChaosPrimitiveFamily>,
    pub pointer_pressure_patterns: Vec<InteractionChaosPointerPressurePattern>,
    pub pointer_motion_policies: Vec<InteractionChaosPointerMotionPolicy>,
    pub hard_failure_oracle_set: Vec<String>,
    pub coverage_summary: InteractionCoverageSummary,
    pub pressure_coverage_summary: InteractionChaosPressureCoverageSummary,
    pub corpus_deltas: Vec<InteractionChaosCorpusEntry>,
    pub unsupported_domains: Vec<String>,
    pub halt_reason: InteractionChaosHaltReason,
    pub failure_count: usize,
    pub confidence_packet_emitted: bool,
    pub visual_diagnostics_status: InteractionVisualDiagnosticsStatus,
    pub constitution_boundary_profile: InteractionConstitutionBoundaryProfile,
    pub constitution_boundary_check_status: InteractionConstitutionBoundaryCheckStatus,
    pub no_failure_claim: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionPrimitiveStreamRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractionChaosFailureFields {
    pub chaos_surface_id: String,
    pub chaos_surface_state_id: String,
    pub chaos_mode: InteractionFuzzMode,
    pub chaos_stream_generator: String,
    pub chaos_stream_generator_version: String,
    pub chaos_stream_policy: InteractionChaosStreamPolicy,
    pub rng_algorithm: String,
    pub rng_version: String,
    pub rng_seed: u64,
    pub primitive_stream_range: InteractionPrimitiveStreamRange,
    pub failing_command_index: usize,
    pub failure_detector: InteractionChaosFailureDetector,
    pub hard_failure_class: InteractionChaosHardFailureClass,
    pub timeout_budget: InteractionTimeoutPolicy,
    pub replay_determinism_status: String,
    pub initial_state_snapshot: InteractionStateSnapshot,
    pub final_state_snapshot: InteractionStateSnapshot,
    pub unsupported_domains: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionFailureSeverity {
    P0,
    P1,
    P2,
    P3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionFailureLayer {
    IntentExpansion,
    NavigationPath,
    IncidentalInteraction,
    PrimitiveInjection,
    InputFusion,
    EventRouting,
    WidgetState,
    Layout,
    Oracle,
    Replay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionOracleStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FiniteGeometrySink {
    FinalWidgetRect,
    PaintRect,
    ClipRect,
    HitTestBounds,
    PointerInteractionZone,
    RealizedRowRect,
    RealizedCellRect,
    TextGlyphQuad,
    SceneGraphNodeBounds,
    GpuSubmissionBuffer,
    OverlayAnchorRect,
    ScrollbarGeometry,
    SeparatorGeometry,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FiniteSinkGeometrySample {
    pub sink: FiniteGeometrySink,
    pub field_path: String,
    pub authority: String,
    pub value: f32,
}

impl FiniteSinkGeometrySample {
    pub fn new(
        sink: FiniteGeometrySink,
        field_path: impl Into<String>,
        authority: impl Into<String>,
        value: f32,
    ) -> Self {
        Self {
            sink,
            field_path: field_path.into(),
            authority: authority.into(),
            value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionOracleEvaluation {
    pub oracle_id: String,
    pub oracle_name: String,
    pub status: InteractionOracleStatus,
    pub failure_layer: Option<InteractionFailureLayer>,
    pub failed_subject: Option<String>,
    pub failed_requirement: Option<String>,
    pub failed_sink: Option<FiniteGeometrySink>,
    pub failed_field_path: Option<String>,
    pub failed_authority: Option<String>,
    pub failed_value: Option<String>,
    pub derived_values: BTreeMap<String, String>,
}

impl InteractionOracleEvaluation {
    pub fn passed(&self) -> bool {
        self.status == InteractionOracleStatus::Passed
    }
}

pub fn evaluate_finite_sink_geometry<I>(samples: I) -> InteractionOracleEvaluation
where
    I: IntoIterator<Item = FiniteSinkGeometrySample>,
{
    let mut sample_count = 0usize;
    let mut finite_count = 0usize;
    let mut first_failure = None;

    for sample in samples {
        sample_count += 1;
        if sample.value.is_finite() {
            finite_count += 1;
        } else if first_failure.is_none() {
            first_failure = Some(sample);
        }
    }

    let non_finite_count = sample_count - finite_count;
    let mut derived_values = BTreeMap::new();
    derived_values.insert("sample_count".to_string(), sample_count.to_string());
    derived_values.insert("finite_count".to_string(), finite_count.to_string());
    derived_values.insert("non_finite_count".to_string(), non_finite_count.to_string());

    if let Some(sample) = first_failure {
        let failed_value = finite_oracle_value_token(sample.value);
        derived_values.insert(
            "first_failure_sink".to_string(),
            format!("{:?}", sample.sink),
        );
        derived_values.insert(
            "first_failure_field_path".to_string(),
            sample.field_path.clone(),
        );
        derived_values.insert(
            "first_failure_authority".to_string(),
            sample.authority.clone(),
        );
        derived_values.insert("first_failure_value".to_string(), failed_value.clone());

        InteractionOracleEvaluation {
            oracle_id: ORACLE_346_U1_ID.to_string(),
            oracle_name: ORACLE_346_U1_NAME.to_string(),
            status: InteractionOracleStatus::Failed,
            failure_layer: Some(InteractionFailureLayer::Layout),
            failed_subject: Some(sample.field_path.clone()),
            failed_requirement: Some("finite_sink_geometry".to_string()),
            failed_sink: Some(sample.sink),
            failed_field_path: Some(sample.field_path),
            failed_authority: Some(sample.authority),
            failed_value: Some(failed_value),
            derived_values,
        }
    } else {
        InteractionOracleEvaluation {
            oracle_id: ORACLE_346_U1_ID.to_string(),
            oracle_name: ORACLE_346_U1_NAME.to_string(),
            status: InteractionOracleStatus::Passed,
            failure_layer: None,
            failed_subject: None,
            failed_requirement: None,
            failed_sink: None,
            failed_field_path: None,
            failed_authority: None,
            failed_value: None,
            derived_values,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureOwnerSnapshot {
    pub capture_exists: bool,
    pub owner_id: Option<String>,
    pub owner_exists: bool,
    pub in_active_interaction_domain: bool,
    pub capture_eligible: bool,
    pub release_or_cancel_required: bool,
    pub release_or_cancel_observed: bool,
    pub authority: String,
}

impl CaptureOwnerSnapshot {
    pub fn new(
        capture_exists: bool,
        owner_id: Option<String>,
        owner_exists: bool,
        in_active_interaction_domain: bool,
        capture_eligible: bool,
        release_or_cancel_required: bool,
        release_or_cancel_observed: bool,
        authority: impl Into<String>,
    ) -> Self {
        Self {
            capture_exists,
            owner_id,
            owner_exists,
            in_active_interaction_domain,
            capture_eligible,
            release_or_cancel_required,
            release_or_cancel_observed,
            authority: authority.into(),
        }
    }
}

pub fn evaluate_capture_owner_validity(
    snapshot: CaptureOwnerSnapshot,
) -> InteractionOracleEvaluation {
    let mut derived_values = BTreeMap::new();
    derived_values.insert(
        "capture_exists".to_string(),
        snapshot.capture_exists.to_string(),
    );
    derived_values.insert(
        "owner_id".to_string(),
        snapshot.owner_id.clone().unwrap_or_default(),
    );
    derived_values.insert(
        "owner_exists".to_string(),
        snapshot.owner_exists.to_string(),
    );
    derived_values.insert(
        "in_active_interaction_domain".to_string(),
        snapshot.in_active_interaction_domain.to_string(),
    );
    derived_values.insert(
        "capture_eligible".to_string(),
        snapshot.capture_eligible.to_string(),
    );
    derived_values.insert(
        "release_or_cancel_required".to_string(),
        snapshot.release_or_cancel_required.to_string(),
    );
    derived_values.insert(
        "release_or_cancel_observed".to_string(),
        snapshot.release_or_cancel_observed.to_string(),
    );
    derived_values.insert("authority".to_string(), snapshot.authority.clone());

    let failure_requirement = if !snapshot.capture_exists {
        None
    } else if snapshot.owner_id.is_none() {
        Some("capture_owner_id_present")
    } else if !snapshot.owner_exists {
        Some("capture_owner_exists")
    } else if !snapshot.in_active_interaction_domain {
        Some("capture_owner_in_active_interaction_domain")
    } else if !snapshot.capture_eligible {
        Some("capture_owner_capture_eligible")
    } else if snapshot.release_or_cancel_required && !snapshot.release_or_cancel_observed {
        Some("capture_owner_received_release_or_cancel")
    } else {
        None
    };

    if let Some(requirement) = failure_requirement {
        InteractionOracleEvaluation {
            oracle_id: ORACLE_346_U3_ID.to_string(),
            oracle_name: ORACLE_346_U3_NAME.to_string(),
            status: InteractionOracleStatus::Failed,
            failure_layer: Some(InteractionFailureLayer::EventRouting),
            failed_subject: snapshot.owner_id,
            failed_requirement: Some(requirement.to_string()),
            failed_sink: None,
            failed_field_path: None,
            failed_authority: Some(snapshot.authority),
            failed_value: None,
            derived_values,
        }
    } else {
        InteractionOracleEvaluation {
            oracle_id: ORACLE_346_U3_ID.to_string(),
            oracle_name: ORACLE_346_U3_NAME.to_string(),
            status: InteractionOracleStatus::Passed,
            failure_layer: None,
            failed_subject: None,
            failed_requirement: None,
            failed_sink: None,
            failed_field_path: None,
            failed_authority: None,
            failed_value: None,
            derived_values,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutSettledSnapshot {
    pub settle_boundary: InteractionSettleBoundary,
    pub layout_dirty: bool,
    pub dirty_reasons: Vec<String>,
    pub continuous_layout_invalidation_allowed: bool,
    pub authority: String,
}

impl LayoutSettledSnapshot {
    pub fn new(
        settle_boundary: InteractionSettleBoundary,
        layout_dirty: bool,
        dirty_reasons: Vec<String>,
        continuous_layout_invalidation_allowed: bool,
        authority: impl Into<String>,
    ) -> Self {
        Self {
            settle_boundary,
            layout_dirty,
            dirty_reasons,
            continuous_layout_invalidation_allowed,
            authority: authority.into(),
        }
    }
}

pub fn evaluate_layout_settled(snapshot: LayoutSettledSnapshot) -> InteractionOracleEvaluation {
    let mut dirty_reasons = snapshot.dirty_reasons;
    dirty_reasons.sort();
    dirty_reasons.dedup();

    let mut derived_values = BTreeMap::new();
    derived_values.insert(
        "settle_boundary".to_string(),
        format!("{:?}", snapshot.settle_boundary),
    );
    derived_values.insert(
        "layout_dirty".to_string(),
        snapshot.layout_dirty.to_string(),
    );
    derived_values.insert("dirty_reasons".to_string(), dirty_reasons.join(","));
    derived_values.insert(
        "continuous_layout_invalidation_allowed".to_string(),
        snapshot.continuous_layout_invalidation_allowed.to_string(),
    );
    derived_values.insert("authority".to_string(), snapshot.authority.clone());

    if snapshot.layout_dirty && !snapshot.continuous_layout_invalidation_allowed {
        InteractionOracleEvaluation {
            oracle_id: ORACLE_346_U7_ID.to_string(),
            oracle_name: ORACLE_346_U7_NAME.to_string(),
            status: InteractionOracleStatus::Failed,
            failure_layer: Some(InteractionFailureLayer::Layout),
            failed_subject: Some(format!("{:?}", snapshot.settle_boundary)),
            failed_requirement: Some("layout_clean_at_settle_boundary".to_string()),
            failed_sink: None,
            failed_field_path: None,
            failed_authority: Some(snapshot.authority),
            failed_value: Some("layout_dirty".to_string()),
            derived_values,
        }
    } else {
        InteractionOracleEvaluation {
            oracle_id: ORACLE_346_U7_ID.to_string(),
            oracle_name: ORACLE_346_U7_NAME.to_string(),
            status: InteractionOracleStatus::Passed,
            failure_layer: None,
            failed_subject: None,
            failed_requirement: None,
            failed_sink: None,
            failed_field_path: None,
            failed_authority: None,
            failed_value: None,
            derived_values,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IncidentalNavigationSnapshot {
    pub active_intent_index: usize,
    pub primitive_step_index: usize,
    pub active_intent_command: Option<ReplayInteractionIntentCommand>,
    pub intended_target: InteractionTargetSnapshot,
    pub incidental_target: InteractionTargetSnapshot,
    pub path_policy: Option<PointerPathPolicy>,
    pub side_effects_suppressed: bool,
    pub universal_oracle_evaluations: Vec<InteractionOracleEvaluation>,
    pub authority: String,
}

pub fn evaluate_incidental_navigation_integrity(
    snapshot: IncidentalNavigationSnapshot,
) -> InteractionOracleEvaluation {
    let failed_universal_oracle = snapshot
        .universal_oracle_evaluations
        .iter()
        .find(|evaluation| !evaluation.passed());
    let failed_requirement = if snapshot.side_effects_suppressed {
        Some("incidental_side_effects_not_suppressed")
    } else if failed_universal_oracle.is_some() {
        Some("incidental_target_satisfies_universal_oracles")
    } else {
        None
    };

    let failed_oracle_id = failed_universal_oracle
        .map(|evaluation| evaluation.oracle_id.clone())
        .unwrap_or_default();
    let failed_oracle_name = failed_universal_oracle
        .map(|evaluation| evaluation.oracle_name.clone())
        .unwrap_or_default();
    let failed_value = if snapshot.side_effects_suppressed {
        Some("side_effects_suppressed".to_string())
    } else {
        failed_universal_oracle.map(|evaluation| evaluation.oracle_id.clone())
    };

    let universal_oracle_failure_count = snapshot
        .universal_oracle_evaluations
        .iter()
        .filter(|evaluation| !evaluation.passed())
        .count();
    let mut derived_values = BTreeMap::new();
    derived_values.insert(
        "active_intent_index".to_string(),
        snapshot.active_intent_index.to_string(),
    );
    derived_values.insert(
        "primitive_step_index".to_string(),
        snapshot.primitive_step_index.to_string(),
    );
    derived_values.insert(
        "active_intent_command_class".to_string(),
        snapshot
            .active_intent_command
            .as_ref()
            .map(replay_intent_command_class_token)
            .unwrap_or("none")
            .to_string(),
    );
    derived_values.insert(
        "intended_target".to_string(),
        target_snapshot_label(&snapshot.intended_target),
    );
    derived_values.insert(
        "incidental_target".to_string(),
        target_snapshot_label(&snapshot.incidental_target),
    );
    derived_values.insert(
        "path_policy".to_string(),
        snapshot
            .path_policy
            .as_ref()
            .map(|policy| format!("{:?}", policy))
            .unwrap_or_else(|| "none".to_string()),
    );
    derived_values.insert(
        "side_effects_suppressed".to_string(),
        snapshot.side_effects_suppressed.to_string(),
    );
    derived_values.insert(
        "universal_oracle_count".to_string(),
        snapshot.universal_oracle_evaluations.len().to_string(),
    );
    derived_values.insert(
        "universal_oracle_failure_count".to_string(),
        universal_oracle_failure_count.to_string(),
    );
    derived_values.insert("failed_universal_oracle_id".to_string(), failed_oracle_id);
    derived_values.insert(
        "failed_universal_oracle_name".to_string(),
        failed_oracle_name,
    );
    derived_values.insert("authority".to_string(), snapshot.authority.clone());

    if let Some(requirement) = failed_requirement {
        InteractionOracleEvaluation {
            oracle_id: ORACLE_346_U11_ID.to_string(),
            oracle_name: ORACLE_346_U11_NAME.to_string(),
            status: InteractionOracleStatus::Failed,
            failure_layer: Some(InteractionFailureLayer::IncidentalInteraction),
            failed_subject: Some(target_snapshot_label(&snapshot.incidental_target)),
            failed_requirement: Some(requirement.to_string()),
            failed_sink: None,
            failed_field_path: None,
            failed_authority: Some(snapshot.authority),
            failed_value,
            derived_values,
        }
    } else {
        InteractionOracleEvaluation {
            oracle_id: ORACLE_346_U11_ID.to_string(),
            oracle_name: ORACLE_346_U11_NAME.to_string(),
            status: InteractionOracleStatus::Passed,
            failure_layer: None,
            failed_subject: None,
            failed_requirement: None,
            failed_sink: None,
            failed_field_path: None,
            failed_authority: None,
            failed_value: None,
            derived_values,
        }
    }
}

fn target_snapshot_label(target: &InteractionTargetSnapshot) -> String {
    target
        .target_id
        .as_ref()
        .cloned()
        .unwrap_or_else(|| target.target_kind.clone())
}

fn replay_intent_command_class_token(command: &ReplayInteractionIntentCommand) -> &'static str {
    match command {
        ReplayInteractionIntentCommand::MovePointerTo { .. } => "move_pointer_to",
        ReplayInteractionIntentCommand::ClickTarget { .. } => "click_target",
        ReplayInteractionIntentCommand::DragPointerTo { .. } => "drag_pointer_to",
        ReplayInteractionIntentCommand::WheelAtTarget { .. } => "wheel_at_target",
        ReplayInteractionIntentCommand::KeyboardNavigation { .. } => "keyboard_navigation",
        ReplayInteractionIntentCommand::TextAtTarget { .. } => "text_at_target",
    }
}

fn intent_target_trace(command: &InteractionIntentCommand) -> Vec<InteractionIntentTargetTrace> {
    match command {
        InteractionIntentCommand::MovePointerTo { target, .. }
        | InteractionIntentCommand::ClickTarget { target, .. }
        | InteractionIntentCommand::WheelAtTarget { target, .. }
        | InteractionIntentCommand::TextAtTarget { target, .. } => vec![target.trace()],
        InteractionIntentCommand::DragPointerTo { from, to, .. } => {
            vec![from.trace(), to.trace()]
        }
        InteractionIntentCommand::KeyboardNavigation { .. } => Vec::new(),
    }
}

fn point_intent_target_id(target_kind: &str, point: Point) -> String {
    format!("{target_kind}:logical_window:{:.3}:{:.3}", point.x, point.y)
}

fn finite_oracle_value_token(value: f32) -> String {
    if value.is_nan() {
        "nan".to_string()
    } else if value == f32::INFINITY {
        "inf".to_string()
    } else if value == f32::NEG_INFINITY {
        "neg_inf".to_string()
    } else {
        value.to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionTargetSnapshot {
    pub target_id: Option<String>,
    pub target_kind: String,
    pub target_class: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionStateSnapshot {
    pub state_id: String,
    pub fields: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractionProbeSnapshotArtifact {
    pub source_layer: String,
    pub source_owner: String,
    pub layout_epoch: u64,
    pub frame_number: u64,
    pub settle_boundary: InteractionSettleBoundary,
    pub snapshot_reason: String,
    pub payload: serde_json::Value,
}

impl InteractionProbeSnapshotArtifact {
    pub fn from_serializable<T: Serialize>(
        source_layer: impl Into<String>,
        source_owner: impl Into<String>,
        layout_epoch: u64,
        frame_number: u64,
        settle_boundary: InteractionSettleBoundary,
        snapshot_reason: impl Into<String>,
        snapshot: &T,
    ) -> serde_json::Result<Self> {
        Ok(Self {
            source_layer: source_layer.into(),
            source_owner: source_owner.into(),
            layout_epoch,
            frame_number,
            settle_boundary,
            snapshot_reason: snapshot_reason.into(),
            payload: serde_json::to_value(snapshot)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractionFailurePacket {
    pub schema_version: u32,
    pub schema: String,
    pub failure_id: String,
    pub run_profile_id: String,
    pub run_profile_hash: InteractionFuzzProfileHash,
    pub invariant_id: String,
    pub invariant_name: String,
    pub severity: InteractionFailureSeverity,
    pub fixture_id: String,
    pub root_seed: u64,
    pub shrink_seed: Option<u64>,
    pub replay_command: String,
    pub injection_path: InteractionInjectionPath,
    pub settle_boundary: InteractionSettleBoundary,
    pub failing_intent_index: Option<usize>,
    pub failing_transaction_index: Option<usize>,
    pub failing_primitive_step_index: Option<usize>,
    pub active_intent_command: Option<ReplayInteractionIntentCommand>,
    pub command_that_failed: Option<ReplayInteractionCommand>,
    pub intended_target: Option<InteractionTargetSnapshot>,
    pub resolved_target: Option<InteractionTargetSnapshot>,
    pub incidental_target: Option<InteractionTargetSnapshot>,
    pub path_policy: Option<PointerPathPolicy>,
    pub intent_trace: Vec<ReplayIntentTraceStep>,
    pub primitive_trace: Vec<ReplayPrimitiveTraceStep>,
    pub shrunk_trace: InteractionReplayArtifact,
    pub unshrunk_trace_reference: Option<String>,
    pub pre_state: InteractionStateSnapshot,
    pub post_state: InteractionStateSnapshot,
    pub expected_state: InteractionStateSnapshot,
    pub observed_state: InteractionStateSnapshot,
    pub probe_snapshots: Vec<InteractionProbeSnapshotArtifact>,
    pub layer_classification: InteractionFailureLayer,
    pub ownership_chain: Vec<String>,
    pub coverage_key: Option<CoverageKey>,
    pub timeout_policy: InteractionTimeoutPolicy,
    pub progress_marker_history: Vec<InteractionProgressMarker>,
    pub visual_diagnostics_status: InteractionVisualDiagnosticsStatus,
    pub visual_trace_artifacts: Vec<InteractionVisualTraceArtifact>,
    pub constitution_boundary_profile: InteractionConstitutionBoundaryProfile,
    pub constitution_boundary_check_status: InteractionConstitutionBoundaryCheckStatus,
    pub chaos_failure_fields: Option<InteractionChaosFailureFields>,
    pub related_specs: Vec<String>,
    pub promotion_target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionElapsedTime {
    pub milliseconds: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractionCoverageSummary {
    pub total_keys: usize,
    pub exercised_keys: Vec<CoverageKey>,
    pub path_policy_classes: Vec<PointerPathPolicyClass>,
    pub keyboard_navigation_policies: Vec<KeyboardNavigationPolicy>,
    pub path_sample_count_classes: Vec<PointerPathSampleCountClass>,
    pub path_segments: Vec<PointerPathSegment>,
    pub frame_policies: Vec<PointerFramePolicy>,
    pub coalesced_move_classes: Vec<PointerMoveExposureClass>,
    pub crossed_hover_target_classes: Vec<String>,
    pub crossed_separator_classes: Vec<String>,
    pub capture_transition_classes: Vec<PointerCaptureTransitionClass>,
    pub incidental_route_target_classes: Vec<String>,
    pub endpoint_teleportation_modes: Vec<EndpointTeleportationMode>,
    pub incidental_target_classes: Vec<String>,
}

impl InteractionCoverageSummary {
    pub fn from_coverage<I, S>(coverage: &InteractionCoverage, incidental_target_classes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut exercised_keys: Vec<CoverageKey> = coverage.keys().cloned().collect();
        exercised_keys.sort();

        let mut path_policy_classes = BTreeSet::new();
        let mut keyboard_navigation_policies = BTreeSet::new();
        let mut path_sample_count_classes = BTreeSet::new();
        let mut path_segments = BTreeSet::new();
        let mut frame_policies = BTreeSet::new();
        let mut coalesced_move_classes = BTreeSet::new();
        let mut crossed_hover_target_classes = BTreeSet::new();
        let mut crossed_separator_classes = BTreeSet::new();
        let mut capture_transition_classes = BTreeSet::new();
        let mut incidental_route_target_classes = BTreeSet::new();
        let mut endpoint_teleportation_modes = BTreeSet::new();

        for key in exercised_keys.iter() {
            match key {
                CoverageKey::PointerPathPolicyClass(value) => {
                    path_policy_classes.insert(*value);
                }
                CoverageKey::KeyboardNavigationPolicy(value) => {
                    keyboard_navigation_policies.insert(*value);
                }
                CoverageKey::PointerPathSampleCountClass(value) => {
                    path_sample_count_classes.insert(*value);
                }
                CoverageKey::PointerPathSegment(value) => {
                    path_segments.insert(*value);
                }
                CoverageKey::PointerFramePolicy(value) => {
                    frame_policies.insert(*value);
                }
                CoverageKey::CoalescedMoveClass(value) => {
                    coalesced_move_classes.insert(*value);
                }
                CoverageKey::CrossedHoverTargetClass(value) => {
                    crossed_hover_target_classes.insert(value.clone());
                }
                CoverageKey::CrossedSeparatorClass(value) => {
                    crossed_separator_classes.insert(value.clone());
                }
                CoverageKey::CaptureTransitionClass(value) => {
                    capture_transition_classes.insert(*value);
                }
                CoverageKey::IncidentalRouteTargetClass(value) => {
                    incidental_route_target_classes.insert(value.clone());
                }
                CoverageKey::EndpointTeleportationMode(value) => {
                    endpoint_teleportation_modes.insert(*value);
                }
                CoverageKey::IntentCommandClass(_) | CoverageKey::ThresholdCrossing => {}
            }
        }

        let mut incidental_target_classes: Vec<String> = incidental_target_classes
            .into_iter()
            .map(Into::into)
            .collect();
        incidental_target_classes.sort();
        incidental_target_classes.dedup();

        Self {
            total_keys: exercised_keys.len(),
            exercised_keys,
            path_policy_classes: path_policy_classes.into_iter().collect(),
            keyboard_navigation_policies: keyboard_navigation_policies.into_iter().collect(),
            path_sample_count_classes: path_sample_count_classes.into_iter().collect(),
            path_segments: path_segments.into_iter().collect(),
            frame_policies: frame_policies.into_iter().collect(),
            coalesced_move_classes: coalesced_move_classes.into_iter().collect(),
            crossed_hover_target_classes: crossed_hover_target_classes.into_iter().collect(),
            crossed_separator_classes: crossed_separator_classes.into_iter().collect(),
            capture_transition_classes: capture_transition_classes.into_iter().collect(),
            incidental_route_target_classes: incidental_route_target_classes.into_iter().collect(),
            endpoint_teleportation_modes: endpoint_teleportation_modes.into_iter().collect(),
            incidental_target_classes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrimitiveStepMetrics {
    pub primitive_step_index: usize,
    pub command_class: PrimitiveCommandClass,
    pub emitted_event_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentStepMetrics {
    pub intent_index: usize,
    pub command_class: IntentCommandClass,
    pub primitive_step_count: usize,
    pub path_policy_class: Option<PointerPathPolicyClass>,
    pub keyboard_navigation_policy: Option<KeyboardNavigationPolicy>,
    pub pointer_sample_count: usize,
    pub frame_boundary_count: usize,
    pub threshold_crossing_count: usize,
    pub endpoint_teleportation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionTraceMetrics {
    pub primitive_step_count: usize,
    pub intent_count: usize,
    pub fused_event_count: usize,
    pub max_emitted_events_per_primitive: usize,
    pub coverage_key_count: usize,
    pub pointer_sample_count: usize,
    pub frame_boundary_count: usize,
    pub threshold_crossing_count: usize,
    pub endpoint_teleportation_intent_count: usize,
    pub sampled_path_intent_count: usize,
    pub primitive_steps: Vec<PrimitiveStepMetrics>,
    pub intent_steps: Vec<IntentStepMetrics>,
}

impl InteractionTraceMetrics {
    pub fn from_driver(driver: &InteractionDriver) -> Self {
        let primitive_steps: Vec<PrimitiveStepMetrics> = driver
            .primitive_trace()
            .iter()
            .map(|step| PrimitiveStepMetrics {
                primitive_step_index: step.primitive_step_index,
                command_class: PrimitiveCommandClass::from(&step.command),
                emitted_event_count: step.fused_events.len(),
            })
            .collect();

        let fused_event_count = primitive_steps
            .iter()
            .map(|step| step.emitted_event_count)
            .sum();
        let max_emitted_events_per_primitive = primitive_steps
            .iter()
            .map(|step| step.emitted_event_count)
            .max()
            .unwrap_or(0);

        let intent_steps: Vec<IntentStepMetrics> = driver
            .intent_trace()
            .iter()
            .map(IntentStepMetrics::from)
            .collect();

        let pointer_sample_count = intent_steps
            .iter()
            .map(|step| step.pointer_sample_count)
            .sum();
        let frame_boundary_count = intent_steps
            .iter()
            .map(|step| step.frame_boundary_count)
            .sum();
        let threshold_crossing_count = intent_steps
            .iter()
            .map(|step| step.threshold_crossing_count)
            .sum();
        let endpoint_teleportation_intent_count = intent_steps
            .iter()
            .filter(|step| step.endpoint_teleportation)
            .count();
        let sampled_path_intent_count = intent_steps
            .iter()
            .filter(|step| !step.endpoint_teleportation && step.path_policy_class.is_some())
            .count();

        Self {
            primitive_step_count: primitive_steps.len(),
            intent_count: intent_steps.len(),
            fused_event_count,
            max_emitted_events_per_primitive,
            coverage_key_count: driver.coverage().total_keys(),
            pointer_sample_count,
            frame_boundary_count,
            threshold_crossing_count,
            endpoint_teleportation_intent_count,
            sampled_path_intent_count,
            primitive_steps,
            intent_steps,
        }
    }
}

impl From<&IntentTraceStep> for IntentStepMetrics {
    fn from(value: &IntentTraceStep) -> Self {
        let pointer_sample_count = value
            .expansion_steps
            .iter()
            .filter(|step| matches!(step.command, InteractionCommand::PointerMove { .. }))
            .count();
        let frame_boundary_count = value
            .expansion_steps
            .iter()
            .filter(|step| {
                matches!(
                    step.command,
                    InteractionCommand::Frame { .. } | InteractionCommand::Reconcile
                )
            })
            .count();
        let threshold_crossing_count = value
            .expansion_steps
            .iter()
            .filter(|step| step.segment == PointerPathSegment::ThresholdCrossing)
            .count();
        let endpoint_teleportation = pointer_sample_count <= 1
            && matches!(value.path_policy, Some(PointerPathPolicy::Instant));
        let keyboard_navigation_policy = if let InteractionIntentCommand::KeyboardNavigation {
            navigation_policy,
            ..
        } = &value.intent_command
        {
            Some(*navigation_policy)
        } else {
            None
        };

        Self {
            intent_index: value.intent_index,
            command_class: value.intent_command.command_class(),
            primitive_step_count: value.primitive_command_range.len(),
            path_policy_class: value.path_policy.as_ref().map(PointerPathPolicyClass::from),
            keyboard_navigation_policy,
            pointer_sample_count,
            frame_boundary_count,
            threshold_crossing_count,
            endpoint_teleportation,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractionSaturationSummary {
    pub frontier_keys_seen: usize,
    pub frontier_keys_required: usize,
    pub frontier_saturated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractionConfidencePacket {
    pub schema_version: u32,
    pub schema: String,
    pub run_id: String,
    pub run_profile_id: String,
    pub run_profile_hash: InteractionFuzzProfileHash,
    pub halcyon_version: String,
    pub git_revision: String,
    pub fixture_set: Vec<String>,
    pub generator_set: Vec<String>,
    pub oracle_set: Vec<String>,
    pub seed_set: Vec<u64>,
    pub injection_path: InteractionInjectionPath,
    pub settle_boundary: InteractionSettleBoundary,
    pub transaction_count: usize,
    pub coverage_summary: InteractionCoverageSummary,
    pub saturation_summary: InteractionSaturationSummary,
    pub elapsed_time: InteractionElapsedTime,
    pub failure_count: usize,
    pub skipped_oracles: Vec<String>,
    pub disabled_generators: Vec<String>,
    pub out_of_scope_domains: Vec<String>,
    pub timeout_policy: InteractionTimeoutPolicy,
    pub visual_diagnostics_status: InteractionVisualDiagnosticsStatus,
    pub constitution_boundary_profile: InteractionConstitutionBoundaryProfile,
    pub constitution_boundary_check_status: InteractionConstitutionBoundaryCheckStatus,
    pub positive_claim: String,
}

impl InteractionConfidencePacket {
    pub fn from_input(input: InteractionConfidencePacketInput) -> Self {
        Self {
            schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
            run_id: input.run_id,
            run_profile_id: input.run_profile_id,
            run_profile_hash: input.run_profile_hash,
            halcyon_version: input.halcyon_version,
            git_revision: input.git_revision,
            fixture_set: input.fixture_set,
            generator_set: input.generator_set,
            oracle_set: input.oracle_set,
            seed_set: input.seed_set,
            injection_path: input.injection_path,
            settle_boundary: input.settle_boundary,
            transaction_count: input.transaction_count,
            coverage_summary: input.coverage_summary,
            saturation_summary: input.saturation_summary,
            elapsed_time: input.elapsed_time,
            failure_count: input.failure_count,
            skipped_oracles: input.skipped_oracles,
            disabled_generators: input.disabled_generators,
            out_of_scope_domains: input.out_of_scope_domains,
            timeout_policy: input.timeout_policy,
            visual_diagnostics_status: input.visual_diagnostics_status,
            constitution_boundary_profile: input.constitution_boundary_profile,
            constitution_boundary_check_status: input.constitution_boundary_check_status,
            positive_claim: INTERACTION_CONFIDENCE_POSITIVE_CLAIM.to_string(),
            schema: INTERACTION_FUZZ_SCHEMA.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractionConfidencePacketInput {
    pub run_id: String,
    pub run_profile_id: String,
    pub run_profile_hash: InteractionFuzzProfileHash,
    pub halcyon_version: String,
    pub git_revision: String,
    pub fixture_set: Vec<String>,
    pub generator_set: Vec<String>,
    pub oracle_set: Vec<String>,
    pub seed_set: Vec<u64>,
    pub injection_path: InteractionInjectionPath,
    pub settle_boundary: InteractionSettleBoundary,
    pub transaction_count: usize,
    pub coverage_summary: InteractionCoverageSummary,
    pub saturation_summary: InteractionSaturationSummary,
    pub elapsed_time: InteractionElapsedTime,
    pub failure_count: usize,
    pub skipped_oracles: Vec<String>,
    pub disabled_generators: Vec<String>,
    pub out_of_scope_domains: Vec<String>,
    pub timeout_policy: InteractionTimeoutPolicy,
    pub visual_diagnostics_status: InteractionVisualDiagnosticsStatus,
    pub constitution_boundary_profile: InteractionConstitutionBoundaryProfile,
    pub constitution_boundary_check_status: InteractionConstitutionBoundaryCheckStatus,
}

#[derive(Debug, Clone)]
pub struct PrimitiveTraceStep {
    pub primitive_step_index: usize,
    pub command: InteractionCommand,
    pub fused_events: Vec<FusedInputEvent>,
}

#[derive(Debug, Clone)]
pub struct IntentExpansionStep {
    pub primitive_offset: usize,
    pub command: InteractionCommand,
    pub segment: PointerPathSegment,
    pub frame_policy: PointerFramePolicy,
}

#[derive(Debug, Clone)]
pub struct IntentTraceStep {
    pub intent_index: usize,
    pub intent_command: InteractionIntentCommand,
    pub target_trace: Vec<InteractionIntentTargetTrace>,
    pub path_policy: Option<PointerPathPolicy>,
    pub primitive_command_range: Range<usize>,
    pub expansion_steps: Vec<IntentExpansionStep>,
}

#[derive(Debug)]
pub struct InteractionDriver {
    config: InteractionDriverConfig,
    fusion: InputFusion,
    base_time: Instant,
    tick: u64,
    pointer_pos: Point,
    fused_events: Vec<FusedInputEvent>,
    next_intent_index: usize,
    intent_trace: Vec<IntentTraceStep>,
    coverage: InteractionCoverage,
    next_primitive_step_index: usize,
    primitive_trace: Vec<PrimitiveTraceStep>,
}

impl InteractionDriver {
    pub fn new() -> Self {
        Self::with_config(InteractionDriverConfig::default())
    }

    pub fn with_config(config: InteractionDriverConfig) -> Self {
        Self {
            config,
            fusion: InputFusion::new(),
            base_time: Instant::now(),
            tick: 0,
            pointer_pos: Point::zero(),
            fused_events: Vec::new(),
            next_intent_index: 0,
            intent_trace: Vec::new(),
            coverage: InteractionCoverage::new(),
            next_primitive_step_index: 0,
            primitive_trace: Vec::new(),
        }
    }

    pub fn execute(&mut self, command: InteractionCommand) -> Vec<FusedInputEvent> {
        self.try_execute(command)
            .expect("interaction command must not send non-finite values to runtime sinks")
    }

    pub fn try_execute(
        &mut self,
        command: InteractionCommand,
    ) -> InteractionDriverResult<Vec<FusedInputEvent>> {
        validate_interaction_command(&command, self.next_primitive_step_index)?;
        Ok(self.execute_traced(command))
    }

    pub fn execute_all<I>(&mut self, commands: I) -> Vec<FusedInputEvent>
    where
        I: IntoIterator<Item = InteractionCommand>,
    {
        self.try_execute_all(commands)
            .expect("interaction command sequence must not send non-finite values to runtime sinks")
    }

    pub fn try_execute_all<I>(
        &mut self,
        commands: I,
    ) -> InteractionDriverResult<Vec<FusedInputEvent>>
    where
        I: IntoIterator<Item = InteractionCommand>,
    {
        let commands: Vec<InteractionCommand> = commands.into_iter().collect();
        validate_interaction_commands(commands.iter(), self.next_primitive_step_index)?;

        let start = self.fused_events.len();
        for command in commands {
            self.execute_traced(command);
        }
        Ok(self.fused_events[start..].to_vec())
    }

    pub fn execute_intent(
        &mut self,
        intent_command: InteractionIntentCommand,
    ) -> Vec<FusedInputEvent> {
        self.try_execute_intent(intent_command)
            .expect("interaction intent must not expand non-finite values into runtime sinks")
    }

    pub fn try_execute_intent(
        &mut self,
        intent_command: InteractionIntentCommand,
    ) -> InteractionDriverResult<Vec<FusedInputEvent>> {
        let fused_start = self.fused_events.len();
        let intent_index = self.next_intent_index;

        let primitive_start = self.next_primitive_step_index;
        let path_policy = intent_command.pointer_path_policy().cloned();
        let expansion_steps = self.expand_intent(&intent_command);
        validate_interaction_commands(
            expansion_steps.iter().map(|step| &step.command),
            self.next_primitive_step_index,
        )?;

        self.next_intent_index += 1;

        for step in expansion_steps.iter() {
            self.execute_traced(step.command.clone());
        }

        let primitive_end = self.next_primitive_step_index;
        let target_trace = intent_target_trace(&intent_command);
        let trace_step = IntentTraceStep {
            intent_index,
            intent_command,
            target_trace,
            path_policy,
            primitive_command_range: primitive_start..primitive_end,
            expansion_steps,
        };
        self.coverage.record_intent_trace(&trace_step);
        self.intent_trace.push(trace_step);

        Ok(self.fused_events[fused_start..].to_vec())
    }

    pub fn drain_fused(&mut self) -> Vec<FusedInputEvent> {
        std::mem::take(&mut self.fused_events)
    }

    pub fn flush(&mut self) {
        self.fusion.flush(&mut self.fused_events);
    }

    pub fn fused_events(&self) -> &[FusedInputEvent] {
        &self.fused_events
    }

    pub fn intent_trace(&self) -> &[IntentTraceStep] {
        &self.intent_trace
    }

    pub fn drain_intent_trace(&mut self) -> Vec<IntentTraceStep> {
        std::mem::take(&mut self.intent_trace)
    }

    pub fn coverage(&self) -> &InteractionCoverage {
        &self.coverage
    }

    pub fn record_coverage_key(&mut self, key: CoverageKey) {
        self.coverage.record_key(key);
    }

    pub fn trace_metrics(&self) -> InteractionTraceMetrics {
        InteractionTraceMetrics::from_driver(self)
    }

    pub fn primitive_trace(&self) -> &[PrimitiveTraceStep] {
        &self.primitive_trace
    }

    pub fn drain_primitive_trace(&mut self) -> Vec<PrimitiveTraceStep> {
        std::mem::take(&mut self.primitive_trace)
    }

    pub fn pointer_position(&self) -> Point {
        self.pointer_pos
    }

    fn execute_traced(&mut self, command: InteractionCommand) -> Vec<FusedInputEvent> {
        let fused_start = self.fused_events.len();
        let primitive_step_index = self.next_primitive_step_index;
        self.next_primitive_step_index += 1;

        self.apply(command.clone());

        let fused_events = self.fused_events[fused_start..].to_vec();
        self.primitive_trace.push(PrimitiveTraceStep {
            primitive_step_index,
            command,
            fused_events: fused_events.clone(),
        });
        fused_events
    }

    fn apply(&mut self, command: InteractionCommand) {
        match command {
            InteractionCommand::PointerMove { x, y } => {
                let next = Point::new(x, y);
                let delta = (next.x - self.pointer_pos.x, next.y - self.pointer_pos.y);
                let time = self.next_time();
                self.pointer_pos = next;
                self.push_raw(RawInputEvent::PointerMoved {
                    window_id: self.config.window_id,
                    time,
                    device: self.config.default_pointer_device,
                    position: next,
                    delta,
                });
            }
            InteractionCommand::PointerDown { x, y, button } => {
                let position = Point::new(x, y);
                let time = self.next_time();
                self.pointer_pos = position;
                self.push_raw(RawInputEvent::PointerButton {
                    window_id: self.config.window_id,
                    time,
                    device: self.config.default_pointer_device,
                    button,
                    state: RawButtonState::Pressed,
                    position,
                });
            }
            InteractionCommand::PointerUp { x, y, button } => {
                let position = Point::new(x, y);
                let time = self.next_time();
                self.pointer_pos = position;
                self.push_raw(RawInputEvent::PointerButton {
                    window_id: self.config.window_id,
                    time,
                    device: self.config.default_pointer_device,
                    button,
                    state: RawButtonState::Released,
                    position,
                });
            }
            InteractionCommand::WheelPixels {
                x,
                y,
                delta_x,
                delta_y,
            } => {
                self.push_wheel(x, y, WheelDelta::Pixels(delta_x, delta_y));
            }
            InteractionCommand::WheelLines {
                x,
                y,
                delta_x,
                delta_y,
            } => {
                self.push_wheel(x, y, WheelDelta::Lines(delta_x, delta_y));
            }
            InteractionCommand::KeyDown {
                key_code,
                modifiers,
                repeat,
            } => {
                let time = self.next_time();
                self.push_raw(RawInputEvent::Key {
                    window_id: self.config.window_id,
                    time,
                    key_code,
                    physical_key: crate::core::event::PhysicalKeyCode::Unknown,
                    reservation: None,
                    state: KeyState::Down,
                    repeat,
                    modifiers,
                });
            }
            InteractionCommand::KeyUp {
                key_code,
                modifiers,
            } => {
                let time = self.next_time();
                self.push_raw(RawInputEvent::Key {
                    window_id: self.config.window_id,
                    time,
                    key_code,
                    physical_key: crate::core::event::PhysicalKeyCode::Unknown,
                    reservation: None,
                    state: KeyState::Up,
                    repeat: false,
                    modifiers,
                });
            }
            InteractionCommand::TextInput { text, modifiers } => {
                let time = self.next_time();
                self.push_raw(RawInputEvent::TextInput {
                    window_id: self.config.window_id,
                    time,
                    text,
                    modifiers,
                });
            }
            InteractionCommand::Frame { count } => {
                for _ in 0..count {
                    self.flush();
                    self.advance_tick();
                }
            }
            InteractionCommand::Reconcile => {
                self.flush();
                self.advance_tick();
            }
            InteractionCommand::SetWindowSize { width, height } => {
                self.push_raw(RawInputEvent::Resize {
                    window_id: self.config.window_id,
                    width,
                    height,
                });
            }
            InteractionCommand::SetScaleFactor { scale_factor } => {
                self.fusion.set_scale_factor(scale_factor);
                self.advance_tick();
            }
        }
    }

    fn expand_intent(&self, intent_command: &InteractionIntentCommand) -> Vec<IntentExpansionStep> {
        match intent_command {
            InteractionIntentCommand::MovePointerTo {
                target,
                path_policy,
            } => {
                let mut steps = Vec::new();
                let target_point = target.resolved_point;
                self.expand_pointer_path(
                    self.pointer_pos,
                    target_point,
                    path_policy,
                    PointerPathSegment::Move,
                    &mut steps,
                );
                steps
            }
            InteractionIntentCommand::ClickTarget {
                target,
                button,
                path_policy,
            } => {
                let mut steps = Vec::new();
                let target_point = target.resolved_point;
                self.expand_pointer_path(
                    self.pointer_pos,
                    target_point,
                    path_policy,
                    PointerPathSegment::Approach,
                    &mut steps,
                );
                push_expansion_step(
                    &mut steps,
                    InteractionCommand::PointerDown {
                        x: target_point.x,
                        y: target_point.y,
                        button: *button,
                    },
                    PointerPathSegment::Press,
                    PointerFramePolicy::FrameSeparated,
                );
                push_frame_policy_boundary(
                    &mut steps,
                    PointerPathSegment::Press,
                    PointerFramePolicy::FrameSeparated,
                );
                push_expansion_step(
                    &mut steps,
                    InteractionCommand::PointerUp {
                        x: target_point.x,
                        y: target_point.y,
                        button: *button,
                    },
                    PointerPathSegment::Release,
                    PointerFramePolicy::FrameSeparated,
                );
                push_frame_policy_boundary(
                    &mut steps,
                    PointerPathSegment::Release,
                    PointerFramePolicy::FrameSeparated,
                );
                steps
            }
            InteractionIntentCommand::DragPointerTo {
                from,
                to,
                button,
                path_policy,
            } => {
                let mut steps = Vec::new();
                let from_point = from.resolved_point;
                let to_point = to.resolved_point;
                self.expand_pointer_path(
                    self.pointer_pos,
                    from_point,
                    path_policy,
                    PointerPathSegment::Approach,
                    &mut steps,
                );
                push_expansion_step(
                    &mut steps,
                    InteractionCommand::PointerDown {
                        x: from_point.x,
                        y: from_point.y,
                        button: *button,
                    },
                    PointerPathSegment::Press,
                    PointerFramePolicy::FrameSeparated,
                );
                push_frame_policy_boundary(
                    &mut steps,
                    PointerPathSegment::Press,
                    PointerFramePolicy::FrameSeparated,
                );
                self.expand_drag_path(from_point, to_point, path_policy, &mut steps);
                push_expansion_step(
                    &mut steps,
                    InteractionCommand::PointerUp {
                        x: to_point.x,
                        y: to_point.y,
                        button: *button,
                    },
                    PointerPathSegment::Release,
                    PointerFramePolicy::FrameSeparated,
                );
                steps
            }
            InteractionIntentCommand::WheelAtTarget {
                target,
                delta_x,
                delta_y,
                delta_mode,
                path_policy,
            } => {
                let mut steps = Vec::new();
                let target_point = target.resolved_point;
                self.expand_pointer_path(
                    self.pointer_pos,
                    target_point,
                    path_policy,
                    PointerPathSegment::Approach,
                    &mut steps,
                );
                let command = match delta_mode {
                    WheelDeltaMode::Pixels => InteractionCommand::WheelPixels {
                        x: target_point.x,
                        y: target_point.y,
                        delta_x: *delta_x,
                        delta_y: *delta_y,
                    },
                    WheelDeltaMode::Lines => InteractionCommand::WheelLines {
                        x: target_point.x,
                        y: target_point.y,
                        delta_x: *delta_x,
                        delta_y: *delta_y,
                    },
                };
                push_expansion_step(
                    &mut steps,
                    command,
                    PointerPathSegment::Wheel,
                    PointerFramePolicy::FrameSeparated,
                );
                push_frame_policy_boundary(
                    &mut steps,
                    PointerPathSegment::Wheel,
                    PointerFramePolicy::FrameSeparated,
                );
                steps
            }
            InteractionIntentCommand::KeyboardNavigation {
                keys,
                activation,
                modifiers,
                ..
            } => {
                let mut steps = Vec::new();
                for key_code in keys.iter().copied() {
                    push_keyboard_key(
                        &mut steps,
                        key_code,
                        *modifiers,
                        PointerPathSegment::KeyboardTraversal,
                    );
                }
                if let Some(key_code) = activation {
                    push_keyboard_key(
                        &mut steps,
                        *key_code,
                        *modifiers,
                        PointerPathSegment::KeyboardActivation,
                    );
                }
                steps
            }
            InteractionIntentCommand::TextAtTarget {
                target,
                text,
                modifiers,
                path_policy,
            } => {
                let mut steps = Vec::new();
                let target_point = target.resolved_point;
                self.expand_pointer_path(
                    self.pointer_pos,
                    target_point,
                    path_policy,
                    PointerPathSegment::Approach,
                    &mut steps,
                );
                push_expansion_step(
                    &mut steps,
                    InteractionCommand::PointerDown {
                        x: target_point.x,
                        y: target_point.y,
                        button: PointerButton::Primary,
                    },
                    PointerPathSegment::Press,
                    PointerFramePolicy::FrameSeparated,
                );
                push_frame_policy_boundary(
                    &mut steps,
                    PointerPathSegment::Press,
                    PointerFramePolicy::FrameSeparated,
                );
                push_expansion_step(
                    &mut steps,
                    InteractionCommand::PointerUp {
                        x: target_point.x,
                        y: target_point.y,
                        button: PointerButton::Primary,
                    },
                    PointerPathSegment::Release,
                    PointerFramePolicy::FrameSeparated,
                );
                push_frame_policy_boundary(
                    &mut steps,
                    PointerPathSegment::Release,
                    PointerFramePolicy::FrameSeparated,
                );
                push_expansion_step(
                    &mut steps,
                    InteractionCommand::TextInput {
                        text: text.clone(),
                        modifiers: *modifiers,
                    },
                    PointerPathSegment::TextInput,
                    PointerFramePolicy::FrameSeparated,
                );
                push_frame_policy_boundary(
                    &mut steps,
                    PointerPathSegment::TextInput,
                    PointerFramePolicy::FrameSeparated,
                );
                steps
            }
        }
    }

    fn expand_drag_path(
        &self,
        from: Point,
        to: Point,
        path_policy: &PointerPathPolicy,
        out: &mut Vec<IntentExpansionStep>,
    ) {
        let samples = pointer_path_samples(from, to, path_policy);
        let mut threshold_crossed = false;
        for (sample, mut frame_policy) in samples {
            let distance = sample.distance(from);
            let segment = if !threshold_crossed && distance >= FUSION_DRAG_DISTANCE_THRESHOLD {
                threshold_crossed = true;
                frame_policy = PointerFramePolicy::ThresholdCrossing;
                PointerPathSegment::ThresholdCrossing
            } else if threshold_crossed {
                PointerPathSegment::PostThresholdDrag
            } else {
                PointerPathSegment::PreThresholdDrag
            };
            push_pointer_sample(out, sample, segment, frame_policy);
        }
    }

    fn expand_pointer_path(
        &self,
        from: Point,
        to: Point,
        path_policy: &PointerPathPolicy,
        segment: PointerPathSegment,
        out: &mut Vec<IntentExpansionStep>,
    ) {
        for (sample, frame_policy) in pointer_path_samples(from, to, path_policy) {
            push_pointer_sample(out, sample, segment, frame_policy);
        }
    }

    fn push_wheel(&mut self, x: f32, y: f32, delta: WheelDelta) {
        let position = Point::new(x, y);
        let time = self.next_time();
        self.pointer_pos = position;
        self.push_raw(RawInputEvent::PointerWheel {
            window_id: self.config.window_id,
            time,
            device: self.config.default_pointer_device,
            delta,
            position,
        });
    }

    fn push_raw(&mut self, raw: RawInputEvent) {
        self.fusion.process_raw(raw, &mut self.fused_events);
    }

    fn next_time(&mut self) -> Instant {
        let time = self.base_time + self.config.tick_step.mul_f64(self.tick as f64);
        self.tick += 1;
        time
    }

    fn advance_tick(&mut self) {
        self.tick += 1;
    }
}

fn validate_interaction_commands<'a, I>(
    commands: I,
    first_primitive_step_index: usize,
) -> InteractionDriverResult<()>
where
    I: IntoIterator<Item = &'a InteractionCommand>,
{
    for (offset, command) in commands.into_iter().enumerate() {
        validate_interaction_command(command, first_primitive_step_index + offset)?;
    }
    Ok(())
}

fn validate_interaction_command(
    command: &InteractionCommand,
    primitive_step_index: usize,
) -> InteractionDriverResult<()> {
    match command {
        InteractionCommand::PointerMove { x, y } => {
            validate_finite_sink_field(command, primitive_step_index, "x", *x)?;
            validate_finite_sink_field(command, primitive_step_index, "y", *y)?;
        }
        InteractionCommand::PointerDown { x, y, .. }
        | InteractionCommand::PointerUp { x, y, .. } => {
            validate_finite_sink_field(command, primitive_step_index, "x", *x)?;
            validate_finite_sink_field(command, primitive_step_index, "y", *y)?;
        }
        InteractionCommand::WheelPixels {
            x,
            y,
            delta_x,
            delta_y,
        }
        | InteractionCommand::WheelLines {
            x,
            y,
            delta_x,
            delta_y,
        } => {
            validate_finite_sink_field(command, primitive_step_index, "x", *x)?;
            validate_finite_sink_field(command, primitive_step_index, "y", *y)?;
            validate_finite_sink_field(command, primitive_step_index, "delta_x", *delta_x)?;
            validate_finite_sink_field(command, primitive_step_index, "delta_y", *delta_y)?;
        }
        InteractionCommand::SetWindowSize { width, height } => {
            validate_finite_sink_field(command, primitive_step_index, "width", *width)?;
            validate_finite_sink_field(command, primitive_step_index, "height", *height)?;
        }
        InteractionCommand::SetScaleFactor { scale_factor } => {
            validate_finite_sink_field(
                command,
                primitive_step_index,
                "scale_factor",
                *scale_factor,
            )?;
        }
        InteractionCommand::KeyDown { .. }
        | InteractionCommand::KeyUp { .. }
        | InteractionCommand::TextInput { .. }
        | InteractionCommand::Frame { .. }
        | InteractionCommand::Reconcile => {}
    }
    Ok(())
}

fn validate_finite_sink_field(
    command: &InteractionCommand,
    primitive_step_index: usize,
    field: &'static str,
    value: f32,
) -> InteractionDriverResult<()> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(InteractionDriverError::NonFiniteSinkValue {
            primitive_step_index,
            command: ReplayInteractionCommand::from(command),
            field,
            value,
        })
    }
}

const FUSION_DRAG_DISTANCE_THRESHOLD: f32 = 4.0;

fn push_pointer_sample(
    out: &mut Vec<IntentExpansionStep>,
    sample: Point,
    segment: PointerPathSegment,
    frame_policy: PointerFramePolicy,
) {
    push_expansion_step(
        out,
        InteractionCommand::PointerMove {
            x: sample.x,
            y: sample.y,
        },
        segment,
        frame_policy,
    );
    push_frame_policy_boundary(out, segment, frame_policy);
}

fn push_keyboard_key(
    out: &mut Vec<IntentExpansionStep>,
    key_code: KeyCode,
    modifiers: Modifiers,
    segment: PointerPathSegment,
) {
    push_expansion_step(
        out,
        InteractionCommand::KeyDown {
            key_code,
            modifiers,
            repeat: false,
        },
        segment,
        PointerFramePolicy::ReconcileSeparated,
    );
    push_expansion_step(
        out,
        InteractionCommand::KeyUp {
            key_code,
            modifiers,
        },
        segment,
        PointerFramePolicy::ReconcileSeparated,
    );
    push_expansion_step(
        out,
        InteractionCommand::Reconcile,
        segment,
        PointerFramePolicy::ReconcileSeparated,
    );
}

fn push_frame_policy_boundary(
    out: &mut Vec<IntentExpansionStep>,
    segment: PointerPathSegment,
    frame_policy: PointerFramePolicy,
) {
    match frame_policy {
        PointerFramePolicy::SameFrameCoalesced => {}
        PointerFramePolicy::FrameSeparated | PointerFramePolicy::ThresholdCrossing => {
            push_expansion_step(out, InteractionCommand::frame(), segment, frame_policy);
        }
        PointerFramePolicy::HoverSettled { frames } => {
            if frames > 0 {
                push_expansion_step(
                    out,
                    InteractionCommand::Frame { count: frames },
                    segment,
                    frame_policy,
                );
            }
        }
        PointerFramePolicy::ReconcileSeparated => {
            push_expansion_step(out, InteractionCommand::Reconcile, segment, frame_policy);
        }
    }
}

fn push_expansion_step(
    out: &mut Vec<IntentExpansionStep>,
    command: InteractionCommand,
    segment: PointerPathSegment,
    frame_policy: PointerFramePolicy,
) {
    let primitive_offset = out.len();
    out.push(IntentExpansionStep {
        primitive_offset,
        command,
        segment,
        frame_policy,
    });
}

fn pointer_path_samples(
    from: Point,
    to: Point,
    path_policy: &PointerPathPolicy,
) -> Vec<(Point, PointerFramePolicy)> {
    match path_policy {
        PointerPathPolicy::Instant => {
            vec![(to, PointerFramePolicy::SameFrameCoalesced)]
        }
        PointerPathPolicy::Linear { steps } => linear_samples(
            from,
            to,
            (*steps).max(1),
            PointerFramePolicy::FrameSeparated,
        ),
        PointerPathPolicy::Jittered {
            steps,
            amplitude,
            seed,
        } => jittered_samples(from, to, (*steps).max(1), *amplitude, *seed),
        PointerPathPolicy::EdgeAware { approach, steps } => {
            edge_aware_samples(from, to, *approach, (*steps).max(2))
        }
        PointerPathPolicy::OvershootCorrect {
            overshoot,
            correction_steps,
        } => overshoot_samples(from, to, *overshoot, (*correction_steps).max(1)),
        PointerPathPolicy::HoverPause {
            steps,
            hover_frames,
        } => {
            let mut samples = linear_samples(
                from,
                to,
                (*steps).max(1),
                PointerFramePolicy::FrameSeparated,
            );
            if let Some(last) = samples.last_mut() {
                last.1 = PointerFramePolicy::HoverSettled {
                    frames: *hover_frames,
                };
            }
            samples
        }
    }
}

fn linear_samples(
    from: Point,
    to: Point,
    steps: u32,
    frame_policy: PointerFramePolicy,
) -> Vec<(Point, PointerFramePolicy)> {
    (1..=steps)
        .map(|step| {
            let t = step as f32 / steps as f32;
            (interpolate_point(from, to, t), frame_policy)
        })
        .collect()
}

fn jittered_samples(
    from: Point,
    to: Point,
    steps: u32,
    amplitude: f32,
    seed: u64,
) -> Vec<(Point, PointerFramePolicy)> {
    (1..=steps)
        .map(|step| {
            let t = step as f32 / steps as f32;
            let mut point = interpolate_point(from, to, t);
            if step != steps {
                let jitter = deterministic_jitter(seed, step);
                point.x += jitter.0 * amplitude;
                point.y += jitter.1 * amplitude;
            }
            (point, PointerFramePolicy::FrameSeparated)
        })
        .collect()
}

fn edge_aware_samples(
    from: Point,
    to: Point,
    approach: PointerApproach,
    steps: u32,
) -> Vec<(Point, PointerFramePolicy)> {
    let corner = match approach {
        PointerApproach::HorizontalFirst => Point::new(to.x, from.y),
        PointerApproach::VerticalFirst => Point::new(from.x, to.y),
    };
    let first_steps = steps / 2;
    let second_steps = steps - first_steps;
    let mut samples = linear_samples(
        from,
        corner,
        first_steps.max(1),
        PointerFramePolicy::FrameSeparated,
    );
    samples.extend(linear_samples(
        corner,
        to,
        second_steps.max(1),
        PointerFramePolicy::FrameSeparated,
    ));
    samples
}

fn overshoot_samples(
    from: Point,
    to: Point,
    overshoot: f32,
    correction_steps: u32,
) -> Vec<(Point, PointerFramePolicy)> {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let length = (dx * dx + dy * dy).sqrt();
    if length == 0.0 {
        return vec![(to, PointerFramePolicy::FrameSeparated)];
    }
    let overshoot_target = Point::new(
        to.x + dx / length * overshoot,
        to.y + dy / length * overshoot,
    );
    let mut samples = linear_samples(
        from,
        overshoot_target,
        correction_steps,
        PointerFramePolicy::FrameSeparated,
    );
    samples.extend(linear_samples(
        overshoot_target,
        to,
        correction_steps,
        PointerFramePolicy::ReconcileSeparated,
    ));
    samples
}

fn interpolate_point(from: Point, to: Point, t: f32) -> Point {
    Point::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t)
}

fn deterministic_jitter(seed: u64, step: u32) -> (f32, f32) {
    let mut state = seed ^ ((step as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    state ^= state >> 30;
    state = state.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    state ^= state >> 27;
    state = state.wrapping_mul(0x94D0_49BB_1331_11EB);
    state ^= state >> 31;

    let x = (((state & 0xffff) as f32) / 32767.5) - 1.0;
    let y = ((((state >> 16) & 0xffff) as f32) / 32767.5) - 1.0;
    (x, y)
}

impl Default for InteractionDriver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_timeout_policy() -> InteractionTimeoutPolicy {
        InteractionTimeoutPolicy {
            per_transaction_timeout_ms: 250,
            run_timeout_ms: 10_000,
            settle_iteration_budget: 8,
            frame_progress_budget: 8,
            no_progress_frame_budget: 4,
            no_progress_settle_budget: 4,
            monotonic_clock_seed: 42,
        }
    }

    fn test_visual_diagnostics_policy() -> InteractionVisualDiagnosticsPolicy {
        InteractionVisualDiagnosticsPolicy {
            capture_enabled: false,
            headless: true,
            rendering_available: false,
        }
    }

    fn test_constitution_boundary_profile() -> InteractionConstitutionBoundaryProfile {
        InteractionConstitutionBoundaryProfile {
            input_fusion_owner: "InputFusion".to_string(),
            event_routing_owner: "EventRouter".to_string(),
            capture_owner: "PointerCapture".to_string(),
            focus_owner: "FocusManager".to_string(),
            layout_owner: "Layout".to_string(),
            scene_build_owner: "SceneGraph".to_string(),
            rendering_owner: "Renderer".to_string(),
            scroll_owner: "Scroll".to_string(),
            identity_owner: "WidgetIdentity".to_string(),
            probe_owner: "interaction_probe".to_string(),
            oracle_owner: "interaction_oracle".to_string(),
            shrinking_owner: "interaction_shrinker".to_string(),
            replay_owner: "interaction_replay".to_string(),
            reporting_owner: "interaction_reporter".to_string(),
            visual_diagnostics_owner: "interaction_visual_diagnostics".to_string(),
        }
    }

    fn test_constitution_boundary_check_status() -> InteractionConstitutionBoundaryCheckStatus {
        InteractionConstitutionBoundaryCheckStatus {
            passed: true,
            failed_checks: Vec::new(),
        }
    }

    fn test_profile() -> InteractionFuzzProfile {
        InteractionFuzzProfile {
            schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
            schema: INTERACTION_FUZZ_SCHEMA.to_string(),
            profile_id: "test-profile".to_string(),
            mode: InteractionFuzzMode::ContractSmoke,
            artifact_dir: "target/interaction-fuzz/test-profile".to_string(),
            injection_path: InteractionInjectionPath::IntentExpansion,
            settle_boundary: InteractionSettleBoundary::Frame,
            fixture_ids: vec!["lv1".to_string()],
            chaos_surface_ids: Vec::new(),
            generator_ids: vec!["pth1".to_string()],
            chaos_stream_generator_ids: Vec::new(),
            generator_versions: vec![InteractionGeneratorVersionRecord {
                generator_id: "pth1".to_string(),
                version: "1".to_string(),
            }],
            chaos_stream_generator_versions: Vec::new(),
            oracle_ids: vec!["oracle-346-u11".to_string()],
            hard_failure_oracle_ids: vec!["oracle-346-u11".to_string()],
            root_seed: 42,
            seed_set: vec![42],
            rng_algorithm: "halcyon-fixed-seed".to_string(),
            rng_version: "1".to_string(),
            command_budget: 64,
            transaction_budget: 8,
            coverage_window: 2,
            timeout_policy: test_timeout_policy(),
            visual_diagnostics_policy: test_visual_diagnostics_policy(),
            constitution_boundary_profile: test_constitution_boundary_profile(),
            disabled_domains: Vec::new(),
            expected_unsupported_domains: Vec::new(),
        }
    }

    #[test]
    fn interaction_fuzz_profile_hash_is_deterministic_and_content_sensitive() {
        let profile = test_profile();
        let same_profile = test_profile();
        let hash = profile.profile_hash();

        assert!(profile.is_complete());
        assert_eq!(hash, same_profile.profile_hash());
        assert_eq!(hash.len(), "sha256:".len() + 64);

        let mut changed_profile = test_profile();
        changed_profile.transaction_budget += 1;
        assert_ne!(hash, changed_profile.profile_hash());
    }

    #[test]
    fn interaction_fuzz_profile_rejects_implicit_timeout_defaults() {
        let mut profile = test_profile();

        profile.timeout_policy.no_progress_frame_budget = 0;

        assert!(!profile.is_complete());
    }

    #[test]
    fn interaction_fuzz_profile_rejects_missing_generator_version_records() {
        let mut profile = test_profile();

        profile.generator_versions.clear();

        assert!(!profile.is_complete());
    }

    #[test]
    fn interaction_fuzz_profile_rejects_wrong_schema_identity() {
        let mut profile = test_profile();

        profile.schema = "halcyon.interaction_fuzz.v999".to_string();

        assert!(!profile.is_complete());
    }

    #[test]
    fn pointer_move_is_coalesced_until_frame() {
        let mut driver = InteractionDriver::new();

        let immediate = driver.execute(InteractionCommand::pointer_move(10.0, 20.0));
        assert!(immediate.is_empty());

        let flushed = driver.execute(InteractionCommand::frame());
        assert_eq!(flushed.len(), 1);
        match &flushed[0] {
            FusedInputEvent::PointerMove {
                pos,
                delta,
                buttons,
                ..
            } => {
                assert_eq!((pos.x, pos.y), (10.0, 20.0));
                assert_eq!(*delta, (10.0, 20.0));
                assert!(!buttons.any_pressed());
            }
            other => panic!("expected PointerMove, got {other:?}"),
        }
    }

    #[test]
    fn pointer_down_flushes_pending_move_before_press() {
        let mut driver = InteractionDriver::new();

        let events = driver.execute_all([
            InteractionCommand::pointer_move(10.0, 20.0),
            InteractionCommand::pointer_down(10.0, 20.0),
        ]);

        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], FusedInputEvent::PointerMove { .. }));
        assert!(matches!(events[1], FusedInputEvent::PointerDown { .. }));
    }

    #[test]
    fn drag_trace_uses_input_fusion_lifecycle() {
        let mut driver = InteractionDriver::new();

        let events = driver.execute_all([
            InteractionCommand::pointer_move(10.0, 10.0),
            InteractionCommand::frame(),
            InteractionCommand::pointer_down(10.0, 10.0),
            InteractionCommand::pointer_move(15.0, 10.0),
            InteractionCommand::pointer_move(25.0, 10.0),
            InteractionCommand::pointer_up(25.0, 10.0),
        ]);

        assert_eq!(events.len(), 7);
        assert!(matches!(events[0], FusedInputEvent::PointerMove { .. }));
        assert!(matches!(events[1], FusedInputEvent::PointerDown { .. }));
        assert!(matches!(events[2], FusedInputEvent::DragStart { .. }));
        assert!(matches!(events[3], FusedInputEvent::DragUpdate { .. }));
        assert!(matches!(events[4], FusedInputEvent::DragUpdate { .. }));
        assert!(matches!(events[5], FusedInputEvent::DragEnd { .. }));
        assert!(matches!(events[6], FusedInputEvent::PointerUp { .. }));

        match &events[2] {
            FusedInputEvent::DragStart { origin, .. } => {
                assert_eq!((origin.x, origin.y), (10.0, 10.0));
            }
            other => panic!("expected DragStart, got {other:?}"),
        }

        match &events[4] {
            FusedInputEvent::DragUpdate {
                origin,
                current,
                delta,
                ..
            } => {
                assert_eq!((origin.x, origin.y), (10.0, 10.0));
                assert_eq!((current.x, current.y), (25.0, 10.0));
                assert_eq!(*delta, (10.0, 0.0));
            }
            other => panic!("expected coalesced DragUpdate, got {other:?}"),
        }
    }

    #[test]
    fn primitive_trace_records_command_steps_and_emitted_events() {
        let mut driver = InteractionDriver::new();

        let immediate = driver.execute(InteractionCommand::pointer_move(10.0, 20.0));
        assert!(immediate.is_empty());

        let flushed = driver.execute(InteractionCommand::frame());
        assert_eq!(flushed.len(), 1);

        let trace = driver.primitive_trace();
        assert_eq!(trace.len(), 2);
        assert_eq!(trace[0].primitive_step_index, 0);
        assert_eq!(trace[1].primitive_step_index, 1);

        match &trace[0].command {
            InteractionCommand::PointerMove { x, y } => {
                assert_eq!((*x, *y), (10.0, 20.0));
            }
            other => panic!("expected PointerMove command, got {other:?}"),
        }
        assert!(trace[0].fused_events.is_empty());

        assert!(matches!(
            trace[1].command,
            InteractionCommand::Frame { count: 1 }
        ));
        assert_eq!(trace[1].fused_events.len(), 1);
        assert!(matches!(
            trace[1].fused_events[0],
            FusedInputEvent::PointerMove { .. }
        ));
    }

    #[test]
    fn primitive_trace_drain_does_not_reset_step_index() {
        let mut driver = InteractionDriver::new();

        driver.execute(InteractionCommand::pointer_down(5.0, 6.0));
        let drained = driver.drain_primitive_trace();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].primitive_step_index, 0);
        assert!(driver.primitive_trace().is_empty());

        driver.execute(InteractionCommand::pointer_up(5.0, 6.0));
        let trace = driver.primitive_trace();
        assert_eq!(trace.len(), 1);
        assert_eq!(trace[0].primitive_step_index, 1);
        assert!(matches!(
            trace[0].command,
            InteractionCommand::PointerUp { .. }
        ));
    }

    #[test]
    fn linear_pointer_intent_records_frame_separated_expansion() {
        let mut driver = InteractionDriver::new();

        let events = driver.execute_intent(InteractionIntentCommand::move_pointer_to(
            30.0,
            0.0,
            PointerPathPolicy::Linear { steps: 3 },
        ));

        assert_eq!(events.len(), 3);
        assert_eq!(driver.primitive_trace().len(), 6);

        let intent_trace = driver.intent_trace();
        assert_eq!(intent_trace.len(), 1);
        assert_eq!(intent_trace[0].intent_index, 0);
        assert_eq!(intent_trace[0].primitive_command_range, 0..6);
        assert_eq!(intent_trace[0].expansion_steps.len(), 6);

        for pair in intent_trace[0].expansion_steps.chunks_exact(2) {
            assert!(matches!(
                pair[0].command,
                InteractionCommand::PointerMove { .. }
            ));
            assert!(matches!(
                pair[1].command,
                InteractionCommand::Frame { count: 1 }
            ));
            assert_eq!(pair[0].segment, PointerPathSegment::Move);
            assert_eq!(pair[0].frame_policy, PointerFramePolicy::FrameSeparated);
        }

        match &events[2] {
            FusedInputEvent::PointerMove { pos, delta, .. } => {
                assert_eq!((pos.x, pos.y), (30.0, 0.0));
                assert_eq!(*delta, (10.0, 0.0));
            }
            other => panic!("expected PointerMove, got {other:?}"),
        }
    }

    #[test]
    fn drag_intent_records_threshold_crossing_segment() {
        let mut driver = InteractionDriver::new();

        driver.execute_intent(InteractionIntentCommand::drag_pointer_to(
            0.0,
            0.0,
            10.0,
            0.0,
            PointerPathPolicy::Linear { steps: 5 },
        ));

        let intent_trace = driver.intent_trace();
        assert_eq!(intent_trace.len(), 1);
        assert!(
            intent_trace[0]
                .expansion_steps
                .iter()
                .any(|step| step.segment == PointerPathSegment::Press
                    && matches!(step.command, InteractionCommand::PointerDown { .. }))
        );
        assert!(intent_trace[0].expansion_steps.iter().any(|step| {
            step.segment == PointerPathSegment::ThresholdCrossing
                && step.frame_policy == PointerFramePolicy::ThresholdCrossing
        }));
        assert!(
            intent_trace[0]
                .expansion_steps
                .iter()
                .any(|step| step.segment == PointerPathSegment::Release
                    && matches!(step.command, InteractionCommand::PointerUp { .. }))
        );
    }

    #[test]
    fn pointer_intent_records_sampled_path_coverage() {
        let mut driver = InteractionDriver::new();

        driver.execute_intent(InteractionIntentCommand::move_pointer_to(
            30.0,
            0.0,
            PointerPathPolicy::Linear { steps: 3 },
        ));

        let coverage = driver.coverage();
        assert!(coverage.contains(&CoverageKey::IntentCommandClass(
            IntentCommandClass::MovePointerTo
        )));
        assert!(coverage.contains(&CoverageKey::PointerPathPolicyClass(
            PointerPathPolicyClass::Linear
        )));
        assert!(coverage.contains(&CoverageKey::PointerPathSampleCountClass(
            PointerPathSampleCountClass::MultiSample
        )));
        assert!(coverage.contains(&CoverageKey::PointerFramePolicy(
            PointerFramePolicy::FrameSeparated
        )));
        assert!(coverage.contains(&CoverageKey::CoalescedMoveClass(
            PointerMoveExposureClass::FrameSeparated
        )));
        assert!(coverage.contains(&CoverageKey::PointerPathSegment(PointerPathSegment::Move)));
        assert!(coverage.contains(&CoverageKey::EndpointTeleportationMode(
            EndpointTeleportationMode::SampledPath
        )));
    }

    #[test]
    fn wheel_at_target_intent_records_scroll_expansion_and_replays() {
        let mut driver = InteractionDriver::new();

        let events = driver.execute_intent(InteractionIntentCommand::wheel_at_target(
            24.0,
            12.0,
            0.0,
            -64.0,
            WheelDeltaMode::Pixels,
            PointerPathPolicy::Linear { steps: 2 },
        ));

        assert_eq!(events.len(), 3);
        assert_eq!(driver.primitive_trace().len(), 6);

        let intent_trace = driver.intent_trace();
        assert_eq!(intent_trace.len(), 1);
        assert_eq!(intent_trace[0].primitive_command_range, 0..6);
        assert_eq!(intent_trace[0].expansion_steps.len(), 6);
        assert_eq!(
            intent_trace[0]
                .expansion_steps
                .iter()
                .filter(|step| step.segment == PointerPathSegment::Wheel)
                .count(),
            2
        );
        assert!(intent_trace[0].expansion_steps.iter().any(|step| {
            step.segment == PointerPathSegment::Wheel
                && matches!(step.command, InteractionCommand::WheelPixels { .. })
        }));

        match events.last().expect("wheel frame must emit scroll") {
            FusedInputEvent::Scroll { pos, delta, .. } => {
                assert_eq!((pos.x, pos.y), (24.0, 12.0));
                assert_eq!(*delta, (0.0, -64.0));
            }
            other => panic!("expected Scroll, got {other:?}"),
        }

        let coverage = driver.coverage();
        assert!(coverage.contains(&CoverageKey::IntentCommandClass(
            IntentCommandClass::WheelAtTarget
        )));
        assert!(coverage.contains(&CoverageKey::PointerPathPolicyClass(
            PointerPathPolicyClass::Linear
        )));
        assert!(coverage.contains(&CoverageKey::PointerPathSegment(PointerPathSegment::Wheel)));
        assert!(coverage.contains(&CoverageKey::PointerFramePolicy(
            PointerFramePolicy::FrameSeparated
        )));

        let artifact = InteractionReplayArtifact::from_driver(
            ReplayMode::ExactIntent,
            "lv1",
            42,
            None,
            &driver,
        );
        let decoded: InteractionReplayArtifact = serde_json::from_str(
            &serde_json::to_string(&artifact).expect("wheel artifact should serialize"),
        )
        .expect("wheel artifact should deserialize");
        assert!(matches!(
            decoded.intent_trace[0].intent_command,
            ReplayInteractionIntentCommand::WheelAtTarget {
                delta_y: -64.0,
                delta_mode: WheelDeltaMode::Pixels,
                ..
            }
        ));

        let mut replay_driver = InteractionDriver::new();
        let replay_events = decoded
            .replay_into(&mut replay_driver)
            .expect("wheel artifact should replay through primitive injection");
        assert_eq!(replay_events.len(), events.len());
        assert!(matches!(
            replay_events.last().expect("replay must emit scroll"),
            FusedInputEvent::Scroll { .. }
        ));
    }

    #[test]
    fn click_target_intent_records_click_expansion_and_replays() {
        let mut driver = InteractionDriver::new();

        let events = driver.execute_intent(InteractionIntentCommand::click_target(
            20.0,
            10.0,
            PointerPathPolicy::Linear { steps: 2 },
        ));

        assert_eq!(events.len(), 4);
        assert_eq!(driver.primitive_trace().len(), 8);

        let intent_trace = driver.intent_trace();
        assert_eq!(intent_trace.len(), 1);
        assert_eq!(intent_trace[0].primitive_command_range, 0..8);
        assert_eq!(
            intent_trace[0].intent_command.command_class(),
            IntentCommandClass::ClickTarget
        );
        assert!(intent_trace[0].expansion_steps.iter().any(|step| {
            step.segment == PointerPathSegment::Press
                && matches!(step.command, InteractionCommand::PointerDown { .. })
        }));
        assert!(intent_trace[0].expansion_steps.iter().any(|step| {
            step.segment == PointerPathSegment::Release
                && matches!(step.command, InteractionCommand::PointerUp { .. })
        }));

        assert!(matches!(events[0], FusedInputEvent::PointerMove { .. }));
        assert!(matches!(events[1], FusedInputEvent::PointerMove { .. }));
        assert!(matches!(events[2], FusedInputEvent::PointerDown { .. }));
        assert!(matches!(events[3], FusedInputEvent::PointerUp { .. }));

        let coverage = driver.coverage();
        assert!(coverage.contains(&CoverageKey::IntentCommandClass(
            IntentCommandClass::ClickTarget
        )));
        assert!(coverage.contains(&CoverageKey::PointerPathSegment(
            PointerPathSegment::Approach
        )));
        assert!(coverage.contains(&CoverageKey::CaptureTransitionClass(
            PointerCaptureTransitionClass::PointerDown
        )));
        assert!(coverage.contains(&CoverageKey::CaptureTransitionClass(
            PointerCaptureTransitionClass::PointerUp
        )));

        let artifact = InteractionReplayArtifact::from_driver(
            ReplayMode::ExactIntent,
            "lv1",
            42,
            None,
            &driver,
        );
        let decoded: InteractionReplayArtifact = serde_json::from_str(
            &serde_json::to_string(&artifact).expect("click artifact should serialize"),
        )
        .expect("click artifact should deserialize");
        assert!(matches!(
            decoded.intent_trace[0].intent_command,
            ReplayInteractionIntentCommand::ClickTarget { .. }
        ));

        let mut replay_driver = InteractionDriver::new();
        let replay_events = decoded
            .replay_into(&mut replay_driver)
            .expect("click artifact should replay through primitive injection");
        assert_eq!(replay_events.len(), events.len());
        assert!(matches!(
            replay_events.last().expect("replay must emit pointer up"),
            FusedInputEvent::PointerUp { .. }
        ));
    }

    #[test]
    fn text_at_target_intent_records_focus_click_text_expansion_and_replays() {
        let mut driver = InteractionDriver::new();

        let events = driver.execute_intent(InteractionIntentCommand::text_at_target(
            32.0,
            14.0,
            "abc",
            Modifiers::default(),
            PointerPathPolicy::Linear { steps: 1 },
        ));

        assert_eq!(events.len(), 4);
        assert_eq!(driver.primitive_trace().len(), 8);

        let intent_trace = driver.intent_trace();
        assert_eq!(intent_trace.len(), 1);
        assert_eq!(intent_trace[0].primitive_command_range, 0..8);
        assert_eq!(
            intent_trace[0].intent_command.command_class(),
            IntentCommandClass::TextAtTarget
        );
        assert!(intent_trace[0].expansion_steps.iter().any(|step| {
            step.segment == PointerPathSegment::TextInput
                && matches!(step.command, InteractionCommand::TextInput { .. })
        }));

        assert!(matches!(events[0], FusedInputEvent::PointerMove { .. }));
        assert!(matches!(events[1], FusedInputEvent::PointerDown { .. }));
        assert!(matches!(events[2], FusedInputEvent::PointerUp { .. }));
        match &events[3] {
            FusedInputEvent::TextInput { text, .. } => assert_eq!(text, "abc"),
            other => panic!("expected TextInput, got {other:?}"),
        }

        let coverage = driver.coverage();
        assert!(coverage.contains(&CoverageKey::IntentCommandClass(
            IntentCommandClass::TextAtTarget
        )));
        assert!(coverage.contains(&CoverageKey::PointerPathSegment(
            PointerPathSegment::TextInput
        )));
        assert!(coverage.contains(&CoverageKey::PointerFramePolicy(
            PointerFramePolicy::FrameSeparated
        )));

        let artifact = InteractionReplayArtifact::from_driver(
            ReplayMode::ExactIntent,
            "lv1",
            42,
            None,
            &driver,
        );
        let decoded: InteractionReplayArtifact = serde_json::from_str(
            &serde_json::to_string(&artifact).expect("text artifact should serialize"),
        )
        .expect("text artifact should deserialize");
        assert!(matches!(
            decoded.intent_trace[0].intent_command,
            ReplayInteractionIntentCommand::TextAtTarget { ref text, .. } if text == "abc"
        ));

        let mut replay_driver = InteractionDriver::new();
        let replay_events = decoded
            .replay_into(&mut replay_driver)
            .expect("text artifact should replay through primitive injection");
        assert_eq!(replay_events.len(), events.len());
        assert!(matches!(
            replay_events.last().expect("replay must emit text input"),
            FusedInputEvent::TextInput { .. }
        ));
    }

    #[test]
    fn instant_pointer_intent_records_endpoint_teleportation_coverage() {
        let mut driver = InteractionDriver::new();

        driver.execute_intent(InteractionIntentCommand::move_pointer_to(
            30.0,
            0.0,
            PointerPathPolicy::Instant,
        ));

        let coverage = driver.coverage();
        assert!(coverage.contains(&CoverageKey::PointerPathPolicyClass(
            PointerPathPolicyClass::Instant
        )));
        assert!(coverage.contains(&CoverageKey::PointerPathSampleCountClass(
            PointerPathSampleCountClass::SingleSample
        )));
        assert!(coverage.contains(&CoverageKey::PointerFramePolicy(
            PointerFramePolicy::SameFrameCoalesced
        )));
        assert!(coverage.contains(&CoverageKey::CoalescedMoveClass(
            PointerMoveExposureClass::SameFrameCoalesced
        )));
        assert!(coverage.contains(&CoverageKey::EndpointTeleportationMode(
            EndpointTeleportationMode::EndpointTeleportation
        )));
    }

    #[test]
    fn drag_intent_records_threshold_crossing_coverage() {
        let mut driver = InteractionDriver::new();

        driver.execute_intent(InteractionIntentCommand::drag_pointer_to(
            0.0,
            0.0,
            10.0,
            0.0,
            PointerPathPolicy::Linear { steps: 5 },
        ));

        let coverage = driver.coverage();
        assert!(coverage.contains(&CoverageKey::IntentCommandClass(
            IntentCommandClass::DragPointerTo
        )));
        assert!(coverage.contains(&CoverageKey::PointerPathSegment(
            PointerPathSegment::ThresholdCrossing
        )));
        assert!(coverage.contains(&CoverageKey::ThresholdCrossing));
        assert!(coverage.contains(&CoverageKey::CaptureTransitionClass(
            PointerCaptureTransitionClass::PointerDown
        )));
        assert!(coverage.contains(&CoverageKey::CaptureTransitionClass(
            PointerCaptureTransitionClass::DragThresholdCrossed
        )));
        assert!(coverage.contains(&CoverageKey::CaptureTransitionClass(
            PointerCaptureTransitionClass::PointerUp
        )));
    }

    #[test]
    fn keyboard_navigation_intent_records_traversal_steps_and_coverage() {
        let mut driver = InteractionDriver::new();

        driver.execute_intent(InteractionIntentCommand::keyboard_navigation(
            vec![KeyCode::Tab, KeyCode::Down],
            Some(KeyCode::Enter),
            Modifiers::default(),
            KeyboardNavigationPolicy::TraversalBeforeActivation,
        ));

        let trace = driver.intent_trace();
        assert_eq!(trace.len(), 1);
        assert_eq!(trace[0].path_policy, None);
        assert_eq!(trace[0].expansion_steps.len(), 9);
        assert_eq!(trace[0].primitive_command_range, 0..9);
        assert_eq!(
            trace[0]
                .expansion_steps
                .iter()
                .filter(|step| matches!(step.command, InteractionCommand::KeyDown { .. }))
                .count(),
            3
        );
        assert_eq!(
            trace[0]
                .expansion_steps
                .iter()
                .filter(|step| matches!(step.command, InteractionCommand::KeyUp { .. }))
                .count(),
            3
        );
        assert_eq!(
            trace[0]
                .expansion_steps
                .iter()
                .filter(|step| matches!(step.command, InteractionCommand::Reconcile))
                .count(),
            3
        );
        assert!(
            trace[0]
                .expansion_steps
                .iter()
                .any(|step| step.segment == PointerPathSegment::KeyboardTraversal)
        );
        assert!(
            trace[0]
                .expansion_steps
                .iter()
                .any(|step| step.segment == PointerPathSegment::KeyboardActivation)
        );

        let coverage = driver.coverage();
        assert!(coverage.contains(&CoverageKey::IntentCommandClass(
            IntentCommandClass::KeyboardNavigation
        )));
        assert!(coverage.contains(&CoverageKey::KeyboardNavigationPolicy(
            KeyboardNavigationPolicy::TraversalBeforeActivation
        )));
        assert!(!coverage.contains(&CoverageKey::EndpointTeleportationMode(
            EndpointTeleportationMode::EndpointTeleportation
        )));

        let metrics = driver.trace_metrics();
        assert_eq!(metrics.intent_count, 1);
        assert_eq!(metrics.primitive_step_count, 9);
        assert_eq!(metrics.pointer_sample_count, 0);
        assert_eq!(metrics.endpoint_teleportation_intent_count, 0);
        assert_eq!(metrics.sampled_path_intent_count, 0);
        assert_eq!(metrics.intent_steps[0].path_policy_class, None);
        assert_eq!(
            metrics.intent_steps[0].keyboard_navigation_policy,
            Some(KeyboardNavigationPolicy::TraversalBeforeActivation)
        );

        let artifact = InteractionReplayArtifact::from_driver(
            ReplayMode::ExactIntent,
            "lv1",
            42,
            None,
            &driver,
        );
        let decoded: InteractionReplayArtifact = serde_json::from_str(
            &serde_json::to_string(&artifact).expect("artifact should serialize"),
        )
        .expect("artifact should deserialize");
        assert_eq!(decoded.intent_trace.len(), 1);
        assert_eq!(decoded.intent_trace[0].path_policy, None);
        assert!(matches!(
            decoded.intent_trace[0].intent_command,
            ReplayInteractionIntentCommand::KeyboardNavigation { .. }
        ));
    }

    #[test]
    fn trace_metrics_record_sampled_pointer_intent_counts() {
        let mut driver = InteractionDriver::new();

        driver.execute_intent(InteractionIntentCommand::move_pointer_to(
            30.0,
            0.0,
            PointerPathPolicy::Linear { steps: 3 },
        ));

        let metrics = driver.trace_metrics();
        assert_eq!(metrics.primitive_step_count, 6);
        assert_eq!(metrics.intent_count, 1);
        assert_eq!(metrics.fused_event_count, 3);
        assert_eq!(metrics.max_emitted_events_per_primitive, 1);
        assert_eq!(metrics.pointer_sample_count, 3);
        assert_eq!(metrics.frame_boundary_count, 3);
        assert_eq!(metrics.threshold_crossing_count, 0);
        assert_eq!(metrics.endpoint_teleportation_intent_count, 0);
        assert_eq!(metrics.sampled_path_intent_count, 1);
        assert_eq!(metrics.primitive_steps.len(), 6);
        assert_eq!(metrics.intent_steps.len(), 1);
        assert_eq!(
            metrics.intent_steps[0].path_policy_class,
            Some(PointerPathPolicyClass::Linear)
        );
    }

    #[test]
    fn trace_metrics_record_endpoint_teleportation_inputs() {
        let mut driver = InteractionDriver::new();

        driver.execute_intent(InteractionIntentCommand::move_pointer_to(
            30.0,
            0.0,
            PointerPathPolicy::Instant,
        ));

        let metrics = driver.trace_metrics();
        assert_eq!(metrics.primitive_step_count, 1);
        assert_eq!(metrics.fused_event_count, 0);
        assert_eq!(metrics.pointer_sample_count, 1);
        assert_eq!(metrics.frame_boundary_count, 0);
        assert_eq!(metrics.endpoint_teleportation_intent_count, 1);
        assert_eq!(metrics.sampled_path_intent_count, 0);
        assert!(metrics.intent_steps[0].endpoint_teleportation);
    }

    #[test]
    fn shrunk_trace_replay_artifact_round_trips_and_replays() {
        let mut driver = InteractionDriver::new();
        driver.execute_intent(InteractionIntentCommand::drag_pointer_to(
            0.0,
            0.0,
            12.0,
            0.0,
            PointerPathPolicy::Linear { steps: 4 },
        ));

        let artifact = InteractionReplayArtifact::from_shrunk_driver("lv1", 42, 7, &driver);
        assert_eq!(artifact.replay_mode, ReplayMode::ShrunkTrace);
        assert_eq!(artifact.shrink_seed, Some(7));
        assert!(artifact.is_supported_schema());

        let json =
            serde_json::to_string(&artifact).expect("shrunk trace artifact should serialize");
        let decoded: InteractionReplayArtifact =
            serde_json::from_str(&json).expect("shrunk trace artifact should deserialize");

        let mut replay_driver = InteractionDriver::new();
        let events = decoded
            .replay_shrunk_trace_into(&mut replay_driver)
            .expect("shrunk trace should replay through primitive injection");

        assert!(!events.is_empty());
        assert_eq!(
            replay_driver.primitive_trace().len(),
            decoded.primitive_trace.len()
        );
        let pointer = replay_driver.pointer_position();
        assert_eq!((pointer.x, pointer.y), (12.0, 0.0));
    }

    #[test]
    fn try_execute_rejects_non_finite_pointer_move_before_injection() {
        let mut driver = InteractionDriver::new();

        let error = driver
            .try_execute(InteractionCommand::pointer_move(f32::NAN, 20.0))
            .expect_err("non-finite pointer move must be rejected");

        match error {
            InteractionDriverError::NonFiniteSinkValue {
                primitive_step_index: 0,
                command: ReplayInteractionCommand::PointerMove { x, y: 20.0 },
                field: "x",
                value,
            } => {
                assert!(x.is_nan());
                assert!(value.is_nan());
            }
            other => panic!("expected non-finite pointer move error, got {other:?}"),
        }
        assert!(driver.primitive_trace().is_empty());
        assert!(driver.fused_events().is_empty());
        let pointer = driver.pointer_position();
        assert_eq!((pointer.x, pointer.y), (0.0, 0.0));
    }

    #[test]
    fn try_execute_all_rejects_invalid_batch_without_partial_execution() {
        let mut driver = InteractionDriver::new();

        let error = driver
            .try_execute_all([
                InteractionCommand::pointer_move(10.0, 20.0),
                InteractionCommand::WheelPixels {
                    x: 10.0,
                    y: 20.0,
                    delta_x: f32::INFINITY,
                    delta_y: 0.0,
                },
            ])
            .expect_err("non-finite wheel delta must reject the whole batch");

        assert_eq!(
            error,
            InteractionDriverError::NonFiniteSinkValue {
                primitive_step_index: 1,
                command: ReplayInteractionCommand::WheelPixels {
                    x: 10.0,
                    y: 20.0,
                    delta_x: f32::INFINITY,
                    delta_y: 0.0,
                },
                field: "delta_x",
                value: f32::INFINITY,
            }
        );
        assert!(driver.primitive_trace().is_empty());
        assert!(driver.fused_events().is_empty());
        let pointer = driver.pointer_position();
        assert_eq!((pointer.x, pointer.y), (0.0, 0.0));
    }

    #[test]
    fn try_execute_intent_rejects_non_finite_expansion_without_mutation() {
        let mut driver = InteractionDriver::new();

        let error = driver
            .try_execute_intent(InteractionIntentCommand::move_pointer_to(
                f32::INFINITY,
                0.0,
                PointerPathPolicy::Linear { steps: 3 },
            ))
            .expect_err("non-finite expanded pointer sample must be rejected");

        assert_eq!(driver.intent_trace().len(), 0);
        assert_eq!(driver.primitive_trace().len(), 0);
        assert!(driver.fused_events().is_empty());
        assert!(matches!(
            error,
            InteractionDriverError::NonFiniteSinkValue {
                primitive_step_index: 0,
                field: "x",
                value,
                ..
            } if value.is_infinite()
        ));
    }

    #[test]
    fn finite_sink_geometry_oracle_passes_finite_samples() {
        let evaluation = evaluate_finite_sink_geometry([
            FiniteSinkGeometrySample::new(
                FiniteGeometrySink::FinalWidgetRect,
                "list_view.rect.x",
                "layout",
                10.0,
            ),
            FiniteSinkGeometrySample::new(
                FiniteGeometrySink::SeparatorGeometry,
                "list_view.columns[0].separator_x",
                "list_view_probe",
                127.0,
            ),
            FiniteSinkGeometrySample::new(
                FiniteGeometrySink::HitTestBounds,
                "list_view.header.hit_bounds.right",
                "hit_test",
                240.0,
            ),
        ]);

        assert!(evaluation.passed());
        assert_eq!(evaluation.oracle_id, ORACLE_346_U1_ID);
        assert_eq!(evaluation.oracle_name, ORACLE_346_U1_NAME);
        assert_eq!(evaluation.status, InteractionOracleStatus::Passed);
        assert_eq!(evaluation.failure_layer, None);
        assert_eq!(
            evaluation.derived_values.get("sample_count"),
            Some(&"3".to_string())
        );
        assert_eq!(
            evaluation.derived_values.get("non_finite_count"),
            Some(&"0".to_string())
        );
    }

    #[test]
    fn finite_sink_geometry_oracle_reports_first_non_finite_sink() {
        let evaluation = evaluate_finite_sink_geometry([
            FiniteSinkGeometrySample::new(
                FiniteGeometrySink::FinalWidgetRect,
                "list_view.rect.x",
                "layout",
                10.0,
            ),
            FiniteSinkGeometrySample::new(
                FiniteGeometrySink::SeparatorGeometry,
                "list_view.columns[0].separator_x",
                "list_view_probe",
                f32::INFINITY,
            ),
            FiniteSinkGeometrySample::new(
                FiniteGeometrySink::HitTestBounds,
                "list_view.header.hit_bounds.right",
                "hit_test",
                f32::NAN,
            ),
        ]);

        assert!(!evaluation.passed());
        assert_eq!(evaluation.status, InteractionOracleStatus::Failed);
        assert_eq!(
            evaluation.failure_layer,
            Some(InteractionFailureLayer::Layout)
        );
        assert_eq!(
            evaluation.failed_sink,
            Some(FiniteGeometrySink::SeparatorGeometry)
        );
        assert_eq!(
            evaluation.failed_field_path,
            Some("list_view.columns[0].separator_x".to_string())
        );
        assert_eq!(
            evaluation.failed_authority,
            Some("list_view_probe".to_string())
        );
        assert_eq!(evaluation.failed_value, Some("inf".to_string()));
        assert_eq!(
            evaluation.derived_values.get("non_finite_count"),
            Some(&"2".to_string())
        );

        let json = serde_json::to_string(&evaluation).expect("oracle evaluation should serialize");
        assert!(!json.contains("Infinity"));
        assert!(!json.contains("NaN"));
        let decoded: InteractionOracleEvaluation =
            serde_json::from_str(&json).expect("oracle evaluation should deserialize");
        assert_eq!(decoded.failed_value, Some("inf".to_string()));
    }

    #[test]
    fn capture_owner_validity_oracle_passes_active_capture_owner() {
        let evaluation = evaluate_capture_owner_validity(CaptureOwnerSnapshot::new(
            true,
            Some("list_view.header.separator.0".to_string()),
            true,
            true,
            true,
            false,
            false,
            "event_router",
        ));

        assert!(evaluation.passed());
        assert_eq!(evaluation.oracle_id, ORACLE_346_U3_ID);
        assert_eq!(evaluation.oracle_name, ORACLE_346_U3_NAME);
        assert_eq!(evaluation.status, InteractionOracleStatus::Passed);
        assert_eq!(evaluation.failure_layer, None);
        assert_eq!(
            evaluation.derived_values.get("capture_exists"),
            Some(&"true".to_string())
        );
        assert_eq!(
            evaluation.derived_values.get("owner_id"),
            Some(&"list_view.header.separator.0".to_string())
        );
    }

    #[test]
    fn capture_owner_validity_oracle_reports_missing_release_path() {
        let evaluation = evaluate_capture_owner_validity(CaptureOwnerSnapshot::new(
            true,
            Some("list_view.header.separator.0".to_string()),
            true,
            true,
            true,
            true,
            false,
            "event_router",
        ));

        assert!(!evaluation.passed());
        assert_eq!(evaluation.status, InteractionOracleStatus::Failed);
        assert_eq!(
            evaluation.failure_layer,
            Some(InteractionFailureLayer::EventRouting)
        );
        assert_eq!(
            evaluation.failed_subject,
            Some("list_view.header.separator.0".to_string())
        );
        assert_eq!(
            evaluation.failed_requirement,
            Some("capture_owner_received_release_or_cancel".to_string())
        );
        assert_eq!(
            evaluation.failed_authority,
            Some("event_router".to_string())
        );

        let json = serde_json::to_string(&evaluation).expect("oracle evaluation should serialize");
        let decoded: InteractionOracleEvaluation =
            serde_json::from_str(&json).expect("oracle evaluation should deserialize");
        assert_eq!(
            decoded.failed_requirement,
            Some("capture_owner_received_release_or_cancel".to_string())
        );
    }

    #[test]
    fn layout_settled_oracle_passes_clean_settle_boundary() {
        let evaluation = evaluate_layout_settled(LayoutSettledSnapshot::new(
            InteractionSettleBoundary::Reconcile,
            false,
            Vec::new(),
            false,
            "layout_engine",
        ));

        assert!(evaluation.passed());
        assert_eq!(evaluation.oracle_id, ORACLE_346_U7_ID);
        assert_eq!(evaluation.oracle_name, ORACLE_346_U7_NAME);
        assert_eq!(evaluation.status, InteractionOracleStatus::Passed);
        assert_eq!(evaluation.failure_layer, None);
        assert_eq!(
            evaluation.derived_values.get("settle_boundary"),
            Some(&"Reconcile".to_string())
        );
        assert_eq!(
            evaluation.derived_values.get("layout_dirty"),
            Some(&"false".to_string())
        );
    }

    #[test]
    fn layout_settled_oracle_allows_declared_continuous_invalidation() {
        let evaluation = evaluate_layout_settled(LayoutSettledSnapshot::new(
            InteractionSettleBoundary::Frame,
            true,
            vec!["animated_overlay".to_string()],
            true,
            "layout_engine",
        ));

        assert!(evaluation.passed());
        assert_eq!(evaluation.status, InteractionOracleStatus::Passed);
        assert_eq!(
            evaluation
                .derived_values
                .get("continuous_layout_invalidation_allowed"),
            Some(&"true".to_string())
        );
        assert_eq!(
            evaluation.derived_values.get("dirty_reasons"),
            Some(&"animated_overlay".to_string())
        );
    }

    #[test]
    fn layout_settled_oracle_reports_dirty_settle_boundary() {
        let evaluation = evaluate_layout_settled(LayoutSettledSnapshot::new(
            InteractionSettleBoundary::Reconcile,
            true,
            vec!["column_resize".to_string(), "column_resize".to_string()],
            false,
            "layout_engine",
        ));

        assert!(!evaluation.passed());
        assert_eq!(evaluation.status, InteractionOracleStatus::Failed);
        assert_eq!(
            evaluation.failure_layer,
            Some(InteractionFailureLayer::Layout)
        );
        assert_eq!(evaluation.failed_subject, Some("Reconcile".to_string()));
        assert_eq!(
            evaluation.failed_requirement,
            Some("layout_clean_at_settle_boundary".to_string())
        );
        assert_eq!(evaluation.failed_value, Some("layout_dirty".to_string()));
        assert_eq!(
            evaluation.derived_values.get("dirty_reasons"),
            Some(&"column_resize".to_string())
        );

        let json = serde_json::to_string(&evaluation).expect("oracle evaluation should serialize");
        let decoded: InteractionOracleEvaluation =
            serde_json::from_str(&json).expect("oracle evaluation should deserialize");
        assert_eq!(
            decoded.failed_requirement,
            Some("layout_clean_at_settle_boundary".to_string())
        );
    }

    #[test]
    fn incidental_navigation_integrity_oracle_passes_valid_incidental_target() {
        let active_intent = InteractionIntentCommand::move_pointer_to(
            100.0,
            20.0,
            PointerPathPolicy::Linear { steps: 4 },
        );
        let universal = evaluate_finite_sink_geometry([FiniteSinkGeometrySample::new(
            FiniteGeometrySink::HitTestBounds,
            "row.3.hit_bounds.left",
            "hit_test",
            16.0,
        )]);

        let evaluation = evaluate_incidental_navigation_integrity(IncidentalNavigationSnapshot {
            active_intent_index: 2,
            primitive_step_index: 7,
            active_intent_command: Some(ReplayInteractionIntentCommand::from(&active_intent)),
            intended_target: InteractionTargetSnapshot {
                target_id: Some("header.separator.0".to_string()),
                target_kind: "separator".to_string(),
                target_class: Some("header_separator".to_string()),
            },
            incidental_target: InteractionTargetSnapshot {
                target_id: Some("row.3.cell.name".to_string()),
                target_kind: "cell".to_string(),
                target_class: Some("body_cell".to_string()),
            },
            path_policy: Some(PointerPathPolicy::Linear { steps: 4 }),
            side_effects_suppressed: false,
            universal_oracle_evaluations: vec![universal],
            authority: "interaction_runner".to_string(),
        });

        assert!(evaluation.passed());
        assert_eq!(evaluation.oracle_id, ORACLE_346_U11_ID);
        assert_eq!(evaluation.oracle_name, ORACLE_346_U11_NAME);
        assert_eq!(
            evaluation.derived_values.get("active_intent_command_class"),
            Some(&"move_pointer_to".to_string())
        );
        assert_eq!(
            evaluation.derived_values.get("incidental_target"),
            Some(&"row.3.cell.name".to_string())
        );
        assert_eq!(
            evaluation
                .derived_values
                .get("universal_oracle_failure_count"),
            Some(&"0".to_string())
        );
    }

    #[test]
    fn incidental_navigation_integrity_oracle_reports_failed_universal_oracle() {
        let failed_universal = evaluate_finite_sink_geometry([FiniteSinkGeometrySample::new(
            FiniteGeometrySink::HitTestBounds,
            "row.3.hit_bounds.left",
            "hit_test",
            f32::NAN,
        )]);

        let evaluation = evaluate_incidental_navigation_integrity(IncidentalNavigationSnapshot {
            active_intent_index: 0,
            primitive_step_index: 4,
            active_intent_command: None,
            intended_target: InteractionTargetSnapshot {
                target_id: Some("header.separator.0".to_string()),
                target_kind: "separator".to_string(),
                target_class: Some("header_separator".to_string()),
            },
            incidental_target: InteractionTargetSnapshot {
                target_id: Some("row.3.cell.name".to_string()),
                target_kind: "cell".to_string(),
                target_class: Some("body_cell".to_string()),
            },
            path_policy: Some(PointerPathPolicy::Jittered {
                steps: 5,
                amplitude: 1.0,
                seed: 9,
            }),
            side_effects_suppressed: false,
            universal_oracle_evaluations: vec![failed_universal],
            authority: "interaction_runner".to_string(),
        });

        assert!(!evaluation.passed());
        assert_eq!(evaluation.status, InteractionOracleStatus::Failed);
        assert_eq!(
            evaluation.failure_layer,
            Some(InteractionFailureLayer::IncidentalInteraction)
        );
        assert_eq!(
            evaluation.failed_subject,
            Some("row.3.cell.name".to_string())
        );
        assert_eq!(
            evaluation.failed_requirement,
            Some("incidental_target_satisfies_universal_oracles".to_string())
        );
        assert_eq!(evaluation.failed_value, Some(ORACLE_346_U1_ID.to_string()));
        assert_eq!(
            evaluation.derived_values.get("failed_universal_oracle_id"),
            Some(&ORACLE_346_U1_ID.to_string())
        );

        let json = serde_json::to_string(&evaluation).expect("oracle evaluation should serialize");
        let decoded: InteractionOracleEvaluation =
            serde_json::from_str(&json).expect("oracle evaluation should deserialize");
        assert_eq!(decoded.failed_value, Some(ORACLE_346_U1_ID.to_string()));
    }

    #[test]
    fn incidental_navigation_integrity_oracle_reports_suppressed_side_effects() {
        let evaluation = evaluate_incidental_navigation_integrity(IncidentalNavigationSnapshot {
            active_intent_index: 1,
            primitive_step_index: 3,
            active_intent_command: None,
            intended_target: InteractionTargetSnapshot {
                target_id: Some("header.separator.0".to_string()),
                target_kind: "separator".to_string(),
                target_class: Some("header_separator".to_string()),
            },
            incidental_target: InteractionTargetSnapshot {
                target_id: Some("row.1.cell.name".to_string()),
                target_kind: "cell".to_string(),
                target_class: Some("body_cell".to_string()),
            },
            path_policy: Some(PointerPathPolicy::EdgeAware {
                approach: PointerApproach::HorizontalFirst,
                steps: 4,
            }),
            side_effects_suppressed: true,
            universal_oracle_evaluations: Vec::new(),
            authority: "interaction_runner".to_string(),
        });

        assert!(!evaluation.passed());
        assert_eq!(
            evaluation.failed_requirement,
            Some("incidental_side_effects_not_suppressed".to_string())
        );
        assert_eq!(
            evaluation.failed_value,
            Some("side_effects_suppressed".to_string())
        );
        assert_eq!(
            evaluation.derived_values.get("side_effects_suppressed"),
            Some(&"true".to_string())
        );
    }

    #[test]
    fn replay_artifact_round_trips_and_replays_primitive_commands() {
        let mut driver = InteractionDriver::new();
        driver.execute_intent(InteractionIntentCommand::move_pointer_to(
            30.0,
            0.0,
            PointerPathPolicy::Linear { steps: 3 },
        ));

        let artifact = InteractionReplayArtifact::from_driver(
            ReplayMode::ExactIntent,
            "lv1",
            42,
            None,
            &driver,
        );
        assert!(artifact.is_supported_schema());
        assert_eq!(artifact.intent_trace.len(), 1);
        assert_eq!(artifact.primitive_trace.len(), 6);

        let json = serde_json::to_string(&artifact).expect("artifact should serialize");
        let decoded: InteractionReplayArtifact =
            serde_json::from_str(&json).expect("artifact should deserialize");

        let mut replay_driver = InteractionDriver::new();
        replay_driver.execute_all(decoded.primitive_commands());

        let pointer = replay_driver.pointer_position();
        assert_eq!((pointer.x, pointer.y), (30.0, 0.0));
        assert_eq!(replay_driver.primitive_trace().len(), 6);
    }

    #[test]
    fn intent_trace_records_click_target_metadata_in_replay_artifact() {
        let mut driver = InteractionDriver::new();
        driver.execute_intent(InteractionIntentCommand::click_target(
            12.5,
            7.25,
            PointerPathPolicy::Linear { steps: 2 },
        ));

        let trace = driver
            .intent_trace()
            .first()
            .expect("intent trace should record click target");
        assert_eq!(trace.target_trace.len(), 1);

        let target = &trace.target_trace[0];
        assert_eq!(target.target_id, "click_target:logical_window:12.500:7.250");
        assert_eq!(target.target_kind, "click_target");
        assert_eq!(target.target_class, Some("click_target".to_string()));
        assert_eq!(target.coordinate_space, "logical_window");
        assert_eq!(
            (target.resolved_point.x, target.resolved_point.y),
            (12.5, 7.25)
        );
        assert_eq!(
            target.source_probe_id,
            "interaction_driver.synthetic_point_target"
        );
        assert_eq!(target.probe_epoch, 0);

        let artifact = InteractionReplayArtifact::from_driver(
            ReplayMode::ExactIntent,
            "lv1",
            42,
            None,
            &driver,
        );
        assert_eq!(artifact.intent_trace[0].target_trace, trace.target_trace);

        let json = serde_json::to_string(&artifact).expect("artifact should serialize");
        assert!(json.contains("\"target_trace\""));
        let decoded: InteractionReplayArtifact =
            serde_json::from_str(&json).expect("artifact should deserialize");
        assert_eq!(
            decoded.intent_trace[0].target_trace[0].target_id,
            "click_target:logical_window:12.500:7.250"
        );
        assert_eq!(
            decoded.intent_trace[0].target_trace[0].source_probe_id,
            "interaction_driver.synthetic_point_target"
        );
    }

    #[test]
    fn replay_intent_command_records_probe_derived_target_metadata() {
        let target = InteractionIntentTarget::resolved_point(
            "lv1.column_separator.index_name",
            "column_separator",
            Some("header_separator".to_string()),
            "logical_window",
            Point::new(99.0, 12.0),
            "list_view_resize_probe",
            17,
        );
        let mut driver = InteractionDriver::new();
        driver.execute_intent(InteractionIntentCommand::click_interaction_target(
            target.clone(),
            PointerPathPolicy::Linear { steps: 2 },
        ));

        match &driver.intent_trace()[0].intent_command {
            InteractionIntentCommand::ClickTarget { target, .. } => {
                assert_eq!(target.target_id, "lv1.column_separator.index_name");
                assert_eq!(target.source_probe_id, "list_view_resize_probe");
                assert_eq!(target.probe_epoch, 17);
            }
            other => panic!("expected click target intent, got {other:?}"),
        }

        let artifact = InteractionReplayArtifact::from_driver(
            ReplayMode::ExactIntent,
            "lv1",
            42,
            None,
            &driver,
        );
        match &artifact.intent_trace[0].intent_command {
            ReplayInteractionIntentCommand::ClickTarget { target, .. } => {
                assert_eq!(target.target_id, "lv1.column_separator.index_name");
                assert_eq!(target.target_kind, "column_separator");
                assert_eq!(target.target_class, Some("header_separator".to_string()));
                assert_eq!(target.coordinate_space, "logical_window");
                assert_eq!(
                    (target.resolved_point.x, target.resolved_point.y),
                    (99.0, 12.0)
                );
                assert_eq!(target.source_probe_id, "list_view_resize_probe");
                assert_eq!(target.probe_epoch, 17);
            }
            other => panic!("expected replay click target intent, got {other:?}"),
        }

        let replay_intent =
            InteractionIntentCommand::from(&artifact.intent_trace[0].intent_command);
        match replay_intent {
            InteractionIntentCommand::ClickTarget { target, .. } => {
                assert_eq!(target.target_id, "lv1.column_separator.index_name");
                assert_eq!(target.source_probe_id, "list_view_resize_probe");
                assert_eq!(target.probe_epoch, 17);
            }
            other => panic!("expected decoded click target intent, got {other:?}"),
        }

        let value = serde_json::to_value(&artifact).expect("artifact should convert to JSON value");
        let command_target = &value["intent_trace"][0]["intent_command"]["target"];
        assert_eq!(
            command_target["target_id"],
            "lv1.column_separator.index_name"
        );
        assert_eq!(command_target["source_probe_id"], "list_view_resize_probe");
        assert!(command_target.get("x").is_none());
        assert_eq!(command_target["resolved_point"]["x"], 99.0);
        assert_eq!(command_target["resolved_point"]["y"], 12.0);
    }

    #[test]
    fn intent_trace_records_drag_endpoint_target_metadata() {
        let mut driver = InteractionDriver::new();
        driver.execute_intent(InteractionIntentCommand::drag_pointer_to(
            1.0,
            2.0,
            11.0,
            12.0,
            PointerPathPolicy::Linear { steps: 4 },
        ));

        let trace = driver
            .intent_trace()
            .first()
            .expect("intent trace should record drag target endpoints");
        assert_eq!(trace.target_trace.len(), 2);

        assert_eq!(
            trace.target_trace[0].target_id,
            "drag_from:logical_window:1.000:2.000"
        );
        assert_eq!(trace.target_trace[0].target_kind, "drag_from");
        assert_eq!(
            trace.target_trace[0].target_class,
            Some("drag_start_target".to_string())
        );
        assert_eq!(
            (
                trace.target_trace[0].resolved_point.x,
                trace.target_trace[0].resolved_point.y,
            ),
            (1.0, 2.0)
        );

        assert_eq!(
            trace.target_trace[1].target_id,
            "drag_to:logical_window:11.000:12.000"
        );
        assert_eq!(trace.target_trace[1].target_kind, "drag_to");
        assert_eq!(
            trace.target_trace[1].target_class,
            Some("drag_end_target".to_string())
        );
        assert_eq!(
            (
                trace.target_trace[1].resolved_point.x,
                trace.target_trace[1].resolved_point.y,
            ),
            (11.0, 12.0)
        );
    }

    #[test]
    fn confidence_packet_records_path_and_incidental_coverage() {
        let mut driver = InteractionDriver::new();
        driver.execute_intent(InteractionIntentCommand::move_pointer_to(
            30.0,
            0.0,
            PointerPathPolicy::Linear { steps: 3 },
        ));
        driver.record_coverage_key(CoverageKey::CrossedHoverTargetClass(
            "body_cell".to_string(),
        ));
        driver.record_coverage_key(CoverageKey::CrossedSeparatorClass(
            "header_separator".to_string(),
        ));
        driver.record_coverage_key(CoverageKey::IncidentalRouteTargetClass(
            "body_cell".to_string(),
        ));

        let coverage_summary = InteractionCoverageSummary::from_coverage(
            driver.coverage(),
            ["header_separator", "header_separator", "body_cell"],
        );
        assert_eq!(
            coverage_summary.path_policy_classes,
            vec![PointerPathPolicyClass::Linear]
        );
        assert_eq!(
            coverage_summary.incidental_target_classes,
            vec!["body_cell".to_string(), "header_separator".to_string()]
        );
        assert_eq!(
            coverage_summary.path_sample_count_classes,
            vec![PointerPathSampleCountClass::MultiSample]
        );
        assert_eq!(
            coverage_summary.coalesced_move_classes,
            vec![PointerMoveExposureClass::FrameSeparated]
        );
        assert_eq!(
            coverage_summary.crossed_hover_target_classes,
            vec!["body_cell".to_string()]
        );
        assert_eq!(
            coverage_summary.crossed_separator_classes,
            vec!["header_separator".to_string()]
        );
        assert_eq!(
            coverage_summary.incidental_route_target_classes,
            vec!["body_cell".to_string()]
        );

        let profile = test_profile();
        let packet = InteractionConfidencePacket::from_input(InteractionConfidencePacketInput {
            run_id: "run-1".to_string(),
            run_profile_id: profile.profile_id.clone(),
            run_profile_hash: profile.profile_hash(),
            halcyon_version: "0.82.0-alpha".to_string(),
            git_revision: "test-revision".to_string(),
            fixture_set: vec!["lv1".to_string()],
            generator_set: vec!["pth1".to_string()],
            oracle_set: vec!["oracle-346-u11".to_string()],
            seed_set: vec![42],
            injection_path: InteractionInjectionPath::IntentExpansion,
            settle_boundary: InteractionSettleBoundary::Frame,
            transaction_count: 1,
            coverage_summary,
            saturation_summary: InteractionSaturationSummary {
                frontier_keys_seen: driver.coverage().total_keys(),
                frontier_keys_required: driver.coverage().total_keys(),
                frontier_saturated: true,
            },
            elapsed_time: InteractionElapsedTime { milliseconds: 12 },
            failure_count: 0,
            skipped_oracles: Vec::new(),
            disabled_generators: Vec::new(),
            out_of_scope_domains: Vec::new(),
            timeout_policy: profile.timeout_policy.clone(),
            visual_diagnostics_status: profile.visual_diagnostics_policy.status(),
            constitution_boundary_profile: profile.constitution_boundary_profile.clone(),
            constitution_boundary_check_status: test_constitution_boundary_check_status(),
        });

        let json = serde_json::to_string(&packet).expect("confidence packet should serialize");
        let decoded: InteractionConfidencePacket =
            serde_json::from_str(&json).expect("confidence packet should deserialize");

        assert_eq!(decoded.failure_count, 0);
        assert_eq!(decoded.run_profile_id, "test-profile");
        assert!(decoded.run_profile_hash.starts_with("sha256:"));
        assert_eq!(decoded.timeout_policy, test_timeout_policy());
        assert!(decoded.visual_diagnostics_status.unavailable);
        assert_eq!(
            decoded.constitution_boundary_profile.input_fusion_owner,
            "InputFusion"
        );
        assert!(decoded.constitution_boundary_check_status.passed);
        assert_eq!(
            decoded.positive_claim,
            INTERACTION_CONFIDENCE_POSITIVE_CLAIM
        );
        assert_eq!(
            decoded.coverage_summary.path_policy_classes,
            vec![PointerPathPolicyClass::Linear]
        );
        assert_eq!(
            decoded.coverage_summary.incidental_target_classes,
            vec!["body_cell".to_string(), "header_separator".to_string()]
        );
    }

    #[test]
    fn failure_packet_schema_records_replayable_intent_and_primitive_traces() {
        let mut driver = InteractionDriver::new();
        driver.execute_intent(InteractionIntentCommand::drag_pointer_to(
            0.0,
            0.0,
            10.0,
            0.0,
            PointerPathPolicy::Linear { steps: 5 },
        ));

        let artifact = InteractionReplayArtifact::from_driver(
            ReplayMode::ExactIntent,
            "lv1",
            42,
            Some(7),
            &driver,
        );
        let mut pre_fields = BTreeMap::new();
        pre_fields.insert("width".to_string(), "100".to_string());
        let mut post_fields = BTreeMap::new();
        post_fields.insert("width".to_string(), "110".to_string());
        let profile = test_profile();

        let packet = InteractionFailurePacket {
            schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
            schema: INTERACTION_FUZZ_SCHEMA.to_string(),
            failure_id: "failure-1".to_string(),
            run_profile_id: profile.profile_id.clone(),
            run_profile_hash: profile.profile_hash(),
            invariant_id: "ORACLE-346-R4".to_string(),
            invariant_name: "No Opposite-Direction First Drag".to_string(),
            severity: InteractionFailureSeverity::P1,
            fixture_id: "lv1".to_string(),
            root_seed: 42,
            shrink_seed: Some(7),
            replay_command: "cargo test interaction_fuzz_replay -- failure-1".to_string(),
            injection_path: InteractionInjectionPath::IntentExpansion,
            settle_boundary: InteractionSettleBoundary::Frame,
            failing_intent_index: Some(0),
            failing_transaction_index: Some(0),
            failing_primitive_step_index: Some(4),
            active_intent_command: artifact
                .intent_trace
                .first()
                .map(|trace| trace.intent_command.clone()),
            command_that_failed: artifact
                .primitive_trace
                .get(4)
                .map(|trace| trace.command.clone()),
            intended_target: Some(InteractionTargetSnapshot {
                target_id: Some("column-0-separator".to_string()),
                target_kind: "separator".to_string(),
                target_class: Some("header_separator".to_string()),
            }),
            resolved_target: Some(InteractionTargetSnapshot {
                target_id: Some("column-0-separator".to_string()),
                target_kind: "separator".to_string(),
                target_class: Some("header_separator".to_string()),
            }),
            incidental_target: None,
            path_policy: Some(PointerPathPolicy::Linear { steps: 5 }),
            intent_trace: artifact.intent_trace.clone(),
            primitive_trace: artifact.primitive_trace.clone(),
            shrunk_trace: artifact.clone(),
            unshrunk_trace_reference: Some("failure-1.unshrunk.json".to_string()),
            pre_state: InteractionStateSnapshot {
                state_id: "pre".to_string(),
                fields: pre_fields,
            },
            post_state: InteractionStateSnapshot {
                state_id: "post".to_string(),
                fields: post_fields.clone(),
            },
            expected_state: InteractionStateSnapshot {
                state_id: "expected".to_string(),
                fields: post_fields.clone(),
            },
            observed_state: InteractionStateSnapshot {
                state_id: "observed".to_string(),
                fields: post_fields,
            },
            probe_snapshots: vec![InteractionProbeSnapshotArtifact {
                source_layer: "widget_probe".to_string(),
                source_owner: "ListViewResizeProbe".to_string(),
                layout_epoch: 1,
                frame_number: 2,
                settle_boundary: InteractionSettleBoundary::Frame,
                snapshot_reason: "oracle_failure_context".to_string(),
                payload: serde_json::json!({
                    "widget_id": 1,
                    "resize_state": "dragging"
                }),
            }],
            layer_classification: InteractionFailureLayer::WidgetState,
            ownership_chain: vec![
                "generator".to_string(),
                "driver".to_string(),
                "oracle".to_string(),
            ],
            coverage_key: Some(CoverageKey::ThresholdCrossing),
            timeout_policy: profile.timeout_policy.clone(),
            progress_marker_history: vec![InteractionProgressMarker {
                command_index: 4,
                frame_index: 2,
                layout_epoch: 1,
                scene_epoch: 1,
                dirty_state_hash: "clean".to_string(),
                deferred_mutation_count: 0,
                capture_owner_class: "separator".to_string(),
                focus_owner_class: "list_view".to_string(),
                routed_target_class: "header_separator".to_string(),
            }],
            visual_diagnostics_status: profile.visual_diagnostics_policy.status(),
            visual_trace_artifacts: vec![InteractionVisualTraceArtifact {
                artifact_kind: "pointer_path_svg".to_string(),
                path: "diagnostics/pointer-path.svg".to_string(),
                detector_id: "ORACLE-346-R4".to_string(),
            }],
            constitution_boundary_profile: profile.constitution_boundary_profile.clone(),
            constitution_boundary_check_status: test_constitution_boundary_check_status(),
            chaos_failure_fields: None,
            related_specs: vec!["SPEC-HAL-346".to_string()],
            promotion_target: Some("src/tests/interaction_fuzz".to_string()),
        };

        let json = serde_json::to_string(&packet).expect("failure packet should serialize");
        let decoded: InteractionFailurePacket =
            serde_json::from_str(&json).expect("failure packet should deserialize");

        assert_eq!(decoded.intent_trace.len(), 1);
        assert!(!decoded.primitive_trace.is_empty());
        assert_eq!(decoded.failing_intent_index, Some(0));
        assert_eq!(decoded.failing_primitive_step_index, Some(4));
        assert_eq!(decoded.coverage_key, Some(CoverageKey::ThresholdCrossing));
        assert_eq!(decoded.run_profile_id, "test-profile");
        assert!(decoded.run_profile_hash.starts_with("sha256:"));
        assert_eq!(decoded.progress_marker_history.len(), 1);
        assert_eq!(decoded.probe_snapshots.len(), 1);
        assert_eq!(decoded.probe_snapshots[0].source_layer, "widget_probe");
        assert_eq!(
            decoded.probe_snapshots[0].source_owner,
            "ListViewResizeProbe"
        );
        assert_eq!(decoded.probe_snapshots[0].layout_epoch, 1);
        assert_eq!(decoded.probe_snapshots[0].frame_number, 2);
        assert_eq!(
            decoded.probe_snapshots[0].settle_boundary,
            InteractionSettleBoundary::Frame
        );
        assert_eq!(
            decoded.probe_snapshots[0].snapshot_reason,
            "oracle_failure_context"
        );
        assert_eq!(decoded.probe_snapshots[0].payload["widget_id"], 1);
        assert_eq!(
            decoded.probe_snapshots[0].payload["resize_state"],
            "dragging"
        );
        assert!(decoded.visual_diagnostics_status.unavailable);
        assert_eq!(
            decoded.visual_diagnostics_status.reason,
            Some("headless".to_string())
        );
        assert_eq!(decoded.visual_diagnostics_status.profile_policy, "headless");
        assert_eq!(decoded.visual_trace_artifacts.len(), 1);
        assert!(decoded.constitution_boundary_check_status.passed);
        assert!(decoded.chaos_failure_fields.is_none());
        assert_eq!(
            decoded.shrunk_trace.primitive_commands().len(),
            decoded.primitive_trace.len()
        );
    }
}
