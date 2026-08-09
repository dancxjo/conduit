use alloc::collections::BTreeSet;
use alloc::format;
use alloc::vec::Vec;
use conduit_core::{CapabilityId, CheckedFormId, PlanId};
use conduit_observatory::{
    validate_snapshot, CapabilityAvailability, CapabilityStatusReport, CapabilitySupport,
    HostReport, ObservatorySnapshot, OfferFreshness, OperationalState,
};

use crate::{
    ContinuityError, DelegatedTransitionGrant, DurableSystemId, ExactAssignment, HostInstance,
    RoleRequirement, SystemRecord,
};

impl SystemRecord {
    pub fn from_snapshot(
        system_id: DurableSystemId,
        checked_form_id: CheckedFormId,
        member_hosts: Vec<HostInstance>,
        requirements: Vec<RoleRequirement>,
        plan_id: &PlanId,
        transition_grants: Vec<DelegatedTransitionGrant>,
        snapshot: &ObservatorySnapshot,
    ) -> Result<Self, ContinuityError> {
        validate_snapshot(snapshot).map_err(ContinuityError::InvalidSnapshot)?;
        unique_members(&member_hosts)?;
        unique_requirements(&requirements)?;

        let mut plans = snapshot
            .plans
            .iter()
            .filter(|plan| &plan.plan_id == plan_id);
        let plan = plans.next().ok_or(ContinuityError::MissingPlan)?;
        if plans.next().is_some() {
            return Err(ContinuityError::AmbiguousPlan);
        }
        if plan.checked_form_id != checked_form_id {
            return Err(ContinuityError::CheckedFormMismatch);
        }

        let placements = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .collect::<Vec<_>>();
        if placements.len() != requirements.len() {
            return Err(ContinuityError::MissingRole(
                "role requirements must cover every planned operation".into(),
            ));
        }
        let mut assignments = Vec::with_capacity(requirements.len());
        for requirement in &requirements {
            let mut matches = placements
                .iter()
                .filter(|placement| placement.gear_id == requirement.gear_id);
            let placement = matches
                .next()
                .ok_or_else(|| ContinuityError::MissingRole(requirement.role_id.as_str().into()))?;
            if matches.next().is_some() {
                return Err(ContinuityError::AmbiguousRole(
                    requirement.role_id.as_str().into(),
                ));
            }
            let host = HostInstance {
                host_id: placement.host_id.clone(),
                boot_id: placement.boot_id.clone(),
            };
            if !member_hosts.contains(&host) {
                return Err(ContinuityError::MissingMember(
                    requirement.role_id.as_str().into(),
                ));
            }
            let report = exact_host_report(snapshot, &host)?;
            require_available_offer(report, &placement.capability_id)?;
            let offer = report
                .advertisement
                .capabilities
                .iter()
                .find(|offer| offer.capability_id == placement.capability_id)
                .ok_or_else(|| {
                    ContinuityError::CapabilityUnavailable(placement.capability_id.as_str().into())
                })?;
            if offer.kind_id != placement.kind_id
                || offer.kind_contract_revision != placement.kind_contract_revision
                || offer.implementation.execution_profile_id != placement.execution_profile_id
                || offer.implementation.implementation_id != placement.implementation_id
                || offer.implementation.artifact_id != placement.artifact_id
                || offer.inputs != placement.inputs
                || offer.outputs != placement.outputs
            {
                return Err(ContinuityError::SelectedRealizationMismatch(
                    requirement.role_id.as_str().into(),
                ));
            }
            let face = offer.checked_face();
            if face != requirement.checked_face {
                return Err(ContinuityError::CheckedFaceMismatch(
                    requirement.role_id.as_str().into(),
                ));
            }
            assignments.push(ExactAssignment {
                role_id: requirement.role_id.clone(),
                placement_id: placement.placement_id.clone(),
                host,
                capability_id: placement.capability_id.clone(),
                implementation_id: placement.implementation_id.clone(),
                artifact_id: placement.artifact_id.clone(),
                checked_face: face,
            });
        }

        let play_ids = snapshot
            .plays
            .iter()
            .filter(|play| &play.plan_id == plan_id)
            .map(|play| play.active_play_id.clone())
            .collect::<Vec<_>>();
        let clue_ids = snapshot
            .observations
            .iter()
            .filter(|observation| observation.plan_id.as_ref() == Some(plan_id))
            .map(|observation| observation.clue_id.clone())
            .collect::<Vec<_>>();
        for member in assignments.iter().map(|assignment| &assignment.host) {
            if !snapshot.plays.iter().any(|play| {
                &play.plan_id == plan_id
                    && play.host_id == member.host_id
                    && play.boot_id == member.boot_id
            }) {
                return Err(ContinuityError::MissingPlay(member.host_id.as_str().into()));
            }
        }

        let boot_scoped_authority = placements
            .iter()
            .flat_map(|placement| &placement.authority)
            .map(|binding| (*binding).clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let observed_links = snapshot
            .links
            .iter()
            .filter(|link| link.state == OperationalState::Available)
            .map(|link| link.binding.binding_id.clone())
            .collect();

        Ok(Self {
            system_id,
            checked_form_id,
            members: member_hosts,
            requirements,
            assignments,
            observed_links,
            boot_scoped_authority,
            transition_grants,
            plan_id: plan_id.clone(),
            play_ids,
            clue_ids,
        })
    }
}

fn unique_members(members: &[HostInstance]) -> Result<(), ContinuityError> {
    let unique = members
        .iter()
        .map(|member| (member.host_id.as_str(), member.boot_id.as_str()))
        .collect::<BTreeSet<_>>();
    if unique.len() == members.len() {
        Ok(())
    } else {
        Err(ContinuityError::MissingMember("duplicate member".into()))
    }
}

fn unique_requirements(requirements: &[RoleRequirement]) -> Result<(), ContinuityError> {
    let roles = requirements
        .iter()
        .map(|item| item.role_id.as_str())
        .collect::<BTreeSet<_>>();
    let operations = requirements
        .iter()
        .map(|item| item.gear_id.as_str())
        .collect::<BTreeSet<_>>();
    if roles.len() == requirements.len() && operations.len() == requirements.len() {
        Ok(())
    } else {
        Err(ContinuityError::AmbiguousRole(
            "duplicate requirement".into(),
        ))
    }
}

fn exact_host_report<'a>(
    snapshot: &'a ObservatorySnapshot,
    host: &HostInstance,
) -> Result<&'a HostReport, ContinuityError> {
    let mut reports = snapshot.hosts.iter().filter(|report| {
        report.advertisement.host_id == host.host_id && report.advertisement.boot_id == host.boot_id
    });
    let report = reports.next().ok_or_else(|| {
        ContinuityError::MissingHostReport(format!(
            "{}@{}",
            host.host_id.as_str(),
            host.boot_id.as_str()
        ))
    })?;
    if reports.next().is_some() {
        return Err(ContinuityError::MissingHostReport(
            "duplicate exact host report".into(),
        ));
    }
    if report.state != OperationalState::Available {
        return Err(ContinuityError::HostUnavailable(
            host.host_id.as_str().into(),
        ));
    }
    Ok(report)
}

fn require_available_offer(
    report: &HostReport,
    capability_id: &CapabilityId,
) -> Result<(), ContinuityError> {
    if status_is_available(&report.capabilities, capability_id) {
        Ok(())
    } else {
        Err(ContinuityError::CapabilityUnavailable(
            capability_id.as_str().into(),
        ))
    }
}

pub(crate) fn status_is_available(
    statuses: &[CapabilityStatusReport],
    capability_id: &CapabilityId,
) -> bool {
    statuses.iter().any(|status| {
        &status.capability_id == capability_id
            && status.freshness == OfferFreshness::Fresh
            && status.support == CapabilitySupport::Supported
            && status.availability == CapabilityAvailability::Available
    })
}
