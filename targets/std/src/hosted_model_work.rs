//! Host-prepared implementations of portable model-session operations.
use conduit_core::{CapabilityOffer, PlannedGear, ResourceOffer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelWorkTerminal {
    Produced,
    Refused,
    Failed,
    Cancelled,
    ProviderLost,
    MalformedInput,
}

pub trait HostedModelWorkAdapter: Send {
    fn capability_offer(&self) -> &CapabilityOffer;
    fn resource_offer(&self) -> &ResourceOffer;
    fn execute(
        &mut self,
        placement: &PlannedGear,
        input: &[u8],
        output: &mut Vec<u8>,
    ) -> ModelWorkTerminal;
    fn cancel(&mut self);
}
