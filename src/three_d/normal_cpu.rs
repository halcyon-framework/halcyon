//! HAL373 CPU input custody and the existing normal-proof owner's handoff.
//! This module starts no worker and owns no App wake, Scene, or GPU operation.
use super::context::{ContextId, NormalCpuPendingCharge3D};
use super::material_api::*;
use super::mip_sampling as m;
use super::{Context3D, Context3DError};
use std::fmt;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Untrusted producer-authored CPU values, before a live sampler is bound.
#[derive(Debug, Clone)]
pub struct PbrNormalCpuProofSource3D {
    pub identity: u64,
    pub revision: u64,
    pub source: Arc<PbrMipChainDescriptor3D>,
    pub policy: PbrMipSamplerDescriptor3D,
    pub normal_scale_bits: u32,
    pub cells: u64,
    pub maximum_work: u64,
    pub maximum_metadata_bytes: u64,
    pub default: PbrNormalWitness3D,
    pub exceptions: Arc<[PbrNormalException3D]>,
}

/// Operational capacity/cancellation/binding is distinct from a source inequality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PbrNormalCpuProofErrorKind3D {
    Cancelled,
    Capacity,
    Binding,
    Source,
}

/// A finish refusal retains its CPU result. Move the error to the producer's
/// CPU owner for disposal; no large proof payload is destroyed inside UI finish.
pub struct PbrNormalCpuProofError3D {
    kind: PbrNormalCpuProofErrorKind3D,
    error: Option<PbrSourceError3D>,
    work: u64,
    cleanup: Option<PbrNormalCpuProofResult3D>,
}
impl PbrNormalCpuProofError3D {
    pub fn kind(&self) -> PbrNormalCpuProofErrorKind3D {
        self.kind
    }
    pub fn source_error(&self) -> Option<&PbrSourceError3D> {
        self.error.as_ref()
    }
    pub fn work(&self) -> u64 {
        self.work
    }
    fn binding(error: impl Into<PbrSourceError3D>) -> Self {
        Self {
            kind: PbrNormalCpuProofErrorKind3D::Binding,
            error: Some(error.into()),
            work: 0,
            cleanup: None,
        }
    }
    fn capacity(cause: &'static str) -> Self {
        Self {
            kind: PbrNormalCpuProofErrorKind3D::Capacity,
            error: Some(PbrSourceError3D::Association(cause)),
            work: 0,
            cleanup: None,
        }
    }
    fn cancelled(work: u64) -> Self {
        Self {
            kind: PbrNormalCpuProofErrorKind3D::Cancelled,
            error: None,
            work,
            cleanup: None,
        }
    }
    fn proof(error: m::NormalCpuFailure3D, work: u64) -> Self {
        match error {
            m::NormalCpuFailure3D::Cancelled => Self::cancelled(work),
            m::NormalCpuFailure3D::Source(error) => {
                let error = PbrSourceError3D::from(error);
                let capacity = matches!(
                    &error,
                    PbrSourceError3D::Context(Context3DError::PolicyBudgetExceeded { .. })
                        | PbrSourceError3D::Association(
                            "bounded source adapter storage unavailable"
                        )
                        | PbrSourceError3D::Sampling {
                            field: "normal grid",
                            cause: "bounded grid allocation unavailable",
                        }
                        | PbrSourceError3D::Sampling {
                            field: "normal branches",
                            cause: "bounded allocation unavailable",
                        }
                );
                Self {
                    kind: if capacity {
                        PbrNormalCpuProofErrorKind3D::Capacity
                    } else {
                        PbrNormalCpuProofErrorKind3D::Source
                    },
                    error: Some(error),
                    work,
                    cleanup: None,
                }
            }
        }
    }
}
impl fmt::Debug for PbrNormalCpuProofError3D {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PbrNormalCpuProofError3D")
            .field("kind", &self.kind)
            .field("error", &self.error)
            .field("work", &self.work)
            .field("owns_cpu_cleanup", &self.cleanup.is_some())
            .finish()
    }
}
impl fmt::Display for PbrNormalCpuProofError3D {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.error {
            Some(error) => write!(f, "{:?}: {error} (normal work {})", self.kind, self.work),
            None => write!(f, "normal CPU proof cancelled (normal work {})", self.work),
        }
    }
}
impl std::error::Error for PbrNormalCpuProofError3D {}

