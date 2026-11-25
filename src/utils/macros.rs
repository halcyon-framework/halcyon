#[macro_export]
macro_rules! replace_widget {
    ($tree:expr, $id:expr, $widget:expr) => {{
        // Replace only the widget, keeping the existing rect.
        // Safe during build() because it doesn't mutate the rect
        // (layout will set the rect later).
        $tree.replace_widget_only($id, Box::new($widget));
    }};
}
