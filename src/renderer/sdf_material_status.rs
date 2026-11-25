//! Immutable resource-publication metadata, not GPU custody or mount authority.

use std::num::NonZeroU64;

use crate::compositor::SurfaceId;
use crate::core::widget_tree::WidgetTreeLifetimeToken;
use crate::utils::id::TreeWidgetId;

use super::custom_render_prepare::{CustomRenderActivity, CustomRenderTargetMetrics};
use super::image_binding::{
    ImageAdmissionError, ImageBinding, ImageBindingFailure, ImageBindingIdentity,
};

pub(crate) const CHAMPAGNE_V4_RADIUS: f32 = 8.0;

/// Availability selects the immutable content. Hover and press select a cell
/// in the same enabled generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChampagneV4Basis {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SdfMaterialSource {
    V1 { dark: bool },
    V2 { dark: bool },
    V3 { dark: bool },
    ChampagneV4(ChampagneV4Basis),
}

impl SdfMaterialSource {
    pub(crate) fn uses_basis_revision(self) -> bool {
        matches!(self, Self::V3 { .. } | Self::ChampagneV4(_))
    }

    pub(crate) fn has_pressed_cell(self) -> bool {
        matches!(
            self,
            Self::V3 { .. } | Self::ChampagneV4(ChampagneV4Basis::Enabled)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct SdfMaterialRevision(NonZeroU64);

impl SdfMaterialRevision {
    /// Issued only when ordinary Update first authorizes an SDF request.
    pub(crate) fn initial() -> Self {
        Self(NonZeroU64::MIN)
    }

    /// Call only for changed authored input; equal input retains this value.
    pub(crate) fn checked_increment(self) -> Result<Self, SdfMaterialRejection> {
        self.0
            .get()
            .checked_add(1)
            .and_then(NonZeroU64::new)
            .map(Self)
            .ok_or(SdfMaterialRejection::RevisionOverflow)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SdfMaterialKey {
    tree_lifetime: WidgetTreeLifetimeToken,
    owner: TreeWidgetId,
    surface: SurfaceId,
    revision: SdfMaterialRevision,
    metrics: CustomRenderTargetMetrics,
    // Scene placement and clipped logical bounds remain exact even when two
    // fractional placements cover the same physical pixels.
    composited_placement: Option<(crate::core::math::RectF, crate::core::math::RectF)>,
}

impl PartialEq for SdfMaterialKey {
    fn eq(&self, other: &Self) -> bool {
        // Attempt identity is observation provenance, not a resource dependency.
        let mut left = self.metrics;
        let mut right = other.metrics;
        left.app_frame_attempt = None;
        right.app_frame_attempt = None;
        self.tree_lifetime == other.tree_lifetime
            && self.owner == other.owner
            && self.surface == other.surface
            && self.revision == other.revision
            && self.composited_placement == other.composited_placement
            && left == right
    }
}

impl SdfMaterialKey {
    /// A pending revision may share geometry, never another owner/target basis.
    pub(crate) fn same_owner_target(&self, other: &Self) -> bool {
        let mut same_revision = other.clone();
        same_revision.revision = self.revision;
        self == &same_revision
    }
    /// A coarse owner/geometry guard, never full-K equality or texture evidence.
    /// Staging separately verifies exact local mapping and endpoint uniform bytes.
    pub(crate) fn same_content_geometry_basis(&self, other: &Self) -> bool {
        self.tree_lifetime == other.tree_lifetime
            && self.owner == other.owner
            && self.surface == other.surface
            && self.revision == other.revision
            && self.metrics.surface_id == other.metrics.surface_id
            && self.metrics.final_logical_rect.size() == other.metrics.final_logical_rect.size()
            && self.metrics.scale_factor == other.metrics.scale_factor
            && matches!(
                (self.metrics.activity, other.metrics.activity),
                (
                    CustomRenderActivity::Active { physical_render_extent: left, .. },
                    CustomRenderActivity::Active { physical_render_extent: right, .. }
                ) if left == right
            )
    }

    pub(crate) fn new(
        tree_lifetime: WidgetTreeLifetimeToken,
        owner: TreeWidgetId,
        surface: SurfaceId,
        revision: SdfMaterialRevision,
        metrics: CustomRenderTargetMetrics,
    ) -> Result<Self, SdfMaterialRejection> {
        if surface != metrics.surface_id {
            return Err(SdfMaterialRejection::KeyMismatch);
        }
        if !matches!(metrics.activity, CustomRenderActivity::Active { .. }) {
            return Err(SdfMaterialRejection::InactiveTarget);
        }
        Ok(Self {
            tree_lifetime,
            owner,
            surface,
            revision,
            metrics,
            composited_placement: None,
        })
    }

    pub(crate) fn with_composited_placement(
        mut self,
        face: crate::core::math::RectF,
        visible: crate::core::math::RectF,
    ) -> Self {
        self.composited_placement = Some((face, visible));
        self
    }

    pub(crate) fn tree_lifetime(&self) -> &WidgetTreeLifetimeToken {
        &self.tree_lifetime
    }

    pub(crate) fn owner(&self) -> TreeWidgetId {
        self.owner
    }

    pub(crate) fn metrics(&self) -> CustomRenderTargetMetrics {
        self.metrics
    }

    pub(crate) fn revision(&self) -> SdfMaterialRevision {
        self.revision
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SdfMaterialUnavailableCause {
    ProducerPreparationFailed,
    UnsupportedScale,
    UnsupportedGeometry,
    BindingAdmissionFailed(ImageAdmissionError),
    BindingRevoked {
        withdrawn: ImageBindingIdentity,
        cause: ImageBindingFailure,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SdfMaterialStatus {
    Ready { binding: ImageBinding },
    Unavailable { cause: SdfMaterialUnavailableCause },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SdfMaterialRejection {
    RevisionOverflow,
    InactiveTarget,
    KeyMismatch,
    MissingEvidence,
    BindingIdentityMismatch,
    BindingUnavailable(ImageBindingFailure),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SdfMaterialPublication {
    key: SdfMaterialKey,
    status: SdfMaterialStatus,
}

impl SdfMaterialPublication {
    /// The authorized engine/resource owner supplies the current K and exact
    /// current/withdrawn binding identity. This record cannot issue that authority,
    /// certify the producer front's lifetime, or verify tree membership.
    pub(crate) fn new(
        current: &SdfMaterialKey,
        producer_key: SdfMaterialKey,
        current_identity: Option<ImageBindingIdentity>,
        status: SdfMaterialStatus,
    ) -> Result<Self, SdfMaterialRejection> {
        let publication = Self {
            key: producer_key,
            status,
        };
        publication.validate(current, current_identity)?;
        Ok(publication)
    }

    fn validate(
        &self,
        current: &SdfMaterialKey,
        current_identity: Option<ImageBindingIdentity>,
    ) -> Result<(), SdfMaterialRejection> {
        if &self.key != current {
            return Err(SdfMaterialRejection::KeyMismatch);
        }
        let identity = match &self.status {
            SdfMaterialStatus::Ready { binding } => Some(binding.identity()),
            SdfMaterialStatus::Unavailable {
                cause: SdfMaterialUnavailableCause::BindingRevoked { withdrawn, .. },
            } => Some(*withdrawn),
            SdfMaterialStatus::Unavailable { .. } => None,
        };
        if let Some(identity) = identity {
            let current_identity = current_identity.ok_or(SdfMaterialRejection::MissingEvidence)?;
            if identity != current_identity {
                return Err(SdfMaterialRejection::BindingIdentityMismatch);
            }
        }
        if let SdfMaterialStatus::Ready { binding } = &self.status {
            binding
                .availability()
                .map_err(SdfMaterialRejection::BindingUnavailable)?;
        }
        Ok(())
    }

    /// Replaces a CPU publication slot only after complete validation. Missing
    /// evidence is rejection, not an intentional-absence status or a fallback.
    /// The engine must separately check is_live_mount before its real commit.
    pub(crate) fn replace_if_current(
        selected_publication: &mut Option<Self>,
        current: &SdfMaterialKey,
        current_identity: Option<ImageBindingIdentity>,
        publication: Option<Self>,
    ) -> Result<(), SdfMaterialRejection> {
        let publication = publication.ok_or(SdfMaterialRejection::MissingEvidence)?;
        publication.validate(current, current_identity)?;
        *selected_publication = Some(publication);
        Ok(())
    }

    pub(crate) fn key(&self) -> &SdfMaterialKey {
        &self.key
    }

    pub(crate) fn status(&self) -> &SdfMaterialStatus {
        &self.status
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::compositor::{Compositor, SceneKey};
    use crate::core::math::RectF;
    use crate::core::widget_tree::WidgetTree;
    use crate::renderer::custom_render_prepare::CustomRenderTargetSnapshot;
    use crate::renderer::pixel_coverage::PhysicalExtent;
    use crate::renderer::wgpu_renderer::WgpuRenderer;
    use crate::widgets::label::Label;

    #[test]
    fn sdf_material_status_rejects_obsolete_publications() {
        let revision = SdfMaterialRevision::initial();
        let next_revision = revision.checked_increment().unwrap();
        assert_eq!(revision.0.get(), 1);
        assert_eq!(next_revision.0.get(), 2);
        assert_eq!(
            SdfMaterialRevision(NonZeroU64::MAX).checked_increment(),
            Err(SdfMaterialRejection::RevisionOverflow)
        );

        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Label::new("first")), None, 0);
        let owner = tree.node(root).unwrap().id;
        let mut other_tree = WidgetTree::new();
        let other_root = other_tree.create_root(Box::new(Label::new("second")), None, 0);
        let other_owner = other_tree.node(other_root).unwrap().id;
        assert_eq!(owner, other_owner);
        let rect = RectF::new(0.0, 0.0, 120.0, 30.0);
        let mut compositor = Compositor::new(rect);
        let surface = compositor.create_surface(0, rect, SceneKey::BASE);
        let other_surface = compositor.create_surface(1, rect, SceneKey::BASE);
        let metrics = |surface| {
            CustomRenderTargetSnapshot::new(surface, PhysicalExtent::new(120, 30), 1.0)
                .unwrap()
                .metrics_for(rect, rect)
                .unwrap()
        };
        let key = |tree: &WidgetTree, owner, surface, revision| {
            SdfMaterialKey::new(
                tree.snapshot_tree_lifetime(),
                owner,
                surface,
                revision,
                metrics(surface),
            )
            .unwrap()
        };
        let initial_key = key(&tree, owner, surface, revision);
        let current_key = key(&tree, owner, surface, next_revision);
        let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
            .expect("real Image admission requires a headless renderer");
        let texture_id = 4_202_610_302;
        renderer.register_image_rgba8_srgb(texture_id, 1, 1, &[255; 4]);
        let old_binding = renderer.admit_image(texture_id).unwrap();
        let old_identity = old_binding.identity();
        let initial = SdfMaterialPublication::new(
            &initial_key,
            initial_key.clone(),
            Some(old_identity),
            SdfMaterialStatus::Ready {
                binding: old_binding.clone(),
            },
        )
        .unwrap();
        let mut selected = Some(initial.clone());
        assert_eq!(
            SdfMaterialPublication::replace_if_current(
                &mut selected,
                &initial_key,
                Some(old_identity),
                None,
            ),
            Err(SdfMaterialRejection::MissingEvidence)
        );
        assert_eq!(selected.as_ref(), Some(&initial));

        renderer.register_image_rgba8_srgb(texture_id, 1, 1, &[255; 4]);
        let binding = renderer.admit_image(texture_id).unwrap();
        let identity = binding.identity();
        assert_ne!(old_identity, identity);
        let ready = SdfMaterialPublication::new(
            &current_key,
            current_key.clone(),
            Some(identity),
            SdfMaterialStatus::Ready {
                binding: binding.clone(),
            },
        )
        .unwrap();
        SdfMaterialPublication::replace_if_current(
            &mut selected,
            &current_key,
            Some(identity),
            Some(ready.clone()),
        )
        .unwrap();
        assert_eq!(
            SdfMaterialPublication::replace_if_current(
                &mut selected,
                &current_key,
                Some(identity),
                Some(initial),
            ),
            Err(SdfMaterialRejection::KeyMismatch)
        );
        for wrong_key in [
            key(&other_tree, other_owner, surface, next_revision),
            key(&tree, owner, other_surface, next_revision),
        ] {
            assert_eq!(
                SdfMaterialPublication::new(
                    &current_key,
                    wrong_key,
                    Some(identity),
                    SdfMaterialStatus::Ready {
                        binding: binding.clone()
                    },
                ),
                Err(SdfMaterialRejection::KeyMismatch)
            );
        }
        assert_eq!(
            SdfMaterialPublication::new(
                &current_key,
                current_key.clone(),
                Some(identity),
                SdfMaterialStatus::Ready {
                    binding: old_binding.clone()
                },
            ),
            Err(SdfMaterialRejection::BindingIdentityMismatch)
        );
        assert_eq!(
            SdfMaterialPublication::new(
                &current_key,
                current_key.clone(),
                Some(old_identity),
                SdfMaterialStatus::Ready {
                    binding: old_binding
                },
            ),
            Err(SdfMaterialRejection::BindingUnavailable(
                ImageBindingFailure::Replaced
            ))
        );
        assert_eq!(selected.as_ref(), Some(&ready));

        renderer.unregister_image(texture_id);
        let cause = binding.availability().unwrap_err();
        assert_eq!(cause, ImageBindingFailure::Unregistered);
        let unavailable = SdfMaterialPublication::new(
            &current_key,
            current_key.clone(),
            Some(identity),
            SdfMaterialStatus::Unavailable {
                cause: SdfMaterialUnavailableCause::BindingRevoked {
                    withdrawn: identity,
                    cause,
                },
            },
        )
        .unwrap();
        SdfMaterialPublication::replace_if_current(
            &mut selected,
            &current_key,
            Some(identity),
            Some(unavailable.clone()),
        )
        .unwrap();
        assert_eq!(selected.as_ref(), Some(&unavailable));
        assert!(matches!(
            selected.as_ref().unwrap().status(),
            SdfMaterialStatus::Unavailable { .. }
        ));
        assert_eq!(
            SdfMaterialPublication::new(
                &current_key,
                current_key.clone(),
                Some(identity),
                SdfMaterialStatus::Unavailable {
                    cause: SdfMaterialUnavailableCause::BindingRevoked {
                        withdrawn: old_identity,
                        cause: ImageBindingFailure::Replaced,
                    },
                },
            ),
            Err(SdfMaterialRejection::BindingIdentityMismatch)
        );
        let admission_failure = renderer.admit_image(texture_id).unwrap_err();
        let failed_admission = SdfMaterialPublication::new(
            &current_key,
            current_key.clone(),
            None,
            SdfMaterialStatus::Unavailable {
                cause: SdfMaterialUnavailableCause::BindingAdmissionFailed(admission_failure),
            },
        )
        .unwrap();
        assert!(
            matches!(failed_admission.status(), SdfMaterialStatus::Unavailable {
            cause: SdfMaterialUnavailableCause::BindingAdmissionFailed(
                ImageAdmissionError::MissingRegistration(id)
            )
        } if *id == texture_id)
        );
    }
}
