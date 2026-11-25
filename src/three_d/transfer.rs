//! Bounded mounted flat-viewport publication under HAL-354 G1, HAL-266A and HAL-355.
//!
//! AppContext owns this mechanical registry and every pending request/terminal
//! receipt. Widgets retain only opaque link identities. The canonical Update
//! owner lends publication authority during one traversal and completes cutoff
//! after widget borrows end, before Layout. This is not a semantic event bus.

use std::fmt;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::core::frame_realization::AppFrameAttemptId;
use crate::core::widget_tree::{WidgetMountToken, WidgetTree};
use crate::utils::id::TreeWidgetId;

use super::{
    ModelState, RenderSnapshot, SampleCount3D, ViewDescriptor3, Viewport3D, Viewport3DError,
    Viewport3DViewRevision,
};

struct WriterState {
    active: AtomicBool,
    source_mount: WidgetMountToken,
    target_mount: WidgetMountToken,
}

/// Private engine authority: construction and retirement belong to the App owner.
#[derive(Clone)]
pub(crate) struct FlatViewport3DWriterAuthority(Arc<WriterState>);

impl FlatViewport3DWriterAuthority {
    fn new(source_mount: WidgetMountToken, target_mount: WidgetMountToken) -> Self {
        Self(Arc::new(WriterState {
            active: AtomicBool::new(true),
            source_mount,
            target_mount,
        }))
    }

    pub(crate) fn is_active(&self) -> bool {
        self.0.active.load(Ordering::Acquire)
            && self.0.source_mount.is_live()
            && self.0.target_mount.is_live()
    }

    pub(crate) fn retire(&self) {
        self.0.active.store(false, Ordering::Release);
    }
}

impl PartialEq for FlatViewport3DWriterAuthority {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for FlatViewport3DWriterAuthority {}

impl fmt::Debug for FlatViewport3DWriterAuthority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlatViewport3DWriterAuthority")
            .field("active", &self.is_active())
            .finish_non_exhaustive()
    }
}

/// Opaque, nonrecycled application/tree/concrete-mount qualified link identity.
///
/// This handle contains no widget reference or receipt storage. Copies do not
/// confer peer access or publication authority outside the source's engine Update.
#[derive(Clone)]
pub struct FlatViewport3DLinkId {
    application: Arc<()>,
    sequence: u64,
    source: TreeWidgetId,
    target: TreeWidgetId,
    source_mount: WidgetMountToken,
    target_mount: WidgetMountToken,
    writer: FlatViewport3DWriterAuthority,
}

impl FlatViewport3DLinkId {
    /// Original tree-managed source identity; not a live-membership assertion.
    pub fn source(&self) -> TreeWidgetId {
        self.source
    }

    /// Original tree-managed recipient identity; not a live-membership assertion.
    pub fn target(&self) -> TreeWidgetId {
        self.target
    }
}

impl PartialEq for FlatViewport3DLinkId {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.application, &other.application)
            && self.sequence == other.sequence
            && self.source == other.source
            && self.target == other.target
            && self.source_mount == other.source_mount
            && self.target_mount == other.target_mount
            && self.writer == other.writer
    }
}

impl Eq for FlatViewport3DLinkId {}

impl fmt::Debug for FlatViewport3DLinkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlatViewport3DLinkId")
            .field("sequence", &self.sequence)
            .field("source", &self.source)
            .field("target", &self.target)
            .finish_non_exhaustive()
    }
}

/// Immediate publication admission is pending, never setter/GPU success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlatViewport3DPublicationStatus {
    AcceptedForCutoff,
}

/// The original qualified mechanical association that ceased to be current.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlatViewport3DStaleReason {
    SourceMountEnded,
    TargetMountEnded,
    BothMountsEnded,
    SourceQualificationChanged,
    TargetQualificationChanged,
    RecipientTypeChanged,
    WriterAuthorityEnded,
}

/// Explicit lifetime termination, kept distinct from stale qualification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlatViewport3DRetirementReason {
    Explicit,
    ApplicationShutdown,
}

/// Actual inability to establish an outcome must retain this typed cause.
///
/// Normal checked-setter results never produce this category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlatViewport3DIndeterminateCause {
    MechanicalOutcomeUnavailable,
}

