//! Exact installed realization of the domain-owned Thermostat transition.
use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    ImplementationId,
};
pub const THERMOSTAT_COMBINE_IMPLEMENTATION: &str = "std/kernel-thermostat-combine@1";
pub const THERMOSTAT_COMBINE_ARTIFACT: &str = "conduit-std-host/thermostat-combine@1";
pub const THERMOSTAT_COMBINE_PROFILE: &str = "conduit.std/thermostat-combine-bounded@1";
pub fn thermostat_combine_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_thermostat_plot::thermostat_kind(),
        Back {
            capability_id: CapabilityId::from("std-thermostat-combine-v1"),
            execution_profile_id: ExecutionProfileId::from(THERMOSTAT_COMBINE_PROFILE),
            implementation_id: ImplementationId::from(THERMOSTAT_COMBINE_IMPLEMENTATION),
            artifact_id: ArtifactId::from(THERMOSTAT_COMBINE_ARTIFACT),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}
