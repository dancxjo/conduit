//! Exact native Body ownership, not a fabricated standalone Plot Play.
use super::{DomainBinding, DomainRefusal};
use crate::protection_domain::ProtectionDomainId;
use conduit_body::{BodyPlan, BodyPlayIdentity, ResidentPlot};
use conduit_core::{BootId, ExecutionRegionId, HostId, PlanId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BodyRegionBinding {
    pub active: BodyPlayIdentity,
    pub plot: ResidentPlot,
    pub partition_plan: PlanId,
    pub host: HostId,
    pub boot: BootId,
    pub region: ExecutionRegionId,
    pub domain: ProtectionDomainId,
}

impl BodyRegionBinding {
    pub fn admit(
        plan: &BodyPlan,
        active: &BodyPlayIdentity,
        plot: &ResidentPlot,
        host: &HostId,
        boot: &BootId,
        region: &ExecutionRegionId,
        domain: ProtectionDomainId,
    ) -> Result<Self, DomainRefusal> {
        if domain.0 == 0 || plan.verify_seal().is_err() || !active.validate_for(plan) {
            return Err(DomainRefusal::WrongBinding);
        }
        let partition = plan
            .plots
            .iter()
            .find(|partition| &partition.plot == plot)
            .ok_or(DomainRefusal::WrongBinding)?;
        let mut regions = partition
            .plan
            .fragments
            .iter()
            .filter(|fragment| &fragment.host_id == host && &fragment.boot_id == boot)
            .flat_map(|fragment| &fragment.execution_regions)
            .filter(|candidate| &candidate.region_id == region);
        let selected = regions.next().ok_or(DomainRefusal::WrongBinding)?;
        if regions.next().is_some() || selected.admitted_placements.is_empty() {
            return Err(DomainRefusal::WrongBinding);
        }
        Ok(Self {
            active: active.clone(),
            plot: plot.clone(),
            partition_plan: partition.plan.plan_id.clone(),
            host: host.clone(),
            boot: boot.clone(),
            region: region.clone(),
            domain,
        })
    }
}
impl DomainBinding for BodyRegionBinding {
    fn domain(&self) -> ProtectionDomainId {
        self.domain
    }
}

#[cfg(test)]
mod tests;
