//! Browser realization of the reusable bounded ordered framed-record queue.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ConfigurationValue,
    ExecutionProfileId, HostOperationContractId, HostOperationRequirement, ImplementationId,
    ImplementationOffer, KindContractRevision, PlannedGear, StructuredInfoValue,
    StructuredInfoValueShape,
};
use conduit_kernel::{Failure, FailureCode, HostedValueStore};

pub(crate) const HOST_OPERATION: &str = "conduit.host/browser-ordered-record-queue@1";
const IMPLEMENTATION: &str = "browser/ordered-record-queue@1";
const MAXIMUM: u32 = super::MAXIMUM_BROWSER_VALUE_BYTES as u32;

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

fn offer() -> CapabilityOffer {
    let definition = conduit_net::ordered_record_queue_kind_definition();
    CapabilityOffer {
        startup_parameters: vec![
            conduit_core::FaceStartupParameter {
                name: "maximum-items".into(),
                value_type: "Count".into(),
                has_default: true,
            },
            conduit_core::FaceStartupParameter {
                name: "maximum-frame-bytes".into(),
                value_type: "Count".into(),
                has_default: true,
            },
        ],
        shorthand: None,
        capability_id: CapabilityId::from(IMPLEMENTATION),
        kind_id: definition.kind_id.clone(),
        kind_contract_revision: KindContractRevision::from(
            conduit_net::ORDERED_RECORD_QUEUE_CONTRACT_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("browser/ordered-record-queue@1"),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-net/ordered-record-queue@1"),
        },
        inputs: definition.inputs,
        outputs: definition.outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(HOST_OPERATION),
            target_kind: Some(definition.kind_id),
            maximum_in_flight: 1,
            maximum_input_bytes: MAXIMUM,
            maximum_output_bytes: MAXIMUM,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: conduit_net::MAXIMUM_ORDERED_RECORD_QUEUE_ITEMS as u16,
            max_queue_bytes: MAXIMUM,
        },
    }
}

fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserOperation, String> {
    validate_placement(placement, &offer())?;
    let (maximum_items, _) = limits(placement).map_err(|_| "invalid ordered queue limits")?;
    Ok(BrowserOperation::unary(MAXIMUM, maximum_items as u32))
}

pub(crate) fn execute(placement: &PlannedGear, input: &[u8]) -> Result<Vec<u8>, Failure> {
    let (_, maximum_frame_bytes) = limits(placement).map_err(failure)?;
    let value = StructuredInfoValue::from_canonical_bytes(input).map_err(|_| failure(10))?;
    if value.value_type() != &conduit_net::framed_typed_record_type() {
        return Err(failure(11));
    }
    let StructuredInfoValueShape::Leaf(frame) = value.shape() else {
        return Err(failure(12));
    };
    if frame.len() > maximum_frame_bytes {
        return Err(failure(13));
    }
    conduit_net::decode_typed_record(frame).map_err(|error| failure(20 + error as u16))?;
    Ok(input.to_vec())
}

fn limits(placement: &PlannedGear) -> Result<(usize, usize), u16> {
    let [items, bytes] = placement.configuration.as_slice() else {
        return Err(1);
    };
    if items.key != "maximum-items" || bytes.key != "maximum-frame-bytes" {
        return Err(2);
    }
    let (ConfigurationValue::U64(items), ConfigurationValue::U64(bytes)) =
        (&items.value, &bytes.value)
    else {
        return Err(3);
    };
    let items = usize::try_from(*items).map_err(|_| 4_u16)?;
    let bytes = usize::try_from(*bytes).map_err(|_| 5_u16)?;
    if items == 0
        || items > conduit_net::MAXIMUM_ORDERED_RECORD_QUEUE_ITEMS
        || !(conduit_net::TYPED_RECORD_FRAME_HEADER_BYTES
            ..=conduit_net::MAXIMUM_TYPED_RECORD_FRAME_BYTES)
            .contains(&bytes)
    {
        return Err(6);
    }
    Ok((items, bytes))
}

fn failure(detail: u16) -> Failure {
    Failure {
        code: FailureCode::InvalidInput,
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{ConfigurationEntry, OfferGeneration, StructuredInfoValue};

    fn placement(maximum_frame_bytes: u64) -> PlannedGear {
        let offer = offer();
        PlannedGear {
            placement_id: "queue-placement".into(),
            gear_id: "queue".into(),
            kind_id: offer.kind_id,
            kind_contract_revision: offer.kind_contract_revision,
            execution_profile_id: offer.implementation.execution_profile_id,
            configuration: vec![
                ConfigurationEntry {
                    key: "maximum-items".into(),
                    value: ConfigurationValue::U64(4),
                },
                ConfigurationEntry {
                    key: "maximum-frame-bytes".into(),
                    value: ConfigurationValue::U64(maximum_frame_bytes),
                },
            ],
            host_id: "browser/queue".into(),
            boot_id: "browser-boot/queue".into(),
            offer_generation: OfferGeneration(1),
            capability_id: offer.capability_id,
            implementation_id: offer.implementation.implementation_id,
            artifact_id: offer.implementation.artifact_id,
            realization_characteristics: Vec::new(),
            limits: offer.limits,
            inputs: offer.inputs,
            outputs: offer.outputs,
            host_operations: offer.host_operations,
            resources: Vec::new(),
            authority: Vec::new(),
            pool_references: Vec::new(),
        }
    }

    fn framed() -> Vec<u8> {
        let text =
            StructuredInfoValue::leaf(conduit_net::text_type(), b"CALLING".to_vec()).unwrap();
        let record = conduit_net::typed_record_from_text(&text).unwrap();
        let mut bytes = vec![0; conduit_net::MAXIMUM_TYPED_RECORD_FRAME_BYTES];
        let length = conduit_net::frame_typed_record_value_into(&record, &mut bytes).unwrap();
        bytes.truncate(length);
        conduit_net::framed_typed_record_value(&bytes)
            .unwrap()
            .canonical_bytes()
            .unwrap()
    }

    #[test]
    fn exact_framed_record_passes_and_malformed_input_refuses() {
        let placement = placement(conduit_net::MAXIMUM_TYPED_RECORD_FRAME_BYTES as u64);
        let input = framed();
        assert_eq!(execute(&placement, &input).unwrap(), input);
        assert_eq!(
            execute(&placement, b"not a canonical frame")
                .unwrap_err()
                .code,
            FailureCode::InvalidInput
        );
    }

    #[test]
    fn planned_frame_bound_is_enforced_before_queue_output() {
        let input = framed();
        let value = StructuredInfoValue::from_canonical_bytes(&input).unwrap();
        let StructuredInfoValueShape::Leaf(frame) = value.shape() else {
            panic!("framed value must be a leaf")
        };
        let placement = placement((frame.len() - 1) as u64);
        assert_eq!(execute(&placement, &input).unwrap_err().detail, 13);
    }
}
