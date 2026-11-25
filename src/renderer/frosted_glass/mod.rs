//! HAL385 §8: current same-surface prefix filtering. Scene/Build own CPU data;
//! the renderer owns bounded backing and completion custody. Passes only encode.
pub(crate) mod backing;
mod graph;
pub(crate) mod parameters;

pub(crate) use backing::{EncodedTargetCommands, FrostPool};
pub(crate) use graph::FrostedSurfacePlan;
pub(crate) use parameters::{FrostedGlassParams, Occurrence};

use crate::compositor::SurfaceId;
use crate::core::scene_identity::SceneNodeKey;
use crate::renderer::render_graph::GraphError;
use std::sync::{Arc, Mutex, atomic::{AtomicBool, AtomicUsize, Ordering}};

/// HAL385 §8.3: waiting for browser allocation qualification is not an error
/// and never publishes an executable plan.
#[derive(Debug)]
pub enum RendererCompileOutcome<T> {
    Ready(T),
    NoTargets,
    Pending(RendererPreparationIdentity),
}

impl<T> RendererCompileOutcome<T> {
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }
    pub fn is_no_targets(&self) -> bool {
        matches!(self, Self::NoTargets)
    }
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> RendererCompileOutcome<U> {
        match self {
            Self::Ready(value) => RendererCompileOutcome::Ready(f(value)),
            Self::NoTargets => RendererCompileOutcome::NoTargets,
            Self::Pending(identity) => RendererCompileOutcome::Pending(identity),
        }
    }

    /// Explicit fail-fast convenience for callers whose contract requires an
    /// immediately ready result, such as a native single-frame test.
    pub fn expect_ready(self, message: &str) -> T {
        match self {
            Self::Ready(value) => value,
            Self::NoTargets => panic!("{message}: no targets"),
            Self::Pending(_) => panic!("{message}: preparation pending"),
        }
    }
}

struct PreparationState {
    token: Arc<()>,
    scopes: AtomicUsize,
    out_of_memory: AtomicBool,
    cancelled: AtomicBool,
    registered: AtomicBool,
    notice: AtomicBool,
    wake: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

/// Opaque identity of one renderer-owned suspended preparation. It is not a
/// command, resource lease, GPU completion receipt or permission to Encode.
#[derive(Clone)]
pub struct RendererPreparationIdentity(Arc<PreparationState>);

impl std::fmt::Debug for RendererPreparationIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RendererPreparationIdentity")
    }
}
impl PartialEq for RendererPreparationIdentity {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for RendererPreparationIdentity {}

impl RendererPreparationIdentity {
    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn new(token: Arc<()>) -> Self {
        Self(Arc::new(PreparationState {
            token,
            scopes: AtomicUsize::new(0),
            out_of_memory: AtomicBool::new(false),
            cancelled: AtomicBool::new(false),
            registered: AtomicBool::new(false),
            notice: AtomicBool::new(false),
            wake: Mutex::new(None),
        }))
    }

    pub(crate) fn token(&self) -> &Arc<()> {
        &self.0.token
    }

    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn add_scope(&self) {
        self.0.scopes.fetch_add(1, Ordering::AcqRel);
    }

    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn finish_scope(&self, out_of_memory: bool) {
        if out_of_memory {
            self.0.out_of_memory.store(true, Ordering::Release);
        }
        assert!(
            self.0.scopes.fetch_sub(1, Ordering::AcqRel) > 0,
            "HAL385: one completion per allocation scope"
        );
        self.notify();
    }

    pub(crate) fn qualification(&self) -> Result<bool, RendererCompileError> {
        if self.0.cancelled.load(Ordering::Acquire) {
            return Err(crate::renderer::RendererSurfaceError::Outdated.into());
        }
        if self.0.out_of_memory.load(Ordering::Acquire) {
            return Err(crate::renderer::RendererSurfaceError::OutOfMemory.into());
        }
        Ok(self.0.scopes.load(Ordering::Acquire) == 0)
    }

    /// Register one completion wake with the caller's ordinary event loop.
    /// Completion may precede registration; either ordering emits one wake.
    pub fn on_completed(&self, wake: impl FnOnce() + Send + 'static) {
        assert!(
            !self.0.registered.swap(true, Ordering::AcqRel),
            "HAL385: completion wake is registered once"
        );
        *self.0.wake.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(wake));
        self.notify();
    }