/// Six terminal mechanical transfer outcomes, not renderer lifecycle states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlatViewport3DTransferOutcome {
    Transferred,
    Unchanged,
    Rejected(Viewport3DError),
    Stale(FlatViewport3DStaleReason),
    Retired(FlatViewport3DRetirementReason),
    Indeterminate(FlatViewport3DIndeterminateCause),
}

/// Pending admission and terminal settlement remain explicitly distinct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlatViewport3DTransferStage {
    AcceptedForCutoff,
    Settled(FlatViewport3DTransferOutcome),
}

/// Owned historical evidence under the original qualified link identity.
///
/// A transferred request is not prepared, committed or displayed merely because
/// this receipt exists. Prepared status is observed separately at a lawful Screen
/// boundary and requalified by the application before semantic use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatViewport3DLinkResult {
    pub link: FlatViewport3DLinkId,
    pub source: TreeWidgetId,
    pub target: TreeWidgetId,
    pub correlation: u64,
    pub view_revision: Viewport3DViewRevision,
    pub stage: FlatViewport3DTransferStage,
    pub cutoff_frame: Option<AppFrameAttemptId>,
}

/// Typed admission/readback refusals; none erase an earlier admitted result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlatViewport3DTransferError {
    Disabled,
    NotEngineUpdate,
    AllocationFailed,
    CapacityExhausted,
    IdentityExhausted,
    ForeignApplication,
    UnknownLink,
    SameEndpoint,
    MissingSource,
    MissingTarget,
    WrongTarget,
    WriterConflict,
    Retired,
    Stale(FlatViewport3DStaleReason),
    CandidatePending,
    ResultUnconsumed,
    WrongSource,
    ApplicationShutdown,
}

impl fmt::Display for FlatViewport3DTransferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HAL-354 G1 flat viewport transfer: {self:?}")
    }
}

impl std::error::Error for FlatViewport3DTransferError {}

struct PendingRequest {
    correlation: u64,
    view_revision: Viewport3DViewRevision,
    view: ViewDescriptor3,
    snapshot: RenderSnapshot,
    model: ModelState,
    sample_count: SampleCount3D,
}

struct LinkEntry {
    id: FlatViewport3DLinkId,
    pending: Option<PendingRequest>,
    result: Option<FlatViewport3DLinkResult>,
    retired: bool,
}

impl LinkEntry {
    fn receipt(
        &self,
        candidate: &PendingRequest,
        stage: FlatViewport3DTransferStage,
        cutoff_frame: Option<AppFrameAttemptId>,
    ) -> FlatViewport3DLinkResult {
        FlatViewport3DLinkResult {
            link: self.id.clone(),
            source: self.id.source,
            target: self.id.target,
            correlation: candidate.correlation,
            view_revision: candidate.view_revision,
            stage,
            cutoff_frame,
        }
    }

    fn retire(
        &mut self,
        outcome: FlatViewport3DTransferOutcome,
        cutoff_frame: Option<AppFrameAttemptId>,
    ) -> bool {
        let was_active = !self.retired;
        self.retired = true;
        self.id.writer.retire();
        if let Some(candidate) = self.pending.take() {
            // INV-354-G1-SETTLEMENT: pending and unconsumed are exclusive.
            debug_assert!(self.result.is_none());
            self.result = Some(self.receipt(
                &candidate,
                FlatViewport3DTransferStage::Settled(outcome),
                cutoff_frame,
            ));
        }
        was_active
    }

    fn can_release(&self) -> bool {
        self.retired && self.pending.is_none() && self.result.is_none()
    }
}

/// The sole bounded result owner, installed only by explicit App admission.
///
/// Storage for one complete pending request and one receipt is reserved per slot.
/// Retired links keep that same slot until their terminal receipt is consumed;
/// there is no retired-link history or tombstone collection.
pub(crate) struct FlatViewport3DTransfers {
    application: Arc<()>,
    slots: Vec<Option<LinkEntry>>,
    next_sequence: Option<u64>,
    active_links: usize,
    shutdown: bool,
}

