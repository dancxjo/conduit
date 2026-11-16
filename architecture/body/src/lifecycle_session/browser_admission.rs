//! Browser challenge admission while lulled, without reopening lifecycle state.
use super::{sign, BodyLifecycleSession, BodyLifecycleSessionError as Error};
use crate::{
    AdmissionManager, AdmissionSigns, AmbientAdmissionProof, BodyBiographyError, BodyState,
    CandidateInventory, MembershipCredential, PartReturnProof,
};
use alloc::vec::Vec;
use conduit_core::{BootId, HostAdvertisement, HostId};

impl BodyLifecycleSession {
    fn require_lulled_admission(&self, host: &HostId, boot: &BootId) -> Result<(), Error> {
        self.require_host(host, boot)?;
        if self.evidence.body.state != BodyState::Lulled || self.realization.is_some() {
            return Err(Error::NotLulled);
        }
        Ok(())
    }

    /// Read-only eligibility of a currently attached, admitted member's return.
    /// This neither authenticates the proposed Boot nor authorizes admission.
    /// Same-Boot reconnects are eligible; the existing fresh proof is still
    /// mandatory. An absent prior attachment conservatively refuses here.
    ///
    /// Only typed references in this session's complete Plot/Mask realization
    /// are inspected. The owner must ALSO fence external reservations, pending
    /// operations (including whole-biography dependencies) and retire old
    /// Shows/actions before any active return integration. The ordinary
    /// `admit_returning_host` Lulled guard remains unchanged.
    pub fn check_unused_member_return(
        &self,
        part: &crate::PartId,
        returning: &HostAdvertisement,
        authority_host: &HostId,
        authority_boot: &BootId,
    ) -> Result<(), Error> {
        self.require_host(authority_host, authority_boot)?;
        let current = self
            .evidence
            .membership
            .parts
            .iter()
            .find(|p| &p.part_id == part && p.state == crate::MembershipState::Admitted)
            .and_then(|p| p.current.as_ref())
            .ok_or(Error::StaleHost)?;
        if current.host_id != returning.host_id
            || returning.boot_id.as_str().is_empty()
            || returning.offer_generation.0 == 0
        {
            return Err(Error::StaleHost);
        }
        if &current.host_id == authority_host
            || matches!(self.evidence.body.state, BodyState::Fulfilled { .. })
        {
            return Err(Error::NotLulled);
        }
        if let Some(realization) = &self.realization {
            if realization
                .plan
                .plots
                .iter()
                .any(|p| plan_references_host(&p.plan, &current.host_id, 0))
                || realization
                    .plan
                    .mask_topologies
                    .iter()
                    .flat_map(|t| &t.chains)
                    .any(|c| plan_references_host(&c.plan, &current.host_id, 0))
            {
                return Err(Error::NotLulled);
            }
        }
        // BodyTimeRequirement contains only a semantic body basis/tolerance/
        // duration, not a typed Host scope. Concrete clock/correlation custody
        // belongs to the owner's separately checked external obligations.
        Ok(())
    }

    /// Complete an explicitly authorized browser challenge atomically with the
    /// authoritative biography. Candidate discovery alone does not authorize this call.
    #[allow(clippy::too_many_arguments)]
    pub fn admit_ambient_host(
        &mut self,
        admissions: &mut AdmissionManager,
        candidates: &mut CandidateInventory,
        proof: &AmbientAdmissionProof,
        now_millis: u64,
        authority_host: &HostId,
        authority_boot: &BootId,
    ) -> Result<MembershipCredential, Error> {
        self.require_lulled_admission(authority_host, authority_boot)?;
        let mut next = self.clone();
        next.make_membership_room(2)?;
        let first = next.next_sequence()?;
        let second = first.checked_add(1).ok_or(Error::SequenceExhausted)?;
        let mut membership = next.evidence.membership.clone();
        let prior = membership.events.len();
        let mut manager = admissions.clone();
        let mut inventory = candidates.clone();
        let credential = manager
            .complete_ambient(
                &mut inventory,
                &mut membership,
                proof,
                now_millis,
                AdmissionSigns {
                    part_admitted: sign(authority_host, authority_boot, first),
                    host_attached: sign(authority_host, authority_boot, second),
                    candidate_admitted: sign(authority_host, authority_boot, second),
                },
            )
            .map_err(Error::Admission)?;
        let events = membership.events[prior..]
            .iter()
            .zip([first, second])
            .map(|(event, sequence)| (event.change_id.clone(), sequence))
            .collect::<Vec<_>>();
        if events.len() != 2 {
            return Err(Error::Biography(BodyBiographyError::InvalidEvidence));
        }
        next.evidence
            .append_membership_events(membership, &events)
            .map_err(Error::Biography)?;
        *self = next;
        *admissions = manager;
        *candidates = inventory;
        Ok(credential)
    }

    /// Authenticate an admitted Part's return under its current Host and Boot;
    /// preserve all workset, foreground, and lifecycle state.
    #[allow(clippy::too_many_arguments)]
    pub fn admit_returning_host(
        &mut self,
        admissions: &mut AdmissionManager,
        advertisement: &HostAdvertisement,
        proof: &PartReturnProof,
        now_millis: u64,
        authority_host: &HostId,
        authority_boot: &BootId,
    ) -> Result<MembershipCredential, Error> {
        self.require_lulled_admission(authority_host, authority_boot)?;
        self.complete_return_transition(
            admissions,
            advertisement,
            proof,
            now_millis,
            authority_host,
            authority_boot,
        )
    }