    fn notify(&self) {
        if self.0.scopes.load(Ordering::Acquire) != 0 {
            return;
        }
        let mut slot = self.0.wake.lock().unwrap_or_else(|e| e.into_inner());
        if self.0.cancelled.load(Ordering::Acquire) {
            slot.take();
            return;
        }
        let wake = slot.take();
        drop(slot);
        if let Some(wake) = wake {
            self.0.notice.store(true, Ordering::Release);
            wake();
        }
    }

    pub(crate) fn take_completion_notice(&self) -> bool {
        self.0.notice.swap(false, Ordering::AcqRel)
    }

    pub(crate) fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::Release);
        self.0.wake.lock().unwrap_or_else(|e| e.into_inner()).take();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrostedGlassCompileRefusal {
    TargetUnavailable,
    InvalidTarget,
    UnsupportedFormat,
    NonPremultipliedClear,
    KernelLimit {
        required: u32,
        limit: u32,
    },
    ResourceBudget {
        required_bytes: u64,
        limit_bytes: u64,
    },
    IncompletePrefix,
    UnsupportedCompanionGraph,
    Graph(GraphError),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RendererCompileError {
    Surface(crate::renderer::RendererSurfaceError),
    FrostedGlass {
        surface_id: SurfaceId,
        node_key: Option<SceneNodeKey>,
        reason: FrostedGlassCompileRefusal,
    },
    Bloom {
        surface_id: SurfaceId,
        node_key: Option<SceneNodeKey>,
        reason: crate::renderer::BloomCompileRefusal,
    },
}
impl std::fmt::Display for RendererCompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Surface(e) => write!(f, "Surface Compile: {e}"),
            Self::FrostedGlass {
                surface_id,
                node_key,
                reason,
            } => write!(
                f,
                "HAL385 Compile refused surface={surface_id:?} key={node_key:?}: {reason:?}"
            ),
            Self::Bloom { surface_id, node_key, reason } => write!(
                f,
                "HAL395 Compile refused surface={surface_id:?} key={node_key:?}: {reason:?}"
            ),
        }
    }
}
impl std::error::Error for RendererCompileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Surface(e) => Some(e),
            Self::FrostedGlass {
                reason: FrostedGlassCompileRefusal::Graph(e),
                ..
            } | Self::Bloom {
                reason: crate::renderer::BloomCompileRefusal::Graph(e),
                ..
            } => Some(e),
            _ => None,
        }
    }
}
impl From<crate::renderer::RendererSurfaceError> for RendererCompileError {
    fn from(value: crate::renderer::RendererSurfaceError) -> Self {
        Self::Surface(value)
    }
}
pub(crate) const PLAN_BUDGET: u64 = 256 * 1024 * 1024;
pub(crate) const RESIDENT_BUDGET: u64 = 768 * 1024 * 1024;

/// Both existing material paths compile the same pure optical helpers.
pub(crate) fn shader_source(source: &str) -> String {
    format!("{}\n{}\n{}", include_str!("../shaders/glass_optics.wgsl"),
        include_str!("../shaders/surface_emission.wgsl"), source)
}

