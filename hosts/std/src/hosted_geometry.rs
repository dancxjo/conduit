//! Finite std-host offers for portable geometry semantics.

use conduit_core::{
    kind_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    FaceStartupParameter, HostOperationContractId, HostOperationRequirement, ImplementationId,
    ImplementationOffer, KindContractRevision, PortDescriptor, PortDirection,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_presentation::{
    geometry_port, path2_type, point2_type, APPLY_TRANSFORM2_KIND, GEOMETRY_REVISION,
    POINT2_LITERAL_KIND, POINT2_TYPE, TRANSFORM2_TYPE, TRANSFORM_PATH2_FOUR_KIND,
};
use std::{format, vec, vec::Vec};

pub const GEOMETRY_PROFILE: &str = "std/geometry-kernel-hosted@1";
pub const GEOMETRY_ARTIFACT: &str = "conduit-std-host/geometry@1";
pub const GEOMETRY_HOST_OPERATION: &str = "conduit.host/geometry-transform@1";

pub fn geometry_std_offers() -> Vec<CapabilityOffer> {
    vec![
        offer(
            POINT2_LITERAL_KIND,
            vec![],
            vec![geometry_port(
                "point",
                &point2_type(),
                PortDirection::Output,
            )],
            false,
        ),
        offer(
            APPLY_TRANSFORM2_KIND,
            vec![geometry_port("point", &point2_type(), PortDirection::Input)],
            vec![geometry_port(
                "point",
                &point2_type(),
                PortDirection::Output,
            )],
            true,
        ),
        offer(
            TRANSFORM_PATH2_FOUR_KIND,
            vec![geometry_port(
                "path",
                &path2_type(4).expect("four-point path is bounded"),
                PortDirection::Input,
            )],
            vec![geometry_port(
                "path",
                &path2_type(4).expect("four-point path is bounded"),
                PortDirection::Output,
            )],
            true,
        ),
    ]
}

fn offer(
    kind: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
    uses_operation: bool,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![FaceStartupParameter {
            name: if kind == POINT2_LITERAL_KIND {
                "value"
            } else {
                "transform"
            }
            .into(),
            value_type: if kind == POINT2_LITERAL_KIND {
                POINT2_TYPE
            } else {
                TRANSFORM2_TYPE
            }
            .into(),
            has_default: false,
        }],
        shorthand: None,
        capability_id: CapabilityId::from(format!("std/{kind}@1")),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(GEOMETRY_REVISION),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(GEOMETRY_PROFILE),
            implementation_id: ImplementationId::from(format!("std/{kind}@1")),
            artifact_id: ArtifactId::from(GEOMETRY_ARTIFACT),
        },
        inputs,
        outputs,
        host_operations: if uses_operation {
            vec![HostOperationRequirement {
                contract_id: HostOperationContractId::from(GEOMETRY_HOST_OPERATION),
                target_kind: Some(kind_id(kind)),
                maximum_in_flight: 1,
                maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            }]
        } else {
            Vec::new()
        },
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 4,
            max_queue_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        },
    }
}
