//! Actual local publication/extent observations, never possession grants.
use super::{HostedResourceGeneration, ResourcePublicationRefusal};
use conduit_core::{BoundedResourceRef, ResourceBinding, ResourceHandleId};
use conduit_planner::resource_cost::{CostResidenceRefusal, CostResourceResidence};

impl CostResourceResidence for HostedResourceGeneration {
    fn cost_binding(&self) -> &ResourceBinding {
        &self.binding
    }
    fn cost_handle(&self) -> &ResourceHandleId {
        &self.access_binding.handle
    }
    fn cost_published_reference(&self) -> Result<&BoundedResourceRef, CostResidenceRefusal> {
        self.published().map_err(|error| match error {
            ResourcePublicationRefusal::Lost => CostResidenceRefusal::Lost,
            _ => CostResidenceRefusal::NotPublished,
        })?;
        Ok(&self.reference)
    }
    fn cost_resident_bytes(&self) -> u64 {
        u64::from(self.resident_bytes())
    }
}
