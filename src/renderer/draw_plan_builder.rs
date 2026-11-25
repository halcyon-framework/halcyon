//! SPEC-HAL-084: DrawPlanBuilder — Build Phase Entry Point
//! SPEC-HAL-163: Extended for surface-scoped scene partitioning.
//!
//! This module owns the Build phase, producing DrawPlan from per-surface SceneGraphs.
//! It completely isolates scene consumption from the renderer.
//!
//! # Architectural Role
//!
//! ```text
//! HashMap<SurfaceId, SceneGraph>  ← SPEC-HAL-163: per-surface scenes
//!     ↓
//! DrawPlanBuilder::build_multi()  ← This module
//!     ↓
//! DrawPlan (CPU-only, no GPU, per-surface data)
//!     ↓
//! WgpuRenderer::compile_multi() + render_multi()
//! ```
//!
//! # Invariants
//!
//! - INV-RG-002: DrawPlanBuilder is the ONLY consumer of SceneGraph
//! - INV-SCENE-001: SceneGraphs are surface-scoped
//! - INV-SCENE-002: No cross-surface scene or derived-resource dependency
//! - WgpuRenderer never imports or sees SceneGraph
//! - DrawPlan can be constructed without GPU initialization
//! - Batcher is owned here, not by renderer

use crate::compositor::{
    Contract, MultiSurfaceContract, SceneKey, SurfaceId, SurfaceStackSnapshot,
};
use crate::core::scene::SceneGraph;
use crate::renderer::batcher::Batcher;
use crate::renderer::custom_render_prepare::ImageOccurrenceRecorder;
use crate::renderer::draw_plan::{DrawPlan, SurfaceSceneData};
use crate::renderer::mesh_cache::MeshCache;
use std::collections::HashMap;
use std::marker::PhantomData;

/// HAL-388: final composed-scene admission, before any GPU Compile/acquisition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrostedGlassBuildRefusal {
    InvalidClip {depth:usize},
    UnsupportedClip {depth:usize},
    InvalidGeometry,
    AmbiguousOrder,
    OccurrenceLimit {limit:usize,requested:usize},
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrawPlanBuildError {
    Bloom {
        surface_id: SurfaceId,
        node_key: Option<crate::core::scene_identity::SceneNodeKey>,
        reason: crate::renderer::BloomBuildRefusal,
    },
    FrostedGlass {
        surface_id:SurfaceId,
        node_key:Option<crate::core::scene_identity::SceneNodeKey>,
        reason:FrostedGlassBuildRefusal,
    },
    UnsupportedAnalyticSurfaceClip {
        surface_id: SurfaceId,
        node_key: Option<crate::core::scene_identity::SceneNodeKey>,
        rounded_clip_depth: usize,
    },
    InvalidAnalyticSurfaceClip {
        surface_id: SurfaceId,
        node_key: Option<crate::core::scene_identity::SceneNodeKey>,
        rounded_clip_depth: usize,
    },
    InvalidAnalyticSurfaceTarget {
        surface_id: SurfaceId,
        node_key: Option<crate::core::scene_identity::SceneNodeKey>,
    },
    AnalyticSurfaceCapacityExceeded {
        surface_id: SurfaceId,
        limit: usize,
        requested: usize,
    },
    UnsupportedLocalAmbientGlowClip {
        surface_id: SurfaceId,
        node_key: Option<crate::core::scene_identity::SceneNodeKey>,
        rounded_clip_depth: usize,
    },
}
impl std::fmt::Display for DrawPlanBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DrawPlan Build refused: {self:?}")
    }
}
impl std::error::Error for DrawPlanBuildError {}

/// SPEC-HAL-084: Build phase entry point.
///
/// Owns the Batcher and produces DrawPlan from SceneGraph.
/// This is the sole consumer of scene data in the render pipeline.
///
/// # Contract Parameterization (SPEC-HAL-081B)
///
/// The `P` type parameter encodes the compositor contract at the type level.
/// Use `build_multi()` for all rendering — one window is simply N=1 targets.
pub struct DrawPlanBuilder<P: Contract = MultiSurfaceContract> {
    batcher: Batcher,
    _phase: PhantomData<P>,
}

