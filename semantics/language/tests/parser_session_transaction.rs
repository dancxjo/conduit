//! Aggregate custody regression gates. Fixture executors establish admission and
//! cancellation contracts; target Plan execution remains a separate proof.
#![cfg(all(feature = "parser-model-selection", target_has_atomic = "ptr"))]
extern crate alloc;
pub use conduit_language::*;
#[path = "parser_session_execution.rs"]
mod execution_fixture;
#[path = "../src/parser_custody_budget.rs"]
mod parser_custody_budget;
#[path = "../src/parser_custody_initialization.rs"]
mod parser_custody_initialization;
#[path = "../src/parser_protected_origin.rs"]
mod parser_protected_origin;
#[path = "../src/parser_retained_commit.rs"]
mod parser_retained_commit;
#[path = "../src/parser_session_transaction.rs"]
mod parser_session_transaction;
#[path = "../src/parser_transaction_ingress.rs"]
mod parser_transaction_ingress;
// The same sealed execution module is used by the port and aggregate.
use execution_fixture::execution as parser_session_execution;
#[path = "common/parser_model_resource.rs"]
mod model_resource;
#[path = "../src/parser_session_canonical_ingress.rs"]
mod parser_session_canonical_ingress;
use parser_custody_budget::{Limits, Usage};
use parser_custody_initialization::PreparedCustodyInitialization;
use parser_model_selection::*;
use parser_session_transaction::{SessionTransactionCustody, TransactionEnvelope};
use parser_transaction_ingress::TransactionIngressRegistry;
use std::sync::Arc;

fn setup() -> (
    Arc<PreparedParserModelSelection>,
    LanguageParserSessionSeedRequest,
    PreparedCustodyInitialization,
) {
    let profile = pinned_v2_lexical_profile().unwrap();
    let model = PreparedParserModelSelection::prepare(
        model_resource::categorical(
            include_bytes!("../training/ewt_joint_v2/ewt_joint.i16").to_vec(),
            pinned_v2_model_signature().unwrap(),
            1,
        ),
        &profile,
    )
    .unwrap();
    let revision = LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            LanguageTextId::new("fixture/atomic".into()).unwrap(),
            profile.language().clone(),
            LanguageTextRevisionId::new("source/1".into()).unwrap(),
            "record".into(),
        )
        .unwrap(),
        None,
        profile.provenance().clone(),
        0,
        None,
    )
    .unwrap();
    let tape = lexical::prepare_lexical_tape(&revision, &profile, None).unwrap();
    let lexical = LanguageParserJointLexical::new(tape.tape().clone(), 1).unwrap();
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("analysis/1".into()).unwrap(),
        revision.material().revision().clone(),
        revision.material().identity().clone(),
    )
    .unwrap();
    let relation = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    let sentinel =
        LinguisticTokenIdentity::new(4, basis.text().clone(), basis.source_revision().clone())
            .unwrap();
    let edge = LanguageParserProtectedEdgeProposal::new(
        basis.clone(),
        4,
        0,
        sentinel.clone(),
        4,
        0,
        sentinel,
        basis.clone(),
        relation.clone(),
    )
    .unwrap();
    let initialization = PreparedCustodyInitialization::prepare(edge).unwrap();
    let request = LanguageParserSessionSeedRequest::new(
        LanguageParserBegin::new(basis, relation, 1).unwrap(),
        17,
        lexical,
    )
    .unwrap();
    (Arc::new(model), request, initialization)
}
fn limits() -> Limits {
    Limits {
        retained: Usage {
            origins: 4,
            facts: 8,
            rebases: 8,
            snapshots: 8,
            commits: 8,
            bytes: 1_000_000_000,
        },
        peak_bytes: 2_000_000_000,
    }
}
fn envelope(old: Usage) -> TransactionEnvelope {
    TransactionEnvelope {
        maximum_next: Usage {
            bytes: old.bytes + 100_000_000,
            snapshots: old.snapshots + 1,
            ..old
        },
        maximum_candidate_bytes: 100_000_000,
    }
}
#[test]
fn consumed_late_refusal_closes_all_registered_ports_and_preserves_publication() {
    let (model, request, initialization) = setup();
    let registry = TransactionIngressRegistry::new(2).unwrap();
    let mut first = registry
        .register_verified(execution_fixture::port(false))
        .unwrap();
    let sibling = registry
        .register_verified(execution_fixture::port(false))
        .unwrap();
    let mut session =
        SessionTransactionCustody::prepare(model, limits(), initialization, registry.clone())
            .unwrap();
    let prior = session.usage();
    {
        let mut stage = session.stage(envelope(prior)).unwrap();
        let execution = first.transact(&mut stage, request.clone()).ok().unwrap();
        stage.seed(execution).unwrap();
        stage.publish().unwrap();
    }
    let published = session.usage();
    assert!(published.bytes > prior.bytes);
    {
        let mut stage = session.stage(envelope(published)).unwrap();
        let execution = first.transact(&mut stage, request.clone()).ok().unwrap();
        // A new independently valid initial graph cannot replace the owned beam.
        assert!(stage.seed(execution).is_err());
        assert!(stage.publish().is_err());
    }
    assert!(registry.closed());
    assert_eq!(session.usage(), published);
    assert!(session.stage(envelope(published)).is_err());
    // Sibling target has not consumed anything, but can no longer be entered.
    assert!(first.is_cancelled());
    assert!(sibling.is_cancelled());
    assert_eq!(sibling.next_ordinal(), 0);
}
#[test]
fn abandonment_after_consumption_closes_every_port_without_publishing_seed() {
    let (model, request, initialization) = setup();
    let registry = TransactionIngressRegistry::new(2).unwrap();
    let mut first = registry
        .register_verified(execution_fixture::port(false))
        .unwrap();
    let sibling = registry
        .register_verified(execution_fixture::port(false))
        .unwrap();
    let mut session =
        SessionTransactionCustody::prepare(model, limits(), initialization, registry.clone())
            .unwrap();
    let prior = session.usage();
    {
        let mut stage = session.stage(envelope(prior)).unwrap();
        let execution = first.transact(&mut stage, request).ok().unwrap();
        stage.seed(execution).unwrap();
    }
    assert!(registry.closed());
    assert_eq!(session.usage(), prior);
    assert!(first.is_cancelled());
    assert!(sibling.is_cancelled());
    assert_eq!(sibling.next_ordinal(), 0);
}

