//! Private registrar capability; no App admission or Button consumer is wired.

use super::*;
use crate::renderer::custom_render_pass::{
    CustomRenderPass, CustomRenderPassError, UnadmittedPreparationError,
};
use crate::renderer::image_binding::{ImageAdmissionError, ImageBinding, ImageBindingIdentity};
use crate::renderer::sdf_image_generation_custody::{
    SdfImageGenerationCustody, SdfImageRetirementCertificate, SdfMaterialProducerRecord,
    SdfMaterialProducerRecordError,
};
use crate::renderer::sdf_material_status::SdfMaterialKey;

#[derive(Debug)]
pub(in crate::renderer) enum SdfReservationError {
    Exhausted,
    ReservationMismatch,
    KeyMismatch,
    AlreadyRegistered,
    MissingProducer,
    ProducerMismatch,
    QueuedMutation,
    Front(CustomRenderPassError),
    Admission(ImageAdmissionError),
    Preparation(UnadmittedPreparationError),
}

#[derive(Clone)]
pub(in crate::renderer) struct SdfTextureReservation {
    texture_id: u64,
    key: SdfMaterialKey,
    proof: Arc<()>,
}

impl SdfTextureReservation {
    pub(in crate::renderer) fn texture_id(&self) -> u64 {
        self.texture_id
    }
    pub(in crate::renderer) fn key(&self) -> &SdfMaterialKey {
        &self.key
    }
}

pub(super) struct SdfReservationState {
    key: SdfMaterialKey,
    proof: Arc<()>,
    registered: Option<ImageBindingIdentity>,
}

/// One preparation publication, never a replacement producer admission.
pub(in crate::renderer) struct SdfTargetEligibility {
    current: SdfMaterialKey,
    original: SdfMaterialKey,
    identity: ImageBindingIdentity,
    preparation: Arc<()>,
}

impl SdfTargetEligibility {
    pub(in crate::renderer) fn matches(
        &self,
        current: &SdfMaterialKey,
        preparation: &Arc<()>,
    ) -> bool {
        self.current == *current
            && self.current.metrics() == current.metrics()
            && Arc::ptr_eq(&self.preparation, preparation)
    }
}

/// Resource failure settlement carries no Ready or sampling credential.
pub(in crate::renderer) struct SdfTargetFailure {
    current: SdfMaterialKey,
    preparation: Arc<()>,
    publication: crate::renderer::sdf_material_status::SdfMaterialPublication,
}

impl SdfTargetFailure {
    pub(in crate::renderer) fn matches(
        &self,
        current: &SdfMaterialKey,
        preparation: &Arc<()>,
    ) -> bool {
        self.current == *current
            && self.current.metrics() == current.metrics()
            && Arc::ptr_eq(&self.preparation, preparation)
    }

    pub(in crate::renderer) fn publication(
        &self,
    ) -> crate::renderer::sdf_material_status::SdfMaterialPublication {
        self.publication.clone()
    }
}

/// Constructible only by actual checked registration, not an ordinary mailbox.
pub(in crate::renderer) struct SdfAuthenticatedAdmission {
    key: SdfMaterialKey,
    binding: ImageBinding,
    generation: SdfImageGenerationCustody,
}

impl SdfAuthenticatedAdmission {
    pub(in crate::renderer) fn binding(&self) -> &ImageBinding {
        &self.binding
    }

    pub(in crate::renderer) fn retain_scene_reference(
        &self,
        current: &SdfMaterialKey,
    ) -> Result<
        crate::renderer::sdf_frame_custody::SdfImageReference,
        crate::renderer::sdf_image_generation_custody::SdfImageCustodyError,
    > {
        use crate::renderer::sdf_image_generation_custody::SdfImageCustodyError;
        if current != &self.key {
            return Err(SdfImageCustodyError::Reservation(
                SdfReservationError::KeyMismatch,
            ));
        }
        Ok(crate::renderer::sdf_frame_custody::SdfImageReference::new(
            self.key.clone(),
            self.generation.acquire()?,
        ))
    }

    /// Read only authenticated, previously completed admission at Frame B's cutoff.
    pub(in crate::renderer) fn publication_for_current(
        &self,
        renderer: &mut WgpuRenderer,
        current: &SdfMaterialKey,
    ) -> Result<
        (
            crate::renderer::sdf_material_status::SdfMaterialPublication,
            Option<ImageBindingIdentity>,
        ),
        crate::renderer::sdf_material_status::SdfMaterialRejection,
    > {
        use crate::renderer::sdf_material_status::{
            SdfMaterialPublication, SdfMaterialRejection, SdfMaterialStatus,
            SdfMaterialUnavailableCause,
        };

        if &self.key != current {
            return Err(SdfMaterialRejection::KeyMismatch);
        }
        let identity = self.binding.identity();
        let revoked = |cause| SdfMaterialStatus::Unavailable {
            cause: SdfMaterialUnavailableCause::BindingRevoked {
                withdrawn: identity,
                cause,
            },
        };
        let status = match self.binding.availability() {
            Ok(()) => {
                let registered = renderer.admit_image(identity.texture_id());
                match registered {
                    Ok(registered) if registered.identity() == identity => {
                        SdfMaterialStatus::Ready {
                            binding: self.binding.clone(),
                        }
                    }
                    other => match self.binding.availability() {
                        Err(cause) => revoked(cause),
                        Ok(()) => {
                            return Err(if other.is_ok() {
                                SdfMaterialRejection::BindingIdentityMismatch
                            } else {
                                SdfMaterialRejection::MissingEvidence
                            });
                        }
                    },
                }
            }
            Err(cause) => revoked(cause),
        };
        let publication =
            match SdfMaterialPublication::new(current, self.key.clone(), Some(identity), status) {
                // A failure observed during final Ready validation is still before
                // commit; preserve that exact withdrawn generation and real cause.
                Err(SdfMaterialRejection::BindingUnavailable(cause)) => {
                    SdfMaterialPublication::new(
                        current,
                        self.key.clone(),
                        Some(identity),
                        revoked(cause),
                    )?
                }
                other => other?,
            };
        Ok((publication, Some(identity)))
    }

