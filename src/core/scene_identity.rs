//! SPEC-HAL-083: Scene Node Identity Types
//!
//! # Identity Contract (FROZEN - SPEC-HAL-083 Phase-2)
//!
//! **This API is structurally frozen. Any extension requires explicit SPEC-HAL-083 Phase-3.**
//!
//! ## Invariants
//!
//! 1. **Identity is WIDGET-OWNED ONLY.** Widgets derive keys from their stable IDs.
//! 2. **SceneBuilder NEVER invents identity.** It only passes through what widgets provide.
//! 3. **`key == None` means ephemeral.** The node is recreated every frame without tracking.
//! 4. **Uniqueness is enforced per frame.** Duplicate keys panic in debug builds.
//!
//! These rules are load-bearing for future diff logic. Do not weaken them.

/// Authored paint identity, optionally qualified by an engine-owned occurrence.
/// Uniqueness is enforced per frame in debug builds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SceneNodeKey {
    leaf: u64,
    occurrence: u64,
    anonymous: bool,
}

impl SceneNodeKey {
    /// Create a key from a u64 value.
    pub const fn new(value: u64) -> Self {
        Self {
            leaf: value,
            occurrence: 0,
            anonymous: false,
        }
    }

    pub(crate) fn for_occurrence(self, occurrence: u64) -> Self {
        debug_assert_ne!(occurrence, 0);
        Self { occurrence, ..self }
    }

    pub(crate) fn anonymous_occurrence(occurrence: u64, ordinal: u64) -> Self {
        debug_assert_ne!(occurrence, 0);
        Self {
            leaf: ordinal,
            occurrence,
            anonymous: true,
        }
    }
}

impl From<u64> for SceneNodeKey {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

/// Classification of scene node types for future diffing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SceneNodeKind {
    Rect = 1,
    BorderRect = 2,
    RoundedRect = 3,
    Text = 4,
    Image = 5,
    Clip = 6,
    Layer = 7,
    Scroll = 8,
    Circle = 9,
    Ellipse = 10,
    Line = 11,
    Polyline = 12,
    Path = 13,
    ScrollbarV = 14,
    ScrollbarH = 15,
    Shadow = 16,
    GradientRect = 17,
    ColorPickerSv = 18,
    RadialGauge = 19,
    /// SPEC-HAL-354: Engine-owned retained fragment boundary.
    RetainedFragment = 20,
    /// SPEC-HAL-358: Explicit luminous line/polyline contour.
    LuminousContour = 21,
    /// HAL-388: finite local ambient glow.
    LocalAmbientGlow = 22,
    /// HAL386/389: checked direct analytic face.
    AnalyticSurface = 23,
    FrostedGlass = 24,
}