struct PreparedCapture {
    raw: PbrNormalCpuProofSource3D,
    retained_bytes: u64,
    reservation_bytes: u64,
}

/// HAL-sealed size and exact immutable captures; not a mathematical certificate.
/// Keep this producer-owned value until its jobs/results are settled, and release
/// it on the CPU producer. Begin only borrows it, including on quota refusal.
///
/// A consumer cannot fabricate a prepared seal:
/// ```compile_fail
/// use halcyon::three_d::PbrNormalCpuProofPrepared3D;
/// let forged = PbrNormalCpuProofPrepared3D {};
/// ```
pub struct PbrNormalCpuProofPrepared3D {
    capture: Arc<PreparedCapture>,
}
impl fmt::Debug for PbrNormalCpuProofPrepared3D {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PbrNormalCpuProofPrepared3D")
            .field("retained_bytes", &self.capture.retained_bytes)
            .field("reservation_bytes", &self.capture.reservation_bytes)
            .finish()
    }
}

#[cfg(test)]
impl PbrNormalCpuProofPrepared3D {
    pub(crate) fn storage_for_test(&self) -> (u64, u64, usize) {
        (
            self.capture.retained_bytes,
            self.capture.reservation_bytes,
            Arc::strong_count(&self.capture),
        )
    }
}

fn checked_bytes(count: usize, size: usize) -> Result<u64, PbrNormalCpuProofError3D> {
    (count as u64)
        .checked_mul(size as u64)
        .ok_or_else(|| PbrNormalCpuProofError3D::capacity("normal CPU storage size overflow"))
}
fn add_bytes(total: &mut u64, bytes: u64) -> Result<(), PbrNormalCpuProofError3D> {
    *total = total
        .checked_add(bytes)
        .ok_or_else(|| PbrNormalCpuProofError3D::capacity("normal CPU storage sum overflow"))?;
    Ok(())
}
fn within(bytes: u64, maximum: u64) -> Result<(), PbrNormalCpuProofError3D> {
    if bytes > maximum {
        Err(PbrNormalCpuProofError3D::capacity(
            "normal CPU producer byte budget exceeded",
        ))
    } else {
        Ok(())
    }
}
const ARC_HEADER: u64 = (2 * std::mem::size_of::<usize>()) as u64;
type Allocation = (usize, usize);
fn count_arc<T: ?Sized>(
    value: &Arc<T>,
    payload: u64,
    seen: &mut Vec<Allocation>,
    total: &mut u64,
    control: &m::NormalCpuControl3D<'_>,
) -> Result<(), PbrNormalCpuProofError3D> {
    let identity = (
        Arc::as_ptr(value) as *const () as usize,
        std::mem::size_of_val(value.as_ref()),
    );
    count_allocation(identity, payload, seen, total, control)
}
fn count_allocation(
    identity: Allocation,
    payload: u64,
    seen: &mut Vec<Allocation>,
    total: &mut u64,
    control: &m::NormalCpuControl3D<'_>,
) -> Result<(), PbrNormalCpuProofError3D> {
    for old in seen.iter() {
        control
            .checkpoint()
            .map_err(|e| PbrNormalCpuProofError3D::proof(e, 0))?;
        if *old == identity {
            return Ok(());
        }
    }
    seen.push(identity); // Exact finite capacity reserved before the walk.
    add_bytes(total, payload)?;
    add_bytes(total, ARC_HEADER)
}

