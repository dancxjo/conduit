//! Exact fixed ConduitOS realizations of the portable text contracts.

use crate::{
    machine::BaseKind,
    offer::{
        CapabilityOffer, PortDirection, PortOffer, SERIAL_MAXIMUM_BYTES, SERIAL_OPERATION_CAPACITY,
        TEXT_LITERAL_IMPLEMENTATION, TEXT_PRESENTATION_IMPLEMENTATION,
    },
};

pub(super) fn literal(build_id: &str) -> CapabilityOffer<'_> {
    CapabilityOffer {
        kind: "text/literal",
        contract_revision: "conduit.std/text-literal@1",
        implementation: TEXT_LITERAL_IMPLEMENTATION,
        artifact_build: build_id,
        host_operation: None,
        required_base: BaseKind::Memory,
        secondary_base: None,
        input: None,
        output: Some(PortOffer {
            name: "text",
            value_kind: "value/text@1",
            direction: PortDirection::Output,
            closes: true,
        }),
        maximum_in_flight: 1,
        maximum_input_bytes: conduit_std_catalog::MAX_TEXT_BYTES,
        maximum_output_bytes: conduit_std_catalog::MAX_TEXT_BYTES,
    }
}

pub(super) fn presentation(build_id: &str) -> CapabilityOffer<'_> {
    CapabilityOffer {
        kind: "presentation/text",
        contract_revision: "conduit.std/presentation-text@1",
        implementation: TEXT_PRESENTATION_IMPLEMENTATION,
        artifact_build: build_id,
        host_operation: Some("conduit.host/present@1"),
        required_base: BaseKind::Serial,
        secondary_base: None,
        input: Some(PortOffer {
            name: "text",
            value_kind: "value/text@1",
            direction: PortDirection::Input,
            closes: true,
        }),
        output: None,
        maximum_in_flight: SERIAL_OPERATION_CAPACITY,
        maximum_input_bytes: SERIAL_MAXIMUM_BYTES,
        maximum_output_bytes: 0,
    }
}
