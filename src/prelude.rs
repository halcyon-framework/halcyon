pub use crate::core::app::*;
pub use crate::core::event::UiInteractionPolicy;
pub use crate::core::layout::*;
pub use crate::core::scene::*;
// Paint types for widget APIs (Color, RoundedCorners, etc.)
// Must come after scene::* to shadow scene::Color with paint::Color
pub use crate::core::paint::{Color, RoundedCorners, LAYER_CONTENT, LAYER_FOCUS, LAYER_OVERLAY, LAYER_SCROLLBAR};
// Text placement invariants: canonical helpers for text Y positioning
pub use crate::core::text_engine::{text_top_y_centered, text_top_y_in_rect};
pub use crate::core::widget::*;
pub use crate::widgets::*;

// Export macros.
// Because macros defined with #[macro_export] live in the crate root namespace,
// we re-export the module to give users a discoverable path.
pub use crate::core::macros::*;
