//! The grouped choice example through the ordinary browser App path.

#[cfg(target_arch = "wasm32")]
#[path = "grouped_combo_box.rs"]
mod grouped_combo_box;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn browser_start() {
    grouped_combo_box::run_grouped_combo_box();
}
