use crate::core::math::Point;
use crate::core::scene::{Color, SceneBuilder};

/// SPEC-HAL-159: Verify that push_text panics when called from unauthorized location.
///
/// This test runs in ALL builds (release and debug).
/// The test file path (tests/scene_text_ownership.rs) is not in the allowlist, so it should panic.
#[test]
#[should_panic(expected = "SPEC-HAL-159 VIOLATION")]
fn test_push_text_unauthorized_panics() {
    let mut builder = SceneBuilder::new();
    builder.push_text(
        Point::new(0.0, 0.0),
        "test".to_string(),
        12.0,
        Color::WHITE,
        0,
        None,
    );
}

/// SPEC-HAL-159: Verify that push_text_with_backgrounds panics when called from unauthorized location.
///
/// This test runs in ALL builds (release and debug).
#[test]
#[should_panic(expected = "SPEC-HAL-159 VIOLATION")]
fn test_push_text_with_backgrounds_unauthorized_panics() {
    let mut builder = SceneBuilder::new();
    builder.push_text_with_backgrounds(
        Point::new(0.0, 0.0),
        "test".to_string(),
        12.0,
        Color::WHITE,
        vec![],
        0,
        None,
    );
}
