//! Finite cumulative reservations against one exact Body resource envelope.
//!
//! This ledger owns Body-quota accounting only. It does not prepare a Host,
//! start a Play, or replace the Host adapter's concrete resource reservation.

use alloc::vec::Vec;
use conduit_core::{
    HostAdvertisement, PlanId, ResourceBinding, ResourceObservation, ResourceRequirement,
};
use serde::Serialize;

use crate::{
    BodyPlan, BodyResourceEnvelope, BodyResourceEnvelopeError, BodyResourceEnvelopeId, ResidentForm,
};

/// Maximum number of exact Plans concurrently retained by one envelope ledger.
pub const MAX_BODY_RESOURCE_RESERVATIONS: usize = 64;
pub const MAX_BODY_RESOURCE_BINDINGS: usize = 256;

#[derive(Debug, Clone, Copy)]
pub struct BodyFormResourceRequests<'a> {
    pub form: &'a ResidentForm,
    pub requests: &'a [(&'a ResourceRequirement, &'a ResourceBinding)],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BodyResourceReservation {
    plan_id: PlanId,
    envelope_id: BodyResourceEnvelopeId,
    bindings: Vec<ResourceBinding>,
}

impl BodyResourceReservation {
    pub fn plan_id(&self) -> &PlanId {
        &self.plan_id
    }

    pub fn envelope_id(&self) -> &BodyResourceEnvelopeId {
        &self.envelope_id
    }

    pub fn bindings(&self) -> &[ResourceBinding] {
        &self.bindings
    }
}

/// Inspectable cumulative accounting for one immutable envelope revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BodyResourceReservationLedger {
    envelope_id: BodyResourceEnvelopeId,
    reservations: Vec<BodyResourceReservation>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BodyResourceReservationError {
    Empty,
    CapacityExceeded,
    EnvelopeMismatch,
    WrongBody,
    WorksetMismatch,
    DuplicatePlan,
    UnknownPlan,
    MissingObservation,
    ArithmeticOverflow,
    Envelope(BodyResourceEnvelopeError),
}

impl core::fmt::Display for BodyResourceReservationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Body resource reservation refused: {self:?}")
    }
}

impl BodyResourceReservationLedger {
    pub fn new(envelope: &BodyResourceEnvelope) -> Self {
        Self {
            envelope_id: envelope.envelope_id().clone(),
            reservations: Vec::with_capacity(MAX_BODY_RESOURCE_RESERVATIONS),
        }
    }

    pub fn envelope_id(&self) -> &BodyResourceEnvelopeId {
        &self.envelope_id
    }

    pub fn reservations(&self) -> &[BodyResourceReservation] {
        &self.reservations
    }

    pub fn reserved_units(&self, binding: &ResourceBinding) -> u32 {
        self.reservations
            .iter()
            .flat_map(|reservation| &reservation.bindings)
            .filter(|reserved| {
                reserved.pool_id == binding.pool_id && reserved.class_id == binding.class_id
            })
            .fold(0u32, |total, reserved| total.saturating_add(reserved.units))
    }

