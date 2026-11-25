//! SPEC-HAL-346 deterministic interaction fixture registry.

use super::list_view_resize::{
    LIST_VIEW_RESIZE_LV2_INITIAL_HORIZONTAL_SCROLL, LIST_VIEW_RESIZE_ROW_COUNT,
    LIST_VIEW_RESIZE_WINDOW_WIDTH, ListViewResizeFixture,
};
use super::tree_view_projection::{
    TREE_VIEW_PROJECTION_ROW_COUNT, TREE_VIEW_PROJECTION_WINDOW_WIDTH, TreeViewProjectionFixture,
    tree_view_projection_fixture,
};

pub const LIST_VIEW_RESIZE_LV1_FIXTURE_ID: &str = "lv1";
pub const LIST_VIEW_RESIZE_LV2_FIXTURE_ID: &str = "lv2";
pub const TREE_VIEW_PROJECTION_TV1_FIXTURE_ID: &str = "tv1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionFixtureKind {
    ListViewResize,
    TreeViewProjection,
}

#[derive(Debug, Clone, Copy)]
pub struct InteractionFixtureDefinition {
    pub fixture_id: &'static str,
    pub kind: InteractionFixtureKind,
    pub row_count: usize,
    pub horizontal_scroll: bool,
    pub initial_horizontal_scroll: f32,
    builder: fn() -> InteractionFixtureInstance,
}

impl InteractionFixtureDefinition {
    pub fn build(self) -> InteractionFixtureInstance {
        (self.builder)()
    }
}

pub enum InteractionFixtureInstance {
    ListViewResize(ListViewResizeFixture),
    TreeViewProjection(TreeViewProjectionFixture),
}

pub const INTERACTION_FIXTURE_REGISTRY: [InteractionFixtureDefinition; 3] = [
    InteractionFixtureDefinition {
        fixture_id: LIST_VIEW_RESIZE_LV1_FIXTURE_ID,
        kind: InteractionFixtureKind::ListViewResize,
        row_count: 3,
        horizontal_scroll: true,
        initial_horizontal_scroll: 0.0,
        builder: build_lv1,
    },
    InteractionFixtureDefinition {
        fixture_id: LIST_VIEW_RESIZE_LV2_FIXTURE_ID,
        kind: InteractionFixtureKind::ListViewResize,
        row_count: LIST_VIEW_RESIZE_ROW_COUNT,
        horizontal_scroll: true,
        initial_horizontal_scroll: LIST_VIEW_RESIZE_LV2_INITIAL_HORIZONTAL_SCROLL,
        builder: build_lv2,
    },
    InteractionFixtureDefinition {
        fixture_id: TREE_VIEW_PROJECTION_TV1_FIXTURE_ID,
        kind: InteractionFixtureKind::TreeViewProjection,
        row_count: TREE_VIEW_PROJECTION_ROW_COUNT,
        horizontal_scroll: true,
        initial_horizontal_scroll: 0.0,
        builder: build_tv1,
    },
];

pub fn interaction_fixture_registry() -> &'static [InteractionFixtureDefinition] {
    &INTERACTION_FIXTURE_REGISTRY
}

pub fn find_interaction_fixture(fixture_id: &str) -> Option<InteractionFixtureDefinition> {
    INTERACTION_FIXTURE_REGISTRY
        .iter()
        .copied()
        .find(|definition| definition.fixture_id == fixture_id)
}

fn build_lv1() -> InteractionFixtureInstance {
    InteractionFixtureInstance::ListViewResize(ListViewResizeFixture::with_row_count(3))
}

fn build_lv2() -> InteractionFixtureInstance {
    let fixture = ListViewResizeFixture::new();
    fixture
        .horizontal_scroll_link
        .set(LIST_VIEW_RESIZE_LV2_INITIAL_HORIZONTAL_SCROLL);
    InteractionFixtureInstance::ListViewResize(fixture)
}

