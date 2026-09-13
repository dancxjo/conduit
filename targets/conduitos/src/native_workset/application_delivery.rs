//! Admitted native implementations for the portable application Form seam.

use alloc::{format, vec};
use conduit_core::{CapabilityOffer, HostOperationRequirement, resource_requirement};

pub(super) const EVENT_IMPLEMENTATION: &str = "conduitos/application-event-delivery@1";
pub(super) const STATE_IMPLEMENTATION: &str = "conduitos/retained-application@1";
pub(super) const PRESENTATION_IMPLEMENTATION: &str = "conduitos/application-view-presentation@1";
pub(super) const EVENT_OPERATION: &str = "conduit.host/application-next-event@1";
pub(super) const STATE_OPERATION: &str = "conduit.host/application-apply-event@1";
pub(super) const PRESENTATION_OPERATION: &str = "conduit.host/present-application-view@1";
pub(super) const EVENT_BYTES: u32 = 128;
pub(super) const VIEW_BYTES: u32 = 1024;

pub(super) fn offers(build: &str) -> [CapabilityOffer; 3] {
    [
        offer(
            conduit_semantic_catalog::event_source_contract(),
            EVENT_IMPLEMENTATION,
            EVENT_OPERATION,
            0,
            EVENT_BYTES,
            build,
        ),
        offer(
            conduit_semantic_catalog::retained_application_contract(),
            STATE_IMPLEMENTATION,
            STATE_OPERATION,
            EVENT_BYTES,
            VIEW_BYTES,
            build,
        ),
        offer(
            conduit_semantic_catalog::view_presentation_contract(),
            PRESENTATION_IMPLEMENTATION,
            PRESENTATION_OPERATION,
            VIEW_BYTES,
            0,
            build,
        ),
    ]
}

fn offer(
    contract: conduit_semantic_catalog::StandardKindContract,
    implementation: &'static str,
    operation: &'static str,
    input: u32,
    output: u32,
    build: &str,
) -> CapabilityOffer {
    let mut offer = conduit_semantic_catalog::realization_offer(
        contract,
        conduit_semantic_catalog::APPLICATION_CONTRACT_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: implementation,
            execution_profile: "conduitos/bounded-application@1",
            implementation,
            artifact: "conduitos/application@1",
        },
        vec![HostOperationRequirement {
            contract_id: operation.into(),
            target_kind: None,
            maximum_in_flight: 1,
            maximum_input_bytes: input,
            maximum_output_bytes: output,
        }],
        vec![resource_requirement(
            "conduit.resource/runtime-memory@1",
            VIEW_BYTES,
        )],
        vec![],
    );
    offer.implementation.artifact_id = format!("conduitos-build/{build}").into();
    offer.limits.max_queue_items = 1;
    offer.limits.max_queue_bytes = input.max(output);
    offer
}
