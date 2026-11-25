//! HAL380: the geometry owner's proof, driven by the caller's CPU executor.
//! Context/registry/GPU authority stays in the UI pending and final registrar.
use super::context::{ContextId, NormalCpuPendingCharge3D};
use super::geometry as g;
use super::material_api::{
    self as api, PbrGeometry3D, PbrGeometryDescriptor3D, PbrSourceError3D, PbrSourceResult3D,
};
use super::{Context3D, Context3DError, GeometryIndices, PolicyBudget3D};
#[cfg(test)]
use std::sync::Weak;
use std::{
    fmt,
    marker::PhantomData,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

const HEADER: usize = 2 * std::mem::size_of::<usize>();
fn add(a: usize, b: usize) -> PbrSourceResult3D<usize> {
    a.checked_add(b)
        .ok_or(PbrSourceError3D::Association("geometry CPU size overflow"))
}
fn mul(a: usize, b: usize) -> PbrSourceResult3D<usize> {
    a.checked_mul(b)
        .ok_or(PbrSourceError3D::Association("geometry CPU size overflow"))
}
fn capacity(requested: usize, maximum: usize) -> PbrSourceError3D {
    Context3DError::PolicyBudgetExceeded {
        budget: PolicyBudget3D::RetainedDynamicCpuBytes,
        requested: requested as u64,
        maximum: maximum as u64,
    }
    .into()
}
fn cancelled() -> PbrSourceError3D {
    super::pbr::PbrError3D::GeometryCpuCancelled.into()
}
fn check(cancel: &AtomicBool) -> Result<(), super::pbr::PbrError3D> {
    if cancel.load(Ordering::Acquire) {
        Err(super::pbr::PbrError3D::GeometryCpuCancelled)
    } else {
        Ok(())
    }
}
fn arc<T>(value: &Arc<[T]>) -> (usize, usize) {
    (
        Arc::as_ptr(value) as *const () as usize,
        std::mem::size_of_val(value.as_ref()),
    )
}
fn indices(value: &GeometryIndices) -> (usize, usize) {
    match value {
        GeometryIndices::Unindexed => (0, 0),
        GeometryIndices::U16(v) => arc(v),
        GeometryIndices::U32(v) => arc(v),
    }
}
fn unique(values: &[(usize, usize)], excluded: &[(usize, usize)]) -> PbrSourceResult3D<usize> {
    let mut bytes = 0;
    for (i, &(address, size)) in values.iter().enumerate() {
        if address == 0
            || values[..i]
                .iter()
                .chain(excluded)
                .any(|&(p, _)| p == address)
        {
            continue;
        }
        bytes = add(bytes, add(size, HEADER)?)?;
    }
    Ok(bytes)
}
fn allocation(count: usize, stride: usize) -> PbrSourceResult3D<usize> {
    add(mul(count, stride)?, HEADER)
}
fn overhead() -> usize {
    std::mem::size_of::<PbrGeometryCpuPrepared3D>()
        + std::mem::size_of::<PbrGeometryCpuSuccessorJob3D>()
        + std::mem::size_of::<PbrGeometryCpuPending3D>()
        + std::mem::size_of::<PbrGeometryCpuResult3D>()
        + std::mem::size_of::<Binding>()
        + HEADER
        + HEADER
        + 2 * (HEADER + std::mem::size_of::<AtomicBool>())
        + std::mem::size_of::<NormalCpuPendingCharge3D>()
        + std::mem::size_of::<super::context::ControlledResourceLease3D>()
}
/// HAL-calculated CPU storage bounds, not a geometry certificate or reservation.
/// Input allocations are already owned by the caller; additional bounds exclude
/// them, including input Arcs moved into the completed private source.
/// ```compile_fail
/// use halcyon::three_d::PbrGeometryCpuCost3D;
/// let forged = PbrGeometryCpuCost3D {};
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct PbrGeometryCpuCost3D {
    input: usize,
    additional_peak: usize,
    additional_retained: usize,
}

impl PbrGeometryCpuCost3D {
    /// Unique input Arc payloads and headers, deduplicated within this input.
    pub fn input_cpu_bytes(self) -> usize {
        self.input
    }
    /// Maximum additional conversion/checker storage and fixed lifecycle cost.
    pub fn additional_peak_cpu_bytes(self) -> usize {
        self.additional_peak
    }
    /// Additional private source storage and fixed lifecycle cost after success.
    pub fn additional_retained_cpu_bytes(self) -> usize {
        self.additional_retained
    }

    fn total_peak(self) -> PbrSourceResult3D<usize> {
        add(self.input, self.additional_peak)
    }
}

// Exact allocation identities for moved input Arcs; fresh conversion buffers are
// distinct. The maxima cover Vec->Arc copies and the checker's frame scratch.
// Both preflight and successful preparation use this one checked formula.
fn preparation_cost(input: &PbrGeometryDescriptor3D) -> PbrSourceResult3D<PbrGeometryCpuCost3D> {
    let raw = unique(
        &[
            arc(&input.original.vertices),
            indices(&input.original.indices),
            arc(&input.original.signed_tangents),
            arc(&input.render),
            indices(&input.render_indices),
            arc(&input.render_vertex_sources),
            arc(&input.triangles),
        ],
        &[],
    )?;
    let o = mul(
        input.original.vertices.len(),
        std::mem::size_of::<g::PbrOriginalVertex3D>(),
    )?;
    let t = mul(
        input.triangles.len(),
        std::mem::size_of::<g::PbrTriangleCertificate3D>(),
    )?;
    let r = mul(
        input.render.len(),
        std::mem::size_of::<g::PbrVertexSource3D>(),
    )?;
    let packed = mul(input.render.len(), std::mem::size_of::<g::PbrVertex52>())?;
    let f = mul(
        input.triangles.len(),
        std::mem::size_of::<g::PbrCheckedFrame3D>(),
    )?;
    let oa = allocation(
        input.original.vertices.len(),
        std::mem::size_of::<g::PbrOriginalVertex3D>(),
    )?;
    let ta = allocation(
        input.triangles.len(),
        std::mem::size_of::<g::PbrTriangleCertificate3D>(),
    )?;
    let ra = allocation(
        input.render.len(),
        std::mem::size_of::<g::PbrVertexSource3D>(),
    )?;
    let fa = allocation(
        input.triangles.len(),
        std::mem::size_of::<g::PbrCheckedFrame3D>(),
    )?;
    let a = add(o, oa)?;
    let b = add(oa, add(t, ta)?)?;
    let c = add(add(oa, ta)?, add(packed, r)?)?;
    let d = add(add(oa, ta)?, add(r, ra)?)?;
    let e = add(
        add(oa, ta)?,
        add(
            ra,
            add(
                add(f, fa)?,
                add(std::mem::size_of::<g::PbrCertifiedGeometry3D>(), HEADER)?,
            )?,
        )?,
    )?;
    let additional_peak = add(a.max(b).max(c).max(d).max(e), overhead())?;
    let additional_retained = add(
        add(
            add(oa, ta)?,
            add(
                ra,
                add(
                    fa,
                    add(std::mem::size_of::<g::PbrCertifiedGeometry3D>(), HEADER)?,
                )?,
            )?,
        )?,
        overhead(),
    )?;
    additional_peak
        .checked_sub(additional_retained)
        .ok_or(PbrSourceError3D::Association(
            "geometry CPU retained storage exceeds peak",
        ))?;
    let cost = PbrGeometryCpuCost3D {
        input: raw,
        additional_peak,
        additional_retained,
    };
    cost.total_peak()?;
    Ok(cost)
}

/// HAL-sealed complete original geometry proof. No Context/GPU is reachable.
/// ```compile_fail
/// use halcyon::three_d::PbrGeometryCpuPrepared3D;
/// let forged = PbrGeometryCpuPrepared3D {};
/// ```
pub struct PbrGeometryCpuPrepared3D {
    verified: g::VerifiedCpuGeometry3D,
    cancel: Arc<AtomicBool>,
    cost: PbrGeometryCpuCost3D,
}
impl fmt::Debug for PbrGeometryCpuPrepared3D {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PbrGeometryCpuPrepared3D")
            .field("cpu_cost", &self.cost)
            .finish_non_exhaustive()
    }
}
impl PbrGeometryCpuPrepared3D {
    /// Pure bounded accounting before conversion or verification. This inspects
    /// only the fixed set of input Arc identities/lengths; it proves no source.
    pub fn cost_for_input(
        input: &PbrGeometryDescriptor3D,
    ) -> PbrSourceResult3D<PbrGeometryCpuCost3D> {
        preparation_cost(input)
    }