impl<P: Contract> DrawPlanBuilder<P> {
    /// Create a new DrawPlanBuilder.
    pub fn new() -> Self {
        Self {
            batcher: Batcher::new(),
            _phase: PhantomData,
        }
    }
}

// ============================================================================
// SPEC-HAL-081B Multi-Surface Contract APIs (Production Path)
// ============================================================================

impl DrawPlanBuilder<MultiSurfaceContract> {
    /// Build a DrawPlan for multi-surface rendering (SPEC-HAL-081B + SPEC-HAL-163).
    ///
    /// # SPEC-HAL-163: Surface-Scoped Scene Partitioning
    ///
    /// Each surface has its own SceneGraph. This enforces:
    /// - INV-SCENE-001: SceneGraphs are surface-scoped
    /// - INV-SCENE-002: No cross-surface scene or derived-resource dependency
    /// - INV-SCENE-004: Renderer never sees a mixed scene
    ///
    /// # Arguments
    ///
    /// * `surface_scenes` - Per-surface SceneGraphs (INV-SCENE-001)
    /// * `surface_stack` - Compositor surface stack with one or more surfaces
    ///
    pub fn build_multi<F>(
        &mut self,
        surface_scenes: &HashMap<SurfaceId, SceneGraph>,
        surface_stack: SurfaceStackSnapshot<MultiSurfaceContract>,
        mesh_cache: &mut MeshCache,
        scale_factor_for_surface: F,
        current_frame: u64,
    ) -> Result<DrawPlan<MultiSurfaceContract>, DrawPlanBuildError>
    where
        F: Fn(SurfaceId) -> f32,
    {
        self.build_multi_impl(
            surface_scenes,
            surface_stack,
            mesh_cache,
            None,
            scale_factor_for_surface,
            current_frame,
        )
    }

    pub(crate) fn build_multi_with_presentation_bindings<F>(
        &mut self,
        surface_scenes: &HashMap<SurfaceId, SceneGraph>,
        surface_stack: SurfaceStackSnapshot<MultiSurfaceContract>,
        mesh_cache: &mut MeshCache,
        recorder: &mut ImageOccurrenceRecorder<'_>,
        scale_factor_for_surface: F,
        current_frame: u64,
    ) -> Result<DrawPlan<MultiSurfaceContract>, DrawPlanBuildError>
    where
        F: Fn(SurfaceId) -> f32,
    {
        self.build_multi_impl(
            surface_scenes,
            surface_stack,
            mesh_cache,
            Some(recorder),
            scale_factor_for_surface,
            current_frame,
        )
    }

