use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, FaceStartupParameter, ImplementationId, ImplementationOffer,
    KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub const RECURRENCE_STD_PROFILE: &str = "std/recurrence-kernel@1";
pub const RECURRENCE_STD_IMPLEMENTATION: &str = "std/kernel-expand-recurrence@1";
pub const RECURRENCE_STD_ARTIFACT: &str = "conduit-std-host/expand-recurrence@1";
pub const CALENDAR_PROPOSAL_STD_PROFILE: &str = "std/calendar-proposal-kernel@1";
pub const CALENDAR_PROPOSAL_STD_IMPLEMENTATION: &str = "std/kernel-calendar-proposal@1";
pub const CALENDAR_PROPOSAL_STD_ARTIFACT: &str = "conduit-std-host/calendar-proposal@1";

pub fn recurrence_std_offer() -> CapabilityOffer {
    let result = conduit_std_catalog::recurrence_result_type();
    offer(
        "time-expand-recurrence",
        conduit_std_catalog::RECURRENCE_KIND,
        conduit_std_catalog::RECURRENCE_REVISION,
        vec![FaceStartupParameter {
            name: "request".into(),
            value_type: conduit_std_catalog::RECURRENCE_REQUEST_TYPE.into(),
            has_default: false,
        }],
        vec![PortDescriptor {
            port_id: port_id("occurrences"),
            value_kind: result.profile().unwrap().value_kind().clone(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        conduit_std_catalog::RECURRENCE_MAXIMUM_RESULTS,
        RECURRENCE_STD_PROFILE,
        RECURRENCE_STD_IMPLEMENTATION,
        RECURRENCE_STD_ARTIFACT,
    )
}

pub fn calendar_proposal_std_offer() -> CapabilityOffer {
    let result = conduit_std_catalog::calendar_proposal_result_type();
    offer(
        "calendar-propose-meeting",
        conduit_std_catalog::CALENDAR_PROPOSAL_KIND,
        conduit_std_catalog::CALENDAR_PROPOSAL_REVISION,
        vec![FaceStartupParameter {
            name: "request".into(),
            value_type: conduit_std_catalog::CALENDAR_PROPOSAL_REQUEST_TYPE.into(),
            has_default: false,
        }],
        vec![PortDescriptor {
            port_id: port_id("proposal"),
            value_kind: result.profile().unwrap().value_kind().clone(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        conduit_std_catalog::CALENDAR_PROPOSAL_MAXIMUM_RESULTS,
        CALENDAR_PROPOSAL_STD_PROFILE,
        CALENDAR_PROPOSAL_STD_IMPLEMENTATION,
        CALENDAR_PROPOSAL_STD_ARTIFACT,
    )
}

#[allow(clippy::too_many_arguments)]
fn offer(
    capability: &str,
    kind: &str,
    revision: &str,
    startup_parameters: Vec<FaceStartupParameter>,
    outputs: Vec<PortDescriptor>,
    maximum_results: u16,
    execution_profile: &str,
    implementation: &str,
    artifact: &str,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters,
        shorthand: None,
        capability_id: CapabilityId::from(capability),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(revision),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(execution_profile),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(artifact),
        },
        inputs: Vec::new(),
        outputs,
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: maximum_results,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * usize::from(maximum_results))
                as u32,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculation_offers_preserve_exact_portable_shapes_and_bounds() {
        for offer in [recurrence_std_offer(), calendar_proposal_std_offer()] {
            assert_eq!(offer.startup_parameters.len(), 1);
            assert!(offer.inputs.is_empty());
            assert_eq!(offer.outputs.len(), 1);
            assert!(offer.host_operations.is_empty());
            assert!(offer.resource_requirements.is_empty());
            assert!(offer.authority_requirements.is_empty());
            assert!(offer.limits.max_queue_items > 0);
            assert!(offer.limits.max_queue_bytes > 0);
        }
    }
}
