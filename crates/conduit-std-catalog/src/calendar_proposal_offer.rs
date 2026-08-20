//! Exact non-effectful std-Host offer for finite meeting proposals.

use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, FaceStartupParameter, ImplementationId, ImplementationOffer,
    KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
};

use crate::{
    calendar_proposal_result_type, CALENDAR_PROPOSAL_KIND, CALENDAR_PROPOSAL_MAXIMUM_RESULTS,
    CALENDAR_PROPOSAL_REQUEST_TYPE, CALENDAR_PROPOSAL_REVISION,
};

pub const CALENDAR_PROPOSAL_STD_PROFILE: &str = "std/calendar-proposal-kernel@1";
pub const CALENDAR_PROPOSAL_STD_IMPLEMENTATION: &str = "std/kernel-calendar-proposal@1";
pub const CALENDAR_PROPOSAL_STD_ARTIFACT: &str = "conduit-std-host/calendar-proposal@1";

pub fn calendar_proposal_std_offer() -> CapabilityOffer {
    let result = calendar_proposal_result_type();
    CapabilityOffer {
        startup_parameters: vec![FaceStartupParameter {
            name: "request".into(),
            value_type: CALENDAR_PROPOSAL_REQUEST_TYPE.into(),
            has_default: false,
        }],
        shorthand: None,
        capability_id: CapabilityId::from("calendar-propose-meeting"),
        kind_id: kind_id(CALENDAR_PROPOSAL_KIND),
        kind_contract_revision: KindContractRevision::from(CALENDAR_PROPOSAL_REVISION),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(CALENDAR_PROPOSAL_STD_PROFILE),
            implementation_id: ImplementationId::from(CALENDAR_PROPOSAL_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(CALENDAR_PROPOSAL_STD_ARTIFACT),
        },
        inputs: vec![],
        outputs: vec![PortDescriptor {
            port_id: port_id("proposal"),
            value_kind: result.profile().unwrap().value_kind().clone(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: CALENDAR_PROPOSAL_MAXIMUM_RESULTS,
            max_queue_bytes: (conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
                * usize::from(CALENDAR_PROPOSAL_MAXIMUM_RESULTS)) as u32,
        },
    }
}
