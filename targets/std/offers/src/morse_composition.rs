//! Exact hosted implementations for the finite leaf Gears in reviewed Morse Backs.

use conduit_core::{
    kind_id, ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId,
};

pub const ARTIFACT: &str = "conduit-std-host/morse-composition@1";
pub const TEXT_CHARACTERS_IMPLEMENTATION: &str = "std/kernel-text-characters@1";
pub const MORSE_LOOKUP_IMPLEMENTATION: &str = "std/kernel-morse-lookup@1";
pub const MORSE_INTERSPERSE_IMPLEMENTATION: &str = "std/kernel-morse-intersperse@1";
pub const MORSE_FLATTEN_IMPLEMENTATION: &str = "std/kernel-morse-flatten@1";
pub const MORSE_SYMBOLS_TO_PATTERN_IMPLEMENTATION: &str = "std/kernel-morse-symbols-to-pattern@1";

pub fn morse_composition_offers() -> Vec<CapabilityOffer> {
    vec![
        offer(
            conduit_text::text_characters_semantics(),
            TEXT_CHARACTERS_IMPLEMENTATION,
        ),
        offer(
            conduit_text::morse_lookup_semantics(),
            MORSE_LOOKUP_IMPLEMENTATION,
        ),
        offer(
            conduit_text::morse_intersperse_semantics(),
            MORSE_INTERSPERSE_IMPLEMENTATION,
        ),
        offer(
            conduit_text::morse_flatten_semantics(),
            MORSE_FLATTEN_IMPLEMENTATION,
        ),
        offer(
            conduit_text::morse_symbols_to_pattern_semantics(),
            MORSE_SYMBOLS_TO_PATTERN_IMPLEMENTATION,
        ),
    ]
}

fn offer(contract: conduit_text::MorseKindContract, implementation: &str) -> CapabilityOffer {
    let maximum_input_bytes = value_bound(contract.inputs[0].value_kind.as_str());
    let maximum_output_bytes = value_bound(contract.outputs[0].value_kind.as_str());
    BackOfferBuilder::new(
        contract.into_semantic_contract(),
        Back {
            capability_id: CapabilityId::from(implementation),
            execution_profile_id: ExecutionProfileId::from(implementation),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(ARTIFACT),
            host_operations: vec![HostOperationRequirement {
                contract_id: HostOperationContractId::from(implementation),
                target_kind: Some(kind_id(implementation)),
                maximum_in_flight: 1,
                maximum_input_bytes,
                maximum_output_bytes,
            }],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

fn value_bound(kind: &str) -> u32 {
    match kind {
        conduit_text::TEXT_VALUE_KIND => conduit_text::MAX_TEXT_BYTES,
        conduit_text::MORSE_CHARACTERS_VALUE_KIND => {
            conduit_text::MAXIMUM_MORSE_CHARACTERS_BYTES as u32
        }
        conduit_text::MORSE_SYMBOL_GROUPS_VALUE_KIND => {
            conduit_text::MAXIMUM_MORSE_SYMBOL_GROUPS_BYTES as u32
        }
        conduit_text::MORSE_GAPPED_GROUPS_VALUE_KIND => {
            conduit_text::MAXIMUM_MORSE_GAPPED_GROUPS_BYTES as u32
        }
        conduit_text::MORSE_SYMBOLS_VALUE_KIND => conduit_text::MAXIMUM_MORSE_SYMBOLS_BYTES as u32,
        conduit_text::MORSE_PATTERN_VALUE_KIND => conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32,
        _ => 0,
    }
}
