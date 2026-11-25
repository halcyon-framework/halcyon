use halcyon::core::theme::{DensityMode, ListGeometrySpec, SpacingSpec, Theme};

fn assert_spacing(
    spacing: SpacingSpec,
    xxs: f32,
    xs: f32,
    s: f32,
    m: f32,
    l: f32,
    xl: f32,
    x2l: f32,
) {
    assert_eq!(spacing.xxs, xxs);
    assert_eq!(spacing.xs, xs);
    assert_eq!(spacing.s, s);
    assert_eq!(spacing.m, m);
    assert_eq!(spacing.l, l);
    assert_eq!(spacing.xl, xl);
    assert_eq!(spacing.x2l, x2l);
}

#[test]
fn spacing_spec_resolves_density_tokens() {
    assert_spacing(
        SpacingSpec::for_density(DensityMode::Compact),
        2.0,
        4.0,
        6.0,
        7.0,
        9.0,
        11.0,
        16.0,
    );
    assert_spacing(
        SpacingSpec::for_density(DensityMode::Regular),
        2.0,
        4.0,
        6.0,
        8.0,
        10.0,
        12.0,
        16.0,
    );
    assert_spacing(
        SpacingSpec::for_density(DensityMode::Comfortable),
        2.0,
        5.0,
        8.0,
        10.0,
        13.0,
        16.0,
        16.0,
    );
    assert_spacing(
        SpacingSpec::for_density(DensityMode::Intermediate30),
        2.0,
        4.5,
        7.0,
        9.0,
        11.5,
        14.0,
        16.0,
    );
}

