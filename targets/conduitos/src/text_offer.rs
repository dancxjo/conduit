//! Exact fixed ConduitOS realizations of the portable text contracts.

use crate::{
    machine::BaseKind,
    offer::{
        CapabilityOffer, INDICATOR_PRESENTATION_IMPLEMENTATION, MORSE_FLATTEN_IMPLEMENTATION,
        MORSE_INTERSPERSE_IMPLEMENTATION, MORSE_LOOKUP_IMPLEMENTATION,
        MORSE_SYMBOLS_TO_PATTERN_IMPLEMENTATION, PortDirection, PortOffer, SERIAL_MAXIMUM_BYTES,
        TEXT_CHARACTERS_IMPLEMENTATION, TEXT_LITERAL_IMPLEMENTATION, TEXT_MORSE_IMPLEMENTATION,
        TEXT_PRESENTATION_IMPLEMENTATION, TEXT_UPPER_IMPLEMENTATION,
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
        maximum_input_bytes: conduit_text::MAX_TEXT_BYTES,
        maximum_output_bytes: conduit_text::MAX_TEXT_BYTES,
    }
}

pub(super) fn upper(build_id: &str) -> CapabilityOffer<'_> {
    CapabilityOffer {
        kind: conduit_text::TEXT_UPPER_KIND,
        contract_revision: conduit_text::TEXT_UPPER_CONTRACT_REVISION,
        implementation: TEXT_UPPER_IMPLEMENTATION,
        artifact_build: build_id,
        host_operation: Some(crate::functional_offers::TEXT_UPPER_HOST_OPERATION),
        required_base: BaseKind::Memory,
        secondary_base: None,
        input: Some(PortOffer {
            name: "text",
            value_kind: conduit_semantic_catalog::TEXT_PRESENTATION_VALUE_KIND,
            direction: PortDirection::Input,
            closes: true,
        }),
        output: Some(PortOffer {
            name: "text",
            value_kind: conduit_semantic_catalog::TEXT_PRESENTATION_VALUE_KIND,
            direction: PortDirection::Output,
            closes: true,
        }),
        maximum_in_flight: 1,
        maximum_input_bytes: conduit_text::MAX_TEXT_BYTES,
        maximum_output_bytes: conduit_text::MAX_TEXT_BYTES,
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
        maximum_in_flight: 1,
        maximum_input_bytes: SERIAL_MAXIMUM_BYTES,
        maximum_output_bytes: 0,
    }
}

pub(super) fn morse(build_id: &str) -> CapabilityOffer<'_> {
    CapabilityOffer {
        kind: conduit_text::TEXT_MORSE_KIND,
        contract_revision: conduit_text::TEXT_MORSE_CONTRACT_REVISION,
        implementation: TEXT_MORSE_IMPLEMENTATION,
        artifact_build: build_id,
        host_operation: Some("conduit.host/text-to-morse@1"),
        required_base: BaseKind::Memory,
        secondary_base: None,
        input: Some(PortOffer {
            name: "text",
            value_kind: conduit_semantic_catalog::TEXT_PRESENTATION_VALUE_KIND,
            direction: PortDirection::Input,
            closes: true,
        }),
        output: Some(PortOffer {
            name: "pattern",
            value_kind: conduit_text::MORSE_PATTERN_VALUE_KIND,
            direction: PortDirection::Output,
            closes: true,
        }),
        maximum_in_flight: 1,
        maximum_input_bytes: conduit_text::MAXIMUM_MORSE_INPUT_BYTES as u32,
        maximum_output_bytes: conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32,
    }
}

pub(super) fn indicator(build_id: &str) -> CapabilityOffer<'_> {
    CapabilityOffer {
        kind: conduit_semantic_catalog::INDICATOR_PRESENTATION_KIND,
        contract_revision: conduit_semantic_catalog::INDICATOR_PRESENTATION_CONTRACT_REVISION,
        implementation: INDICATOR_PRESENTATION_IMPLEMENTATION,
        artifact_build: build_id,
        host_operation: Some("conduit.host/present-indicator@1"),
        required_base: BaseKind::Serial,
        secondary_base: None,
        input: Some(PortOffer {
            name: "pattern",
            value_kind: conduit_text::MORSE_PATTERN_VALUE_KIND,
            direction: PortDirection::Input,
            closes: true,
        }),
        output: None,
        maximum_in_flight: 1,
        maximum_input_bytes: conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32,
        maximum_output_bytes: 0,
    }
}

