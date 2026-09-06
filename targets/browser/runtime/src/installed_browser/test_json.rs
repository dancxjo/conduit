//! Explicit pre-Play input and output fixtures, absent from production builds.
use super::factory::{BrowserHostResult, BrowserInstallation, BrowserManifestation};
use super::BrowserOperation;
use conduit_core::*;
use conduit_kernel::{HostedValueStore, ValueStorage};
use std::cell::RefCell;
thread_local! { static INPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) }; }
pub(crate) fn input(bytes: &[u8]) {
    INPUT.with(|value| *value.borrow_mut() = bytes.to_vec());
}
static REFERENCE_SOURCE: BrowserInstallation = BrowserInstallation {
    implementation_id: "conduit-test/resource-source",
    offer: reference_source_offer,
    prepare: source,
    perform: None,
};
static REFERENCE_SINK: BrowserInstallation = BrowserInstallation {
    implementation_id: "conduit-test/resource-sink",
    offer: reference_sink_offer,
    prepare: sink,
    perform: Some(present),
};
static SOURCE: BrowserInstallation = BrowserInstallation {
    implementation_id: "conduit-test/json-source",
    offer: source_offer,
    prepare: source,
    perform: None,
};
static SINK: BrowserInstallation = BrowserInstallation {
    implementation_id: "conduit-test/json-sink",
    offer: sink_offer,
    prepare: sink,
    perform: Some(present),
};
pub(super) fn factory(id: &str) -> Option<&'static BrowserInstallation> {
    match id {
        "conduit-test/json-source" => Some(&SOURCE),
        "conduit-test/resource-source" => Some(&REFERENCE_SOURCE),
        "conduit-test/resource-sink" => Some(&REFERENCE_SINK),
        "conduit-test/json-sink" => Some(&SINK),
        _ => None,
    }
}
pub(crate) fn source_offer() -> CapabilityOffer {
    offer(false)
}
pub(crate) fn sink_offer() -> CapabilityOffer {
    offer(true)
}
fn offer(sink: bool) -> CapabilityOffer {
    let id = if sink {
        SINK.implementation_id
    } else {
        SOURCE.implementation_id
    };
    let mut contract = conduit_semantic_catalog::json_decode_contract();
    contract.kind_id = kind_id(id);
    if sink {
        contract.outputs.clear();
    } else {
        contract.outputs = contract.inputs.clone();
        contract.outputs[0].direction = PortDirection::Output;
        contract.inputs.clear();
    }
    conduit_semantic_catalog::realization_offer(
        contract,
        "conduit-test/json@1",
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: id,
            execution_profile: "conduit-test/json@1",
            implementation: id,
            artifact: "conduit-test/json@1",
        },
        if sink {
            vec![HostOperationRequirement {
                contract_id: "conduit-test/json-present".into(),
                target_kind: Some(kind_id(id)),
                maximum_in_flight: 1,
                maximum_input_bytes: 4096,
                maximum_output_bytes: 0,
            }]
        } else {
            Vec::new()
        },
        Vec::new(),
        Vec::new(),
    )
}
fn source(_: &PlannedGear, values: &mut HostedValueStore) -> Result<BrowserOperation, String> {
    INPUT
        .with(|bytes| values.store(&bytes.borrow()))
        .map(BrowserOperation::source)
        .map_err(|e| format!("{e:?}"))
}
fn sink(_: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserOperation, String> {
    Ok(BrowserOperation::presentation(4096, 1))
}
fn present(_: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    Ok(BrowserHostResult {
        output: None,
        manifestation: Some(BrowserManifestation {
            kind_id: "conduit-test/json-output",
            canonical_value: input.to_vec(),
        }),
    })
}

pub(crate) fn reference_source_offer() -> CapabilityOffer {
    let mut offer = source_offer();
    offer.kind_id = kind_id("conduit-test/resource-source");
    offer.capability_id = "conduit-test/resource-source".into();
    offer.implementation.implementation_id = "conduit-test/resource-source".into();
    offer.outputs[0].value_kind = kind_id(RESOURCE_REFERENCE_INFO_ID);
    offer
}

pub(crate) fn reference_sink_offer() -> CapabilityOffer {
    let mut offer = sink_offer();
    offer.kind_id = kind_id("conduit-test/resource-sink");
    offer.capability_id = "conduit-test/resource-sink".into();
    offer.implementation.implementation_id = "conduit-test/resource-sink".into();
    offer.inputs[0].value_kind = kind_id(RESOURCE_REFERENCE_INFO_ID);
    offer.host_operations[0].target_kind = Some(offer.kind_id.clone());
    offer
}