    /// The matching HAL-owned receipt, available only after full source proof.
    /// Account only its additional fields beyond an already charged input.
    pub fn cpu_cost(&self) -> PbrGeometryCpuCost3D {
        self.cost
    }

    pub fn prepare(
        input: PbrGeometryDescriptor3D,
        maximum_cpu_bytes: usize,
        cancel: Arc<AtomicBool>,
    ) -> PbrSourceResult3D<Self> {
        check(&cancel)?;
        let cost = Self::cost_for_input(&input)?;
        let peak = cost.total_peak()?;
        if peak > maximum_cpu_bytes {
            return Err(capacity(peak, maximum_cpu_bytes));
        }
        let mut checkpoint = || check(&cancel);
        let (render, indices, source) = api::convert_geometry_input(input, &mut checkpoint)?;
        let verified = g::PbrGeometrySource3D::prepare_cpu_certified(
            render,
            indices,
            source,
            &mut checkpoint,
        )?;
        checkpoint()?;
        Ok(Self {
            verified,
            cancel,
            cost,
        })
    }
    #[cfg(test)]
    pub(crate) fn peak_for_test(&self) -> usize {
        self.cost.total_peak().expect("sealed checked CPU cost")
    }
}

struct Binding {
    issuer: ContextId,
    previous: Option<(u32, u64)>,
    cancel: Arc<AtomicBool>,
    // Binding remains live until all worker CPU source fields have dropped.
    _ticket: Arc<()>,
}
/// CPU-only successor check against the actual captured previous source.
pub struct PbrGeometryCpuSuccessorJob3D {
    verified: g::VerifiedCpuGeometry3D,
    previous: Option<g::PbrGeometrySource3D>,
    binding: Arc<Binding>,
}
/// One-use HAL-created result. It contains no resource handle or lease.
/// ```compile_fail
/// use halcyon::three_d::PbrGeometryCpuResult3D;
/// fn duplicate(result:PbrGeometryCpuResult3D) { let _ = result.clone(); }
/// ```
pub struct PbrGeometryCpuResult3D {
    source: g::AdmissibleCpuGeometry3D,
    cancel: Arc<AtomicBool>,
    binding: Arc<Binding>,
}
impl fmt::Debug for PbrGeometryCpuResult3D {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PbrGeometryCpuResult3D")
            .finish_non_exhaustive()
    }
}
/// UI-only reservation and original predecessor custody.
/// ```compile_fail
/// use halcyon::three_d::PbrGeometryCpuPending3D;
/// fn send<T:Send>() {}
/// send::<PbrGeometryCpuPending3D>();
/// ```
pub struct PbrGeometryCpuPending3D {
    context: Context3D,
    previous: Option<PbrGeometry3D>,
    binding: Option<Arc<Binding>>,
    charge: Option<Box<NormalCpuPendingCharge3D>>,
    _ui: PhantomData<Rc<()>>,
}
impl Drop for PbrGeometryCpuPending3D {
    fn drop(&mut self) {
        drop(self.binding.take());
        if let Some(charge) = self.charge.take() {
            self.context.retire_normal_cpu_charge(charge);
        }
    }
}
impl PbrGeometryCpuSuccessorJob3D {
    #[cfg(test)]
    pub(crate) fn escrow_witness_for_test(
        &self,
    ) -> (
        Weak<()>,
        Weak<[g::PbrVertexSource3D]>,
        Option<Weak<[g::PbrVertexSource3D]>>,
        Option<(u32, u64)>,
    ) {
        (
            Arc::downgrade(&self.binding._ticket),
            Arc::downgrade(&self.verified.source().vertices),
            self.previous
                .as_ref()
                .map(|source| Arc::downgrade(&source.vertices)),
            self.binding.previous,
        )
    }