fn build_tv1() -> InteractionFixtureInstance {
    InteractionFixtureInstance::TreeViewProjection(tree_view_projection_fixture())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::tabular_view::DataSource;

    #[test]
    fn fixture_registry_order_is_deterministic() {
        let fixture_ids: Vec<&str> = interaction_fixture_registry()
            .iter()
            .map(|definition| definition.fixture_id)
            .collect();

        assert_eq!(
            fixture_ids,
            vec![
                LIST_VIEW_RESIZE_LV1_FIXTURE_ID,
                LIST_VIEW_RESIZE_LV2_FIXTURE_ID,
                TREE_VIEW_PROJECTION_TV1_FIXTURE_ID,
            ]
        );
    }

    #[test]
    fn fixture_registry_builds_registered_fixtures() {
        let lv1 = find_interaction_fixture(LIST_VIEW_RESIZE_LV1_FIXTURE_ID)
            .expect("lv1 fixture must be registered");
        let lv2 = find_interaction_fixture(LIST_VIEW_RESIZE_LV2_FIXTURE_ID)
            .expect("lv2 fixture must be registered");
        let tv1 = find_interaction_fixture(TREE_VIEW_PROJECTION_TV1_FIXTURE_ID)
            .expect("tv1 fixture must be registered");

        assert_eq!(lv1.kind, InteractionFixtureKind::ListViewResize);
        assert_eq!(lv1.row_count, 3);
        assert!(lv1.horizontal_scroll);
        assert_eq!(lv1.initial_horizontal_scroll, 0.0);
        assert_eq!(lv2.row_count, LIST_VIEW_RESIZE_ROW_COUNT);
        assert!(lv2.horizontal_scroll);
        assert_eq!(
            lv2.initial_horizontal_scroll,
            LIST_VIEW_RESIZE_LV2_INITIAL_HORIZONTAL_SCROLL
        );
        assert_eq!(tv1.kind, InteractionFixtureKind::TreeViewProjection);
        assert_eq!(tv1.row_count, TREE_VIEW_PROJECTION_ROW_COUNT);
        assert!(tv1.horizontal_scroll);
        assert_eq!(tv1.initial_horizontal_scroll, 0.0);

        let mut lv1_fixture = match lv1.build() {
            InteractionFixtureInstance::ListViewResize(fixture) => fixture,
            InteractionFixtureInstance::TreeViewProjection(_) => {
                panic!("lv1 must build ListViewResize fixture")
            }
        };
        let mut lv2_fixture = match lv2.build() {
            InteractionFixtureInstance::ListViewResize(fixture) => fixture,
            InteractionFixtureInstance::TreeViewProjection(_) => {
                panic!("lv2 must build ListViewResize fixture")
            }
        };
        let tv1_fixture = match tv1.build() {
            InteractionFixtureInstance::TreeViewProjection(fixture) => fixture,
            InteractionFixtureInstance::ListViewResize(_) => {
                panic!("tv1 must build TreeViewProjection fixture")
            }
        };

        assert_eq!(lv1_fixture.window_width, LIST_VIEW_RESIZE_WINDOW_WIDTH);
        assert_eq!(lv2_fixture.window_width, LIST_VIEW_RESIZE_WINDOW_WIDTH);
        assert_eq!(tv1_fixture.window_width, TREE_VIEW_PROJECTION_WINDOW_WIDTH);
        assert_eq!(lv1_fixture.horizontal_scroll_link.get(), 0.0);
        assert_eq!(
            lv2_fixture.horizontal_scroll_link.get(),
            LIST_VIEW_RESIZE_LV2_INITIAL_HORIZONTAL_SCROLL
        );
        assert_eq!(
            lv1_fixture
                .list_view
                .adapter_mut()
                .source_mut()
                .row_ids()
                .len(),
            3
        );
        assert_eq!(
            lv2_fixture
                .list_view
                .adapter_mut()
                .source_mut()
                .row_ids()
                .len(),
            LIST_VIEW_RESIZE_ROW_COUNT
        );
        tv1_fixture
            .tree_view
            .hts_mut()
            .set_viewport_height(tv1_fixture.window_height);
        assert_eq!(
            tv1_fixture.tree_view.hts_mut().visible_rows().len(),
            TREE_VIEW_PROJECTION_ROW_COUNT
        );
    }
}