#[test]
fn theme_constructors_apply_density_spacing() {
    assert_eq!(Theme::default().geometry.button.height, 28.0);
    assert_eq!(Theme::default().density, DensityMode::Regular);
    assert_eq!(DensityMode::default(), DensityMode::Regular);
    for theme in [Theme::default(), Theme::light(), Theme::dark()] {
        assert_eq!(theme.density, DensityMode::Regular);
        assert_eq!(theme.geometry.button.height, 28.0);
        assert_eq!(theme.geometry.text_field.height, 28.0);
        assert_eq!(theme.geometry.spin_box.height, 28.0);
        let regular = if theme.is_dark() {
            Theme::dark_with_density(DensityMode::Regular)
        } else {
            Theme::light_with_density(DensityMode::Regular)
        };
        assert_eq!(format!("{theme:?}"), format!("{regular:?}"));
    }
    assert!(!Theme::default().is_dark());
    assert!(!Theme::light().is_dark());
    assert!(Theme::dark().is_dark());

    for (density, height, padding_y, field_padding_x, button_padding_x, icon_size, spacing) in [
        (
            DensityMode::Compact,
            24.0,
            2.0,
            3.0,
            6.0,
            14.0,
            [2.0, 4.0, 6.0, 7.0, 9.0, 11.0, 16.0],
        ),
        (
            DensityMode::Regular,
            28.0,
            4.0,
            6.0,
            6.0,
            16.0,
            [2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 16.0],
        ),
        (
            DensityMode::Comfortable,
            32.0,
            6.0,
            8.0,
            16.0,
            18.0,
            [2.0, 5.0, 8.0, 10.0, 13.0, 16.0, 16.0],
        ),
        (
            DensityMode::Intermediate30,
            30.0,
            5.0,
            7.0,
            11.0,
            17.0,
            [2.0, 4.5, 7.0, 9.0, 11.5, 14.0, 16.0],
        ),
    ] {
        for mut theme in [
            Theme::light_with_density(density),
            Theme::dark_with_density(density),
        ] {
            let initial_dark = theme.is_dark();
            for expected_dark in [initial_dark, !initial_dark, initial_dark] {
                assert_eq!(theme.is_dark(), expected_dark);
                assert_eq!(theme.density, density);
                let recipe = if expected_dark {
                    Theme::dark_with_density(density)
                } else {
                    Theme::light_with_density(density)
                };
                assert_eq!(format!("{theme:?}"), format!("{recipe:?}"));
                assert_eq!(theme.typography.font_size, 13.0);
                assert_eq!(theme.typography.input_font_size, 14.0);
                assert_eq!(theme.typography.line_height, 20.0);
                assert_eq!(theme.typography.baseline_offset, -1.0);
                let expected_list = match density {
                    DensityMode::Compact => (20.0, 24.0, 6.0),
                    DensityMode::Regular => (24.0, 28.0, 8.0),
                    DensityMode::Comfortable => (28.0, 32.0, 12.0),
                    DensityMode::Intermediate30 => (26.0, 30.0, 10.0),
                };
                assert_eq!(
                    (
                        theme.list_geom.row_height_min,
                        theme.list_geom.row_height,
                        theme.list_geom.padding_x,
                    ),
                    expected_list
                );
                let button = theme.geometry.button;
                assert_eq!(
                    (
                        button.height,
                        button.padding_x,
                        button.padding_y,
                        button.radius
                    ),
                    (height, button_padding_x, padding_y, 0.0)
                );
                let field = theme.geometry.text_field;
                assert_eq!(
                    (
                        field.height,
                        field.padding_x,
                        field.padding_y,
                        field.radius,
                        field.icon_size,
                        field.min_hit_size
                    ),
                    (height, field_padding_x, padding_y, 2.0, icon_size, height)
                );
                assert_spacing(
                    theme.spacing,
                    spacing[0],
                    spacing[1],
                    spacing[2],
                    spacing[3],
                    spacing[4],
                    spacing[5],
                    spacing[6],
                );
                theme.toggle();
            }
        }
    }

    assert_spacing(
        Theme::dark_with_density(DensityMode::Compact).spacing,
        2.0,
        4.0,
        6.0,
        7.0,
        9.0,
        11.0,
        16.0,
    );
    assert_spacing(
        Theme::light_with_density(DensityMode::Comfortable).spacing,
        2.0,
        5.0,
        8.0,
        10.0,
        13.0,
        16.0,
        16.0,
    );

    let list_default = ListGeometrySpec::default();
    assert_eq!(
        (
            list_default.row_height_min,
            list_default.row_height,
            list_default.padding_x,
        ),
        (20.0, 24.0, 6.0)
    );

    let geometry = Theme::light_with_density(DensityMode::Intermediate30).geometry;
    let regular = Theme::light_with_density(DensityMode::Regular).geometry;
    let comfortable = Theme::light_with_density(DensityMode::Comfortable).geometry;
    macro_rules! assert_midpoints {
        ($($part:ident.$field:ident = $expected:expr),* $(,)?) => {
            $(
                assert_eq!(geometry.$part.$field, $expected, stringify!($part.$field));
                assert_eq!(
                    geometry.$part.$field,
                    (regular.$part.$field + comfortable.$part.$field) * 0.5,
                    stringify!($part.$field)
                );
            )*
        };
    }
    assert_midpoints! {
        text_field.height = 30.0,
        text_field.padding_x = 7.0,
        text_field.padding_y = 5.0,
        text_field.radius = 2.0,
        text_field.icon_size = 17.0,
        text_field.min_hit_size = 30.0,
        button.height = 30.0,
        button.padding_x = 11.0,
        button.padding_y = 5.0,
        button.radius = 0.0,
        checkbox.size = 17.0,
        checkbox.radius = 1.0,
        checkbox.border_thickness = 1.0,
        checkbox.focus_ring_thickness = 2.0,
        checkbox.focus_ring_padding = 2.0,
        checkbox.checkmark_inset = 4.25,
        checkbox.checkmark_thickness = 2.5,
        checkbox.checkmark_leg_ratio = 0.5,
        checkbox.label_gap = 8.0,
        icon.size_small = 15.0,
        icon.size_regular = 17.0,
        icon.size_large = 22.0,
        palette.width = 600.0,
        palette.height = 400.0,
        text_area.min_height = 100.0,
        combo_box.affordance_width = 26.0,
        combo_box.radius = 2.0,
        switch.track_width = 41.0,
        switch.track_height = 20.0,
        switch.thumb_diameter = 16.0,
        switch.thumb_inset = 2.0,
        switch.border_thickness = 1.0,
        switch.label_gap = 8.0,
        slider.track_length = 165.0,
        slider.track_thickness = 5.0,
        slider.thumb_diameter = 16.0,
        slider.border_thickness = 1.0,
        slider.label_gap = 8.0,
        progress_bar.track_length = 165.0,
        progress_bar.track_thickness = 14.0,
        progress_bar.border_thickness = 1.0,
        progress_bar.radius = 2.5,
        progress_bar.label_gap = 8.0,
        busy_indicator.track_length = 165.0,
        busy_indicator.track_thickness = 5.0,
        busy_indicator.border_thickness = 1.0,
        busy_indicator.radius = 2.5,
        busy_indicator.indicator_ratio = 0.3,
        busy_indicator.label_gap = 8.0,
        spin_box.height = 30.0,
        spin_box.value_width = 52.0,
        spin_box.button_width = 26.0,
        spin_box.border_thickness = 1.0,
        spin_box.radius = 2.0,
        spin_box.label_gap = 8.0,
        divider.thickness = 1.0,
        color_picker.sv_size = 210.0,
        color_picker.hue_strip_height = 15.0,
        color_picker.strip_gap = 9.0,
        color_picker.hex_field_width = 92.0,
        color_picker.swatch_height = 26.0,
        color_picker.swatch_width = 38.0,
        color_picker.radius = 2.5,
        color_picker.swatch_radius = 2.5,
        color_picker.popup_padding = 11.0,
        segmented_control.height = 30.0,
        segmented_control.min_segment_width = 60.0,
        segmented_control.padding_x = 11.0,
        segmented_control.radius = 2.5,
        segmented_control.separator_width = 1.0,
        time_picker.column_width = 30.0,
        time_picker.column_height = 30.0,
        time_picker.ampm_width = 30.0,
        time_picker.separator_width = 9.0,
        time_picker.radius = 2.5,
        time_picker.field_height = 30.0,
        time_picker.field_padding_x = 9.0,
        time_picker.popup_padding = 8.0,
        tag_input.chip_height = 23.0,
        tag_input.chip_padding_x = 7.0,
        tag_input.chip_gap = 4.5,
        tag_input.chip_radius = 2.5,
        tag_input.chip_close_size = 13.0,
        tag_input.checkbox_size = 15.0,
        tag_input.checkbox_gap = 7.0,
        tag_input.radius = 2.5,
        track_bar.bar_height = 22.0,
        track_bar.handle_hit_width = 11.0,
        track_bar.handle_visual_width = 3.5,
        track_bar.radius = 2.5,
    }
    assert_eq!(geometry.tag_input.popup_max_visible_rows, 9);
    assert_eq!(
        geometry.tag_input.popup_max_visible_rows * 2,
        regular.tag_input.popup_max_visible_rows + comfortable.tag_input.popup_max_visible_rows
    );
    assert_eq!(geometry.root_inset, 11.0);
    assert_eq!(
        geometry.root_inset,
        (regular.root_inset + comfortable.root_inset) * 0.5
    );
}

#[test]
fn set_density_recomputes_spacing() {
    let mut theme = Theme::dark_with_density(DensityMode::Regular);

    for density in [
        DensityMode::Comfortable,
        DensityMode::Intermediate30,
        DensityMode::Compact,
        DensityMode::Regular,
    ] {
        theme.set_density(density);
        let recipe = Theme::dark_with_density(density);
        assert_eq!(format!("{theme:?}"), format!("{recipe:?}"));
    }
}