impl PbrNormalCpuProofPrepared3D {
    /// CPU-only size preparation under the caller's existing finite byte budget.
    /// The budget is a LIMIT, never an asserted size or a source-validity badge.
    pub fn prepare(
        source: PbrNormalCpuProofSource3D,
        maximum_cpu_bytes: u64,
        cancel: &AtomicBool,
    ) -> Result<Self, PbrNormalCpuProofError3D> {
        let control = m::NormalCpuControl3D::new(cancel);
        control
            .checkpoint()
            .map_err(|e| PbrNormalCpuProofError3D::proof(e, 0))?;
        let count = source.exceptions.len().checked_add(6).ok_or_else(|| {
            PbrNormalCpuProofError3D::capacity("normal CPU allocation count overflow")
        })?;
        let table_bytes = checked_bytes(count, std::mem::size_of::<Allocation>())?;
        within(table_bytes, maximum_cpu_bytes)?;
        let mut seen = Vec::new();
        seen.try_reserve_exact(count).map_err(|_| {
            PbrNormalCpuProofError3D::capacity("normal CPU allocation accounting unavailable")
        })?;
        let table_bytes = checked_bytes(seen.capacity(), std::mem::size_of::<Allocation>())?;
        let mut retained = std::mem::size_of::<PreparedCapture>() as u64 + ARC_HEADER;
        count_arc(
            &source.source,
            std::mem::size_of::<PbrMipChainDescriptor3D>() as u64,
            &mut seen,
            &mut retained,
            &control,
        )?;
        count_arc(
            &source.source.levels,
            checked_bytes(
                source.source.levels.len(),
                std::mem::size_of::<PbrMipLevel3D>(),
            )?,
            &mut seen,
            &mut retained,
            &control,
        )?;
        count_arc(
            &source.source.texels,
            source.source.texels.len() as u64,
            &mut seen,
            &mut retained,
            &control,
        )?;
        count_arc(
            &source.source.provenance.original,
            source.source.provenance.original.len() as u64,
            &mut seen,
            &mut retained,
            &control,
        )?;
        // Even an expired Weak retains its Arc allocation. Before source proof
        // it need not name texels, so count its slice metadata without upgrading
        // or dereferencing it. A valid same-allocation derived capture dedupes.
        let derived = source.source.provenance.derived.as_ptr();
        let empty: std::sync::Weak<[u8]> = std::sync::Weak::<[u8; 0]>::new();
        if !std::sync::Weak::ptr_eq(&source.source.provenance.derived, &empty) {
            count_allocation(
                (derived as *const () as usize, derived.len()),
                derived.len() as u64,
                &mut seen,
                &mut retained,
                &control,
            )?;
        }
        count_arc(
            &source.exceptions,
            checked_bytes(
                source.exceptions.len(),
                std::mem::size_of::<PbrNormalException3D>(),
            )?,
            &mut seen,
            &mut retained,
            &control,
        )?;
        // The original adapter creates a distinct private leaf allocation per
        // exception even when public leaves are shared. Retention is deduped;
        // that actual future conversion liability is not.
        let mut conversion = checked_bytes(
            source.source.levels.len(),
            std::mem::size_of::<m::MipLevel3D>(),
        )?;
        let exception_bytes = checked_bytes(
            source.exceptions.len(),
            std::mem::size_of::<m::NormalException3D>(),
        )?;
        let mut transfer_scratch = conversion.max(exception_bytes);
        add_bytes(&mut conversion, exception_bytes)?;
        add_bytes(&mut conversion, ARC_HEADER * 2)?;
        for exception in source.exceptions.iter() {
            control
                .checkpoint()
                .map_err(|e| PbrNormalCpuProofError3D::proof(e, 0))?;
            let bytes = checked_bytes(
                exception.leaves.len(),
                std::mem::size_of::<PbrNormalBox3D>(),
            )?;
            count_arc(&exception.leaves, bytes, &mut seen, &mut retained, &control)?;
            add_bytes(
                &mut conversion,
                checked_bytes(
                    exception.leaves.len(),
                    std::mem::size_of::<m::NormalBox3D>(),
                )?,
            )?;
            transfer_scratch = transfer_scratch.max(checked_bytes(
                exception.leaves.len(),
                std::mem::size_of::<m::NormalBox3D>(),
            )?);
            add_bytes(&mut conversion, ARC_HEADER)?;
            let mut peak = retained;
            add_bytes(&mut peak, table_bytes)?;
            within(peak, maximum_cpu_bytes)?;
            // All leaves are checkpointed; a last huge allocation is charged
            // and refused before spending work on its contents.
            for _ in exception.leaves.iter() {
                control
                    .checkpoint()
                    .map_err(|e| PbrNormalCpuProofError3D::proof(e, 0))?;
            }
        }
        let base = source.source.levels.first();
        let scratch = m::normal_workspace_bytes(
            base.map_or(0, |x| x.width),
            base.map_or(0, |x| x.height),
            source.source.levels.len(),
        )
        .map_err(|e| PbrNormalCpuProofError3D::proof(e.into(), 0))?;
        let mut reservation = retained;
        add_bytes(&mut reservation, conversion)?;
        // Vec -> Arc transfer has a real transient buffer; verification grids
        // run after conversion. Charge the larger working phase, not a guessed
        // multiplier or two simultaneous copies of a shared input allocation.
        add_bytes(&mut reservation, scratch.max(transfer_scratch))?;
        add_bytes(
            &mut reservation,
            (std::mem::size_of::<PbrNormalCpuProofPrepared3D>()
                + std::mem::size_of::<PbrNormalCpuProofJob3D>()
                + std::mem::size_of::<PbrNormalCpuProofResult3D>()
                + std::mem::size_of::<PbrNormalCpuProofPending3D>()
                + std::mem::size_of::<Binding>()
                + std::mem::size_of::<NormalCpuPendingCharge3D>()
                + std::mem::size_of::<m::NormalCpuCertificate3D>()) as u64,
        )?;
        add_bytes(&mut reservation, m::normal_cpu_fixed_storage_bytes())?;
        add_bytes(&mut reservation, ARC_HEADER * 8)?;
        within(reservation, maximum_cpu_bytes)?;
        control
            .checkpoint()
            .map_err(|e| PbrNormalCpuProofError3D::proof(e, 0))?;
        Ok(Self {
            capture: Arc::new(PreparedCapture {
                raw: source,
                retained_bytes: retained,
                reservation_bytes: reservation,
            }),
        })
    }
}