#[test]
fn resource_refusal_before_ingress_preserves_old_custody_and_open_ports() {
    let (model, _, initialization) = setup();
    let registry = TransactionIngressRegistry::new(1).unwrap();
    let first = registry
        .register_verified(execution_fixture::port(false))
        .unwrap();
    let mut session =
        SessionTransactionCustody::prepare(model, limits(), initialization, registry.clone())
            .unwrap();
    let prior = session.usage();
    let refusal = TransactionEnvelope {
        maximum_next: Usage {
            bytes: prior.bytes - 1,
            ..prior
        },
        maximum_candidate_bytes: 0,
    };
    assert!(session.stage(refusal).is_err());
    assert_eq!(session.usage(), prior);
    assert!(!registry.closed());
    assert!(!first.is_cancelled());
    assert_eq!(first.next_ordinal(), 0);
}
#[test]
fn native_valid_first_snapshot_cannot_invent_a_source_seed() {
    let (model, request, initialization) = setup();
    let registry = TransactionIngressRegistry::new(1).unwrap();
    let first = registry
        .register_verified(execution_fixture::port(false))
        .unwrap();
    let mut session =
        SessionTransactionCustody::prepare(model, limits(), initialization, registry.clone())
            .unwrap();
    let b = request.begin();
    let r = b.default_relation();
    let state = LanguageParserState::new(
        b.basis().clone(),
        0,
        1,
        [5, 5, 5, 5, 4],
        r.clone(),
        r.clone(),
        r.clone(),
        r.clone(),
        [4, 4, 4, 4, 4],
        1,
        0,
    )
    .unwrap();
    let active = LanguageParserJointHypothesis::new(
        [0; 4],
        LanguageParserHypothesis::new(true, 17, 0, state.clone()).unwrap(),
    )
    .unwrap();
    let inactive = LanguageParserJointHypothesis::new(
        [0; 4],
        LanguageParserHypothesis::new(false, 0, 0, state.clone()).unwrap(),
    )
    .unwrap();
    let beam = LanguageParserJointBeam::new(
        b.basis().clone(),
        active,
        inactive.clone(),
        inactive.clone(),
        inactive,
        *request.lexical().tape().source().sequence(),
        0,
        request.lexical().clone(),
    )
    .unwrap();
    let available = LanguageParserAvailableState::new(
        LanguageParserAvailableLexical::new(request.lexical().tape().clone(), 1).unwrap(),
        state,
    )
    .unwrap();
    let snapshot = LanguageParserRetainedSnapshotReceipt::new(available, beam).unwrap();
    let prior = session.usage();
    {
        let mut stage = session.stage(envelope(prior)).unwrap();
        assert!(stage.snapshot(snapshot).is_err());
        assert!(stage.publish().is_err());
    }
    assert_eq!(session.usage(), prior);
    assert!(!registry.closed());
    assert!(!first.is_cancelled());
    assert_eq!(first.next_ordinal(), 0);
}

#[test]
fn foreign_native_response_cancels_sibling_even_before_receipt_admission() {
    let (model, request, initialization) = setup();
    let registry = TransactionIngressRegistry::new(2).unwrap();
    let mut first = registry
        .register_verified(execution_fixture::port(true))
        .unwrap();
    let sibling = registry
        .register_verified(execution_fixture::port(false))
        .unwrap();
    let mut session =
        SessionTransactionCustody::prepare(model, limits(), initialization, registry.clone())
            .unwrap();
    let prior = session.usage();
    {
        let mut stage = session.stage(envelope(prior)).unwrap();
        assert!(first.transact(&mut stage, request).is_err());
        assert!(stage.publish().is_err());
    }
    assert_eq!(session.usage(), prior);
    assert!(registry.closed());
    assert!(first.is_cancelled());
    assert!(sibling.is_cancelled());
    assert_eq!(first.next_ordinal(), 1);
    assert_eq!(sibling.next_ordinal(), 0);
}

#[test]
fn target_unwind_closes_every_port_and_preserves_prior_publication() {
    let (model, request, initialization) = setup();
    let registry = TransactionIngressRegistry::new(2).unwrap();
    let mut first = registry
        .register_verified(execution_fixture::panic_port())
        .unwrap();
    let sibling = registry
        .register_verified(execution_fixture::port(false))
        .unwrap();
    let mut session =
        SessionTransactionCustody::prepare(model, limits(), initialization, registry.clone())
            .unwrap();
    let prior = session.usage();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut stage = session.stage(envelope(prior)).unwrap();
        let _ = first.transact(&mut stage, request);
    }));
    assert!(result.is_err());
    assert!(registry.closed());
    assert!(first.is_cancelled());
    assert!(sibling.is_cancelled());
    assert_eq!(sibling.next_ordinal(), 0);
    assert_eq!(session.usage(), prior);
}
