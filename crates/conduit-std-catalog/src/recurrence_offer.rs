//! Exact finite installed-Host offer for recurrence expansion.

use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, FaceStartupParameter, ImplementationId, ImplementationOffer,
    KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
};

use crate::{
    recurrence_result_type, RECURRENCE_KIND, RECURRENCE_MAXIMUM_RESULTS, RECURRENCE_REQUEST_TYPE,
    RECURRENCE_REVISION, RECURRENCE_STD_ARTIFACT, RECURRENCE_STD_IMPLEMENTATION,
    RECURRENCE_STD_PROFILE,
};

pub fn recurrence_std_offer() -> CapabilityOffer {
    let result = recurrence_result_type();
    CapabilityOffer {
        startup_parameters: vec![FaceStartupParameter {
            name: "request".into(),
            value_type: RECURRENCE_REQUEST_TYPE.into(),
            has_default: false,
        }],
        shorthand: None,
        capability_id: CapabilityId::from("time-expand-recurrence"),
        kind_id: kind_id(RECURRENCE_KIND),
        kind_contract_revision: KindContractRevision::from(RECURRENCE_REVISION),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(RECURRENCE_STD_PROFILE),
            implementation_id: ImplementationId::from(RECURRENCE_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(RECURRENCE_STD_ARTIFACT),
        },
        inputs: vec![],
        outputs: vec![PortDescriptor {
            port_id: port_id("occurrences"),
            value_kind: result.profile().unwrap().value_kind().clone(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: RECURRENCE_MAXIMUM_RESULTS,
            max_queue_bytes: (conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
                * usize::from(RECURRENCE_MAXIMUM_RESULTS)) as u32,
        },
    }
}
