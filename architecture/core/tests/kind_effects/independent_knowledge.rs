use super::*;
use conduit_core::{semantic_work_facts, semantic_work_knowledge};

#[test]
fn partial_contract_retains_each_known_axis_without_admitting_purity() {
    let axes = [
        PureExpressionFact::ExternalEffects,
        PureExpressionFact::TemporalState,
        PureExpressionFact::TimeDependence,
        PureExpressionFact::RandomDependence,
        PureExpressionFact::ResourceDependence,
        PureExpressionFact::Suspension,
        PureExpressionFact::Variability,
        PureExpressionFact::Replay,
    ];
    for (index, axis) in axes.into_iter().enumerate() {
        let mut laws = pure_laws();
        laws.remove(index);
        let definition = kind(laws);
        let facts = semantic_work_knowledge(&definition);
        let statuses = [
            facts.external_effects.map(|_| ()),
            facts.temporal_state.map(|_| ()),
            facts.time_dependence.map(|_| ()),
            facts.random_dependence.map(|_| ()),
            facts.resource_dependence.map(|_| ()),
            facts.suspension.map(|_| ()),
            facts.variability.map(|_| ()),
            facts.replay.clone().map(|_| ()),
        ];
        for (other, status) in statuses.into_iter().enumerate() {
            assert_eq!(
                status,
                if other == index {
                    Err(PureExpressionRefusal::MissingFact(axis))
                } else {
                    Ok(())
                }
            );
        }
        assert_eq!(
            facts.complete(),
            Err(PureExpressionRefusal::MissingFact(axis))
        );
        assert_eq!(semantic_work_facts(&definition), facts.complete());
        assert!(conduit_core::pure_expression_facts(&definition).is_err());
    }
}

#[test]
fn duplicate_axis_stays_unknown_after_third_law_and_preserves_first_refusal() {
    let mut laws = pure_laws();
    laws.remove(0); // Missing external effects must not mask the first duplicate.
    laws.push(KindSemanticLaw::Replay(ReplayBehavior::Ineligible));
    laws.push(KindSemanticLaw::TemporalState(
        TemporalStateBehavior::Retained,
    ));
    laws.push(KindSemanticLaw::Replay(ReplayBehavior::Exact));
    let facts = semantic_work_knowledge(&kind(laws));
    assert_eq!(
        facts.replay,
        Err(PureExpressionRefusal::DuplicateFact(
            PureExpressionFact::Replay
        ))
    );
    assert_eq!(
        facts.temporal_state,
        Err(PureExpressionRefusal::DuplicateFact(
            PureExpressionFact::TemporalState
        ))
    );
    assert_eq!(facts.random_dependence, Ok(SemanticDependence::None));
    assert_eq!(
        facts.complete(),
        Err(PureExpressionRefusal::DuplicateFact(
            PureExpressionFact::Replay
        ))
    );
}

#[test]
fn duplicate_on_any_axis_preserves_the_other_seven_facts() {
    let axes = [
        PureExpressionFact::ExternalEffects,
        PureExpressionFact::TemporalState,
        PureExpressionFact::TimeDependence,
        PureExpressionFact::RandomDependence,
        PureExpressionFact::ResourceDependence,
        PureExpressionFact::Suspension,
        PureExpressionFact::Variability,
        PureExpressionFact::Replay,
    ];
    for (index, axis) in axes.into_iter().enumerate() {
        let mut laws = pure_laws();
        laws.push(laws[index].clone());
        let definition = kind(laws);
        let facts = semantic_work_knowledge(&definition);
        let statuses = [
            facts.external_effects.map(|_| ()),
            facts.temporal_state.map(|_| ()),
            facts.time_dependence.map(|_| ()),
            facts.random_dependence.map(|_| ()),
            facts.resource_dependence.map(|_| ()),
            facts.suspension.map(|_| ()),
            facts.variability.map(|_| ()),
            facts.replay.clone().map(|_| ()),
        ];
        for (other, status) in statuses.into_iter().enumerate() {
            assert_eq!(
                status,
                if other == index {
                    Err(PureExpressionRefusal::DuplicateFact(axis))
                } else {
                    Ok(())
                }
            );
        }
        assert_eq!(
            facts.complete(),
            Err(PureExpressionRefusal::DuplicateFact(axis))
        );
        assert_eq!(semantic_work_facts(&definition), facts.complete());
        assert!(conduit_core::pure_expression_facts(&definition).is_err());
    }
}
