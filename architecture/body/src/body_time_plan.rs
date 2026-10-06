use alloc::{boxed::Box, vec::Vec};
use conduit_core::{
    BodyClockCorrelation, BodyTimeQuality, BootId, HostId, MonotonicDuration, MonotonicInstant,
    PlanId,
};

use crate::{BodyPlan, BodyPlanError};

pub const MAX_BODY_TIME_HOSTS: usize = 64;

#[derive(Clone, Copy)]
pub struct BodyHostClockEvidence<'a> {
    pub sample: &'a MonotonicInstant,
    pub correlation: &'a BodyClockCorrelation,
    pub transport_uncertainty: MonotonicDuration,
    pub scheduler_uncertainty: MonotonicDuration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyHostTimeAdmission {
    host_id: HostId,
    boot_id: BootId,
    correlation: BodyClockCorrelation,
    quality: BodyTimeQuality,
    transport_uncertainty: MonotonicDuration,
    scheduler_uncertainty: MonotonicDuration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyPlanTimeAdmission {
    plan_id: PlanId,
    hosts: Vec<BodyHostTimeAdmission>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodyPlanTimeRefusal {
    InvalidPlan(BodyPlanError),
    NotRequired,
    NoHosts,
    Capacity,
    BootConflict(HostId),
    MissingHost(HostId, BootId),
    UnknownHost(HostId, BootId),
    DuplicateHost(HostId, BootId),
    Quality(HostId, BootId, Box<BodyTimeQuality>),
}

impl BodyPlan {
    pub fn admit_body_time(
        &self,
        evidence: &[BodyHostClockEvidence<'_>],
    ) -> Result<BodyPlanTimeAdmission, BodyPlanTimeRefusal> {
        self.verify_seal()
            .map_err(BodyPlanTimeRefusal::InvalidPlan)?;
        let requirement = self
            .body_time_requirement
            .as_ref()
            .ok_or(BodyPlanTimeRefusal::NotRequired)?;
        let mut expected = Vec::<(HostId, BootId)>::new();
        let plot_fragments = self.plots.iter().flat_map(|plot| &plot.plan.fragments);
        let mask_fragments = self
            .mask_topologies
            .iter()
            .flat_map(|topology| &topology.chains)
            .flat_map(|chain| &chain.plan.fragments);
        for fragment in plot_fragments.chain(mask_fragments) {
            if expected
                .iter()
                .any(|(host, boot)| host == &fragment.host_id && boot != &fragment.boot_id)
            {
                return Err(BodyPlanTimeRefusal::BootConflict(fragment.host_id.clone()));
            }
            let pair = (fragment.host_id.clone(), fragment.boot_id.clone());
            if !expected.contains(&pair) {
                if expected.len() == MAX_BODY_TIME_HOSTS {
                    return Err(BodyPlanTimeRefusal::Capacity);
                }
                expected.push(pair);
            }
        }
        if expected.is_empty() {
            return Err(BodyPlanTimeRefusal::NoHosts);
        }
        if evidence.len() > MAX_BODY_TIME_HOSTS {
            return Err(BodyPlanTimeRefusal::Capacity);
        }
        let mut hosts = Vec::with_capacity(expected.len());
        for input in evidence {
            let clock = input.sample.clock();
            let host_id = clock.host_id().clone();
            let boot_id = clock.boot_id().clone();
            if !expected.contains(&(host_id.clone(), boot_id.clone())) {
                return Err(BodyPlanTimeRefusal::UnknownHost(host_id, boot_id));
            }
            if hosts.iter().any(|admitted: &BodyHostTimeAdmission| {
                admitted.host_id == host_id && admitted.boot_id == boot_id
            }) {
                return Err(BodyPlanTimeRefusal::DuplicateHost(host_id, boot_id));
            }
            let quality = requirement.assess_with_execution_bounds(
                input.correlation,
                input.sample,
                input.transport_uncertainty,
                input.scheduler_uncertainty,
            );
            if !matches!(quality, BodyTimeQuality::Ready { .. }) {
                return Err(BodyPlanTimeRefusal::Quality(
                    host_id,
                    boot_id,
                    Box::new(quality),
                ));
            }
            hosts.push(BodyHostTimeAdmission {
                host_id,
                boot_id,
                correlation: input.correlation.clone(),
                quality,
                transport_uncertainty: input.transport_uncertainty,
                scheduler_uncertainty: input.scheduler_uncertainty,
            });
        }
        for (host_id, boot_id) in expected {
            if !hosts
                .iter()
                .any(|admitted| admitted.host_id == host_id && admitted.boot_id == boot_id)
            {
                return Err(BodyPlanTimeRefusal::MissingHost(host_id, boot_id));
            }
        }
        hosts.sort_by(|left, right| {
            (&left.host_id, &left.boot_id).cmp(&(&right.host_id, &right.boot_id))
        });
        Ok(BodyPlanTimeAdmission {
            plan_id: self.plan_id.clone(),
            hosts,
        })
    }
}

impl BodyPlanTimeAdmission {
    pub fn plan_id(&self) -> &PlanId {
        &self.plan_id
    }

    pub fn hosts(&self) -> &[BodyHostTimeAdmission] {
        &self.hosts
    }

    pub fn for_host(&self, host_id: &HostId, boot_id: &BootId) -> Option<&BodyHostTimeAdmission> {
        self.hosts
            .iter()
            .find(|host| &host.host_id == host_id && &host.boot_id == boot_id)
    }
}

impl BodyHostTimeAdmission {
    pub fn host_id(&self) -> &HostId {
        &self.host_id
    }

    pub fn boot_id(&self) -> &BootId {
        &self.boot_id
    }

    pub fn correlation(&self) -> &BodyClockCorrelation {
        &self.correlation
    }

    pub fn quality(&self) -> &BodyTimeQuality {
        &self.quality
    }

    pub const fn execution_bounds(&self) -> (MonotonicDuration, MonotonicDuration) {
        (self.transport_uncertainty, self.scheduler_uncertainty)
    }
}