    fn build_multi_impl<F>(
        &mut self,
        surface_scenes: &HashMap<SurfaceId, SceneGraph>,
        surface_stack: SurfaceStackSnapshot<MultiSurfaceContract>,
        mesh_cache: &mut MeshCache,
        mut recorder: Option<&mut ImageOccurrenceRecorder<'_>>,
        scale_factor_for_surface: F,
        current_frame: u64,
    ) -> Result<DrawPlan<MultiSurfaceContract>, DrawPlanBuildError>
    where
        F: Fn(SurfaceId) -> f32,
    {
        let mut surface_data = HashMap::new();
        let mut analytic_count = 0usize;
        let mut frost_count=0usize;

        // SPEC-HAL-163: Process each surface's scene independently
        // INV-SCENE-001: Each surface has exactly one SceneGraph
        // INV-SCENE-002: No cross-surface dependency
        for surface_desc in surface_stack.iter() {
            if !surface_desc.is_visible() {
                continue;
            }

            let surface_id = surface_desc.id;
            let scene_key = SceneKey::try_new(surface_desc.scene_key)
                .expect("SPEC-HAL-081B violation: compositor provided scene_key = 0");

            // Get the scene for this surface, or use empty scene
            // INV-SCENE-005: Empty surfaces are valid
            let scene = surface_scenes.get(&surface_id);

            // SPEC-HAL-163: Debug validation for missing surface scenes
            // A visible surface with no scene entry is legal (INV-SCENE-005) but suspicious.
            // This helps catch bugs where scene setup is incomplete.
            #[cfg(debug_assertions)]
            if scene.is_none() {
                tracing::debug!(
                    surface_id = ?surface_id.raw(),
                    "SPEC-HAL-163: Visible surface has no scene entry (INV-SCENE-005 allows this, but may indicate incomplete setup)"
                );
            }

            if let Some(scene) = scene {
                let scale_factor = scale_factor_for_surface(surface_id);
                // Process this surface's scene
                self.batcher.begin_frame(scene_key);
                if let Some(recorder) = recorder.as_deref_mut() {
                    self.batcher.flatten_scene_for_surface(
                        &scene.root,
                        mesh_cache,
                        scale_factor,
                        current_frame,
                        surface_id,
                        recorder,
                    );
                } else {
                    self.batcher.flatten_scene(
                        &scene.root,
                        mesh_cache,
                        scale_factor,
                        current_frame,
                    );
                }
                let bloom = self.batcher.bloom_admission(surface_id, scale_factor).map_err(|(node_key, reason)| {
                    DrawPlanBuildError::Bloom { surface_id, node_key, reason }
                })?;
                if let Some(refusal) = self.batcher.local_ambient_glow_refusal() {
                    return Err(DrawPlanBuildError::UnsupportedLocalAmbientGlowClip {
                        surface_id,
                        node_key: refusal.node_key,
                        rounded_clip_depth: refusal.rounded_clip_depth,
                    });
                }
                if let Some(refusal) = self.batcher.analytic_surface_refusal() {
                    use crate::renderer::batcher::AnalyticSurfaceRefusal;
                    return Err(match refusal {
                        AnalyticSurfaceRefusal::UnsupportedClip { node_key, depth } => {
                            DrawPlanBuildError::UnsupportedAnalyticSurfaceClip {
                                surface_id,
                                node_key,
                                rounded_clip_depth: depth,
                            }
                        }
                        AnalyticSurfaceRefusal::InvalidClip { node_key, depth } => {
                            DrawPlanBuildError::InvalidAnalyticSurfaceClip {
                                surface_id,
                                node_key,
                                rounded_clip_depth: depth,
                            }
                        }
                        AnalyticSurfaceRefusal::InvalidTarget { node_key } => {
                            DrawPlanBuildError::InvalidAnalyticSurfaceTarget {
                                surface_id,
                                node_key,
                            }
                        }
                    });
                }
                analytic_count += self.batcher.analytic_surface_count();
                if analytic_count > 512 {
                    return Err(DrawPlanBuildError::AnalyticSurfaceCapacityExceeded {
                        surface_id,
                        limit: 512,
                        requested: analytic_count,
                    });
                }
                frost_count+=self.batcher.frosted_glass_admission().map_err(|(node_key,reason)|
                    DrawPlanBuildError::FrostedGlass {surface_id,node_key,reason})?;
                if frost_count>16 {
                    return Err(DrawPlanBuildError::FrostedGlass {surface_id,node_key:None,
                        reason:FrostedGlassBuildRefusal::OccurrenceLimit{limit:16,requested:frost_count}});
                }
                let frame_batches = self.batcher.build_batches();

                let primitives = self.batcher.primitives().to_vec();

                surface_data.insert(
                    surface_id,
                    SurfaceSceneData {
                        bloom,
                        primitives,
                        path_draws: frame_batches.paths.clone(),
                        path_clips: frame_batches.path_clips.clone(),
                        batches: frame_batches.batches.clone(),
                        total_vertices: frame_batches.total_vertices,
                        total_indices: frame_batches.total_indices,
                        total_image_vertices: frame_batches.total_image_vertices,
                        total_image_indices: frame_batches.total_image_indices,
                        clear_color: scene.clear_color,
                    },
                );
            } else {
                // INV-SCENE-005: Empty surface is valid, just use default clear color
                surface_data.insert(
                    surface_id,
                    SurfaceSceneData {
                        bloom: None,
                        primitives: Vec::new(),
                        path_draws: Vec::new(),
                        path_clips: Vec::new(),
                        batches: Vec::new(),
                        total_vertices: 0,
                        total_indices: 0,
                        total_image_vertices: 0,
                        total_image_indices: 0,
                        clear_color: crate::core::scene::Color::BLACK,
                    },
                );
            }
        }

        Ok(DrawPlan {
            surface_data,
            surface_stack,
        })
    }
}

