//! Finite std-host offers for portable linguistic contracts.

use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostCallContractId, HostCallRequirement, ImplementationId, Kind,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub const LINGUISTICS_PROFILE: &str = "std/linguistics-kernel-hosted@1";
pub const LINGUISTICS_ARTIFACT: &str = "conduit-std-host/linguistics@2";
pub const LINGUISTICS_HOST_CALL: &str = "conduit.host/linguistics@1";

pub fn linguistics_std_offers() -> Vec<CapabilityOffer> {
    vec![
        offer(conduit_language::tokenize_four_semantic_contract()),
        offer(conduit_language::annotate_four_semantic_contract()),
    ]
}

fn offer(contract: Kind) -> CapabilityOffer {
    let kind = contract.kind_id.as_str().to_owned();
    let mut offered = BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(format!("std/{kind}@2")),
            execution_profile_id: ExecutionProfileId::from(LINGUISTICS_PROFILE),
            implementation_id: ImplementationId::from(format!("std/{kind}@2")),
            artifact_id: ArtifactId::from(LINGUISTICS_ARTIFACT),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(LINGUISTICS_HOST_CALL),
                target_kind: Some(conduit_core::kind_id(&kind)),
                maximum_in_flight: 1,
                maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            }],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build();
    offered.realization_properties =
        vec![
            conduit_language::language_coverage_property(english_coverage())
                .expect("bounded fixture coverage"),
        ];
    offered
}

fn english_coverage() -> conduit_language::LanguageCoverage {
    use conduit_plot::rust_binding::BoundedSequence;
    conduit_language::LanguageCoverage::new(
        "repository/four-token-English-rule-fixture".into(),
        BoundedSequence::try_from_iter([conduit_language::LanguageId::new(
            "language/english".into(),
        )
        .expect("finite identity")])
        .expect("one language"),
        BoundedSequence::try_from_iter([]).expect("no mappings"),
        "four-token-rules@1".into(),
        BoundedSequence::try_from_iter([]).expect("undeclared varieties"),
        false,
    )
    .expect("bounded coverage")
}