impl FlatViewport3DTransfers {
    pub(crate) fn new(capacity: NonZeroUsize) -> Result<Self, FlatViewport3DTransferError> {
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(capacity.get())
            .map_err(|_| FlatViewport3DTransferError::AllocationFailed)?;
        slots.resize_with(capacity.get(), || None);
        Ok(Self {
            application: Arc::new(()),
            slots,
            next_sequence: Some(1),
            active_links: 0,
            shutdown: false,
        })
    }

    pub(crate) fn has_active(&self) -> bool {
        self.active_links != 0
    }

    pub(crate) fn has_active_for_tree(&self, tree: &WidgetTree) -> bool {
        self.has_active()
            && self.slots.iter().flatten().any(|entry| {
                !entry.retired
                    && entry.id.writer.is_active()
                    && tree.owns_mount_tree(&entry.id.source_mount)
            })
    }

    pub(crate) fn contains_source(&self, source: TreeWidgetId) -> bool {
        self.has_active()
            && self.slots.iter().flatten().any(|entry| {
                !entry.retired && entry.id.source == source && entry.id.writer.is_active()
            })
    }

    pub(crate) fn admit(
        &mut self,
        tree: &mut WidgetTree,
        source: TreeWidgetId,
        target: TreeWidgetId,
    ) -> Result<FlatViewport3DLinkId, FlatViewport3DTransferError> {
        self.settle_lifetimes();
        if self.shutdown {
            return Err(FlatViewport3DTransferError::ApplicationShutdown);
        }
        if source == target {
            return Err(FlatViewport3DTransferError::SameEndpoint);
        }
        let source_mount = tree
            .concrete_mount(source)
            .ok_or(FlatViewport3DTransferError::MissingSource)?;
        let target_mount = tree
            .concrete_mount(target)
            .ok_or(FlatViewport3DTransferError::MissingTarget)?;
        if self
            .slots
            .iter()
            .flatten()
            .any(|entry| !entry.retired && entry.id.target_mount == target_mount)
        {
            return Err(FlatViewport3DTransferError::WriterConflict);
        }
        let index = self
            .slots
            .iter()
            .position(Option::is_none)
            .ok_or(FlatViewport3DTransferError::CapacityExhausted)?;
        let sequence = self
            .next_sequence
            .ok_or(FlatViewport3DTransferError::IdentityExhausted)?;
        let target_node = tree
            .find_by_id(target)
            .ok_or(FlatViewport3DTransferError::MissingTarget)?;
        let viewport = tree
            .node_mut(target_node)
            .and_then(|node| node.widget_mut().as_any_mut().downcast_mut::<Viewport3D>())
            .ok_or(FlatViewport3DTransferError::WrongTarget)?;
        let writer = FlatViewport3DWriterAuthority::new(source_mount.clone(), target_mount.clone());
        if let Err(error) = viewport.claim_flat_viewport3d_writer(writer.clone()) {
            writer.retire();
            return Err(match error {
                Viewport3DError::RequestKindMismatch => FlatViewport3DTransferError::WrongTarget,
                _ => FlatViewport3DTransferError::WriterConflict,
            });
        }
        let id = FlatViewport3DLinkId {
            application: self.application.clone(),
            sequence,
            source,
            target,
            source_mount,
            target_mount,
            writer,
        };
        self.next_sequence = sequence.checked_add(1);
        self.slots[index] = Some(LinkEntry {
            id: id.clone(),
            pending: None,
            result: None,
            retired: false,
        });
        self.active_links += 1;
        Ok(id)
    }

    pub(crate) fn retire(
        &mut self,
        id: &FlatViewport3DLinkId,
    ) -> Result<(), FlatViewport3DTransferError> {
        self.settle_lifetimes();
        let index = self.index(id)?;
        let entry = self.slots[index]
            .as_mut()
            .ok_or(FlatViewport3DTransferError::UnknownLink)?;
        if entry.retired {
            return Err(FlatViewport3DTransferError::Retired);
        }
        if entry.retire(
            FlatViewport3DTransferOutcome::Retired(FlatViewport3DRetirementReason::Explicit),
            None,
        ) {
            self.active_links -= 1;
        }
        if entry.can_release() {
            self.slots[index] = None;
        }
        Ok(())
    }