    fn authenticate_eligibility(
        &self,
        renderer: &mut WgpuRenderer,
    ) -> Result<(), crate::renderer::sdf_material_status::SdfMaterialRejection> {
        use crate::renderer::sdf_image_generation_custody::SdfImageCustodyError;
        use crate::renderer::sdf_material_status::SdfMaterialRejection;
        let identity = self.binding.identity();
        let state = renderer
            .sdf_texture_reservations
            .get(&identity.texture_id())
            .ok_or(SdfMaterialRejection::MissingEvidence)?;
        if state.key != self.key
            || state.registered != Some(identity)
            || !renderer
                .protected_sdf_texture_ids
                .contains(&identity.texture_id())
            || renderer.sdf_front_has_queued_mutation(identity.texture_id())
        {
            return Err(SdfMaterialRejection::MissingEvidence);
        }
        match self
            .generation
            .validate_qualification(&renderer.gpu_resources())
        {
            Ok(()) => {
                let registered = renderer
                    .admit_image(identity.texture_id())
                    .map_err(|_| SdfMaterialRejection::MissingEvidence)?;
                if registered.identity() != identity {
                    return Err(SdfMaterialRejection::BindingIdentityMismatch);
                }
                Ok(())
            }
            // This is genuine resource-owned failure evidence, not Ready.
            Err(SdfImageCustodyError::Unavailable(cause)) => {
                Err(SdfMaterialRejection::BindingUnavailable(cause))
            }
            Err(_) => Err(SdfMaterialRejection::MissingEvidence),
        }
    }

    pub(in crate::renderer) fn certify_window_extent(
        &self,
        renderer: &mut WgpuRenderer,
        current: &SdfMaterialKey,
        preparation: &Arc<()>,
    ) -> Result<SdfTargetEligibility, crate::renderer::sdf_material_status::SdfMaterialRejection>
    {
        use crate::renderer::sdf_material_status::SdfMaterialRejection;
        if !self.key.same_content_geometry_basis(current) {
            return Err(SdfMaterialRejection::KeyMismatch);
        }
        self.authenticate_eligibility(renderer)?;
        Ok(SdfTargetEligibility {
            current: current.clone(),
            original: self.key.clone(),
            identity: self.binding.identity(),
            preparation: preparation.clone(),
        })
    }

    pub(in crate::renderer) fn window_failure(
        &self,
        renderer: &mut WgpuRenderer,
        current: &SdfMaterialKey,
        preparation: &Arc<()>,
    ) -> Result<SdfTargetFailure, crate::renderer::sdf_material_status::SdfMaterialRejection> {
        use crate::renderer::sdf_material_status::{
            SdfMaterialPublication, SdfMaterialRejection, SdfMaterialStatus,
            SdfMaterialUnavailableCause,
        };
        if !self.key.same_content_geometry_basis(current) {
            return Err(SdfMaterialRejection::KeyMismatch);
        }
        let Err(SdfMaterialRejection::BindingUnavailable(cause)) =
            self.authenticate_eligibility(renderer)
        else {
            return Err(SdfMaterialRejection::MissingEvidence);
        };
        let identity = self.binding.identity();
        let publication = SdfMaterialPublication::new(
            current,
            current.clone(),
            Some(identity),
            SdfMaterialStatus::Unavailable {
                cause: SdfMaterialUnavailableCause::BindingRevoked {
                    withdrawn: identity,
                    cause,
                },
            },
        )?;
        Ok(SdfTargetFailure {
            current: current.clone(),
            preparation: preparation.clone(),
            publication,
        })
    }

    fn validate_eligibility(
        &self,
        current: &SdfMaterialKey,
        eligibility: &SdfTargetEligibility,
        preparation: &Arc<()>,
    ) -> Result<(), crate::renderer::sdf_material_status::SdfMaterialRejection> {
        use crate::renderer::sdf_material_status::SdfMaterialRejection;
        if !eligibility.matches(current, preparation)
            || eligibility.original != self.key
            || eligibility.identity != self.binding.identity()
            || !self.key.same_content_geometry_basis(current)
        {
            return Err(SdfMaterialRejection::KeyMismatch);
        }
        Ok(())
    }

    pub(in crate::renderer) fn retain_eligible_reference(
        &self,
        current: &SdfMaterialKey,
        eligibility: &SdfTargetEligibility,
        preparation: &Arc<()>,
    ) -> Result<
        crate::renderer::sdf_frame_custody::SdfImageReference,
        crate::renderer::sdf_image_generation_custody::SdfImageCustodyError,
    > {
        use crate::renderer::sdf_image_generation_custody::SdfImageCustodyError;
        self.validate_eligibility(current, eligibility, preparation)
            .map_err(|_| SdfImageCustodyError::IdentityMismatch)?;
        Ok(crate::renderer::sdf_frame_custody::SdfImageReference::new(
            current.clone(),
            self.generation.acquire()?,
        ))
    }

    pub(in crate::renderer) fn publication_for_eligibility(
        &self,
        renderer: &mut WgpuRenderer,
        current: &SdfMaterialKey,
        eligibility: &SdfTargetEligibility,
        preparation: &Arc<()>,
    ) -> Result<
        (
            crate::renderer::sdf_material_status::SdfMaterialPublication,
            Option<ImageBindingIdentity>,
        ),
        crate::renderer::sdf_material_status::SdfMaterialRejection,
    > {
        use crate::renderer::sdf_material_status::{
            SdfMaterialPublication, SdfMaterialRejection, SdfMaterialStatus,
            SdfMaterialUnavailableCause,
        };
        self.validate_eligibility(current, eligibility, preparation)?;
        let identity = self.binding.identity();
        let status = match self.authenticate_eligibility(renderer) {
            Ok(()) => SdfMaterialStatus::Ready {
                binding: self.binding.clone(),
            },
            Err(SdfMaterialRejection::BindingUnavailable(cause)) => {
                SdfMaterialStatus::Unavailable {
                    cause: SdfMaterialUnavailableCause::BindingRevoked {
                        withdrawn: identity,
                        cause,
                    },
                }
            }
            Err(error) => return Err(error),
        };
        let publication =
            match SdfMaterialPublication::new(current, current.clone(), Some(identity), status) {
                Err(SdfMaterialRejection::BindingUnavailable(cause)) => {
                    SdfMaterialPublication::new(
                        current,
                        current.clone(),
                        Some(identity),
                        SdfMaterialStatus::Unavailable {
                            cause: SdfMaterialUnavailableCause::BindingRevoked {
                                withdrawn: identity,
                                cause,
                            },
                        },
                    )?
                }
                other => other?,
            };
        Ok((publication, Some(identity)))
    }

    #[cfg(test)]
    pub(super) fn hold_writer_observation_for_test(
        &self,
    ) -> Option<Arc<std::sync::atomic::AtomicBool>> {
        self.generation.hold_writer_observation_for_test()
    }

    #[cfg(test)]
    pub(super) fn hold_sampling_observation_for_test(&self) -> Arc<std::sync::atomic::AtomicBool> {
        self.generation.hold_sampling_observation_for_test()
    }

    pub(in crate::renderer) fn retire_if_ready(
        &mut self,
        renderer: &mut WgpuRenderer,
    ) -> Result<bool, crate::renderer::sdf_image_generation_custody::SdfImageCustodyError> {
        self.generation.retire_if_ready(renderer)
    }