    /// Authenticate an unused member's replacement without changing the current
    /// realization. External owner obligations must be checked by the caller;
    /// this Core transition is not permission to release pending remote work.
    #[allow(clippy::too_many_arguments)]
    pub fn admit_returning_unused_host(
        &mut self,
        admissions: &mut AdmissionManager,
        advertisement: &HostAdvertisement,
        proof: &PartReturnProof,
        now_millis: u64,
        authority_host: &HostId,
        authority_boot: &BootId,
    ) -> Result<MembershipCredential, Error> {
        self.check_unused_member_return(
            &proof.part_id,
            advertisement,
            authority_host,
            authority_boot,
        )?;
        let mut next = self.clone();
        let current = next
            .evidence
            .membership
            .parts
            .iter()
            .find(|part| part.part_id == proof.part_id)
            .and_then(|part| part.current.as_ref())
            .ok_or(Error::StaleHost)?
            .clone();
        next.observe_host_lost(
            &current.host_id,
            &current.boot_id,
            authority_host,
            authority_boot,
        )?;
        let credential = next.complete_return_transition(
            admissions,
            advertisement,
            proof,
            now_millis,
            authority_host,
            authority_boot,
        )?;
        *self = next;
        Ok(credential)
    }

    #[allow(clippy::too_many_arguments)]
    fn complete_return_transition(
        &mut self,
        admissions: &mut AdmissionManager,
        advertisement: &HostAdvertisement,
        proof: &PartReturnProof,
        now_millis: u64,
        authority_host: &HostId,
        authority_boot: &BootId,
    ) -> Result<MembershipCredential, Error> {
        let mut next = self.clone();
        next.make_membership_room(1)?;
        let sequence = next.next_sequence()?;
        let mut membership = next.evidence.membership.clone();
        let prior = membership.events.len();
        let mut manager = admissions.clone();
        let credential = manager
            .complete_return(
                &mut membership,
                advertisement,
                proof,
                now_millis,
                sign(authority_host, authority_boot, sequence),
            )
            .map_err(Error::Admission)?;
        let events = membership.events[prior..]
            .iter()
            .map(|event| (event.change_id.clone(), sequence))
            .collect::<Vec<_>>();
        if events.len() != 1 {
            return Err(Error::Biography(BodyBiographyError::InvalidEvidence));
        }
        next.evidence
            .append_membership_events(membership, &events)
            .map_err(Error::Biography)?;
        *self = next;
        *admissions = manager;
        Ok(credential)
    }
}

// No serialization, string scanning, allocation, authority or state mutation.
fn line_references_host(line: &conduit_core::AdmittedLine, host: &HostId) -> bool {
    &line.binding.source.host_id == host || &line.binding.sink.host_id == host
}
fn resource_references_host(binding: &conduit_core::ResourceBinding, host: &HostId) -> bool {
    binding
        .content
        .as_ref()
        .is_some_and(|content| &content.owner_host == host)
}
fn plan_references_host(plan: &conduit_core::Plan, host: &HostId, depth: u8) -> bool {
    if depth > conduit_core::MAXIMUM_PLANNED_ACTIVATION_DEPTH {
        return true;
    }
    if plan.activation_preparations.iter().any(|binding| {
        &binding.owner_host_id == host
            || binding
                .child_fragments
                .iter()
                .any(|child| &child.host_id == host)
    }) {
        return true;
    }
    if plan.fragments.iter().any(|fragment| {
        &fragment.host_id == host
            || fragment
                .plan_fragments
                .iter()
                .any(|commitment| &commitment.host_id == host)
            || fragment.placements.iter().any(|gear| {
                &gear.host_id == host
                    || gear
                        .authority
                        .iter()
                        .any(|authority| &authority.host_id == host)
                    || gear
                        .resources
                        .iter()
                        .any(|resource| resource_references_host(resource, host))
            })
            || fragment.connections.iter().any(|cord| {
                cord.selected_line
                    .as_ref()
                    .is_some_and(|line| line_references_host(line, host))
                    || cord
                        .admitted_lines
                        .iter()
                        .any(|line| line_references_host(line, host))
                    || cord.resource.as_ref().is_some_and(|resource| {
                        resource_references_host(&resource.source_binding, host)
                    })
            })
            || fragment.fore_ports.iter().any(|port| {
                port.selected_line
                    .as_ref()
                    .is_some_and(|line| line_references_host(line, host))
            })
            || fragment
                .shared_pools
                .iter()
                .flat_map(|pool| &pool.realization_envelope)
                .any(|envelope| {
                    &envelope.host_id == host
                        || envelope
                            .resources
                            .iter()
                            .any(|resource| resource_references_host(resource, host))
                        || envelope
                            .admitted_lines
                            .iter()
                            .any(|line| line_references_host(line, host))
                })
    }) {
        return true;
    }
    plan.activations.iter().any(|activation| {
        let child = match activation {
            conduit_core::PlannedActivationEntry::Unary(value) => &value.selected_plan,
            conduit_core::PlannedActivationEntry::Fold(value) => &value.selected_plan,
            conduit_core::PlannedActivationEntry::Scan(value) => &value.selected_plan,
        };
        plan_references_host(child, host, depth + 1)
    })
}

#[cfg(test)]
mod tests;
