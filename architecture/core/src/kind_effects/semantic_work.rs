//! Independent semantic work facts; no realization or execution policy.
use super::*;

/// Derives every independent transformation fact without deciding a policy.
/// Missing and duplicate axes refuse; absence never implies purity or safety.
pub fn semantic_work_facts(kind: &Kind) -> Result<PureExpressionFacts, PureExpressionRefusal> {
    let mut external_effects = None;
    let mut temporal_state = None;
    let mut time_dependence = None;
    let mut random_dependence = None;
    let mut resource_dependence = None;
    let mut suspension = None;
    let mut variability = None;
    let mut replay = None;

    for law in &kind.semantic_laws {
        match law {
            KindSemanticLaw::RealizationRequirement { .. } => {}
            KindSemanticLaw::Terminal(_) => {}
            KindSemanticLaw::TerminalTransduction(_) => {}
            KindSemanticLaw::ResourcePorts(_) => {}
            KindSemanticLaw::ValueContracts(_) => {}
            KindSemanticLaw::KeyedJoin(_) => {}
            KindSemanticLaw::BoundedCollect(_) => {}
            KindSemanticLaw::FlowSelect(_) => {}
            KindSemanticLaw::FlowFold(_)
            | KindSemanticLaw::FlowEach(_)
            | KindSemanticLaw::FlowScan(_) => {}
            KindSemanticLaw::ExternalEffects(value) => set_once(
                &mut external_effects,
                *value,
                PureExpressionFact::ExternalEffects,
            )?,
            KindSemanticLaw::TemporalState(value) => set_once(
                &mut temporal_state,
                *value,
                PureExpressionFact::TemporalState,
            )?,
            KindSemanticLaw::TimeDependence(value) => set_once(
                &mut time_dependence,
                *value,
                PureExpressionFact::TimeDependence,
            )?,
            KindSemanticLaw::RandomDependence(value) => set_once(
                &mut random_dependence,
                *value,
                PureExpressionFact::RandomDependence,
            )?,
            KindSemanticLaw::ResourceDependence(value) => set_once(
                &mut resource_dependence,
                *value,
                PureExpressionFact::ResourceDependence,
            )?,
            KindSemanticLaw::Suspension(value) => {
                set_once(&mut suspension, *value, PureExpressionFact::Suspension)?
            }
            KindSemanticLaw::Variability(value) => {
                set_once(&mut variability, *value, PureExpressionFact::Variability)?
            }
            KindSemanticLaw::Replay(value) => {
                set_once(&mut replay, value.clone(), PureExpressionFact::Replay)?
            }
        }
    }

    let facts = PureExpressionFacts {
        external_effects: required(external_effects, PureExpressionFact::ExternalEffects)?,
        temporal_state: required(temporal_state, PureExpressionFact::TemporalState)?,
        time_dependence: required(time_dependence, PureExpressionFact::TimeDependence)?,
        random_dependence: required(random_dependence, PureExpressionFact::RandomDependence)?,
        resource_dependence: required(resource_dependence, PureExpressionFact::ResourceDependence)?,
        suspension: required(suspension, PureExpressionFact::Suspension)?,
        variability: required(variability, PureExpressionFact::Variability)?,
        replay: required(replay, PureExpressionFact::Replay)?,
    };
    Ok(facts)
}

fn set_once<T>(
    slot: &mut Option<T>,
    value: T,
    fact: PureExpressionFact,
) -> Result<(), PureExpressionRefusal> {
    if slot.replace(value).is_some() {
        Err(PureExpressionRefusal::DuplicateFact(fact))
    } else {
        Ok(())
    }
}

fn required<T>(value: Option<T>, fact: PureExpressionFact) -> Result<T, PureExpressionRefusal> {
    value.ok_or(PureExpressionRefusal::MissingFact(fact))
}
