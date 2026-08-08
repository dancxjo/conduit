use alloc::vec::Vec;
use conduit_core::{
    ActivePlayId, ArtifactId, AuthorityGrantId, CapabilityId, CheckedFormId, EvidenceId,
    ImplementationId, PlanId,
};
use conduit_observatory::{HostReport, OperationalState};
use serde::{Deserialize, Serialize};

use crate::record::status_is_available;
use crate::{ContinuityError, HostInstance, RoleId, SystemRecord, TransitionId};

impl SystemRecord {
    pub fn begin_transition(
        &self,
        transition_id: TransitionId,
        subject: HostInstance,
        cause: TransitionCause,
    ) -> Result<AcceptedTransition, ContinuityError> {
        if !self
            .assignments
            .iter()
            .any(|assignment| assignment.host == subject)
        {
            return Err(ContinuityError::UnknownSubject);
        }
        if let TransitionCause::Delegated {
            grant_id,
            controller,
            ..
        } = &cause
        {
            let grant = self
                .transition_grants
                .iter()
                .find(|grant| &grant.grant_id == grant_id)
                .ok_or(ContinuityError::MissingTransitionGrant)?;
            if grant.subject != subject || &grant.controller != controller {
                return Err(ContinuityError::TransitionGrantMismatch);
            }
            if grant.maximum_transitions == 0 {
                return Err(ContinuityError::TransitionGrantExhausted);
            }
        }
        Ok(AcceptedTransition {
            transition_id,
            subject,
            cause,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransitionCause {
    Local {
        accepted_evidence_id: EvidenceId,
    },
    Delegated {
        grant_id: AuthorityGrantId,
        controller: HostInstance,
        accepted_evidence_id: EvidenceId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptedTransition {
    pub transition_id: TransitionId,
    pub subject: HostInstance,
    pub cause: TransitionCause,
}

impl AcceptedTransition {
    pub fn old_boot_terminated(self, evidence_id: EvidenceId) -> TerminatedTransition {
        TerminatedTransition {
            accepted: self,
            terminal_evidence_id: evidence_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminatedTransition {
    pub accepted: AcceptedTransition,
    pub terminal_evidence_id: EvidenceId,
}

impl TerminatedTransition {
    pub fn observe_replacement(
        self,
        report: HostReport,
    ) -> Result<ObservedReplacement, ContinuityError> {
        if report.advertisement.host_id != self.accepted.subject.host_id {
            return Err(ContinuityError::ReplacementHostMismatch);
        }
        if report.advertisement.boot_id == self.accepted.subject.boot_id {
            return Err(ContinuityError::ReplacementBootReused);
        }
        if report.state != OperationalState::Available {
            return Err(ContinuityError::ReplacementUnavailable);
        }
        Ok(ObservedReplacement {
            terminated: self,
            report,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedReplacement {
    pub terminated: TerminatedTransition,
    pub report: HostReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompatibleReplacement {
    pub role_id: RoleId,
    pub capability_id: CapabilityId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplanRequired {
    pub transition: ObservedReplacement,
    pub stale_roles: Vec<RoleId>,
    pub stale_authority: Vec<AuthorityGrantId>,
    pub compatible_replacements: Vec<CompatibleReplacement>,
    pub prior_plan_id: PlanId,
    pub prior_play_ids: Vec<ActivePlayId>,
    pub checked_form_id: CheckedFormId,
    pub system_id: crate::DurableSystemId,
    pub requirements: Vec<crate::RoleRequirement>,
}

impl ObservedReplacement {
    pub fn assess(self, record: &SystemRecord) -> ReplanRequired {
        let stale_assignments = record
            .assignments
            .iter()
            .filter(|assignment| assignment.host == self.terminated.accepted.subject)
            .collect::<Vec<_>>();
        let mut stale_authority = record
            .boot_scoped_authority
            .iter()
            .filter(|binding| {
                binding.host_id == self.terminated.accepted.subject.host_id
                    && binding.boot_id == self.terminated.accepted.subject.boot_id
            })
            .map(|binding| binding.grant_id.clone())
            .collect::<Vec<_>>();
        stale_authority.extend(
            record
                .transition_grants
                .iter()
                .filter(|grant| grant.subject == self.terminated.accepted.subject)
                .map(|grant| grant.grant_id.clone()),
        );
        stale_authority.sort();
        stale_authority.dedup();

        let compatible_replacements = stale_assignments
            .iter()
            .flat_map(|assignment| {
                self.report
                    .advertisement
                    .capabilities
                    .iter()
                    .filter(|offer| {
                        offer.checked_face() == assignment.checked_face
                            && status_is_available(&self.report.capabilities, &offer.capability_id)
                    })
                    .map(|offer| CompatibleReplacement {
                        role_id: assignment.role_id.clone(),
                        capability_id: offer.capability_id.clone(),
                        implementation_id: offer.implementation.implementation_id.clone(),
                        artifact_id: offer.implementation.artifact_id.clone(),
                    })
            })
            .collect();

        ReplanRequired {
            transition: self,
            stale_roles: stale_assignments
                .iter()
                .map(|assignment| assignment.role_id.clone())
                .collect(),
            stale_authority,
            compatible_replacements,
            prior_plan_id: record.plan_id.clone(),
            prior_play_ids: record.play_ids.clone(),
            checked_form_id: record.checked_form_id.clone(),
            system_id: record.system_id.clone(),
            requirements: record.requirements.clone(),
        }
    }
}

impl ReplanRequired {
    pub fn accept_replanned(
        self,
        replacement: SystemRecord,
    ) -> Result<SystemRecord, ContinuityError> {
        if replacement.plan_id == self.prior_plan_id {
            return Err(ContinuityError::ReplanStillUsesOldPlan);
        }
        if replacement.checked_form_id != self.checked_form_id {
            return Err(ContinuityError::ReplanChangedCheckedForm);
        }
        if replacement.system_id != self.system_id {
            return Err(ContinuityError::ReplanChangedSystem);
        }
        if replacement.requirements != self.requirements {
            return Err(ContinuityError::ReplanChangedRoles);
        }
        let new_host = HostInstance {
            host_id: self.transition.report.advertisement.host_id.clone(),
            boot_id: self.transition.report.advertisement.boot_id.clone(),
        };
        for role in &self.stale_roles {
            if !replacement
                .assignments
                .iter()
                .any(|assignment| &assignment.role_id == role && assignment.host == new_host)
            {
                return Err(ContinuityError::ReplanMissingReplacement(
                    role.as_str().into(),
                ));
            }
        }
        if replacement
            .boot_scoped_authority
            .iter()
            .any(|grant| self.stale_authority.contains(&grant.grant_id))
            || replacement
                .transition_grants
                .iter()
                .any(|grant| self.stale_authority.contains(&grant.grant_id))
        {
            return Err(ContinuityError::ReplanInheritedStaleAuthority);
        }
        if replacement
            .play_ids
            .iter()
            .any(|play| self.prior_play_ids.contains(play))
        {
            return Err(ContinuityError::ReplanReusedPlay);
        }
        Ok(replacement)
    }
}
