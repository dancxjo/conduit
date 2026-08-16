//! Dynamic installed offer for one exact checked structured selector.

use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId, ImplementationOffer,
    KindContractRevision, PortDescriptor, PortDirection, PortTemporal, StructuredSelector,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub const STRUCTURED_SELECTOR_REVISION: &str = "structured-info/selector-operation@1";
pub const STRUCTURED_SELECTOR_STD_PROFILE: &str = "std/structured-selector-kernel-hosted@1";
pub const STRUCTURED_SELECTOR_STD_IMPLEMENTATION: &str = "std/kernel-structured-selector@1";
pub const STRUCTURED_SELECTOR_STD_ARTIFACT: &str = "conduit-core/structured-selector@1";
pub const STRUCTURED_SELECTOR_HOST_OPERATION: &str = "conduit.host/structured-selector@1";

pub fn structured_selector_std_offer(
    selector: &StructuredSelector,
    temporal: PortTemporal,
) -> CapabilityOffer {
    let kind_id = selector
        .kind_id(temporal)
        .expect("checked selector has finite semantic identity");
    let digest = kind_id
        .as_str()
        .strip_prefix("structured-info/selector-")
        .and_then(|value| value.strip_suffix(&format!("-{}@1", temporal.as_str())))
        .expect("structured selector kind identity is canonical");
    let input_kind = selector
        .input_type()
        .profile()
        .expect("checked selector input has finite profile")
        .value_kind()
        .clone();
    let output_kind = selector
        .output_type()
        .profile()
        .expect("checked selector output has finite profile")
        .value_kind()
        .clone();
    CapabilityOffer {
        startup_parameters: vec![conduit_core::FaceStartupParameter {
            name: "selector".into(),
            value_type: "Text".into(),
            has_default: false,
        }],
        shorthand: Some((port_id("input"), port_id("output"))),
        capability_id: CapabilityId::from(format!(
            "std-structured-selector-{digest}-{}",
            temporal.as_str()
        )),
        kind_id: kind_id.clone(),
        kind_contract_revision: KindContractRevision::from(STRUCTURED_SELECTOR_REVISION),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(STRUCTURED_SELECTOR_STD_PROFILE),
            implementation_id: ImplementationId::from(STRUCTURED_SELECTOR_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(STRUCTURED_SELECTOR_STD_ARTIFACT),
        },
        inputs: vec![PortDescriptor {
            port_id: port_id("input"),
            value_kind: input_kind,
            direction: PortDirection::Input,
            temporal,
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("output"),
            value_kind: output_kind,
            direction: PortDirection::Output,
            temporal,
        }],
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(STRUCTURED_SELECTOR_HOST_OPERATION),
            target_kind: Some(kind_id),
            maximum_in_flight: 1,
            maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 4,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32,
        },
    }
}
