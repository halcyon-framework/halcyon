//! Model layer for UI-agnostic data structures and controllers.

pub mod order_book;
pub mod tabular;

pub mod breadcrumbs;
pub mod canvas;
pub mod collapsible_section;
pub mod context_menu;
pub mod date_time;
pub mod date_value;
pub mod default_selection_resolver;
pub mod dialog;
pub mod dropdown_menu;
pub mod form_binder;
pub mod menu_item;
pub mod menu_model;
pub mod menu_surface;
pub mod navigation_host;
pub mod node_editor;
pub mod node_graph;
pub mod node_graph_projection;
pub mod notification;
pub mod popup_menu;
pub mod search_projection_harness;
pub mod selection_resolver;
pub mod spatial;
pub mod tab_bar;
pub mod time;
pub mod time_value;
pub mod tooltip;
pub mod tree;
pub mod tree_row_provider;
pub mod tree_substrate;
pub mod tree_types;
pub mod tutorial;

pub mod signal_store;
pub mod toast;
pub use toast::{DismissState, ToastAction, ToastHandle, ToastId, ToastManager};
