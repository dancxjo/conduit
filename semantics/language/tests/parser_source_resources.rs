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
fn all_fixed_source_verification_resources() {
    use parser_session_execution::verification::PreparedSourceVerification;
    use parser_session_execution::{ParserSessionEntry as E, ParserSessionVerificationLimits};
    let mut retained = 0usize;
    let mut peak = 0usize;
    let mut decoded = 0usize;
    for entry in [
        E::Availability,
        E::DecodeComplete,
        E::Completion,
        E::IndependentBranch,
        E::IndependentCommit,
        E::IndependentCommitRebase,
        E::IndependentCommitRebaseSets,
        E::IndependentMask,
        E::JointBranch,
        E::Commit,
        E::JointConsensus,
        E::Expansion,
        E::Rebase,
        E::Merge,
        E::JointScoreBand1000,
        E::StableFact,
        E::LegalMask,
        E::ProtectedOriginEdge,
        E::Initialize,
        E::ProtectedInsert,
        E::ProtectedRebase,
        E::ProtectionForestProjection,
        E::RetainedCommitAnchor,
        E::RevisionReset,
        E::ScoreProposal,
        E::Seed,
        E::Transition,
        E::V2ModelFeatures,
        E::V2Pos,
        E::WaitState,
    ] {
        let (_, _, _, r) = PreparedSourceVerification::prepare(
            entry,
            ParserSessionVerificationLimits {
                decoded_program_bytes: 2usize << 30,
                preparation_peak_bytes: 4usize << 30,
                retained_bytes: 2usize << 30,
            },
        )
        .unwrap();
        eprintln!("{} {:?}", entry.name(), r);
        peak = peak.max(
            retained
                .checked_add(r.preparation_peak_heap_bytes_bound)
                .unwrap(),
        );
        retained = retained.checked_add(r.retained_heap_bytes_bound).unwrap();
        decoded = decoded
            .checked_add(r.decoded_program_heap_bytes_bound)
            .unwrap();
    }
    eprintln!("aggregate declared simultaneous Source owners retained={retained} preparation_peak={peak} decoded={decoded}");
}
