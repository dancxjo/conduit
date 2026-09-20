//! Browser realization of reusable bounded record-delivery status projection.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId, PlannedGear,
};
use conduit_kernel::{Failure, FailureCode, HostedValueStore};

pub(crate) const HOST_OPERATION: &str = "conduit.host/browser-record-delivery-status@1";
const IMPLEMENTATION: &str = "browser/record-delivery-status@1";
const MAXIMUM: u32 = conduit_net::MAXIMUM_RECORD_DELIVERY_CANONICAL_BYTES as u32;

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

fn offer() -> CapabilityOffer {
    let contract = conduit_net::record_delivery_status_semantic_contract();
    let target_kind = contract.kind_id.clone();
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(IMPLEMENTATION),
            execution_profile_id: ExecutionProfileId::from("browser/record-delivery-status@1"),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-net/record-delivery-status@1"),
            host_operations: vec![HostOperationRequirement {
                contract_id: HostOperationContractId::from(HOST_OPERATION),
                target_kind: Some(target_kind),
                maximum_in_flight: 1,
                maximum_input_bytes: MAXIMUM,
                maximum_output_bytes: MAXIMUM,
            }],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserOperation, String> {
    validate_placement(placement, &offer())?;
    if !placement.configuration.is_empty() {
        return Err("unexpected delivery-status configuration".into());
    }
    Ok(BrowserOperation::unary(
        MAXIMUM,
        u32::from(conduit_net::MAXIMUM_RECORD_DELIVERY_OBSERVATIONS),
    ))
}

pub(crate) fn prepare_codec(
    placement: &PlannedGear,
) -> Result<Option<conduit_net::BoundedRecordDeliveryStatusCodec>, String> {
    if placement.implementation_id.as_str() != IMPLEMENTATION {
        return Ok(None);
    }
    validate_placement(placement, &offer())?;
    conduit_net::BoundedRecordDeliveryStatusCodec::prepare()
        .map(Some)
        .map_err(|error| format!("prepare browser delivery status: {error:?}"))
}

pub(crate) fn failure(refusal: conduit_net::RecordDeliveryRefusal) -> Failure {
    Failure {
        code: FailureCode::InvalidInput,
        detail: super::record_delivery_refusal_detail(refusal),
    }
}
