use alloc::{string::String, vec::Vec};
use conduit_core::*;
// Admission of a pure Host-owned identity against one exact semantic Fore.
pub(super) fn validate(kind: Kind, offered: &CapabilityOffer) -> Result<(), String> {
    if offered.capability_id.as_str().is_empty()
        || offered.implementation.implementation_id.as_str().is_empty()
        || offered
            .implementation
            .execution_profile_id
            .as_str()
            .is_empty()
        || offered.implementation.artifact_id.as_str().is_empty()
    {
        return Err("finite owner requires complete Host realization identity".into());
    }
    let expected = BackOfferBuilder::new(
        kind,
        Back {
            capability_id: offered.capability_id.clone(),
            execution_profile_id: offered.implementation.execution_profile_id.clone(),
            implementation_id: offered.implementation.implementation_id.clone(),
            artifact_id: offered.implementation.artifact_id.clone(),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build();
    if offered != &expected {
        return Err("finite owner offer differs from its exact semantic Fore".into());
    }
    Ok(())
}