struct Binding {
    issuer: ContextId,
    sampler: (u32, u64),
    previous: Option<(u32, u64)>,
    policy: m::MipSamplerSource3D,
    scale: f32,
    limits: m::NormalCpuLimits3D,
    _ticket: Arc<()>,
    finish_started: AtomicBool,
}

/// CPU-only job; verify consumes it. No Context or resource lease is reachable.
pub struct PbrNormalCpuProofJob3D {
    capture: Arc<PreparedCapture>,
    previous_source: Option<Arc<m::MipChainSource3D>>,
    // Rust drops fields in declaration order. The retirement ticket must die
    // only AFTER every CPU payload held by this job has been relinquished.
    binding: Arc<Binding>,
}

/// UI-owned charge and binding. Rc marker makes movement to a worker a type error.
///
/// ```compile_fail
/// use halcyon::three_d::PbrNormalCpuProofPending3D;
/// fn require_send<T: Send>() {}
/// require_send::<PbrNormalCpuProofPending3D>();
/// ```
pub struct PbrNormalCpuProofPending3D {
    context: Context3D,
    binding: Option<Arc<Binding>>,
    charge: Option<Box<NormalCpuPendingCharge3D>>,
    _ui: PhantomData<Rc<()>>,
}
impl Drop for PbrNormalCpuProofPending3D {
    fn drop(&mut self) {
        // Retire only after dropping this UI ticket. A still-live CPU result/job
        // keeps the charge in its original Context owner until CPU use settles.
        drop(self.binding.take());
        if let Some(charge) = self.charge.take() {
            self.context.retire_normal_cpu_charge(charge);
        }
    }
}

