//! Exact boot-scoped ConduitOS capabilities for the standing Tour timer Form.

use crate::{
    machine::BaseKind,
    offer::{
        COUNT_PRESENTATION_IMPLEMENTATION, CapabilityOffer, PortDirection, PortOffer,
        STATE_COUNT_IMPLEMENTATION, TIME_EVERY_IMPLEMENTATION,
    },
};

pub(super) fn every(build_id: &str) -> CapabilityOffer<'_> {
    CapabilityOffer {
        kind: conduit_time::TIME_EVERY_KIND,
        contract_revision: conduit_time::TIME_EVERY_CONTRACT_REVISION,
        implementation: TIME_EVERY_IMPLEMENTATION,
        artifact_build: build_id,
        host_operation: Some("conduit.host/wait@1"),
        required_base: BaseKind::Timer,
        secondary_base: Some(BaseKind::Clock),
        input: None,
        output: Some(PortOffer {
            name: "tick",
            value_kind: conduit_time::TICK_VALUE_KIND,
            direction: PortDirection::Output,
            closes: false,
        }),
        maximum_in_flight: 1,
        maximum_input_bytes: conduit_time::TICK_ENCODED_LEN,
        maximum_output_bytes: conduit_time::TICK_ENCODED_LEN,
    }
}

pub(super) fn count(build_id: &str) -> CapabilityOffer<'_> {
    CapabilityOffer {
        kind: conduit_semantic_catalog::STATE_COUNT_KIND,
        contract_revision: conduit_semantic_catalog::STATE_COUNT_CONTRACT_REVISION,
        implementation: STATE_COUNT_IMPLEMENTATION,
        artifact_build: build_id,
        host_operation: None,
        required_base: BaseKind::Memory,
        secondary_base: None,
        input: Some(PortOffer {
            name: "bump",
            value_kind: conduit_time::TICK_VALUE_KIND,
            direction: PortDirection::Input,
            closes: false,
        }),
        output: Some(PortOffer {
            name: "value",
            value_kind: conduit_semantic_catalog::STATE_COUNT_VALUE_KIND,
            direction: PortDirection::Output,
            closes: true,
        }),
        maximum_in_flight: 1,
        maximum_input_bytes: conduit_time::TICK_ENCODED_LEN,
        maximum_output_bytes: conduit_semantic_catalog::COUNT_ENCODED_LEN,
    }
}

pub(super) fn presentation(build_id: &str) -> CapabilityOffer<'_> {
    CapabilityOffer {
        kind: conduit_semantic_catalog::COUNT_PRESENTATION_KIND,
        contract_revision: conduit_semantic_catalog::COUNT_PRESENTATION_CONTRACT_REVISION,
        implementation: COUNT_PRESENTATION_IMPLEMENTATION,
        artifact_build: build_id,
        host_operation: Some("conduit.host/present-count@1"),
        required_base: BaseKind::Serial,
        secondary_base: None,
        input: Some(PortOffer {
            name: "value",
            value_kind: conduit_semantic_catalog::STATE_COUNT_VALUE_KIND,
            direction: PortDirection::Input,
            closes: true,
        }),
        output: None,
        maximum_in_flight: 1,
        maximum_input_bytes: conduit_semantic_catalog::COUNT_ENCODED_LEN,
        maximum_output_bytes: 0,
    }
}