pub(super) fn characters(build_id: &str) -> CapabilityOffer<'_> {
    composition_leaf(
        build_id,
        LeafFront {
            kind: conduit_text::TEXT_CHARACTERS_KIND,
            input_name: "in",
            input_kind: conduit_text::TEXT_VALUE_KIND,
            output_name: "out",
            output_kind: conduit_text::MORSE_CHARACTERS_VALUE_KIND,
            maximum_bytes: conduit_text::MAX_TEXT_BYTES,
        },
        TEXT_CHARACTERS_IMPLEMENTATION,
    )
}

pub(super) fn morse_lookup(build_id: &str) -> CapabilityOffer<'_> {
    composition_leaf(
        build_id,
        LeafFront {
            kind: conduit_text::MORSE_LOOKUP_KIND,
            input_name: "in",
            input_kind: conduit_text::MORSE_CHARACTERS_VALUE_KIND,
            output_name: "out",
            output_kind: conduit_text::MORSE_SYMBOL_GROUPS_VALUE_KIND,
            maximum_bytes: conduit_text::MAXIMUM_MORSE_SYMBOL_GROUPS_BYTES as u32,
        },
        MORSE_LOOKUP_IMPLEMENTATION,
    )
}

pub(super) fn morse_intersperse(build_id: &str) -> CapabilityOffer<'_> {
    composition_leaf(
        build_id,
        LeafFront {
            kind: conduit_text::MORSE_INTERSPERSE_KIND,
            input_name: "in",
            input_kind: conduit_text::MORSE_SYMBOL_GROUPS_VALUE_KIND,
            output_name: "out",
            output_kind: conduit_text::MORSE_GAPPED_GROUPS_VALUE_KIND,
            maximum_bytes: conduit_text::MAXIMUM_MORSE_GAPPED_GROUPS_BYTES as u32,
        },
        MORSE_INTERSPERSE_IMPLEMENTATION,
    )
}

pub(super) fn morse_flatten(build_id: &str) -> CapabilityOffer<'_> {
    composition_leaf(
        build_id,
        LeafFront {
            kind: conduit_text::MORSE_FLATTEN_KIND,
            input_name: "in",
            input_kind: conduit_text::MORSE_GAPPED_GROUPS_VALUE_KIND,
            output_name: "out",
            output_kind: conduit_text::MORSE_SYMBOLS_VALUE_KIND,
            maximum_bytes: conduit_text::MAXIMUM_MORSE_SYMBOLS_BYTES as u32,
        },
        MORSE_FLATTEN_IMPLEMENTATION,
    )
}

pub(super) fn morse_symbols_to_pattern(build_id: &str) -> CapabilityOffer<'_> {
    composition_leaf(
        build_id,
        LeafFront {
            kind: conduit_text::MORSE_SYMBOLS_TO_PATTERN_KIND,
            input_name: "in",
            input_kind: conduit_text::MORSE_SYMBOLS_VALUE_KIND,
            output_name: "out",
            output_kind: conduit_text::MORSE_PATTERN_VALUE_KIND,
            maximum_bytes: conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32,
        },
        MORSE_SYMBOLS_TO_PATTERN_IMPLEMENTATION,
    )
}

struct LeafFront {
    kind: &'static str,
    input_name: &'static str,
    input_kind: &'static str,
    output_name: &'static str,
    output_kind: &'static str,
    maximum_bytes: u32,
}

fn composition_leaf<'a>(
    build_id: &'a str,
    front: LeafFront,
    implementation: &'static str,
) -> CapabilityOffer<'a> {
    CapabilityOffer {
        kind: front.kind,
        contract_revision: conduit_text::MORSE_COMPOSITION_CONTRACT_REVISION,
        implementation,
        artifact_build: build_id,
        host_operation: Some(implementation),
        required_base: BaseKind::Memory,
        secondary_base: None,
        input: Some(PortOffer {
            name: front.input_name,
            value_kind: front.input_kind,
            direction: PortDirection::Input,
            closes: true,
        }),
        output: Some(PortOffer {
            name: front.output_name,
            value_kind: front.output_kind,
            direction: PortDirection::Output,
            closes: true,
        }),
        maximum_in_flight: 1,
        maximum_input_bytes: front.maximum_bytes,
        maximum_output_bytes: front.maximum_bytes,
    }
}
