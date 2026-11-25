pub mod backend_text_engine;
pub mod clipboard;
pub mod font_manager;
pub mod font_manager_text_engine;
pub mod font_resolver;
pub mod menu_host;
pub mod native_font_backend;
#[cfg(target_os = "macos")]
pub mod native_menu_macos;
#[cfg(target_os = "ios")]
pub(crate) mod native_choice_ios;
pub mod relative_pointer;
#[cfg(target_os = "macos")]
pub(crate) mod relative_pointer_macos;
#[cfg(target_os = "macos")]
#[doc(hidden)]
pub mod relative_pointer_macos_conformance;
pub mod system_font_provider;

// Legacy glyphon-based implementations (deprecated, WASM fallback only)
#[cfg(target_arch = "wasm32")]
pub mod font_loader;
#[cfg(target_arch = "wasm32")]
pub mod native_font_backend_glyphon;

#[cfg(windows)]
pub mod native_font_backend_directwrite;

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod native_font_backend_coretext;

#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "ios")),
    not(target_arch = "wasm32")
))]
pub mod native_font_backend_freetype;

pub mod window;
#[cfg(target_arch = "wasm32")]
pub(crate) mod web;