    /// Atomically admits one Plan's Body quota reservations.
    ///
    /// `requests` retains each original requirement beside its exact selected
    /// binding. Current observations remain external mutable truth; this
    /// ledger merely accounts for reservations admitted since that truth was
    /// observed. The caller must refresh observations when their basis changes.
    pub fn reserve(
        &mut self,
        plan_id: PlanId,
        envelope: &BodyResourceEnvelope,
        host: &HostAdvertisement,
        observations: &[ResourceObservation],
        requests: &[(&ResourceRequirement, &ResourceBinding)],
    ) -> Result<(), BodyResourceReservationError> {
        if requests.is_empty() {
            return Err(BodyResourceReservationError::Empty);
        }
        if requests.len() > MAX_BODY_RESOURCE_BINDINGS {
            return Err(BodyResourceReservationError::CapacityExceeded);
        }
        if self.envelope_id != *envelope.envelope_id() {
            return Err(BodyResourceReservationError::EnvelopeMismatch);
        }
        if self.reservations.len() == MAX_BODY_RESOURCE_RESERVATIONS {
            return Err(BodyResourceReservationError::CapacityExceeded);
        }
        if self
            .reservations
            .iter()
            .any(|reservation| reservation.plan_id == plan_id)
        {
            return Err(BodyResourceReservationError::DuplicatePlan);
        }

        let mut bindings = Vec::with_capacity(requests.len());
        for (requirement, binding) in requests {
            let observation = observations
                .iter()
                .find(|value| {
                    value.host_id == *envelope.host_id()
                        && value.boot_id == *envelope.boot_id()
                        && value.pool_id == binding.pool_id
                        && value.class_id == binding.class_id
                })
                .ok_or(BodyResourceReservationError::MissingObservation)?;
            envelope
                .validates_reservation(requirement, binding, host, observation)
                .map_err(BodyResourceReservationError::Envelope)?;
            let cumulative = self
                .reserved_units(binding)
                .checked_add(
                    bindings
                        .iter()
                        .filter(|pending: &&ResourceBinding| {
                            pending.pool_id == binding.pool_id
                                && pending.class_id == binding.class_id
                        })
                        .try_fold(0u32, |total, pending| total.checked_add(pending.units))
                        .ok_or(BodyResourceReservationError::ArithmeticOverflow)?,
                )
                .ok_or(BodyResourceReservationError::ArithmeticOverflow)?
                .checked_add(binding.units)
                .ok_or(BodyResourceReservationError::ArithmeticOverflow)?;
            let allowance = envelope
                .allowances()
                .iter()
                .find(|value| {
                    value.pool_id == binding.pool_id && value.class_id == binding.class_id
                })
                .expect("validated reservation has an envelope allowance");
            if cumulative > allowance.maximum_units {
                return Err(BodyResourceReservationError::Envelope(
                    BodyResourceEnvelopeError::ReservationExceedsAllowance,
                ));
            }
            if cumulative > observation.unreserved_units {
                return Err(BodyResourceReservationError::Envelope(
                    BodyResourceEnvelopeError::ReservationUnavailable,
                ));
            }
            bindings.push((*binding).clone());
        }

        self.reservations.push(BodyResourceReservation {
            plan_id,
            envelope_id: self.envelope_id.clone(),
            bindings,
        });
        Ok(())
    }

    /// Atomically admits the combined resource demand of every Form partition
    /// in one sealed Body-wide Plan. Every Plan Form occurs exactly once,
    /// including Forms with no requests, before demand is flattened and keyed
    /// by the Body Plan identity.
    pub fn reserve_body_plan(
        &mut self,
        plan: &BodyPlan,
        envelope: &BodyResourceEnvelope,
        host: &HostAdvertisement,
        observations: &[ResourceObservation],
        form_requests: &[BodyFormResourceRequests<'_>],
    ) -> Result<(), BodyResourceReservationError> {
        if plan.body_id != *envelope.body_id() {
            return Err(BodyResourceReservationError::WrongBody);
        }
        if form_requests.len() != plan.forms.len()
            || form_requests
                .iter()
                .any(|partition| !plan.workset.contains(partition.form))
            || form_requests
                .windows(2)
                .any(|pair| pair[0].form >= pair[1].form)
        {
            return Err(BodyResourceReservationError::WorksetMismatch);
        }
        let request_count = form_requests.iter().try_fold(0usize, |total, partition| {
            total.checked_add(partition.requests.len())
        });
        let Some(request_count) =
            request_count.filter(|count| *count <= MAX_BODY_RESOURCE_BINDINGS)
        else {
            return Err(BodyResourceReservationError::CapacityExceeded);
        };
        let mut requests = Vec::with_capacity(request_count);
        for partition in form_requests {
            requests.extend_from_slice(partition.requests);
        }
        self.reserve(
            plan.plan_id.clone(),
            envelope,
            host,
            observations,
            &requests,
        )
    }

    pub fn release(
        &mut self,
        plan_id: &PlanId,
    ) -> Result<BodyResourceReservation, BodyResourceReservationError> {
        let index = self
            .reservations
            .iter()
            .position(|reservation| &reservation.plan_id == plan_id)
            .ok_or(BodyResourceReservationError::UnknownPlan)?;
        Ok(self.reservations.remove(index))
    }
}