    pub fn verify(self, cancel: Arc<AtomicBool>) -> PbrSourceResult3D<PbrGeometryCpuResult3D> {
        let mut checkpoint = || {
            check(&self.binding.cancel)?;
            check(&cancel)
        };
        let source = self
            .verified
            .successor(self.previous.as_ref(), &mut checkpoint)?;
        checkpoint()?;
        drop(self.previous);
        Ok(PbrGeometryCpuResult3D {
            source,
            cancel,
            binding: self.binding,
        })
    }
}
impl Context3D {
    pub fn begin_pbr_geometry_cpu_admission(
        &self,
        prepared: PbrGeometryCpuPrepared3D,
        previous: Option<&PbrGeometry3D>,
    ) -> PbrSourceResult3D<(PbrGeometryCpuSuccessorJob3D, PbrGeometryCpuPending3D)> {
        check(&prepared.cancel)?;
        if let Some(failure) = self.device_failure() {
            return Err(PbrSourceError3D::Gpu(format!("{failure:?}")));
        }
        self.check_pbr_geometry_limits(prepared.verified.source())?;
        let prior = previous
            .map(|p| self.lease_pbr_geometry(&p.0))
            .transpose()?;
        let allocations = prepared.verified.source().cpu_allocations();
        let prior_allocations = prior
            .as_ref()
            .map(|p| p.data.source.cpu_allocations())
            .unwrap_or([(0, 0); 9]);
        let retained = add(unique(&allocations, &prior_allocations)?, overhead())?;
        let ticket = Arc::new(());
        let mut charge =
            self.reserve_normal_cpu_charge(retained as u64, Arc::downgrade(&ticket))?;
        let mut escrow = Vec::new();
        if let Some(prior) = &prior {
            escrow.try_reserve_exact(1).map_err(|_| {
                PbrSourceError3D::Association("geometry CPU escrow allocation unavailable")
            })?;
            escrow.push(prior.ownership.clone());
        }
        self.escrow_cpu_geometry(&mut charge, escrow);
        let binding = Arc::new(Binding {
            issuer: self.normal_cpu_issuer(),
            previous: previous.map(PbrGeometry3D::resource_generation),
            cancel: prepared.cancel,
            _ticket: ticket,
        });
        let job = PbrGeometryCpuSuccessorJob3D {
            verified: prepared.verified,
            previous: prior.as_ref().map(|p| p.data.source.clone()),
            binding: binding.clone(),
        };
        let pending = PbrGeometryCpuPending3D {
            context: self.clone(),
            previous: previous.cloned(),
            binding: Some(binding),
            charge: Some(charge),
            _ui: PhantomData,
        };
        Ok((job, pending))
    }

