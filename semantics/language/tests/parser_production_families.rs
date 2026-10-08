//! Exact fixed production family readiness and aggregate pre-consumption pressure.
extern crate alloc;
pub use conduit_language::parser_session_runtime;
use generated::*;
#[allow(dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));
}
#[path = "../src/parser_production_families.rs"]
mod parser_production_families;
#[path = "../src/parser_session_execution.rs"]
mod parser_session_execution;
#[test]
fn actual_complete_fixed_groups_and_all_named_ports_prepare() {
    use conduit_plot::rust_binding::PreparedNativeFamilyLimits;
    use parser_production_families::*;
    let owner = PreparedProductionParserFamilies::prepare(ProductionFamilyLimits {
        family: PreparedNativeFamilyLimits {
            maximum_types: 64,
            maximum_laws_per_type: 128,
            maximum_input_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
            maximum_retained_bytes: 1024 * 1024 * 1024,
            maximum_preparation_peak_bytes: 1024 * 1024 * 1024,
            maximum_conversion_requested_bytes: 2 * 1024 * 1024 * 1024,
        },
        maximum_retained_bytes: 7 * 1024 * 1024 * 1024,
        maximum_preparation_peak_bytes: 7 * 1024 * 1024 * 1024,
        maximum_static_artifact_bytes: 1024 * 1024 * 1024,
        maximum_combined_bytes: 12 * 1024 * 1024 * 1024,
        other_reserved_bytes: 0,
        concurrent_native_values: 2,
    })
    .unwrap();
    eprintln!("receipt {:?}", owner.receipt());
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::Availability)
            .is_ok(),
        "Availability"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::Completion)
            .is_ok(),
        "Completion"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::IndependentBranch)
            .is_ok(),
        "IndependentBranch"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::IndependentCommit)
            .is_ok(),
        "IndependentCommit"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::IndependentCommitRebase)
            .is_ok(),
        "IndependentCommitRebase"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::IndependentCommitRebaseSets)
            .is_ok(),
        "IndependentCommitRebaseSets"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::IndependentMask)
            .is_ok(),
        "IndependentMask"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::JointBranch)
            .is_ok(),
        "JointBranch"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::Commit)
            .is_ok(),
        "Commit"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::JointConsensus)
            .is_ok(),
        "JointConsensus"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::Expansion)
            .is_ok(),
        "Expansion"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::Rebase)
            .is_ok(),
        "Rebase"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::Merge)
            .is_ok(),
        "Merge"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::JointScoreBand1000)
            .is_ok(),
        "JointScoreBand1000"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::StableFact)
            .is_ok(),
        "StableFact"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::LegalMask)
            .is_ok(),
        "LegalMask"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::ProtectedOriginEdge)
            .is_ok(),
        "ProtectedOriginEdge"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::Initialize)
            .is_ok(),
        "Initialize"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::ProtectedInsert)
            .is_ok(),
        "ProtectedInsert"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::ProtectedRebase)
            .is_ok(),
        "ProtectedRebase"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::ProtectionForestProjection)
            .is_ok(),
        "ProtectionForestProjection"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::RetainedCommitAnchor)
            .is_ok(),
        "RetainedCommitAnchor"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::RevisionReset)
            .is_ok(),
        "RevisionReset"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::ScoreProposal)
            .is_ok(),
        "ScoreProposal"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::Seed)
            .is_ok(),
        "Seed"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::Transition)
            .is_ok(),
        "Transition"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::V2ModelFeatures)
            .is_ok(),
        "V2ModelFeatures"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::V2Pos)
            .is_ok(),
        "V2Pos"
    );
    assert!(
        owner
            .for_entry(parser_session_execution::ParserSessionEntry::WaitState)
            .is_ok(),
        "WaitState"
    );
    assert!(owner
        .for_entry(parser_session_execution::ParserSessionEntry::DecodeComplete)
        .is_err());
}

#[test]
fn aggregate_pressure_refuses_before_native_preparation() {
    use conduit_plot::rust_binding::PreparedNativeFamilyLimits;
    use parser_production_families::*;
    let mut limits = ProductionFamilyLimits {
        family: PreparedNativeFamilyLimits {
            maximum_types: 64,
            maximum_laws_per_type: 128,
            maximum_input_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
            maximum_retained_bytes: 1024 * 1024 * 1024,
            maximum_preparation_peak_bytes: 1024 * 1024 * 1024,
            maximum_conversion_requested_bytes: 2 * 1024 * 1024 * 1024,
        },
        maximum_retained_bytes: 7 * 1024 * 1024 * 1024,
        maximum_preparation_peak_bytes: 7 * 1024 * 1024 * 1024,
        maximum_static_artifact_bytes: 1024 * 1024 * 1024,
        maximum_combined_bytes: 12 * 1024 * 1024 * 1024,
        other_reserved_bytes: 0,
        concurrent_native_values: 2,
    };
    // Deliberately invalid Native count: combined pressure must win before any
    // Native preparation is attempted, rather than returning Native(Capacity).
    limits.family.maximum_types = 0;
    limits.maximum_combined_bytes = 0;
    assert!(matches!(
        PreparedProductionParserFamilies::prepare(limits),
        Err(ProductionFamilyRefusal::Pressure)
    ));
    limits.maximum_combined_bytes = usize::MAX;
    limits.other_reserved_bytes = usize::MAX;
    assert!(matches!(
        PreparedProductionParserFamilies::prepare(limits),
        Err(ProductionFamilyRefusal::Pressure)
    ));
}