    pub(in crate::renderer) fn into_material_record(
        self,
        current: &SdfMaterialKey,
    ) -> Result<
        SdfMaterialProducerRecord,
        (SdfMaterialProducerRecordError, SdfImageGenerationCustody),
    > {
        SdfMaterialProducerRecord::from_admitted_generation(current, self.key, self.generation)
    }
}

impl WgpuRenderer {
    fn sdf_front_has_queued_mutation(&self, texture_id: u64) -> bool {
        let removals = self
            .pending_unregistrations
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let entries = self
            .pending_registrations
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let rgba8 = self
            .pending_rgba8_registrations
            .get()
            .map(|queue| queue.pending.lock().unwrap_or_else(|e| e.into_inner()));
        removals.iter().any(|entry| entry.texture_id == texture_id)
            || entries.iter().any(|entry| entry.texture_id == texture_id)
            || rgba8
                .as_ref()
                .and_then(|pending| pending.items.as_ref())
                .is_some_and(|items| {
                    items
                        .iter()
                        .any(|entry| entry.admission.texture_id == texture_id)
                })
    }

    /// Reserve K before producer construction; bind its frozen front once at
    /// checked registration. Pending reservation alone is never Ready.
    pub(in crate::renderer) fn reserve_sdf_texture(
        &mut self,
        key: SdfMaterialKey,
    ) -> Result<SdfTextureReservation, SdfReservationError> {
        let removals = self
            .pending_unregistrations
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entries = self
            .pending_registrations
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let rgba8 = self.pending_rgba8_registrations.get().map(|queue| {
            queue
                .pending
                .lock()
                .unwrap_or_else(|error| error.into_inner())
        });
        let mut candidate = self
            .next_sdf_texture_id
            .ok_or(SdfReservationError::Exhausted)?;
        loop {
            let next = candidate.checked_add(1);
            let occupied = self.image_textures.contains_key(&candidate)
                || self.protected_sdf_texture_ids.contains(&candidate)
                || self.sdf_texture_reservations.contains_key(&candidate)
                || removals.iter().any(|entry| entry.texture_id == candidate)
                || entries.iter().any(|entry| entry.texture_id == candidate)
                || rgba8
                    .as_ref()
                    .and_then(|pending| pending.items.as_ref())
                    .is_some_and(|items| {
                        items
                            .iter()
                            .any(|entry| entry.admission.texture_id == candidate)
                    });
            self.next_sdf_texture_id = next;
            if !occupied {
                let proof = Arc::new(());
                self.sdf_texture_reservations.insert(
                    candidate,
                    SdfReservationState {
                        key: key.clone(),
                        proof: proof.clone(),
                        registered: None,
                    },
                );
                assert!(self.protected_sdf_texture_ids.insert(candidate));
                return Ok(SdfTextureReservation {
                    texture_id: candidate,
                    key,
                    proof,
                });
            }
            candidate = next.ok_or(SdfReservationError::Exhausted)?;
        }
    }

    fn checked_sdf_reservation(
        &self,
        reservation: &SdfTextureReservation,
        current: &SdfMaterialKey,
    ) -> Result<&SdfReservationState, SdfReservationError> {
        let state = self
            .sdf_texture_reservations
            .get(&reservation.texture_id)
            .ok_or(SdfReservationError::ReservationMismatch)?;
        if !self
            .protected_sdf_texture_ids
            .contains(&reservation.texture_id)
            || !Arc::ptr_eq(&state.proof, &reservation.proof)
        {
            return Err(SdfReservationError::ReservationMismatch);
        }
        if &state.key != current || &reservation.key != current {
            return Err(SdfReservationError::KeyMismatch);
        }
        Ok(state)
    }
    /// Only the scoped active-size owner calls this after final preparation.
    /// Initial cold callers retain the separate post-Paint completion gate.
    pub(in crate::renderer) fn register_reserved_sdf_replacement_front(
        &mut self,
        reservation: &SdfTextureReservation,
        current: &SdfMaterialKey,
        prepared: &SdfMaterialKey,
        producer: &mut Option<CustomRenderPass>,
    ) -> Result<Option<SdfAuthenticatedAdmission>, SdfReservationError> {
        self.checked_sdf_reservation(reservation, current)?;
        if prepared != current
            || prepared.metrics() != current.metrics()
            || reservation.key().metrics() != current.metrics()
        {
            return Err(SdfReservationError::KeyMismatch);
        }
        let source = producer
            .as_ref()
            .ok_or(SdfReservationError::MissingProducer)?;
        if source.texture_id() != reservation.texture_id()
            || !source.uses_baseline_resources(&self.gpu_resources())
        {
            return Err(SdfReservationError::ProducerMismatch);
        }
        if !source
            .submitted_preparation_ready()
            .map_err(SdfReservationError::Preparation)?
        {
            return Ok(None);
        }
        self.register_reserved_sdf_front(reservation, current, prepared, producer)
            .map(Some)
    }

