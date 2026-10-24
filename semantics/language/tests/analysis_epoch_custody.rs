//! Actual Source receipts cannot cross private owners with identical values.
#[path = "common/analysis_epoch_initialization_fixture.rs"]
mod fixture;
#[path = "common/analysis_epoch_runtime.rs"]
mod runtime;
use conduit_core::revision::*;
use conduit_language::{
    analysis_epoch_owner::{fallback_round::*, *},
    revision::*,
    *,
};
use conduit_plot::rust_binding::NativeRustBinding;
use fixture::*;
fn context() -> RevisionContext<'static> {
    RevisionContext {
        stream: text("custody"),
        subject: text("text"),
        epoch: text("1"),
        producer: text("Source-fixture"),
        policy: text("fallback@1"),
    }
}
fn limits() -> RevisionLimits {
    RevisionLimits {
        history_events: 16,
        revisable_units: 4,
    }
}
fn snapshot(
    owner: &AnalysisEpochOwner<'_>,
) -> (Vec<u8>, u64, RevisionFrontiers<TokenFrontier>, usize) {
    (
        owner.beam().clone().encode().unwrap(),
        owner.generation(),
        owner.frontiers(),
        owner.history().count(),
    )
}
fn check_round_membership(runtime: &mut runtime::Runtime) {
    let material = lexical().tape().source().material().clone();
    let domain = DependencyRevisions {
        source_text: &material,
        tokens: 1,
    };
    let mut original = initialized(&domain, context(), limits());
    let mut foreign = initialized(&domain, context(), limits());
    let before_original = snapshot(&original);
    let before_foreign = snapshot(&foreign);
    assert_eq!(before_original, before_foreign);
    let programs = PreparedFallbackRoundPrograms::prepare().unwrap();
    let provided = match original.prepare_fallback_round_with(&programs, runtime) {
        Ok(p) => p,
        Err(f) => panic!("actual round {:?}", f.refusal),
    };
    let observed =
        match original.prepare_fallback_round(&programs, provided.observations().to_vec()) {
            Ok(p) => p,
            Err(f) => panic!("checked replay {:?}", f.refusal),
        };
    let refused = match foreign.apply_fallback_round(provided) {
        Err(f) => f,
        Ok(()) => panic!("foreign identical owner accepted provider receipt"),
    };
    assert!(matches!(refused.refusal, FallbackRoundRefusal::Stale));
    assert_eq!(snapshot(&original), before_original);
    assert_eq!(snapshot(&foreign), before_foreign);
    match original.apply_fallback_round(refused.prepared) {
        Ok(()) => (),
        Err(f) => panic!("original receipt owner refused {:?}", f.refusal),
    };
    let refused = match foreign.apply_fallback_round(observed) {
        Err(f) => f,
        Ok(()) => panic!("foreign identical owner accepted replay receipt"),
    };
    assert!(matches!(refused.refusal, FallbackRoundRefusal::Stale));
    assert_eq!(snapshot(&foreign), before_foreign);
    let before = snapshot(&original);
    assert!(matches!(
        original.apply_fallback_round(refused.prepared),
        Err(FallbackRoundApplyFailure {
            refusal: FallbackRoundRefusal::Stale,
            ..
        })
    ));
    assert_eq!(snapshot(&original), before);
    runtime.assert_signs();
}
fn check_frontier_membership(runtime: &mut runtime::Runtime) {
    let tape = lexical();
    let material = tape.tape().source().material().clone();
    let domain = DependencyRevisions {
        source_text: &material,
        tokens: 1,
    };
    let context = context();
    let evidence = [RevisionEvidence {
        source: text("custody-fixture"),
        generation: text("1"),
    }];
    // Supplied semantic proposal is not injected into the owner's Source graph.
    let arc = LanguageDependencyArc::new(
        LanguageAnalysisTokenRef::new(
            LanguageAnalysisRevisionId::new("analysis/1".into()).unwrap(),
            tape.tape().tokens()[0].identity().clone(),
        )
        .unwrap(),
        LanguageDependencyHead::Root,
        LanguageDependencyRelation::new(LanguageUniversalDependencyRelation::Root, None).unwrap(),
    )
    .unwrap();
    let delta = DependencyDelta::Assert(arc);
    let proposed = RevisionEvent::new(
        &domain,
        RevisionReference {
            context,
            sequence: 1,
            event: text("proposal"),
        },
        &evidence,
        RevisionChange::Proposed { delta: &delta },
    )
    .unwrap();
    let stable = RevisionEvent::new(
        &domain,
        RevisionReference {
            context,
            sequence: 2,
            event: text("stable"),
        },
        &evidence,
        RevisionChange::Stable {
            revision: proposed.reference(),
            through: TokenFrontier(0),
        },
    )
    .unwrap();
    let mut original = initialized(&domain, context, limits());
    let mut foreign = initialized(&domain, context, limits());
    original.append_event(&proposed, false, false).unwrap();
    foreign.append_event(&proposed, false, false).unwrap();
    let before_original = snapshot(&original);
    let before_foreign = snapshot(&foreign);
    assert_eq!(before_original, before_foreign);
    let programs = PreparedFallbackRoundPrograms::prepare().unwrap();
    let provided = match original.prepare_fallback_frontier_with(&programs, runtime, false, false) {
        Ok(p) => p,
        Err(f) => panic!("actual frontier {:?}", f.refusal),
    };
    assert_eq!(*provided.frontier().stable(), 0);
    let observed = match original.prepare_fallback_frontier(
        &programs,
        provided.observations().to_vec(),
        false,
        false,
    ) {
        Ok(p) => p,
        Err(f) => panic!("frontier replay {:?}", f.refusal),
    };
    let refused = match foreign.append_fallback_frontier(&stable, provided) {
        Err(f) => f,
        Ok(()) => panic!("foreign identical owner accepted provider frontier"),
    };
    assert!(matches!(refused.refusal, AnalysisEpochRefusal::Replay));
    assert_eq!(snapshot(&original), before_original);
    assert_eq!(snapshot(&foreign), before_foreign);
    match original.append_fallback_frontier(&stable, refused.prepared) {
        Ok(()) => (),
        Err(f) => panic!("original frontier owner refused {:?}", f.refusal),
    };
    let refused = match foreign.append_fallback_frontier(&stable, observed) {
        Err(f) => f,
        Ok(()) => panic!("foreign identical owner accepted replay frontier"),
    };
    assert!(matches!(refused.refusal, AnalysisEpochRefusal::Replay));
    assert_eq!(snapshot(&foreign), before_foreign);
    let before = snapshot(&original);
    assert!(original
        .append_fallback_frontier(&stable, refused.prepared)
        .is_err());
    assert_eq!(snapshot(&original), before);
    runtime.assert_signs();
}

#[test]
fn actual_round_and_frontier_receipts_refuse_foreign_identical_owners_and_stale_reuse() {
    // Both contracts share one immutable Source package and ordinary kernel
    // fixture. Each helper still creates independent exact-value owners.
    let mut runtime = runtime::Runtime::new();
    check_frontier_membership(&mut runtime);
    check_round_membership(&mut runtime);
    assert!(runtime.operations > 0);
    eprintln!(
        "private-owner custody checked actual Source {} through {} operations",
        runtime.source_id.as_str(),
        runtime.operations
    );
    runtime.assert_signs();
}