/// One-use HAL-created full proof. PRIVATE fields, deliberately no Clone.
///
/// ```compile_fail
/// use halcyon::three_d::PbrNormalCpuProofResult3D;
/// fn require_clone<T: Clone>() {}
/// require_clone::<PbrNormalCpuProofResult3D>();
/// ```
///
/// Consumption prevents a second finish with the same value:
/// ```compile_fail
/// use halcyon::three_d::{Context3D, PbrMipSampler3D,
///     PbrNormalCpuProofPending3D, PbrNormalCpuProofResult3D};
/// fn twice(context: &Context3D, sampler: &PbrMipSampler3D,
///     a: PbrNormalCpuProofPending3D, b: PbrNormalCpuProofPending3D,
///     result: PbrNormalCpuProofResult3D) {
///     let _ = context.finish_pbr_normal_cpu_proof(a, result, sampler, None);
///     let _ = context.finish_pbr_normal_cpu_proof(b, result, sampler, None);
/// }
/// ```
pub struct PbrNormalCpuProofResult3D {
    verified: m::NormalCpuVerified3D,
    source: Arc<PbrMipChainDescriptor3D>,
    previous_source: Option<Arc<m::MipChainSource3D>>,
    cancel: Arc<AtomicBool>,
    // Last, for the same actual CPU-settlement ordering as the job.
    binding: Arc<Binding>,
}
impl fmt::Debug for PbrNormalCpuProofResult3D {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PbrNormalCpuProofResult3D")
            .field("identity", &self.verified.certificate.identity)
            .field("revision", &self.verified.certificate.revision)
            .field("work", &self.verified.summary.work)
            .finish()
    }
}

#[cfg(test)]
impl PbrNormalCpuProofResult3D {
    pub(crate) fn observation_for_test(&self) -> ([u64; 5], [u8; 32], [usize; 3]) {
        let s = self.verified.summary;
        let source = &self.verified.certificate.source;
        (
            [
                s.cells,
                s.leaves,
                s.work,
                s.minimum_projection.to_bits(),
                s.maximum_interval_width.to_bits(),
            ],
            self.verified.certificate.digest,
            [
                Arc::as_ptr(&source.levels) as *const () as usize,
                Arc::as_ptr(&source.texels) as *const () as usize,
                Arc::as_ptr(&source.provenance) as usize,
            ],
        )
    }
}

#[cfg(test)]
impl Context3D {
    pub(crate) fn normal_cpu_chain_for_test(
        &self,
        chain: &PbrMipChain3D,
    ) -> ([u64; 5], [u8; 32], [usize; 3], bool) {
        let lease = self.lease_mip_chain(&chain.0).unwrap();
        let summary = lease.normal_domain_summary().unwrap();
        let (digest, negative) = lease.normal_cpu_record_for_test().unwrap();
        (
            [
                summary.cells,
                summary.leaves,
                summary.work,
                summary.minimum_projection.to_bits(),
                summary.maximum_interval_width.to_bits(),
            ],
            digest,
            [
                Arc::as_ptr(&lease.source.levels) as *const () as usize,
                Arc::as_ptr(&lease.source.texels) as *const () as usize,
                Arc::as_ptr(&lease.source.provenance) as usize,
            ],
            negative,
        )
    }
}

impl PbrNormalCpuProofJob3D {
    pub fn verify(
        self,
        cancel: Arc<AtomicBool>,
    ) -> Result<PbrNormalCpuProofResult3D, PbrNormalCpuProofError3D> {
        let control = m::NormalCpuControl3D::new(&cancel);
        let run = || -> Result<m::NormalCpuVerified3D, PbrNormalCpuProofError3D> {
            let raw = &self.capture.raw;
            let source = raw
                .source
                .private_reusing_cpu(self.previous_source.as_deref(), &control)
                .map_err(|e| PbrNormalCpuProofError3D::proof(e, control.work.get()))?;
            let mut exceptions = Vec::new();
            exceptions
                .try_reserve_exact(raw.exceptions.len())
                .map_err(|_| {
                    PbrNormalCpuProofError3D::capacity("bounded normal adapter storage unavailable")
                })?;
            for e in raw.exceptions.iter() {
                control
                    .checkpoint()
                    .map_err(|e| PbrNormalCpuProofError3D::proof(e, control.work.get()))?;
                let mut leaves = Vec::new();
                leaves.try_reserve_exact(e.leaves.len()).map_err(|_| {
                    PbrNormalCpuProofError3D::capacity("bounded source adapter storage unavailable")
                })?;
                for leaf in e.leaves.iter() {
                    control
                        .checkpoint()
                        .map_err(|e| PbrNormalCpuProofError3D::proof(e, control.work.get()))?;
                    leaves.push(m::NormalBox3D {
                        path: leaf.path,
                        depth: leaf.depth,
                        witness: leaf.witness.into(),
                    });
                }
                exceptions.push(m::NormalException3D {
                    cell: m::NormalCell3D {
                        mag: e.cell.mag,
                        linear: e.cell.linear,
                        lower: e.cell.lower,
                        upper: e.cell.upper,
                        u: e.cell.u,
                        v: e.cell.v,
                        u_choice: e.cell.u_choice,
                        v_choice: e.cell.v_choice,
                    },
                    leaves: leaves.into(),
                });
            }
            let body = m::NormalCpuCertificate3D::new(
                raw.identity,
                raw.revision,
                Arc::new(source),
                self.binding.sampler,
                raw.policy.into(),
                raw.normal_scale_bits,
                raw.cells,
                raw.maximum_work,
                raw.maximum_metadata_bytes,
                raw.default.into(),
                exceptions.into(),
            );
            m::verify_normal_cpu_source(
                body,
                self.binding.policy,
                self.binding.scale,
                self.binding.limits,
                &control,
            )
            .map_err(|e| PbrNormalCpuProofError3D::proof(e, control.work.get()))
        };
        let verified = run()?;
        Ok(PbrNormalCpuProofResult3D {
            verified,
            binding: self.binding.clone(),
            source: self.capture.raw.source.clone(),
            previous_source: self.previous_source.clone(),
            cancel,
        })
    }
}