pub(crate) fn error(
    surface_id: SurfaceId,
    node_key: Option<SceneNodeKey>,
    reason: FrostedGlassCompileRefusal,
) -> RendererCompileError {
    RendererCompileError::FrostedGlass {
        surface_id,
        node_key,
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compositor::{Compositor, SceneKey};
    use crate::core::{
        math::RectF,
        paint::{
            Color, FrostedGlassAppearance as Appearance, FrostedGlassError as Error,
            RoundedCorners, SurfaceMicrograin,
        },
        scene::{SceneBuilder, SceneNode},
    };
    use crate::renderer::{
        batcher::PrimitiveData,
        draw_plan_builder::{DrawPlanBuildError, DrawPlanBuilder, FrostedGlassBuildRefusal},
        mesh_cache::MeshCache,
    };
    use std::{collections::HashMap, sync::Arc};
    #[test]
    fn frosted_compile_pending_wakes_once_only_after_all_scopes() {
        for register_late in [false, true] {
            let identity = RendererPreparationIdentity::new(Arc::new(()));
            identity.add_scope();
            identity.add_scope();
            let wakes = Arc::new(AtomicUsize::new(0));
            let count = wakes.clone();
            let wake = move || {
                count.fetch_add(1, Ordering::AcqRel);
            };
            if !register_late {
                identity.on_completed(wake);
            }
            assert_eq!(identity.qualification(), Ok(false));
            identity.finish_scope(false);
            assert_eq!(identity.qualification(), Ok(false));
            assert_eq!(wakes.load(Ordering::Acquire), 0);
            identity.finish_scope(false);
            if register_late {
                let count = wakes.clone();
                identity.on_completed(move || {
                    count.fetch_add(1, Ordering::AcqRel);
                });
            }
            assert_eq!(identity.qualification(), Ok(true));
            assert_eq!(wakes.load(Ordering::Acquire), 1);
            assert!(identity.take_completion_notice());
            assert!(!identity.take_completion_notice());
        }
    }

    #[test]
    fn frosted_compile_pending_oom_and_cancel_are_not_success() {
        let identity = RendererPreparationIdentity::new(Arc::new(()));
        identity.add_scope();
        identity.finish_scope(true);
        assert_eq!(
            identity.qualification(),
            Err(crate::renderer::RendererSurfaceError::OutOfMemory.into())
        );
        let cancelled = RendererPreparationIdentity::new(Arc::new(()));
        cancelled.add_scope();
        let wakes = Arc::new(AtomicUsize::new(0));
        let count = wakes.clone();
        cancelled.on_completed(move || {
            count.fetch_add(1, Ordering::AcqRel);
        });
        cancelled.cancel();
        cancelled.finish_scope(false);
        assert_eq!(wakes.load(Ordering::Acquire), 0);
        assert_eq!(
            cancelled.qualification(),
            Err(crate::renderer::RendererSurfaceError::Outdated.into())
        );
        assert!(!cancelled.take_completion_notice());
    }
    fn material() -> Appearance {
        Appearance::try_new(24.0, Color::from_linear(0.1, 0.2, 0.3, 0.5), 0.8).unwrap()
    }
    #[test]
    fn frosted_glass_all_shader_abis_validate_as_1360_bytes() {
        let shared = include_str!("../shaders/micrograin.wgsl");
        for source in [
            include_str!("../shaders/frosted_glass_horizontal.wgsl").to_owned(),
            include_str!("../shaders/frosted_glass_vertical.wgsl").to_owned(),
            format!(
                "{shared}\n{}",
                include_str!("../shaders/frosted_glass_replace.wgsl")
            ),
            include_str!("../shaders/frosted_glass_terminal.wgsl").to_owned(),
        ] {
            let source = shader_source(&source);
            let module = naga::front::wgsl::parse_str(&source)
                .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&source)));
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .unwrap();
            let (_, ty) = module
                .types
                .iter()
                .find(|(_, t)| t.name.as_deref() == Some("FrostedGlassParams"))
                .unwrap();
            let naga::TypeInner::Struct { members, span } = &ty.inner else {
                panic!("uniform struct")
            };
            assert_eq!(*span, 1360);
            assert_eq!(
                members.iter().map(|m| m.offset).collect::<Vec<_>>(),
                [0, 16, 32, 48, 64, 80, 96, 112, 128, 144, 160, 240, 256,
                    1040, 1056, 1072, 1088, 1280, 1344]
            );
        }
    }
    #[test]
    fn frosted_glass_thin_film_only_appends_a_tail() {
        use crate::core::paint::{ThinFilmParams, analytic_surface_tests::coating};

        let rect = RectF::new(12.0, 14.0, 72.0, 48.0);
        let corners = RoundedCorners::uniform(6.0);
        let target = crate::renderer::render_target::TargetDescriptor {
            surface_id: SurfaceId::test_id(1),
            physical_size: (200, 160),
            logical_size: (100.0, 80.0),
            scale_factor: 2.0,
            globals_offset: 0,
            path_clips_offset: 0,
        };
        let [absent, zero, active] = [None, Some(0.0), Some(0.5)].map(|strength| {
            let film = strength.map(|s| ThinFilmParams::try_new(s, 320.0, 1.4).unwrap());
            let optics = coating(0.6).try_with_thin_film(film).unwrap();
            let appearance = material().try_with_optics(Some(optics), 2.0, 0.75).unwrap();
            let mut scene = SceneBuilder::new();
            scene.push_frosted_glass(rect, corners, &appearance, 1, None).unwrap();
            let mut batcher = crate::renderer::batcher::Batcher::new();
            batcher.begin_frame(SceneKey::BASE);
            batcher.flatten_scene(&scene.build().root, &mut MeshCache::new(), 2.0, 0);
            parameters::occurrence(&batcher.primitives()[0], &target, 1.0).unwrap().unwrap().params
        });
        assert_eq!(bytemuck::bytes_of(&absent), bytemuck::bytes_of(&zero));
        assert_eq!(absent.film, [0.0; 4]);
        assert_eq!(active.film, [0.5, 320.0, 1.4, 0.0]);
        assert_eq!(&bytemuck::bytes_of(&absent)[..1344], &bytemuck::bytes_of(&active)[..1344]);
        assert_eq!(FrostedGlassParams::terminal((200, 160)).film, [0.0; 4]);
    }

    #[test]
    fn frosted_glass_checked_optics_geometry_and_identity() {
        let m = material();
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(
                Appearance::try_new(value, Color::WHITE, 0.5),
                Err(Error::NonFiniteInput)
            );
            assert_eq!(
                m.clone().try_with_edge_feather(value),
                Err(Error::NonFiniteInput)
            );
        }
        assert_eq!(
            Appearance::try_new(-0.1, Color::WHITE, 0.5),
            Err(Error::InvalidSigma)
        );
        assert_eq!(
            Appearance::try_new(24.1, Color::WHITE, 0.5),
            Err(Error::InvalidSigma)
        );
        assert_eq!(
            Appearance::try_new(1.0, Color::from_linear(-0.1, 0.0, 0.0, 1.0), 0.5),
            Err(Error::InvalidTint)
        );
        assert_eq!(
            Appearance::try_new(1.0, Color::WHITE, 1.1),
            Err(Error::InvalidTransmission)
        );
        assert_eq!(m.edge_feather_logical().to_bits(), 0);
        for value in [0.0, -0.0] {
            let explicit = m.clone().try_with_edge_feather(value).unwrap();
            assert_eq!(explicit.edge_feather_logical().to_bits(), 0);
            assert_eq!(explicit, m);
        }
        for value in [-0.01, 32.01] {
            assert_eq!(
                m.clone().try_with_edge_feather(value),
                Err(Error::InvalidEdgeFeather)
            );
        }
        let feathered = m.clone().try_with_edge_feather(32.0).unwrap();
        assert_ne!(feathered, m);
        assert_eq!(feathered.edge_feather_logical(), 32.0);
        let zero = m
            .clone()
            .with_micrograin(Some(SurfaceMicrograin::try_new(1.0, 0.0, 1).unwrap()));
        assert_eq!(m, zero);
        assert_eq!(m.micrograin(), None);
        let mut s = SceneBuilder::new();
        let r = RectF::new(0.0, 0.0, 40.0, 30.0);
        assert_eq!(
            s.push_frosted_glass(r, RoundedCorners::uniform(16.0), &m, 1, None),
            Err(Error::InvalidGeometry)
        );
        assert_eq!(
            s.push_frosted_glass(
                RectF::new(f32::NAN, 0.0, 1.0, 1.0),
                RoundedCorners::uniform(0.0),
                &m,
                1,
                None
            ),
            Err(Error::InvalidGeometry)
        );
        s.push_frosted_glass(
            r,
            RoundedCorners::uniform(4.0),
            &Appearance::try_new(0.0, Color::WHITE, 1.0)
                .unwrap()
                .with_micrograin(Some(
                    SurfaceMicrograin::try_new(1.0, 2.0 / 255.0, 5).unwrap(),
                )),
            1,
            None,
        )
        .unwrap();
        s.push_frosted_glass(
            RectF::new(0.0, 0.0, 0.0, 8.0),
            RoundedCorners::uniform(0.0),
            &m,
            1,
            None,
        )
        .unwrap();
        assert!(s.build().root.is_empty());
        let mut s = SceneBuilder::new();
        s.push_frosted_glass(
            r,
            RoundedCorners::uniform(4.0),
            &Appearance::try_new(1.0, Color::TRANSPARENT, 1.0).unwrap(),
            1,
            None,
        )
        .unwrap();
        assert_eq!(
            s.build().root.len(),
            1,
            "alpha-zero tint does not cancel genuine filtering"
        );
    }
    #[test]
    fn frosted_glass_abi_kernel_support_and_typed_target_refusal() {
        use std::mem::{offset_of, size_of};
        assert_eq!(size_of::<FrostedGlassParams>(), 1360);
        assert_eq!(offset_of!(FrostedGlassParams, rect), 0);
        assert_eq!(offset_of!(FrostedGlassParams, tint), 32);
        assert_eq!(offset_of!(FrostedGlassParams, target), 64);
        assert_eq!(offset_of!(FrostedGlassParams, control), 112);
        assert_eq!(offset_of!(FrostedGlassParams, ancestor), 160);
        assert_eq!(offset_of!(FrostedGlassParams, weights), 256);
        assert_eq!(offset_of!(FrostedGlassParams, filtered_domain), 1040);
        assert_eq!(offset_of!(FrostedGlassParams, surface_controls), 1056);
        assert_eq!(offset_of!(FrostedGlassParams, extensions), 1072);
        assert_eq!(offset_of!(FrostedGlassParams, glass_optics), 1088);
        assert_eq!(offset_of!(FrostedGlassParams, emission), 1280);
        assert_eq!(offset_of!(FrostedGlassParams, film), 1344);
        let mut s = SceneBuilder::new();
        s.push_frosted_glass(
            RectF::new(20.25, 19.5, 40.0, 32.0),
            RoundedCorners::uniform(4.0),
            &material(),
            10,
            None,
        )
        .unwrap();
        let mut b = crate::renderer::batcher::Batcher::new();
        b.begin_frame(SceneKey::BASE);
        b.flatten_scene(&s.build().root, &mut MeshCache::new(), 2.0, 0);
        let target = crate::renderer::render_target::TargetDescriptor {
            surface_id: SurfaceId::test_id(1),
            physical_size: (200, 160),
            logical_size: (100.0, 80.0),
            scale_factor: 2.0,
            globals_offset: 0,
            path_clips_offset: 0,
        };
        let o = parameters::occurrence(&b.primitives()[0], &target, 1.0)
            .unwrap()
            .unwrap();
        assert_eq!(o.params.radius(), 144);
        assert_eq!(o.params.snapshot, [0, 0, 200, 160]);
        assert_eq!(o.params.weight(0), 1.0);
        assert!(o.params.weight(144) > 0.0);
        for k in 145..196 {
            assert_eq!(o.params.weights[k / 4][k % 4], 0.0);
        }
        let mut high = target;
        high.scale_factor = 8.0;
        assert!(matches!(
            parameters::occurrence(&b.primitives()[0], &high, 1.0),
            Err(FrostedGlassCompileRefusal::KernelLimit {
                required: 576,
                limit: 192
            })
        ));
    }
    #[test]
    fn frosted_glass_s2_independent_zero_controls_and_signed_padded_regions() {
        use crate::core::paint::analytic_surface_tests::{coating,emission};
        let rect=RectF::new(200.0,180.0,80.0,60.0);
        let corners=RoundedCorners::uniform(12.0);
        let identity=Appearance::try_new(0.0,Color::WHITE,1.0).unwrap();
        assert_eq!(identity.validate_geometry(rect,corners),Ok(false));
        let mut scene = SceneBuilder::new();
        scene
            .push_frosted_glass(
                rect,
                corners,
                &identity.clone().try_with_edge_feather(8.0).unwrap(),
                10,
                None,
            )
            .unwrap();
        assert!(scene.build().root.is_empty(), "feather alone adds no operation");
        assert_eq!(identity.clone().try_with_optics(None,0.0,0.5).unwrap().validate_geometry(rect,corners),Ok(false));
        assert_eq!(identity.clone().try_with_optics(None,1.0,1.0),Err(Error::InvalidOptics));
        assert_eq!(identity.clone().try_with_optics(None,0.0,-0.1),Err(Error::InvalidOpacity));
        assert_eq!(identity.clone().try_with_optics(Some(coating(0.6)),32.01,1.0),Err(Error::InvalidRefraction));
        assert!(identity.clone().try_with_optics(Some(coating(0.6)),0.0,1.0).unwrap().validate_geometry(rect,corners).unwrap());
        let body_zero=material().try_with_optics(Some(coating(0.6)),0.0,0.0).unwrap();
        assert_eq!(body_zero.validate_geometry(rect,corners),Ok(false));
        assert_eq!(body_zero.with_surface_emission(Some(emission())).validate_geometry(rect,corners),Ok(true));
        let feathered = material()
            .try_with_edge_feather(8.0)
            .unwrap()
            .try_with_optics(Some(coating(0.6)), 2.0, 0.5)
            .unwrap()
            .with_surface_emission(Some(emission()))
            .with_micrograin(Some(SurfaceMicrograin::try_new(1.0, 2.0 / 255.0, 5).unwrap()));
        assert_eq!(feathered.edge_feather_logical(), 8.0);
        for scale in [1.0,2.0] {
            for shift in [-32.0,0.0,32.0] {
                let a=Appearance::try_new(24.0,Color::TRANSPARENT,0.9).unwrap()
                    .try_with_optics(Some(coating(0.6)),shift,1.0).unwrap();
                let mut scene=SceneBuilder::new();scene.push_frosted_glass(rect,corners,&a,10,None).unwrap();
                let mut b=crate::renderer::batcher::Batcher::new();b.begin_frame(SceneKey::BASE);
                b.flatten_scene(&scene.build().root,&mut MeshCache::new(),scale,0);
                let target=crate::renderer::render_target::TargetDescriptor{surface_id:SurfaceId::test_id(1),physical_size:(1200,900),
                    logical_size:(1200.0/scale,900.0/scale),scale_factor:scale,globals_offset:0,path_clips_offset:0};
                let o=parameters::occurrence(&b.primitives()[0],&target,1.0).unwrap().unwrap();
                let p=o.params;let reach=if shift==0.0 {0}else{(shift.abs()*scale).ceil() as u32+1};
                assert_eq!(p.filtered_domain[0],p.visible[0]-reach);
                assert_eq!(p.filtered_domain[2],p.visible[2]+2*reach);
                assert_eq!(p.snapshot[0],p.filtered_domain[0].saturating_sub(p.radius()));
                if scale==2.0 {assert_eq!(p.radius(),144);}
                let area=|r:[u32;4]|u64::from(r[2])*u64::from(r[3]);
                assert_eq!(p.bytes(),Some(12*area(p.snapshot)+8*area(p.filtered_domain)+1360));
                assert_eq!(p.surface_controls,[1.0,shift,0.0,0.0]);assert_eq!(p.extensions,[1,0,0,0]);
            }
        }
    }
    #[test]
    fn frosted_glass_retained_edge_feather_packs_only_byte_1064() {
        use bytemuck::bytes_of;

        let viewport = RectF::new(0.0, 0.0, 100.0, 80.0);
        let mut compositor = Compositor::new(viewport);
        let id = compositor.create_surface(0, viewport, SceneKey::BASE);
        let target = crate::renderer::render_target::TargetDescriptor {
            surface_id: id,
            physical_size: (150, 120),
            logical_size: (100.0, 80.0),
            scale_factor: 1.5,
            globals_offset: 0,
            path_clips_offset: 0,
        };
        let key = SceneNodeKey::new(385);
        let mut builder = DrawPlanBuilder::new();
        let mut cache = MeshCache::new();
        let mut pack = |appearance: &Appearance| {
            let mut leaf = SceneBuilder::new();
            leaf.push_frosted_glass(
                RectF::new(20.25, 19.5, 40.0, 32.0),
                RoundedCorners::uniform(4.0),
                appearance,
                10,
                Some(key),
            )
            .unwrap();
            let mut scene = SceneBuilder::new();
            scene.push_retained_fragment(Arc::from(leaf.build().root));
            let scene = scene.build();
            assert_eq!(crate::core::scene_diff::extract_scene_keys(&scene), [key]);
            let plan = builder
                .build_multi(
                    &HashMap::from([(id, scene)]),
                    compositor.snapshot_multi(),
                    &mut cache,
                    |_| target.scale_factor,
                    0,
                )
                .unwrap();
            let marker = &plan.surface_data[&id].primitives[0];
            let PrimitiveData::FrostedGlass { appearance: actual, ordinal, node_key, .. } =
                &marker.data
            else {
                panic!("retained Frosted Glass marker");
            };
            assert_eq!(actual, appearance);
            assert_eq!((*ordinal, *node_key), (1, Some(key)));
            parameters::occurrence(marker, &target, 1.0).unwrap().unwrap().params
        };

        let hard = material();
        let default = pack(&hard);
        let explicit_zero = pack(&hard.clone().try_with_edge_feather(-0.0).unwrap());
        assert_eq!(bytes_of(&default), bytes_of(&explicit_zero));
        let feathered = pack(&hard.try_with_edge_feather(8.0).unwrap());
        let mut expected = bytes_of(&default).to_vec();
        expected[1064..1068].copy_from_slice(&8.0_f32.to_ne_bytes());
        assert_eq!(bytes_of(&feathered), expected);
        assert_eq!(FrostedGlassParams::terminal((150, 120)).surface_controls, [0.0; 4]);
    }
    #[test]
    fn frosted_glass_retained_full_clip_order_capacity_and_recovery() {
        let mut leaf = SceneBuilder::new();
        let key = Some(SceneNodeKey::new(385));
        leaf.push_frosted_glass(
            RectF::new(10.0, 12.0, 70.0, 60.0),
            RoundedCorners::uniform(7.0),
            &material(),
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
        for (outer, inner) in [(0.0, 0.0), (12.0, 0.0), (12.0, 4.0), (0.0, 0.0)] {
            let mut scene = SceneBuilder::new();
            scene.begin_clip(
                RectF::new(0.0, 0.0, 100.0, 100.0),
                Some(crate::core::scene::RoundedCorners::uniform(outer)),
            );
            scene.begin_clip(
                RectF::new(7.0, 9.0, 80.0, 70.0),
                Some(crate::core::scene::RoundedCorners::uniform(inner)),
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
            if inner > 0.0 {
                assert!(
                    matches!(result,Err(DrawPlanBuildError::FrostedGlass{surface_id,node_key,reason:FrostedGlassBuildRefusal::UnsupportedClip{depth:2}})if surface_id==id && node_key==key)
                );
            } else {
                let plan = result.unwrap();
                let p = &plan.surface_data[&id].primitives[0];
                let PrimitiveData::FrostedGlass {
                    ancestor, ordinal, ..
                } = &p.data
                else {
                    panic!("marker")
                };
                assert_eq!(*ordinal, 1);
                assert_eq!(p.clip_state.rect, RectF::new(7.0, 9.0, 80.0, 70.0));
                assert_eq!(
                    *ancestor,
                    if outer > 0.0 {
                        Some((RectF::new(0.0, 0.0, 100.0, 100.0), [outer; 4]))
                    } else {
                        None
                    }
                );
                assert_eq!(
                    plan.surface_data[&id].batches.len(),
                    1,
                    "zero-vertex marker survives Build"
                );
            }
            assert!(Arc::ptr_eq(&local, &original));
        }
        let mut scene = SceneBuilder::new();
        scene.push_retained_fragment(local.clone());
        scene.push_rect(
            RectF::new(0.0, 0.0, 1.0, 1.0),
            Color::WHITE.into(),
            10,
            None,
        );
        assert!(matches!(
            build.build_multi(
                &HashMap::from([(id, scene.build())]),
                compositor.snapshot_multi(),
                &mut cache,
                |_| 1.0,
                0
            ),
            Err(DrawPlanBuildError::FrostedGlass {
                reason: FrostedGlassBuildRefusal::AmbiguousOrder,
                ..
            })
        ));
        for count in [0, 16, 17] {
            let mut s = SceneBuilder::new();
            for n in 0..count {
                s.push_frosted_glass(
                    RectF::new(1.0, 1.0, 20.0, 20.0),
                    RoundedCorners::uniform(1.0),
                    &material(),
                    n + 1,
                    None,
                )
                .unwrap();
            }
            let result = build.build_multi(
                &HashMap::from([(id, s.build())]),
                compositor.snapshot_multi(),
                &mut cache,
                |_| 1.0,
                0,
            );
            assert_eq!(result.is_ok(), count <= 16);
            if count == 17 {
                assert!(matches!(
                    result,
                    Err(DrawPlanBuildError::FrostedGlass {
                        reason: FrostedGlassBuildRefusal::OccurrenceLimit {
                            limit: 16,
                            requested: 17
                        },
                        ..
                    })
                ));
            }
        }
    }
}