impl<P: Contract> Default for DrawPlanBuilder<P> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod analytic_surface_tests {
    use super::*;
    use crate::compositor::Compositor;
    use crate::core::math::RectF;
    use crate::core::paint::{RoundedCorners as PaintCorners, analytic_surface_tests::fixture};
    use crate::core::scene::{RoundedCorners, SceneBuilder, SceneNode};
    use crate::core::scene_identity::SceneNodeKey;
    use crate::renderer::batcher::PrimitiveData;
    use std::sync::Arc;

    #[test]
    fn analytic_surface_retained_raw_clip_refusal_and_recovery() {
        let key = Some(SceneNodeKey::new(386));
        let mut leaf = SceneBuilder::new();
        leaf.push_analytic_surface(
            RectF::new(0.0, 0.0, 90.0, 80.0),
            PaintCorners::uniform(12.0),
            &fixture(1.0, 0.2, 0.6),
            10,
            key,
        )
        .unwrap();
        let local: Arc<[SceneNode]> = Arc::from(leaf.build().root);
        let original = local.clone();
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 100.0, 100.0));
        let id = compositor.create_surface(0, RectF::new(0.0, 0.0, 100.0, 100.0), SceneKey::BASE);
        let mut build = DrawPlanBuilder::new();
        let mut cache = MeshCache::new();
        for (inner_radius, outer_radius) in [(0.0, 12.0), (4.0, 12.0), (0.0, 0.0), (0.0, 12.0)] {
            let mut scene = SceneBuilder::new();
            scene.begin_clip(
                RectF::new(0.0, 0.0, 100.0, 100.0),
                Some(RoundedCorners::uniform(outer_radius)),
            );
            scene.begin_clip(
                RectF::new(7.0, 9.0, 80.0, 70.0),
                Some(RoundedCorners::uniform(inner_radius)),
            );
            scene.push_retained_fragment(local.clone());
            scene.end_clip();
            scene.end_clip();
            let result = build.build_multi(
                &HashMap::from([(id, scene.build())]),
                compositor.snapshot_multi(),
                &mut cache,
                |_| 1.0,
                0,
            );
            if inner_radius > 0.0 {
                assert!(
                    matches!(result,Err(DrawPlanBuildError::UnsupportedAnalyticSurfaceClip { surface_id,node_key,rounded_clip_depth:2 }) if surface_id==id && node_key==key)
                );
            } else {
                let plan = result.unwrap();
                let primitive = &plan.surface_scene(id).unwrap().primitives[0];
                let PrimitiveData::AnalyticSurface {
                    ancestor, ordinal, ..
                } = &primitive.data
                else {
                    panic!("analytic primitive")
                };
                assert_eq!(*ordinal, 1);
                assert_eq!(primitive.clip_state.rect, RectF::new(7.0, 9.0, 80.0, 70.0));
                assert_eq!(
                    *ancestor,
                    if outer_radius > 0.0 {
                        Some((RectF::new(0.0, 0.0, 100.0, 100.0), [12.0; 4]))
                    } else {
                        None
                    }
                );
            }
            assert!(Arc::ptr_eq(&local, &original));
        }
        for radius in [-1.0, f32::NAN, f32::INFINITY, 51.0] {
            let mut scene = SceneBuilder::new();
            scene.begin_clip(
                RectF::new(0.0, 0.0, 100.0, 100.0),
                Some(RoundedCorners::uniform(radius)),
            );
            scene.push_retained_fragment(local.clone());
            scene.end_clip();
            assert!(matches!(
                build.build_multi(
                    &HashMap::from([(id, scene.build())]),
                    compositor.snapshot_multi(),
                    &mut cache,
                    |_| 1.0,
                    0
                ),
                Err(DrawPlanBuildError::InvalidAnalyticSurfaceClip {
                    rounded_clip_depth: 1,
                    ..
                })
            ));
        }
        for raw in [
            RectF::new(f32::NAN, 0.0, 100.0, 100.0),
            RectF::new(0.0, 0.0, f32::INFINITY, 100.0),
        ] {
            // Exercise the final composed boundary directly. SceneBuilder's
            // earlier finite-input programming guard remains independently intact.
            let mut scene = crate::core::scene::SceneGraph::new();
            scene
                .root
                .push(SceneNode::Clip(crate::core::scene::ClipNode {
                    rect: raw,
                    rounded: Some(RoundedCorners::uniform(12.0)),
                    children: local.to_vec(),
                    z_index: 0,
                    key: None,
                }));
            assert!(matches!(
                build.build_multi(
                    &HashMap::from([(id, scene)]),
                    compositor.snapshot_multi(),
                    &mut cache,
                    |_| 1.0,
                    0
                ),
                Err(DrawPlanBuildError::InvalidAnalyticSurfaceClip {
                    rounded_clip_depth: 1,
                    ..
                })
            ));
        }
        let mut scene = SceneBuilder::new();
        scene.push_retained_fragment(local.clone());
        let scenes = HashMap::from([(id, scene.build())]);
        for scale in [f32::NAN, 0.0, 0.249, 8.01, f32::INFINITY] {
            assert!(matches!(
                build.build_multi(
                    &scenes,
                    compositor.snapshot_multi(),
                    &mut cache,
                    |_| scale,
                    0
                ),
                Err(DrawPlanBuildError::InvalidAnalyticSurfaceTarget { .. })
            ));
        }
        for scale in [0.25, 1.0, 8.0] {
            assert!(
                build
                    .build_multi(
                        &scenes,
                        compositor.snapshot_multi(),
                        &mut cache,
                        |_| scale,
                        0
                    )
                    .is_ok()
            );
        }
    }

    #[test]
    fn analytic_surface_capacity_and_surface_local_identity() {
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 100.0, 100.0));
        let first =
            compositor.create_surface(0, RectF::new(0.0, 0.0, 100.0, 100.0), SceneKey::BASE);
        let second =
            compositor.create_surface(1, RectF::new(0.0, 0.0, 100.0, 100.0), SceneKey::BASE);
        let material = fixture(1.0, 0.2, 0.6);
        let scene = |count| {
            let mut b = SceneBuilder::new();
            for i in 0..count {
                b.push_analytic_surface(
                    RectF::new(10.0, 10.0, 50.0, 40.0),
                    PaintCorners::uniform(8.0),
                    &material,
                    i,
                    None,
                )
                .unwrap();
            }
            b.build()
        };
        let mut build = DrawPlanBuilder::new();
        let mut cache = MeshCache::new();
        let plan = build
            .build_multi(
                &HashMap::from([(first, scene(256)), (second, scene(256))]),
                compositor.snapshot_multi(),
                &mut cache,
                |_| 1.0,
                0,
            )
            .unwrap();
        for id in [first, second] {
            let data = plan.surface_scene(id).unwrap();
            assert_eq!(data.primitives.len(), 256);
            for (i, p) in data.primitives.iter().enumerate() {
                assert_eq!(p.key.texture_id, Some(i as u64 + 1));
                assert_eq!(p.key.image_generation, None);
                assert_eq!(p.key.shape_type_id, None);
            }
        }
        assert!(matches!(
            build.build_multi(
                &HashMap::from([(first, scene(256)), (second, scene(257))]),
                compositor.snapshot_multi(),
                &mut cache,
                |_| 1.0,
                0
            ),
            Err(DrawPlanBuildError::AnalyticSurfaceCapacityExceeded {
                limit: 512,
                requested: 513,
                ..
            })
        ));
        assert!(matches!(
            build.build_multi(
                &HashMap::from([(first, scene(513))]),
                compositor.snapshot_multi(),
                &mut cache,
                |_| 1.0,
                0
            ),
            Err(DrawPlanBuildError::AnalyticSurfaceCapacityExceeded {
                limit: 512,
                requested: 513,
                ..
            })
        ));
        assert!(
            build
                .build_multi(
                    &HashMap::from([(first, scene(0))]),
                    compositor.snapshot_multi(),
                    &mut cache,
                    |_| 1.0,
                    0
                )
                .unwrap()
                .surface_scene(first)
                .unwrap()
                .primitives
                .is_empty()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_ambient_glow_retained_clip_only_build_refusal_and_recovery() {
        use crate::core::math::Point;
        use crate::core::paint::{Color as PaintColor, LocalAmbientGlowAppearance, LocalGlowLobe};
        use crate::core::scene::{RoundedCorners, SceneNode};
        use crate::core::scene_identity::SceneNodeKey;
        use std::sync::Arc;
        let key = Some(SceneNodeKey::new(388));
        let appearance = LocalAmbientGlowAppearance::try_new(&[LocalGlowLobe::try_new(
            Point::new(0.5, 0.5),
            Point::new(20.0, 15.0),
            Point::new(1.0, 0.0),
            PaintColor::WHITE,
            0.8,
        )
        .unwrap()])
        .unwrap();
        let mut leaf = SceneBuilder::new();
        leaf.push_local_ambient_glow(RectF::new(20.0, 20.0, 40.0, 30.0), appearance, 1, key)
            .unwrap();
        let local: Arc<[SceneNode]> = Arc::from(leaf.build().root);
        let original = local.clone();
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 100.0, 100.0));
        let id = compositor.create_surface(0, RectF::new(0.0, 0.0, 100.0, 100.0), SceneKey::BASE);
        let mut build = DrawPlanBuilder::new();
        let mut cache = MeshCache::new();
        for (radius, expected_depth) in [(0.0, None), (12.0, Some(1)), (0.0, None)] {
            let mut parent = SceneBuilder::new();
            parent.begin_clip(
                RectF::new(0.0, 0.0, 100.0, 100.0),
                Some(RoundedCorners::uniform(radius)),
            );
            parent.begin_clip(RectF::new(5.0, 5.0, 80.0, 80.0), None);
            parent.push_retained_fragment(local.clone());
            parent.end_clip();
            parent.end_clip();
            let scenes = HashMap::from([(id, parent.build())]);
            let result =
                build.build_multi(&scenes, compositor.snapshot_multi(), &mut cache, |_| 1.0, 0);
            match expected_depth {
                Some(depth) => assert!(
                    matches!(result,Err(DrawPlanBuildError::UnsupportedLocalAmbientGlowClip { surface_id,node_key,rounded_clip_depth }) if surface_id==id && node_key==key && rounded_clip_depth==depth)
                ),
                None => assert_eq!(
                    result.unwrap().surface_scene(id).unwrap().primitives.len(),
                    1
                ),
            }
            assert!(
                Arc::ptr_eq(&local, &original),
                "leaf local paint was retained, not rebuilt"
            );
        }
        let mut parent = SceneBuilder::new();
        parent.begin_clip(RectF::new(0.0, 0.0, 100.0, 100.0), None);
        parent.begin_clip(
            RectF::new(5.0, 5.0, 80.0, 80.0),
            Some(RoundedCorners::uniform(4.0)),
        );
        parent.push_retained_fragment(local);
        parent.end_clip();
        parent.end_clip();
        assert!(matches!(
            build.build_multi(
                &HashMap::from([(id, parent.build())]),
                compositor.snapshot_multi(),
                &mut cache,
                |_| 1.0,
                0
            ),
            Err(DrawPlanBuildError::UnsupportedLocalAmbientGlowClip {
                rounded_clip_depth: 2,
                ..
            })
        ));
        for appearance in [
            LocalAmbientGlowAppearance::try_new(&[]).unwrap(),
            LocalAmbientGlowAppearance::try_new(&[LocalGlowLobe::try_new(
                Point::new(0.5, 0.5),
                Point::new(20.0, 15.0),
                Point::new(1.0, 0.0),
                PaintColor::WHITE,
                0.0,
            )
            .unwrap()])
            .unwrap(),
            LocalAmbientGlowAppearance::try_new(&[LocalGlowLobe::try_new(
                Point::new(0.5, 0.5),
                Point::new(20.0, 15.0),
                Point::new(1.0, 0.0),
                PaintColor::TRANSPARENT,
                1.0,
            )
            .unwrap()])
            .unwrap(),
        ] {
            let mut parent = SceneBuilder::new();
            parent.begin_clip(
                RectF::new(0.0, 0.0, 100.0, 100.0),
                Some(RoundedCorners::uniform(12.0)),
            );
            parent
                .push_local_ambient_glow(RectF::new(20.0, 20.0, 40.0, 30.0), appearance, 1, key)
                .unwrap();
            parent.end_clip();
            let scenes = HashMap::from([(id, parent.build())]);
            assert!(
                build
                    .build_multi(&scenes, compositor.snapshot_multi(), &mut cache, |_| 1.0, 0)
                    .unwrap()
                    .surface_scene(id)
                    .unwrap()
                    .primitives
                    .is_empty()
            );
        }
    }
    use crate::compositor::{Compositor, SceneKey};
    use crate::core::math::RectF;
    use crate::core::scene::{Color, SceneBuilder};
    use crate::renderer::batcher::PrimitiveData;
    use crate::renderer::custom_render_pass::CustomRenderAdmission;
    use crate::renderer::custom_render_prepare::{
        CustomRenderPresentationBindings, audit as preparation_audit,
    };
    use std::cell::RefCell;

    fn target_bound_scene(
        presentation_id: crate::renderer::CustomRenderPresentationId,
    ) -> SceneGraph {
        let mut scene = SceneBuilder::new();
        scene.push_target_bound_image(
            RectF::new(2.0, 3.0, 20.0, 12.0),
            presentation_id,
            RectF::new(0.0, 0.0, 1.0, 1.0),
            Color::WHITE,
            4,
            None,
        );
        scene.build()
    }

    fn image_texture_id(
        plan: &DrawPlan<MultiSurfaceContract>,
        surface_id: SurfaceId,
    ) -> Option<u64> {
        plan.surface_scene(surface_id)?
            .primitives
            .iter()
            .find_map(|primitive| match &primitive.data {
                PrimitiveData::Image { texture_id, .. } => Some(*texture_id),
                _ => None,
            })
    }

    #[test]
    fn build_multi_requests_scale_per_surface() {
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));
        let base_id =
            compositor.create_surface(0, RectF::new(0.0, 0.0, 800.0, 600.0), SceneKey::BASE);
        let child_id =
            compositor.create_surface(1, RectF::new(0.0, 0.0, 320.0, 200.0), SceneKey::new(42));

        let snapshot = compositor.snapshot_multi();
        let mut surface_scenes = HashMap::new();
        surface_scenes.insert(base_id, SceneGraph::new());
        surface_scenes.insert(child_id, SceneGraph::new());

        let seen = RefCell::new(Vec::new());
        let mut builder = DrawPlanBuilder::new();
        let _ = builder
            .build_multi(
                &surface_scenes,
                snapshot,
                &mut MeshCache::new(),
                |surface_id| {
                    seen.borrow_mut().push(surface_id);
                    1.0
                },
                0,
            )
            .expect("scene must satisfy DrawPlan Build admission");

        let seen = seen.into_inner();
        assert!(seen.contains(&base_id));
        assert!(seen.contains(&child_id));
    }

    #[test]
    fn target_bound_images_resolve_only_for_the_exact_surface_in_both_publication_orders() {
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));
        let surface_a =
            compositor.create_surface(0, RectF::new(0.0, 0.0, 800.0, 600.0), SceneKey::BASE);
        let surface_b =
            compositor.create_surface(1, RectF::new(0.0, 0.0, 320.0, 200.0), SceneKey::new(42));
        let admission = CustomRenderAdmission::for_test();
        let presentation_id = admission.presentation_id();
        let mut surface_scenes = HashMap::new();
        surface_scenes.insert(surface_a, target_bound_scene(presentation_id));
        surface_scenes.insert(surface_b, target_bound_scene(presentation_id));
        let mut bindings = CustomRenderPresentationBindings::default();
        let mut builder = DrawPlanBuilder::new();

        for publications in [
            [(surface_a, 10_001), (surface_b, 20_002)],
            [(surface_b, 20_002), (surface_a, 10_001)],
        ] {
            bindings.begin_frame();
            for (surface_id, texture_id) in publications {
                bindings
                    .publish_for_target(
                        surface_id,
                        presentation_id,
                        admission.presentation_binding(texture_id),
                    )
                    .unwrap();
            }
            let plan = builder
                .build_multi_with_presentation_bindings(
                    &surface_scenes,
                    compositor.snapshot_multi(),
                    &mut MeshCache::new(),
                    &mut bindings.begin_materialization_for_test(0),
                    |_| 1.0,
                    0,
                )
                .expect("scene must satisfy DrawPlan Build admission");
            assert_eq!(image_texture_id(&plan, surface_a), Some(10_001));
            assert_eq!(image_texture_id(&plan, surface_b), Some(20_002));
        }

        bindings.begin_frame();
        bindings
            .publish_for_target(
                surface_a,
                presentation_id,
                admission.presentation_binding(30_003),
            )
            .unwrap();
        let plan = builder
            .build_multi_with_presentation_bindings(
                &surface_scenes,
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                &mut bindings.begin_materialization_for_test(1),
                |_| 1.0,
                1,
            )
            .expect("scene must satisfy DrawPlan Build admission");
        assert_eq!(image_texture_id(&plan, surface_a), Some(30_003));
        assert_eq!(image_texture_id(&plan, surface_b), None);

        bindings.begin_frame();
        let plan = builder
            .build_multi_with_presentation_bindings(
                &surface_scenes,
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                &mut bindings.begin_materialization_for_test(2),
                |_| 1.0,
                2,
            )
            .expect("scene must satisfy DrawPlan Build admission");
        assert_eq!(image_texture_id(&plan, surface_a), None);
        assert_eq!(image_texture_id(&plan, surface_b), None);
    }

    #[test]
    fn registered_images_keep_the_same_flattened_and_batched_path_without_binding_lookup() {
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));
        let surface =
            compositor.create_surface(0, RectF::new(0.0, 0.0, 800.0, 600.0), SceneKey::BASE);
        let mut scene = SceneBuilder::new();
        scene.push_image(
            RectF::new(2.0, 3.0, 20.0, 12.0),
            77_007,
            RectF::new(0.0, 0.0, 1.0, 1.0),
            Color::WHITE,
            4,
            None,
        );
        let mut surface_scenes = HashMap::new();
        surface_scenes.insert(surface, scene.build());

        let mut direct_builder = DrawPlanBuilder::new();
        let direct = direct_builder
            .build_multi(
                &surface_scenes,
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| 1.0,
                0,
            )
            .expect("scene must satisfy DrawPlan Build admission");
        let mut bindings = CustomRenderPresentationBindings::default();
        preparation_audit::reset();
        let mut bound_builder = DrawPlanBuilder::new();
        let bound = bound_builder
            .build_multi_with_presentation_bindings(
                &surface_scenes,
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                &mut bindings.begin_materialization_for_test(0),
                |_| 1.0,
                0,
            )
            .expect("scene must satisfy DrawPlan Build admission");
        assert_eq!(preparation_audit::presentation_lookups(), 0);
        assert_eq!(image_texture_id(&direct, surface), Some(77_007));
        assert_eq!(image_texture_id(&bound, surface), Some(77_007));

        let direct = direct.surface_scene(surface).unwrap();
        let bound = bound.surface_scene(surface).unwrap();
        assert_eq!(direct.primitives.len(), bound.primitives.len());
        assert_eq!(direct.batches.len(), bound.batches.len());
        assert_eq!(direct.total_image_vertices, bound.total_image_vertices);
        assert_eq!(direct.total_image_indices, bound.total_image_indices);
        assert_eq!(direct.primitives[0].key, bound.primitives[0].key);
        assert_eq!(
            direct.primitives[0].order_key,
            bound.primitives[0].order_key
        );
        assert_eq!(direct.batches[0].key, bound.batches[0].key);
        assert_eq!(
            direct.batches[0].vertex_range.start,
            bound.batches[0].vertex_range.start
        );
        assert_eq!(
            direct.batches[0].vertex_range.count,
            bound.batches[0].vertex_range.count
        );
        assert_eq!(
            direct.batches[0].index_range.start,
            bound.batches[0].index_range.start
        );
        assert_eq!(
            direct.batches[0].index_range.count,
            bound.batches[0].index_range.count
        );
    }
}
