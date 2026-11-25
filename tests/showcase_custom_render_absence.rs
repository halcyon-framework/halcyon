#![allow(dead_code)]

#[path = "../examples/showcase/main.rs"]
mod showcase;
#[path = "../examples/showcase/tip.rs"]
mod tip;

#[test]
fn showcase_default_2d_section_defers_custom_render_admission() {
    assert!(showcase::default_2d_section_defers_custom_render_for_test());
}