fn previous_extra_bytes(
    old: &m::MipChainSource3D,
    input: &PbrMipChainDescriptor3D,
) -> Result<u64, PbrNormalCpuProofError3D> {
    let mut bytes = (std::mem::size_of::<m::MipChainSource3D>()
        + std::mem::size_of::<m::ChainProvenance3D>()) as u64
        + ARC_HEADER * 3;
    add_bytes(
        &mut bytes,
        checked_bytes(old.levels.len(), std::mem::size_of::<m::MipLevel3D>())?,
    )?;
    for (i, value) in [&old.texels, &old.provenance.original]
        .into_iter()
        .enumerate()
    {
        if Arc::ptr_eq(value, &input.texels)
            || Arc::ptr_eq(value, &input.provenance.original)
            || i == 1 && Arc::ptr_eq(value, &old.texels)
        {
            continue;
        }
        add_bytes(&mut bytes, value.len() as u64)?;
        add_bytes(&mut bytes, ARC_HEADER)?;
    }
    Ok(bytes)
}

impl Context3D {
    /// Borrowed input: a refused UI begin neither captures nor destroys payload.
    pub fn begin_pbr_normal_cpu_proof(
        &self,
        prepared: &PbrNormalCpuProofPrepared3D,
        sampler: &PbrMipSampler3D,
        normal_scale: f32,
        previous: Option<&PbrMipChain3D>,
    ) -> Result<(PbrNormalCpuProofJob3D, PbrNormalCpuProofPending3D), PbrNormalCpuProofError3D>
    {
        if let Some(error) = self.device_failure() {
            return Err(PbrNormalCpuProofError3D::binding(PbrSourceError3D::Gpu(
                format!("{error:?}"),
            )));
        }
        let live = self
            .lease_mip_sampler(&sampler.0)
            .map_err(|e| PbrNormalCpuProofError3D::binding(PbrSourceError3D::from(e)))?;
        let prior = previous
            .map(|p| self.lease_mip_chain(&p.0))
            .transpose()
            .map_err(|e| PbrNormalCpuProofError3D::binding(PbrSourceError3D::from(e)))?;
        let mut bytes = prepared.capture.reservation_bytes;
        if let Some(prior) = &prior {
            add_bytes(
                &mut bytes,
                previous_extra_bytes(&prior.source, &prepared.capture.raw.source)?,
            )?;
        }
        let ticket = Arc::new(());
        let charge = self
            .reserve_normal_cpu_charge(bytes, Arc::downgrade(&ticket))
            .map_err(|e| {
                let mut error = PbrNormalCpuProofError3D::binding(e);
                if matches!(
                    error.error,
                    Some(PbrSourceError3D::Context(
                        Context3DError::PolicyBudgetExceeded { .. }
                    ))
                ) {
                    error.kind = PbrNormalCpuProofErrorKind3D::Capacity;
                }
                error
            })?;
        let binding = Arc::new(Binding {
            issuer: self.normal_cpu_issuer(),
            sampler: sampler.resource_generation(),
            previous: previous.map(PbrMipChain3D::resource_generation),
            policy: live.source,
            scale: normal_scale,
            limits: m::NormalCpuLimits3D {
                hard_dimension: self.limits().hard.max_texture_dimension_2d,
                policy_dimension: self.limits().policy.max_texture_dimension_2d,
                texture_bytes: self.limits().policy.max_texture_bytes,
            },
            _ticket: ticket,
            finish_started: AtomicBool::new(false),
        });
        // Only after the actual reservation do new HAL CPU strong captures exist.
        let job = PbrNormalCpuProofJob3D {
            capture: prepared.capture.clone(),
            binding: binding.clone(),
            previous_source: prior.as_ref().map(|p| p.source.clone()),
        };
        let pending = PbrNormalCpuProofPending3D {
            context: self.clone(),
            binding: Some(binding),
            charge: Some(charge),
            _ui: PhantomData,
        };
        drop(prior);
        drop(live);
        Ok((job, pending))
    }