    pub fn finish_pbr_geometry_cpu_admission(
        &self,
        pending: PbrGeometryCpuPending3D,
        result: PbrGeometryCpuResult3D,
        previous: Option<&PbrGeometry3D>,
    ) -> PbrSourceResult3D<PbrGeometry3D> {
        let binding = pending
            .binding
            .as_ref()
            .ok_or(PbrSourceError3D::Association(
                "missing geometry CPU pending",
            ))?;
        if !Arc::ptr_eq(binding, &result.binding)
            || !pending.context.same_resource_family(self)
            || !binding.issuer.same_as(&self.normal_cpu_issuer())
            || binding.previous != previous.map(PbrGeometry3D::resource_generation)
            || pending.previous.as_ref() != previous
        {
            return Err(PbrSourceError3D::Association(
                "geometry CPU pending/result/predecessor mismatch",
            ));
        }
        if binding.cancel.load(Ordering::Acquire) || result.cancel.load(Ordering::Acquire) {
            return Err(cancelled());
        }
        if let Some(failure) = self.device_failure() {
            return Err(PbrSourceError3D::Gpu(format!("{failure:?}")));
        }
        let prior = previous
            .map(|p| self.lease_pbr_geometry(&p.0))
            .transpose()?;
        let PbrGeometryCpuResult3D {
            source,
            cancel: _,
            binding,
        } = result;
        let registered = self
            .register_cpu_geometry(source, previous.map(|p| &p.0))
            .map(PbrGeometry3D)
            .map_err(PbrSourceError3D::from);
        drop(prior);
        drop(binding);
        drop(pending);
        self.collect_normal_cpu_charges();
        registered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hal380_cpu_cost_checked_size_arithmetic() {
        for result in [
            add(usize::MAX, 1),
            mul(usize::MAX, 2),
            allocation(usize::MAX, 1),
        ] {
            assert!(matches!(
                result,
                Err(PbrSourceError3D::Association("geometry CPU size overflow"))
            ));
        }
    }

    #[test]
    fn hal380_cancellation_inside_conversion_frame_and_successor_work() {
        let identity = api::PbrOriginalIdentity3D {
            source: 380,
            revision: 1,
            node: 0,
            mesh: 0,
            primitive: 0,
        };
        let positions = [[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [0.0, 1.0, 0.0]];
        let uv = [[0.0, 0.0], [1.0, 0.0], [0.5, 1.0]];
        let input = api::PbrGeometryDescriptor3D {
            original: api::PbrOriginalGeometry3D {
                identity,
                vertices: (0..3)
                    .map(|i| api::PbrOriginalVertex3D {
                        position: positions[i],
                        normal: [0.0, 0.0, 1.0],
                        uv0: Some(uv[i]),
                        color: None,
                    })
                    .collect(),
                indices: GeometryIndices::Unindexed,
                signed_tangents: Arc::from([[1.0, 0.0, 0.0, 1.0]; 3]),
                tangent_provenance: api::PbrTangentProvenance3D::Authored,
            },
            render: (0..3)
                .map(|i| api::PbrVertex3D {
                    position: positions[i],
                    normal: [0.0, 0.0, 1.0],
                    uv0: uv[i],
                    color: [255; 4],
                    tangent: [1.0, 0.0, 0.0, 1.0],
                })
                .collect(),
            render_indices: GeometryIndices::Unindexed,
            render_vertex_sources: Arc::from([0, 1, 2]),
            triangles: Arc::from([api::PbrTriangleCertificate3D {
                original: identity,
                triangle: 0,
                certificate_identity: 380,
                certificate_revision: 1,
                frame: api::PbrFrameCertificate3D::OldCone,
            }]),
            mapped: true,
            maximum_certificate_bytes: 4096,
            maximum_verification_items: 16,
        };
        let input_allocations = [
            arc(&input.original.vertices),
            indices(&input.original.indices),
            arc(&input.original.signed_tangents),
            arc(&input.render),
            indices(&input.render_indices),
            arc(&input.render_vertex_sources),
            arc(&input.triangles),
        ];
        // Keep the original input allocation referents alive: a later allocator
        // address reuse is not a physically shared allocation.
        let prepared = PbrGeometryCpuPrepared3D::prepare(
            input.clone(),
            1 << 20,
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        let receipt = prepared.cpu_cost();
        assert_eq!(
            receipt,
            PbrGeometryCpuPrepared3D::cost_for_input(&input).unwrap()
        );
        assert_eq!(
            receipt.input_cpu_bytes(),
            unique(&input_allocations, &[]).unwrap()
        );
        assert_eq!(
            receipt.additional_retained_cpu_bytes(),
            add(
                unique(
                    &prepared.verified.source().cpu_allocations(),
                    &input_allocations
                )
                .unwrap(),
                overhead()
            )
            .unwrap()
        );
        drop(prepared);
        let mut calls = 0;
        let mut stop = || {
            calls += 1;
            if calls == 3 {
                Err(super::super::pbr::PbrError3D::GeometryCpuCancelled)
            } else {
                Ok(())
            }
        };
        assert!(matches!(
            api::convert_geometry_input(input.clone(), &mut stop),
            Err(PbrSourceError3D::Association(
                "geometry CPU operation cancelled"
            ))
        ));
        let (render, indices, source) =
            api::convert_geometry_input(input.clone(), &mut || Ok(())).unwrap();
        let mut calls = 0;
        let mut stop = || {
            calls += 1;
            if calls == 15 {
                Err(super::super::pbr::PbrError3D::GeometryCpuCancelled)
            } else {
                Ok(())
            }
        };
        assert!(matches!(
            g::PbrGeometrySource3D::prepare_cpu_certified(render, indices, source, &mut stop),
            Err(super::super::pbr::PbrError3D::GeometryCpuCancelled)
        ));
        let previous = PbrGeometryCpuPrepared3D::prepare(
            input.clone(),
            1 << 20,
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        let next =
            PbrGeometryCpuPrepared3D::prepare(input, 1 << 20, Arc::new(AtomicBool::new(false)))
                .unwrap();
        let mut calls = 0;
        let mut stop = || {
            calls += 1;
            if calls == 8 {
                Err(super::super::pbr::PbrError3D::GeometryCpuCancelled)
            } else {
                Ok(())
            }
        };
        assert!(matches!(
            next.verified
                .successor(Some(previous.verified.source()), &mut stop),
            Err(super::super::pbr::PbrError3D::GeometryCpuCancelled)
        ));
    }
}