    pub(crate) fn result(
        &mut self,
        id: &FlatViewport3DLinkId,
    ) -> Result<Option<FlatViewport3DLinkResult>, FlatViewport3DTransferError> {
        self.settle_lifetimes();
        let index = self.index(id)?;
        let entry = self.slots[index]
            .as_ref()
            .ok_or(FlatViewport3DTransferError::UnknownLink)?;
        Ok(entry.result.clone().or_else(|| {
            entry.pending.as_ref().map(|candidate| {
                entry.receipt(
                    candidate,
                    FlatViewport3DTransferStage::AcceptedForCutoff,
                    None,
                )
            })
        }))
    }

    pub(crate) fn take_result(
        &mut self,
        id: &FlatViewport3DLinkId,
    ) -> Result<Option<FlatViewport3DLinkResult>, FlatViewport3DTransferError> {
        self.settle_lifetimes();
        let index = self.index(id)?;
        let entry = self.slots[index]
            .as_mut()
            .ok_or(FlatViewport3DTransferError::UnknownLink)?;
        let result = entry.result.take();
        if entry.can_release() {
            self.slots[index] = None;
        }
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn publish(
        &mut self,
        current_owner: TreeWidgetId,
        current_source: Option<&WidgetMountToken>,
        id: &FlatViewport3DLinkId,
        correlation: u64,
        view_revision: Viewport3DViewRevision,
        view: ViewDescriptor3,
        snapshot: RenderSnapshot,
        model: ModelState,
        sample_count: SampleCount3D,
    ) -> Result<FlatViewport3DPublicationStatus, FlatViewport3DTransferError> {
        self.settle_lifetimes();
        if self.shutdown {
            return Err(FlatViewport3DTransferError::ApplicationShutdown);
        }
        let index = self.index(id)?;
        let entry = self.slots[index]
            .as_mut()
            .ok_or(FlatViewport3DTransferError::UnknownLink)?;
        if entry.retired || !entry.id.writer.is_active() {
            return Err(FlatViewport3DTransferError::Retired);
        }
        if entry.pending.is_some() {
            return Err(FlatViewport3DTransferError::CandidatePending);
        }
        if entry.result.is_some() {
            return Err(FlatViewport3DTransferError::ResultUnconsumed);
        }
        if current_owner != entry.id.source {
            return Err(FlatViewport3DTransferError::WrongSource);
        }
        if !current_source
            .is_some_and(|source| source.is_live() && *source == entry.id.source_mount)
        {
            return Err(FlatViewport3DTransferError::Stale(
                FlatViewport3DStaleReason::SourceQualificationChanged,
            ));
        }
        entry.pending = Some(PendingRequest {
            correlation,
            view_revision,
            view,
            snapshot,
            model,
            sample_count,
        });
        Ok(FlatViewport3DPublicationStatus::AcceptedForCutoff)
    }

    /// INV-354-G1-CUTOFF: called once by the existing Update owner, after the
    /// traversal has released all widget borrows and before Layout.
    pub(crate) fn cutoff(&mut self, tree: &mut WidgetTree, frame: Option<AppFrameAttemptId>) {
        self.settle_lifetimes();
        if !self.has_active() || self.shutdown {
            return;
        }
        for slot in &mut self.slots {
            let Some(entry) = slot.as_mut() else {
                continue;
            };
            if entry.retired || !tree.owns_mount_tree(&entry.id.source_mount) {
                continue;
            }
            let Some(candidate) = entry.pending.take() else {
                continue;
            };
            let qualification = if tree.concrete_mount(entry.id.source).as_ref()
                != Some(&entry.id.source_mount)
            {
                Some(FlatViewport3DStaleReason::SourceQualificationChanged)
            } else if tree.concrete_mount(entry.id.target).as_ref() != Some(&entry.id.target_mount)
            {
                Some(FlatViewport3DStaleReason::TargetQualificationChanged)
            } else if !entry.id.writer.is_active() {
                Some(FlatViewport3DStaleReason::WriterAuthorityEnded)
            } else {
                None
            };
            let mut changed_target = None;
            let outcome = if let Some(reason) = qualification {
                entry.retired = true;
                entry.id.writer.retire();
                self.active_links -= 1;
                FlatViewport3DTransferOutcome::Stale(reason)
            } else if let Some(target_node) = tree.find_by_id(entry.id.target) {
                if let Some(viewport) = tree
                    .node_mut(target_node)
                    .and_then(|node| node.widget_mut().as_any_mut().downcast_mut::<Viewport3D>())
                {
                    match viewport.transfer_flat_viewport3d_request(
                        &entry.id.writer,
                        candidate.view_revision,
                        candidate.view.clone(),
                        candidate.snapshot.clone(),
                        candidate.model.clone(),
                        candidate.sample_count,
                    ) {
                        Ok(true) => {
                            changed_target = Some(target_node);
                            FlatViewport3DTransferOutcome::Transferred
                        }
                        Ok(false) => FlatViewport3DTransferOutcome::Unchanged,
                        Err(error) => FlatViewport3DTransferOutcome::Rejected(error),
                    }
                } else {
                    entry.retired = true;
                    entry.id.writer.retire();
                    self.active_links -= 1;
                    FlatViewport3DTransferOutcome::Stale(
                        FlatViewport3DStaleReason::RecipientTypeChanged,
                    )
                }
            } else {
                entry.retired = true;
                entry.id.writer.retire();
                self.active_links -= 1;
                FlatViewport3DTransferOutcome::Stale(
                    FlatViewport3DStaleReason::TargetQualificationChanged,
                )
            };
            // The synchronous checked Result is definitive. It is never
            // converted to Indeterminate, a renderer commit or a display claim.
            debug_assert!(entry.result.is_none());
            entry.result = Some(entry.receipt(
                &candidate,
                FlatViewport3DTransferStage::Settled(outcome),
                frame,
            ));
            if let Some(target_node) = changed_target {
                tree.mark_paint_dirty(target_node);
            }
        }
    }

    /// Ordinary application shutdown preserves terminal truth and settles only
    /// still-pending admissions; no process persistence or external delivery.
    pub(crate) fn shutdown(&mut self) {
        if self.shutdown {
            return;
        }
        self.settle_lifetimes();
        self.shutdown = true;
        for slot in &mut self.slots {
            if let Some(entry) = slot.as_mut() {
                entry.retire(
                    FlatViewport3DTransferOutcome::Retired(
                        FlatViewport3DRetirementReason::ApplicationShutdown,
                    ),
                    None,
                );
                if entry.can_release() {
                    *slot = None;
                }
            }
        }
        self.active_links = 0;
    }

    fn index(&self, id: &FlatViewport3DLinkId) -> Result<usize, FlatViewport3DTransferError> {
        if !Arc::ptr_eq(&self.application, &id.application) {
            return Err(FlatViewport3DTransferError::ForeignApplication);
        }
        self.slots
            .iter()
            .position(|slot| slot.as_ref().is_some_and(|entry| entry.id == *id))
            .ok_or_else(|| {
                if let Some(reason) = stale_reason(id) {
                    FlatViewport3DTransferError::Stale(reason)
                } else if !id.writer.is_active() {
                    FlatViewport3DTransferError::Retired
                } else {
                    FlatViewport3DTransferError::UnknownLink
                }
            })
    }

    fn settle_lifetimes(&mut self) {
        for slot in &mut self.slots {
            let Some(entry) = slot.as_mut() else {
                continue;
            };
            if !entry.retired {
                let reason = stale_reason(&entry.id).or_else(|| {
                    (!entry.id.writer.is_active())
                        .then_some(FlatViewport3DStaleReason::WriterAuthorityEnded)
                });
                if let Some(reason) = reason {
                    if entry.retire(FlatViewport3DTransferOutcome::Stale(reason), None) {
                        self.active_links -= 1;
                    }
                }
            }
            if entry.can_release() {
                *slot = None;
            }
        }
    }
}

impl Drop for FlatViewport3DTransfers {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn stale_reason(id: &FlatViewport3DLinkId) -> Option<FlatViewport3DStaleReason> {
    match (id.source_mount.is_live(), id.target_mount.is_live()) {
        (false, false) => Some(FlatViewport3DStaleReason::BothMountsEnded),
        (false, true) => Some(FlatViewport3DStaleReason::SourceMountEnded),
        (true, false) => Some(FlatViewport3DStaleReason::TargetMountEnded),
        (true, true) => None,
    }
}
