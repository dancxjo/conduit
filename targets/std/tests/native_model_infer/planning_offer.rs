//! Planning-only fixture clones the actual prepared owner's offers. The real
//! PreparedModelInfer executes the negative at the ordinary Host Call boundary.
use conduit_core::{CapabilityOffer, PlannedGear, ResourceOffer};
use conduit_std_host::hosted_model_work::{HostedModelWorkAdapter, ModelWorkTerminal};
pub struct PlanningOffer {
    capability: CapabilityOffer,
    resource: ResourceOffer,
}
impl PlanningOffer {
    pub fn from_prepared(value: &impl HostedModelWorkAdapter) -> Self {
        Self {
            capability: value.capability_offer().clone(),
            resource: value.resource_offer().clone(),
        }
    }
}
impl HostedModelWorkAdapter for PlanningOffer {
    fn capability_offer(&self) -> &CapabilityOffer {
        &self.capability
    }
    fn resource_offer(&self) -> &ResourceOffer {
        &self.resource
    }
    fn execute(&mut self, _: &PlannedGear, _: &[u8], _: &mut Vec<u8>) -> ModelWorkTerminal {
        panic!("planning fixture cannot substitute an execution owner");
    }
    fn cancel(&mut self) {}
}