    pub fn finish_pbr_normal_cpu_proof(
        &self,
        pending: PbrNormalCpuProofPending3D,
        result: PbrNormalCpuProofResult3D,
        sampler: &PbrMipSampler3D,
        previous: Option<&PbrMipChain3D>,
    ) -> Result<PbrMipChain3D, PbrNormalCpuProofError3D> {
        let finish = || -> Result<PbrMipChain3D, PbrNormalCpuProofError3D> {
            let mismatch = || {
                PbrNormalCpuProofError3D::binding(PbrSourceError3D::Association(
                    "normal CPU pending/result binding mismatch",
                ))
            };
            let binding = pending.binding.as_ref().ok_or_else(mismatch)?;
            if !Arc::ptr_eq(binding, &result.binding)
                || !binding.issuer.same_as(&self.normal_cpu_issuer())
                || !pending.context.same_resource_family(self)
                || binding.sampler != sampler.resource_generation()
                || binding.previous != previous.map(PbrMipChain3D::resource_generation)
            {
                return Err(mismatch());
            }
            if binding.finish_started.swap(true, Ordering::AcqRel) {
                return Err(mismatch());
            }
            if result.cancel.load(Ordering::Acquire) {
                return Err(PbrNormalCpuProofError3D::cancelled(
                    result.verified.summary.work,
                ));
            }
            if let Some(error) = self.device_failure() {
                return Err(PbrNormalCpuProofError3D::binding(PbrSourceError3D::Gpu(
                    format!("{error:?}"),
                )));
            }
            let live = self
                .lease_mip_sampler(&sampler.0)
                .map_err(|e| PbrNormalCpuProofError3D::binding(PbrSourceError3D::from(e)))?;
            if live.source != binding.policy {
                return Err(mismatch());
            }
            let old = previous
                .map(|p| self.lease_mip_chain(&p.0))
                .transpose()
                .map_err(|e| PbrNormalCpuProofError3D::binding(PbrSourceError3D::from(e)))?;
            if !match (&old, &result.previous_source) {
                (None, None) => true,
                (Some(a), Some(b)) => Arc::ptr_eq(&a.source, b),
                _ => false,
            } {
                return Err(mismatch());
            }
            // The public source captured at prepare remains immutable. Full proof
            // already checked its exact derived bytes/private source association.
            if !Arc::ptr_eq(
                &result.source.texels,
                &result.verified.certificate.source.texels,
            ) || !Arc::ptr_eq(
                &result.source.provenance.original,
                &result.verified.certificate.source.provenance.original,
            ) {
                return Err(mismatch());
            }
            self.finish_normal_cpu_source(&result.verified, &sampler.0, previous.map(|p| &p.0))
                .map(PbrMipChain3D)
                .map_err(|e| PbrNormalCpuProofError3D::binding(PbrSourceError3D::from(e)))
        };
        match finish() {
            Ok(chain) => {
                drop(result);
                drop(pending);
                self.collect_normal_cpu_charges();
                Ok(chain)
            }
            Err(mut error) => {
                error.cleanup = Some(result);
                drop(pending);
                Err(error)
            }
        }
    }
}
