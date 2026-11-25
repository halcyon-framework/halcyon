#![cfg_attr(test, allow(deprecated, non_snake_case, unused_imports))]

#[cfg(all(
    target_arch = "wasm32",
    target_os = "unknown",
    target_feature = "atomics"
))]
compile_error!(
    "Halcyon browser transport requires non-atomic WASM; shared-memory WebGPU is not admitted"
);

#[cfg(not(target_arch = "wasm32"))]
pub extern crate winit;
#[cfg(target_arch = "wasm32")]
pub extern crate winit_wasm as winit;

pub mod compositor;
pub mod core;
pub mod editing;
pub mod engines;
pub mod model;
pub mod oracles;
pub mod platform;
pub mod plot;
pub mod renderer;
pub mod runtime;
pub mod semantics;
pub mod three_d;
pub mod utils;
pub mod widgets;

#[cfg(test)]
mod tests;

pub mod prelude {
    pub use crate::core::app::*;
    pub use crate::core::event::*;
    pub use crate::core::layout::*;
    pub use crate::core::math::*;
    pub use crate::core::scene::*;
    pub use crate::core::scheduler::*;
    pub use crate::core::text_engine::*;
    pub use crate::core::text_linebreaker::*;
    pub use crate::core::theme::*;
    pub use crate::core::widget::*;
    pub use crate::core::widget_tree::*;
    pub use crate::platform::font_manager_text_engine::*;
    pub use crate::platform::window::*;
    pub use crate::renderer::wgpu_renderer::*;
    pub use crate::utils::id::*;
    // SPEC-HAL-238: DatePicker
    pub use crate::model::date_value::*;
    pub use crate::widgets::date_picker::*;
    // SPEC-HAL-011 v2.0: Tier-1 Widgets
    pub use crate::widgets::auxiliary_text::*;
    pub use crate::widgets::button::*;
    pub use crate::widgets::container::*;
    pub use crate::widgets::divider::*;
    pub use crate::widgets::fixed_spacer::*;
    pub use crate::widgets::flex::*;
    pub use crate::widgets::flex_spacer::*;
    pub use crate::widgets::flow_grid::*;
    pub use crate::widgets::groupbox::*;
    pub use crate::widgets::label::*;
    pub use crate::widgets::notification_log::*;
    pub use crate::widgets::panel::*;
    pub use crate::widgets::scroll_area::*;
    pub use crate::widgets::section_header::*;
    pub use crate::widgets::text_block::*;
    pub use crate::widgets::text_edit::*;
    pub use crate::widgets::text_field::*;
}