    /// Resource-owner call at the admitted registrar boundary, after this front's
    /// writes on the baseline queue. No registration drain or Widget phase runs here.
    /// On error the caller keeps its producer and reservation; no front is dropped.
    pub(in crate::renderer) fn register_reserved_sdf_front(
        &mut self,
        reservation: &SdfTextureReservation,
        current: &SdfMaterialKey,
        prepared: &SdfMaterialKey,
        producer: &mut Option<CustomRenderPass>,
    ) -> Result<SdfAuthenticatedAdmission, SdfReservationError> {
        let state = self.checked_sdf_reservation(reservation, current)?;
        if prepared != current {
            return Err(SdfReservationError::KeyMismatch);
        }
        if state.registered.is_some() || self.image_textures.contains_key(&reservation.texture_id) {
            return Err(SdfReservationError::AlreadyRegistered);
        }
        let source = producer
            .as_ref()
            .ok_or(SdfReservationError::MissingProducer)?;
        let gpu = self.gpu_resources();
        if source.texture_id() != reservation.texture_id || !source.uses_baseline_resources(&gpu) {
            return Err(SdfReservationError::ProducerMismatch);
        }
        if self.sdf_front_has_queued_mutation(reservation.texture_id) {
            return Err(SdfReservationError::QueuedMutation);
        }
        let front = source
            .front_registration()
            .map_err(SdfReservationError::Front)?;
        let sampler = if front.nearest {
            &self.nearest_sampler
        } else {
            &self.image_sampler
        };
        let bind_group = Self::create_image_bind_group(
            &self.device,
            &self.image_bind_group_layout,
            &front.view,
            sampler,
            &self.rounded_clip_buffer,
        );
        let generation = crate::renderer::image_binding::next_image_generation();
        let identity = ImageBindingIdentity::new(reservation.texture_id, generation);
        // Never call a public guarded registrar or replace an occupied entry.
        match self.image_textures.entry(reservation.texture_id) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(ImageTextureEntry {
                    _texture: None,
                    view: front.view,
                    sampling: if front.nearest {
                        ImageSampling::Nearest
                    } else {
                        ImageSampling::Linear
                    },
                    bind_group,
                    generation,
                    admission: None,
                });
            }
            std::collections::hash_map::Entry::Occupied(_) => {
                return Err(SdfReservationError::AlreadyRegistered);
            }
        }
        #[cfg(test)]
        crate::renderer::custom_render_pass::audit::record_registration();
        self.sdf_texture_reservations
            .get_mut(&reservation.texture_id)
            .expect("validated exclusive reservation")
            .registered = Some(identity);
        let binding = self
            .admit_image(reservation.texture_id)
            .map_err(SdfReservationError::Admission)?;
        if binding.identity() != identity {
            return Err(SdfReservationError::ReservationMismatch);
        }
        binding.availability().map_err(|cause| {
            SdfReservationError::Admission(ImageAdmissionError::Unavailable(cause))
        })?;
        let generation = SdfImageGenerationCustody::new_reserved(
            producer.take().expect("validated owned frozen producer"),
            binding.clone(),
            &gpu,
            reservation.clone(),
        );
        Ok(SdfAuthenticatedAdmission {
            key: current.clone(),
            binding,
            generation,
        })
    }

    /// Abort only a never-admitted reservation. Pending work and all errors retain
    /// the producer and reservation; no Image-generation certificate is fabricated.
    pub(in crate::renderer) fn abort_unadmitted_sdf_reservation(
        &mut self,
        reservation: &SdfTextureReservation,
        current: &SdfMaterialKey,
        producer: &mut Option<CustomRenderPass>,
    ) -> Result<bool, SdfReservationError> {
        let state = self.checked_sdf_reservation(reservation, current)?;
        if state.registered.is_some() || self.image_textures.contains_key(&reservation.texture_id) {
            return Err(SdfReservationError::AlreadyRegistered);
        }
        if let Some(source) = producer.as_ref() {
            if source.texture_id() != reservation.texture_id
                || !source.uses_baseline_resources(&self.gpu_resources())
            {
                return Err(SdfReservationError::ProducerMismatch);
            }
            if !source
                .unadmitted_preparation_complete()
                .map_err(SdfReservationError::Preparation)?
            {
                return Ok(false);
            }
        }
        // Poll before locking: device callbacks may themselves enqueue mutations.
        let removals = self
            .pending_unregistrations
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entries = self
            .pending_registrations
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let rgba8 = self.pending_rgba8_registrations.get().map(|queue| {
            queue
                .pending
                .lock()
                .unwrap_or_else(|error| error.into_inner())
        });
        let state = self.checked_sdf_reservation(reservation, current)?;
        if state.registered.is_some() || self.image_textures.contains_key(&reservation.texture_id) {
            return Err(SdfReservationError::AlreadyRegistered);
        }
        if removals
            .iter()
            .any(|entry| entry.texture_id == reservation.texture_id)
            || entries
                .iter()
                .any(|entry| entry.texture_id == reservation.texture_id)
            || rgba8
                .as_ref()
                .and_then(|pending| pending.items.as_ref())
                .is_some_and(|items| {
                    items
                        .iter()
                        .any(|entry| entry.admission.texture_id == reservation.texture_id)
                })
        {
            return Err(SdfReservationError::QueuedMutation);
        }
        self.sdf_texture_reservations
            .remove(&reservation.texture_id);
        self.protected_sdf_texture_ids
            .remove(&reservation.texture_id);
        drop(rgba8);
        drop(entries);
        drop(removals);
        // This exclusive renderer call cannot install an Image between the
        // checked metadata release and the now-harmless bare-ID unregister.
        if let Some(source) = producer.take() {
            source.destroy(self);
        }
        Ok(true)
    }

    /// Only C1 can mint this certificate, after closing acquisition and observing
    /// zero CPU custody and all producer-write and sampling completions.
    pub(in crate::renderer) fn retire_reserved_sdf_generation(
        &mut self,
        reservation: &SdfTextureReservation,
        certificate: &SdfImageRetirementCertificate,
        producer: &mut Option<CustomRenderPass>,
    ) -> Result<(), SdfReservationError> {
        let state = self.checked_sdf_reservation(reservation, reservation.key())?;
        if state.registered != Some(certificate.identity())
            || self
                .admit_image(reservation.texture_id)
                .map_err(SdfReservationError::Admission)?
                .identity()
                != certificate.identity()
        {
            return Err(SdfReservationError::ReservationMismatch);
        }
        let source = producer
            .as_ref()
            .ok_or(SdfReservationError::MissingProducer)?;
        if source.texture_id() != reservation.texture_id {
            return Err(SdfReservationError::ProducerMismatch);
        }
        let entry = self
            .image_textures
            .remove(&reservation.texture_id)
            .expect("validated current generation");
        self.current_preparation = None;
        entry.revoke(crate::renderer::image_binding::ImageBindingFailure::Unregistered);
        self.sdf_texture_reservations
            .remove(&reservation.texture_id);
        self.protected_sdf_texture_ids
            .remove(&reservation.texture_id);
        // The certified entry is already withdrawn. Existing destroy now performs
        // its harmless no-op unregister before releasing the actual textures.
        producer
            .take()
            .expect("validated retained producer")
            .destroy(self);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compositor::{Compositor, SceneKey};
    use crate::core::scene::SceneBuilder;
    use crate::core::widget_tree::WidgetTree;
    use crate::renderer::custom_render_prepare::CustomRenderTargetSnapshot;
    use crate::renderer::image_binding::ImageBindingFailure;
    use crate::renderer::sdf_material_status::{SdfMaterialRevision, SdfMaterialStatus};
    use crate::widgets::label::Label;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[test]
    fn sdf_unadmitted_reservation_abort_certifies_preparation_completion() {
        struct RetainOnFailure(Option<CustomRenderPass>);
        impl Drop for RetainOnFailure {
            fn drop(&mut self) {
                if let Some(producer) = self.0.take() {
                    std::mem::forget(producer);
                }
            }
        }

        let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
        let gpu = renderer.gpu_resources();
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Label::new("abort owner")), None, 0);
        let rect = RectF::new(0.0, 0.0, 16.0, 16.0);
        let mut compositor = Compositor::new(rect);
        let surface = compositor.create_surface(0, rect, SceneKey::BASE);
        renderer
            .register_headless_render_target(surface, (16, 16), 1.0)
            .unwrap();
        let metrics = renderer
            .custom_render_target_snapshot(surface)
            .unwrap()
            .metrics_for(rect, rect)
            .unwrap();
        let key = SdfMaterialKey::new(
            tree.snapshot_tree_lifetime(),
            tree.node(root).unwrap().id,
            surface,
            SdfMaterialRevision::initial(),
            metrics,
        )
        .unwrap();
        let stale = SdfMaterialKey::new(
            tree.snapshot_tree_lifetime(),
            tree.node(root).unwrap().id,
            surface,
            SdfMaterialRevision::initial().checked_increment().unwrap(),
            metrics,
        )
        .unwrap();
        renderer.register_image_rgba8_srgb(42, 1, 1, &[255; 4]);
        let ordinary = renderer.admit_image(42).unwrap();

        let empty = renderer.reserve_sdf_texture(key.clone()).unwrap();
        let mut absent = None;
        assert!(matches!(
            renderer.abort_unadmitted_sdf_reservation(&empty, &stale, &mut absent),
            Err(SdfReservationError::KeyMismatch)
        ));
        assert!(
            renderer
                .protected_sdf_texture_ids
                .contains(&empty.texture_id())
        );
        let mut foreign_renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
        let foreign = foreign_renderer.reserve_sdf_texture(key.clone()).unwrap();
        assert_eq!(empty.texture_id(), foreign.texture_id());
        assert!(matches!(
            renderer.abort_unadmitted_sdf_reservation(&foreign, &key, &mut absent),
            Err(SdfReservationError::ReservationMismatch)
        ));
        assert!(
            foreign_renderer
                .abort_unadmitted_sdf_reservation(&foreign, &key, &mut absent)
                .unwrap()
        );
        assert!(
            renderer
                .abort_unadmitted_sdf_reservation(&empty, &key, &mut absent)
                .unwrap()
        );
        assert!(
            !renderer
                .protected_sdf_texture_ids
                .contains(&empty.texture_id())
        );
        assert!(
            !renderer
                .sdf_texture_reservations
                .contains_key(&empty.texture_id())
        );
        assert!(matches!(
            renderer.abort_unadmitted_sdf_reservation(&empty, &key, &mut absent),
            Err(SdfReservationError::ReservationMismatch)
        ));

        let reserved = renderer.reserve_sdf_texture(key.clone()).unwrap();
        let mut producer = RetainOnFailure(Some(
            CustomRenderPass::new_unpublished(
                reserved.texture_id(),
                gpu.clone(),
                PhysicalRect::new(0, 0, 16, 16),
            )
            .unwrap(),
        ));
        let mut frame = producer
            .0
            .as_ref()
            .unwrap()
            .begin_frame(Some("unadmitted preparation"))
            .unwrap();
        {
            let (encoder, view) = frame.encoder_and_view();
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("actual unadmitted front write"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                multiview_mask: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
        }
        assert!(
            !renderer
                .abort_unadmitted_sdf_reservation(&reserved, &key, &mut producer.0)
                .unwrap()
        );
        assert!(producer.0.is_some());
        producer
            .0
            .as_mut()
            .unwrap()
            .submit_and_swap_deferred_registration(frame)
            .unwrap();
        let completion = producer
            .0
            .as_ref()
            .unwrap()
            .unadmitted_completion_for_test()
            .unwrap();
        assert!(matches!(
            renderer.abort_unadmitted_sdf_reservation(&reserved, &stale, &mut producer.0),
            Err(SdfReservationError::KeyMismatch)
        ));
        assert!(producer.0.is_some());

        gpu.pending_registrations
            .lock()
            .unwrap()
            .push(producer.0.as_ref().unwrap().front_registration().unwrap());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            match renderer.abort_unadmitted_sdf_reservation(&reserved, &key, &mut producer.0) {
                Err(SdfReservationError::QueuedMutation) => break,
                Ok(false) => {
                    assert!(producer.0.is_some());
                    assert!(
                        renderer
                            .protected_sdf_texture_ids
                            .contains(&reserved.texture_id())
                    );
                    assert!(
                        std::time::Instant::now() < deadline,
                        "preparation completion was not observed"
                    );
                    std::thread::yield_now();
                }
                other => panic!("unexpected queued-front abort result: {other:?}"),
            }
        }
        assert_eq!(gpu.pending_registrations.lock().unwrap().len(), 1);
        assert!(producer.0.is_some());
        gpu.pending_registrations
            .lock()
            .unwrap()
            .retain(|entry| entry.texture_id != reserved.texture_id());
        let queued = producer
            .0
            .as_ref()
            .unwrap()
            .begin_frame(Some("cancelled future preparation"))
            .unwrap();
        assert!(
            !renderer
                .abort_unadmitted_sdf_reservation(&reserved, &key, &mut producer.0)
                .unwrap()
        );
        assert!(
            renderer
                .protected_sdf_texture_ids
                .contains(&reserved.texture_id())
        );
        assert!(matches!(
            renderer.admit_image(reserved.texture_id()),
            Err(ImageAdmissionError::MissingRegistration(_))
        ));
        drop(queued);
        loop {
            if renderer
                .abort_unadmitted_sdf_reservation(&reserved, &key, &mut producer.0)
                .unwrap()
            {
                assert!(completion.load(std::sync::atomic::Ordering::Acquire));
                break;
            }
            assert!(producer.0.is_some());
            assert!(
                renderer
                    .protected_sdf_texture_ids
                    .contains(&reserved.texture_id())
            );
            assert!(
                std::time::Instant::now() < deadline,
                "preparation completion was not observed"
            );
            std::thread::yield_now();
        }
        assert!(producer.0.is_none());
        assert!(
            !renderer
                .protected_sdf_texture_ids
                .contains(&reserved.texture_id())
        );
        assert!(
            !renderer
                .sdf_texture_reservations
                .contains_key(&reserved.texture_id())
        );
        assert_eq!(ordinary.availability(), Ok(()));

        let registered = renderer.reserve_sdf_texture(key.clone()).unwrap();
        let mut admitted_source = RetainOnFailure(Some(
            CustomRenderPass::new_unpublished(
                registered.texture_id(),
                gpu.clone(),
                PhysicalRect::new(0, 0, 16, 16),
            )
            .unwrap(),
        ));
        let frame = admitted_source
            .0
            .as_ref()
            .unwrap()
            .begin_frame(Some("registered control"))
            .unwrap();
        admitted_source
            .0
            .as_mut()
            .unwrap()
            .submit_and_swap_deferred_registration(frame)
            .unwrap();
        let receipt = renderer
            .register_reserved_sdf_front(&registered, &key, &key, &mut admitted_source.0)
            .unwrap();
        let binding = receipt.binding().clone();
        assert!(matches!(
            renderer.abort_unadmitted_sdf_reservation(&registered, &key, &mut admitted_source.0),
            Err(SdfReservationError::AlreadyRegistered)
        ));
        assert_eq!(binding.availability(), Ok(()));
        assert_eq!(
            renderer
                .admit_image(registered.texture_id())
                .unwrap()
                .identity(),
            binding.identity()
        );
        let mut record = match receipt.into_material_record(&key) {
            Ok(record) => record,
            Err(_) => panic!("actual registered generation rejected"),
        };
        let retirement_deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !record.retire_if_ready(&mut renderer).unwrap() {
            assert!(
                std::time::Instant::now() < retirement_deadline,
                "actual writer-certified retirement was not observed"
            );
            std::thread::yield_now();
        }
        assert_eq!(
            binding.availability(),
            Err(ImageBindingFailure::Unregistered)
        );
        assert_eq!(ordinary.availability(), Ok(()));
        renderer.register_image_rgba8_srgb(42, 1, 1, &[0, 0, 0, 255]);
        assert_eq!(ordinary.availability(), Err(ImageBindingFailure::Replaced));
        renderer.unregister_image(42);
    }

    #[test]
    fn sdf_reservation_authenticates_front_and_preserves_generations() {
        let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
        let gpu = renderer.gpu_resources();
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Label::new("reservation fixture")), None, 0);
        let rect = RectF::new(0.0, 0.0, 120.0, 30.0);
        let mut compositor = Compositor::new(rect);
        let surface = compositor.create_surface(0, rect, SceneKey::BASE);
        let metrics = CustomRenderTargetSnapshot::new(surface, PhysicalExtent::new(120, 30), 1.0)
            .unwrap()
            .metrics_for(rect, rect)
            .unwrap();
        let revision = SdfMaterialRevision::initial();
        let key = SdfMaterialKey::new(
            tree.snapshot_tree_lifetime(),
            tree.node(root).unwrap().id,
            surface,
            revision,
            metrics,
        )
        .unwrap();
        let stale = SdfMaterialKey::new(
            tree.snapshot_tree_lifetime(),
            tree.node(root).unwrap().id,
            surface,
            revision.checked_increment().unwrap(),
            metrics,
        )
        .unwrap();
        renderer.register_image_rgba8_srgb(1, 1, 1, &[255; 4]);
        let existing = renderer.admit_image(1).unwrap();
        gpu.pending_unregistrations
            .lock()
            .unwrap()
            .push(PendingImageUnregistration {
                texture_id: 2,
                retained_textures: Vec::new(),
            });
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("reservation queued collision"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: gpu.format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        gpu.pending_registrations
            .lock()
            .unwrap()
            .push(PendingImageRegistration {
                texture_id: 3,
                view: texture.create_view(&Default::default()),
                nearest: true,
            });
        let ordinary = gpu.queue_image_rgba8_srgb(4, 1, 1, vec![255; 4]).unwrap();
        let reservation = renderer.reserve_sdf_texture(key.clone()).unwrap();
        assert_eq!(reservation.texture_id(), 5);
        let independent = renderer.reserve_sdf_texture(key.clone()).unwrap();
        assert_eq!(independent.texture_id(), 6);
        renderer.drain_pending_image_registrations();
        assert!(ordinary.try_resolve().unwrap().is_some());
        assert_eq!(existing.availability(), Ok(()));

        let mut producer = Some(
            CustomRenderPass::new_unpublished(
                reservation.texture_id(),
                gpu.clone(),
                PhysicalRect::new(0, 0, 120, 30),
            )
            .unwrap(),
        );
        let mut frame = producer
            .as_ref()
            .unwrap()
            .begin_frame(Some("reserved front"))
            .unwrap();
        {
            let (encoder, view) = frame.encoder_and_view();
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("prepare authenticated front"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                multiview_mask: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
        }
        producer
            .as_mut()
            .unwrap()
            .submit_and_swap_deferred_registration(frame)
            .unwrap();
        assert!(matches!(
            renderer.register_reserved_sdf_front(&reservation, &stale, &key, &mut producer),
            Err(SdfReservationError::KeyMismatch)
        ));
        assert!(producer.is_some());
        assert!(matches!(
            renderer.admit_image(5),
            Err(ImageAdmissionError::MissingRegistration(5))
        ));
        let receipt = renderer
            .register_reserved_sdf_front(&reservation, &key, &key, &mut producer)
            .unwrap();
        assert!(producer.is_none());
        let binding = receipt.binding().clone();
        assert_eq!(
            renderer.admit_image(5).unwrap().identity(),
            binding.identity()
        );
        assert_eq!(binding.availability(), Ok(()));
        assert!(matches!(
            renderer.register_reserved_sdf_front(&reservation, &key, &key, &mut producer),
            Err(SdfReservationError::AlreadyRegistered)
        ));

        gpu.pending_unregistrations
            .lock()
            .unwrap()
            .push(PendingImageUnregistration {
                texture_id: 5,
                retained_textures: Vec::new(),
            });
        assert!(
            catch_unwind(AssertUnwindSafe(
                || renderer.drain_pending_image_registrations()
            ))
            .is_err()
        );
        assert_eq!(binding.availability(), Ok(()));
        assert_eq!(
            renderer.admit_image(5).unwrap().identity(),
            binding.identity()
        );
        gpu.pending_unregistrations.lock().unwrap().clear();

        let generation = match receipt.into_material_record(&stale) {
            Err((_, generation)) => generation,
            Ok(_) => panic!("stale receipt admitted"),
        };
        // Even direct P3 recovery cannot relabel a reserved generation under a new K.
        let generation = match SdfMaterialProducerRecord::from_admitted_generation(
            &stale,
            stale.clone(),
            generation,
        ) {
            Err((_, generation)) => generation,
            Ok(_) => panic!("reserved front relabeled"),
        };
        let mut record = match SdfMaterialProducerRecord::from_admitted_generation(
            &key,
            key.clone(),
            generation,
        ) {
            Ok(record) => record,
            Err(_) => panic!("authenticated current receipt rejected"),
        };
        assert!(
            matches!(record.publication().status(), SdfMaterialStatus::Ready { binding: current }
            if current.identity() == binding.identity())
        );
        let usage = record.acquire().unwrap();
        let mut scene = SceneBuilder::new();
        scene.push_admitted_image(
            rect,
            usage.binding(),
            RectF::new(0.0, 0.0, 1.0, 1.0),
            Color::WHITE,
            0,
            None,
        );
        let scene = scene.build();
        assert!(!record.retire_if_ready(&mut renderer).unwrap());
        assert!(renderer.protected_sdf_texture_ids.contains(&5));
        assert_eq!(binding.availability(), Ok(()));
        drop(scene);
        drop(usage);
        let retirement_deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !record.retire_if_ready(&mut renderer).unwrap() {
            assert!(
                std::time::Instant::now() < retirement_deadline,
                "actual writer-certified retirement was not observed"
            );
            std::thread::yield_now();
        }
        assert_eq!(
            binding.availability(),
            Err(ImageBindingFailure::Unregistered)
        );
        assert!(!renderer.protected_sdf_texture_ids.contains(&5));
        assert!(!renderer.sdf_texture_reservations.contains_key(&5));
        assert_eq!(existing.availability(), Ok(()));

        renderer.next_sdf_texture_id = Some(u64::MAX);
        let last = renderer.reserve_sdf_texture(key.clone()).unwrap();
        assert_eq!(last.texture_id(), u64::MAX);
        assert!(matches!(
            renderer.reserve_sdf_texture(key),
            Err(SdfReservationError::Exhausted)
        ));
    }

    fn window_eligibility_fixture() -> (WgpuRenderer, SdfMaterialKey, SdfAuthenticatedAdmission) {
        let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
        let gpu = renderer.gpu_resources();
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Label::new("eligibility owner")), None, 0);
        let rect = RectF::new(0.0, 0.0, 120.0, 30.0);
        let viewport = RectF::new(0.0, 0.0, 128.0, 64.0);
        let mut compositor = Compositor::new(viewport);
        let surface = compositor.create_surface(0, viewport, SceneKey::BASE);
        renderer
            .register_headless_render_target(surface, (320, 120), 1.0)
            .unwrap();
        let metrics = renderer
            .custom_render_target_snapshot(surface)
            .unwrap()
            .metrics_for(rect, viewport)
            .unwrap();
        let key = SdfMaterialKey::new(
            tree.snapshot_tree_lifetime(),
            tree.node(root).unwrap().id,
            surface,
            SdfMaterialRevision::initial(),
            metrics,
        )
        .unwrap();
        let reservation = renderer.reserve_sdf_texture(key.clone()).unwrap();
        let mut producer = Some(
            CustomRenderPass::new_unpublished(
                reservation.texture_id(),
                gpu,
                PhysicalRect::new(0, 0, 120, 30),
            )
            .unwrap(),
        );
        let mut frame = producer
            .as_ref()
            .unwrap()
            .begin_frame(Some("eligibility immutable front"))
            .unwrap();
        {
            let (encoder, view) = frame.encoder_and_view();
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("initialize eligibility front"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                multiview_mask: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
        }
        producer
            .as_mut()
            .unwrap()
            .submit_and_swap_deferred_registration(frame)
            .unwrap();
        let admission = renderer
            .register_reserved_sdf_front(&reservation, &key, &key, &mut producer)
            .unwrap();
        assert!(producer.is_none());
        (renderer, key, admission)
    }

    #[test]
    fn sdf_window_eligibility_requires_fresh_publication_and_original_admission() {
        use crate::core::frame_realization::AppFrameAttempts;
        use crate::renderer::sdf_image_generation_custody::SdfImageCustodyError;
        use crate::renderer::sdf_material_status::SdfMaterialRejection;

        let (mut renderer, original, mut admission) = window_eligibility_fixture();
        let identity = admission.binding().identity();
        let surface = original.metrics().surface_id;
        let original_metrics = original.metrics();
        let mut attempts = AppFrameAttempts::new();
        renderer.resize_render_target(surface, (400, 120)).unwrap();
        let snapshot = renderer.custom_render_target_snapshot(surface).unwrap();
        let resized_metrics = snapshot
            .with_app_frame_attempt(attempts.next())
            .metrics_for(
                original_metrics.final_logical_rect,
                original_metrics.final_logical_viewport,
            )
            .unwrap();
        let resized = SdfMaterialKey::new(
            original.tree_lifetime().clone(),
            original.owner(),
            surface,
            original.revision(),
            resized_metrics,
        )
        .unwrap();
        assert_ne!(original, resized);
        let first_publication = Arc::new(());
        let first = admission
            .certify_window_extent(&mut renderer, &resized, &first_publication)
            .unwrap();
        let (ready, registered) = admission
            .publication_for_eligibility(&mut renderer, &resized, &first, &first_publication)
            .unwrap();
        assert_eq!(ready.key().metrics(), resized_metrics);
        assert_eq!(registered, Some(identity));
        assert!(
            matches!(ready.status(), SdfMaterialStatus::Ready { binding }
            if binding.identity() == identity)
        );
        assert!(matches!(
            admission.publication_for_current(&mut renderer, &resized),
            Err(SdfMaterialRejection::KeyMismatch)
        ));
        assert!(admission.retain_scene_reference(&resized).is_err());

        let next_publication = Arc::new(());
        assert!(!first.matches(&resized, &next_publication));
        assert!(matches!(
            admission.publication_for_eligibility(
                &mut renderer,
                &resized,
                &first,
                &next_publication
            ),
            Err(SdfMaterialRejection::KeyMismatch)
        ));
        assert!(
            admission
                .retain_eligible_reference(&resized, &first, &next_publication)
                .is_err()
        );
        let renewed = admission
            .certify_window_extent(&mut renderer, &resized, &next_publication)
            .unwrap();
        assert!(renewed.matches(&resized, &next_publication));

        let next_metrics = snapshot
            .with_app_frame_attempt(attempts.next())
            .metrics_for(
                original_metrics.final_logical_rect,
                original_metrics.final_logical_viewport,
            )
            .unwrap();
        let same_k_new_attempt = SdfMaterialKey::new(
            original.tree_lifetime().clone(),
            original.owner(),
            surface,
            original.revision(),
            next_metrics,
        )
        .unwrap();
        assert_eq!(resized, same_k_new_attempt);
        assert_ne!(resized.metrics(), same_k_new_attempt.metrics());
        assert!(!renewed.matches(&same_k_new_attempt, &next_publication));
        assert!(matches!(
            admission.publication_for_eligibility(
                &mut renderer,
                &same_k_new_attempt,
                &renewed,
                &next_publication,
            ),
            Err(SdfMaterialRejection::KeyMismatch)
        ));

        // Translation and a smaller active clip get NEW current eligibility,
        // while the original immutable generation/admission remain unchanged.
        let placement_metrics = snapshot
            .with_app_frame_attempt(attempts.next())
            .metrics_for(
                RectF::new(0.25, 1.5, 120.0, 30.0),
                RectF::new(0.0, 0.0, 126.0, 20.0),
            )
            .unwrap();
        let placement = SdfMaterialKey::new(
            original.tree_lifetime().clone(),
            original.owner(),
            surface,
            original.revision(),
            placement_metrics,
        )
        .unwrap();
        assert_ne!(placement, original);
        let placement_publication = Arc::new(());
        let placement_certificate = admission
            .certify_window_extent(&mut renderer, &placement, &placement_publication)
            .unwrap();
        let (placement_ready, placed_identity) = admission
            .publication_for_eligibility(
                &mut renderer,
                &placement,
                &placement_certificate,
                &placement_publication,
            )
            .unwrap();
        assert_eq!(placed_identity, Some(identity));
        assert_eq!(placement_ready.key().metrics(), placement_metrics);
        assert!(
            matches!(placement_ready.status(), SdfMaterialStatus::Ready { binding }
            if binding.identity() == identity)
        );
        assert!(!placement_certificate.matches(&resized, &placement_publication));
        assert!(!placement_certificate.matches(&placement, &Arc::new(())));

        // A genuine face-size/content change still cannot qualify.
        let changed_rect = RectF::new(1.0, 0.0, 121.0, 30.0);
        let changed_geometry = SdfMaterialKey::new(
            original.tree_lifetime().clone(),
            original.owner(),
            surface,
            original.revision(),
            snapshot
                .metrics_for(changed_rect, original_metrics.final_logical_viewport)
                .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            admission.certify_window_extent(&mut renderer, &changed_geometry, &next_publication),
            Err(SdfMaterialRejection::KeyMismatch)
        ));

        renderer.resize_render_target(surface, (320, 120)).unwrap();
        let returned_metrics = renderer
            .custom_render_target_snapshot(surface)
            .unwrap()
            .with_app_frame_attempt(attempts.next())
            .metrics_for(
                original_metrics.final_logical_rect,
                original_metrics.final_logical_viewport,
            )
            .unwrap();
        let returned = SdfMaterialKey::new(
            original.tree_lifetime().clone(),
            original.owner(),
            surface,
            original.revision(),
            returned_metrics,
        )
        .unwrap();
        assert_eq!(returned, original);
        let returned_publication = Arc::new(());
        assert!(!first.matches(&returned, &returned_publication));
        let returned_certificate = admission
            .certify_window_extent(&mut renderer, &returned, &returned_publication)
            .unwrap();
        assert_eq!(admission.key.metrics(), original_metrics);
        assert_eq!(
            renderer
                .sdf_texture_reservations
                .get(&identity.texture_id())
                .unwrap()
                .key
                .metrics(),
            original_metrics,
        );
        assert_eq!(
            renderer
                .admit_image(identity.texture_id())
                .unwrap()
                .identity(),
            identity
        );

        let foreign = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
        assert!(matches!(
            admission
                .generation
                .validate_qualification(&foreign.gpu_resources()),
            Err(SdfImageCustodyError::QueueMismatch)
        ));
        let reference = admission
            .retain_eligible_reference(&returned, &returned_certificate, &returned_publication)
            .unwrap();
        renderer
            .pending_unregistrations
            .lock()
            .unwrap()
            .push(PendingImageUnregistration {
                texture_id: identity.texture_id(),
                retained_textures: Vec::new(),
            });
        assert!(matches!(
            admission.certify_window_extent(&mut renderer, &returned, &returned_publication),
            Err(SdfMaterialRejection::MissingEvidence)
        ));
        assert!(matches!(
            admission.publication_for_eligibility(
                &mut renderer,
                &returned,
                &returned_certificate,
                &returned_publication,
            ),
            Err(SdfMaterialRejection::MissingEvidence)
        ));
        // Remove only the injected entry for certified teardown; never requalify it.
        renderer.pending_unregistrations.lock().unwrap().clear();
        assert!(!admission.retire_if_ready(&mut renderer).unwrap());
        assert!(matches!(
            admission.certify_window_extent(&mut renderer, &returned, &returned_publication),
            Err(SdfMaterialRejection::MissingEvidence)
        ));
        assert_eq!(admission.binding().availability(), Ok(()));
        drop(reference);
        let retirement_deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !admission.retire_if_ready(&mut renderer).unwrap() {
            assert!(
                std::time::Instant::now() < retirement_deadline,
                "actual writer-certified retirement was not observed"
            );
            std::thread::yield_now();
        }
        assert_eq!(
            admission.binding().availability(),
            Err(ImageBindingFailure::Unregistered)
        );
    }

    #[test]
    fn sdf_window_eligibility_device_loss_settles_without_false_ready_or_retirement() {
        use crate::renderer::sdf_material_status::{
            SdfMaterialRejection, SdfMaterialUnavailableCause,
        };

        let (mut renderer, original, mut admission) = window_eligibility_fixture();
        let identity = admission.binding().identity();
        let surface = original.metrics().surface_id;
        renderer.resize_render_target(surface, (400, 120)).unwrap();
        let metrics = renderer
            .custom_render_target_snapshot(surface)
            .unwrap()
            .metrics_for(
                original.metrics().final_logical_rect,
                original.metrics().final_logical_viewport,
            )
            .unwrap();
        let current = SdfMaterialKey::new(
            original.tree_lifetime().clone(),
            original.owner(),
            surface,
            original.revision(),
            metrics,
        )
        .unwrap();
        let publication = Arc::new(());
        let certificate = admission
            .certify_window_extent(&mut renderer, &current, &publication)
            .unwrap();
        let (frozen, _) = admission
            .publication_for_eligibility(&mut renderer, &current, &certificate, &publication)
            .unwrap();
        let reference = admission
            .retain_eligible_reference(&current, &certificate, &publication)
            .unwrap();
        let gpu = renderer.gpu_resources();
        DeviceLifetimeObservation::for_device(&gpu.device)
            .state
            .record(
                wgpu::DeviceLostReason::Unknown,
                "qualification test failure".into(),
            );
        assert!(matches!(frozen.status(), SdfMaterialStatus::Ready { .. }));
        let mut rejected = None;
        assert!(matches!(
            crate::renderer::sdf_material_status::SdfMaterialPublication::replace_if_current(
                &mut rejected,
                &current,
                Some(identity),
                Some(frozen.clone()),
            ),
            Err(SdfMaterialRejection::BindingUnavailable(
                ImageBindingFailure::DeviceLost
            ))
        ));
        assert!(rejected.is_none());
        let (settled, _) = admission
            .publication_for_eligibility(&mut renderer, &current, &certificate, &publication)
            .unwrap();
        assert!(matches!(settled.status(), SdfMaterialStatus::Unavailable {
            cause: SdfMaterialUnavailableCause::BindingRevoked { withdrawn, cause }
        } if *withdrawn == identity && cause == &ImageBindingFailure::DeviceLost));
        assert!(matches!(
            admission.certify_window_extent(&mut renderer, &current, &Arc::new(())),
            Err(SdfMaterialRejection::BindingUnavailable(
                ImageBindingFailure::DeviceLost
            ))
        ));
        assert!(
            admission
                .retain_eligible_reference(&current, &certificate, &publication)
                .is_err()
        );
        assert!(admission.retire_if_ready(&mut renderer).is_err());
        drop(reference);
        assert!(admission.retire_if_ready(&mut renderer).is_err());
        assert!(
            renderer
                .protected_sdf_texture_ids
                .contains(&identity.texture_id())
        );
        assert_eq!(
            admission.binding().availability(),
            Err(ImageBindingFailure::DeviceLost)
        );
    }
}
