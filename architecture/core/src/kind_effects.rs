//! Fail-closed derivation for semantic Kind use inside pure expressions.

use crate::{
    Back, ExternalEffectBehavior, Kind, KindSemanticLaw, ReplayBehavior, SemanticDependence,
    SuspensionBehavior, TemporalStateBehavior, VariabilityBehavior,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PureExpressionFacts {
    pub external_effects: ExternalEffectBehavior,
    pub temporal_state: TemporalStateBehavior,
    pub time_dependence: SemanticDependence,
    pub random_dependence: SemanticDependence,
    pub resource_dependence: SemanticDependence,
    pub suspension: SuspensionBehavior,
    pub variability: VariabilityBehavior,
    pub replay: ReplayBehavior,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PureExpressionRefusal {
    MissingFact(PureExpressionFact),
    DuplicateFact(PureExpressionFact),
    ExternalEffect,
    TemporalState,
    TimeDependence,
    RandomDependence,
    ResourceDependence,
    Suspension,
    Variability,
    ReplayIneligible,
    BackHostCall,
    BackResource,
    BackAuthority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PureExpressionFact {
    ExternalEffects,
    TemporalState,
    TimeDependence,
    RandomDependence,
    ResourceDependence,
    Suspension,
    Variability,
    Replay,
}

/// Derives expression eligibility from the reviewed semantic contract.
///
/// Absence is not purity. Each independent fact must occur exactly once and
/// carry the pure law; authored Form source cannot supply or strengthen these
/// facts.
pub fn pure_expression_facts(kind: &Kind) -> Result<PureExpressionFacts, PureExpressionRefusal> {
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
            KindSemanticLaw::Terminal(_) => {}
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
                set_once(&mut replay, *value, PureExpressionFact::Replay)?
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
    if facts.external_effects != ExternalEffectBehavior::None {
        return Err(PureExpressionRefusal::ExternalEffect);
    }
    if facts.temporal_state != TemporalStateBehavior::None {
        return Err(PureExpressionRefusal::TemporalState);
    }
    if facts.time_dependence == SemanticDependence::Ambient {
        return Err(PureExpressionRefusal::TimeDependence);
    }
    if facts.random_dependence == SemanticDependence::Ambient {
        return Err(PureExpressionRefusal::RandomDependence);
    }
    if facts.resource_dependence == SemanticDependence::Ambient {
        return Err(PureExpressionRefusal::ResourceDependence);
    }
    if facts.suspension != SuspensionBehavior::Never {
        return Err(PureExpressionRefusal::Suspension);
    }
    if facts.variability != VariabilityBehavior::DeterministicFromInputs {
        return Err(PureExpressionRefusal::Variability);
    }
    if facts.replay != ReplayBehavior::Exact {
        return Err(PureExpressionRefusal::ReplayIneligible);
    }
    Ok(facts)
}

/// Checks the independently selected realization envelope.
///
/// A pure semantic contract cannot make a Back pure when that Back requests a
/// Host Call, resource, or authority. Planning must enforce both boundaries.
pub fn check_pure_expression_back(back: &Back) -> Result<(), PureExpressionRefusal> {
    if !back.host_calls.is_empty() {
        return Err(PureExpressionRefusal::BackHostCall);
    }
    if !back.resource_requirements.is_empty() {
        return Err(PureExpressionRefusal::BackResource);
    }
    if !back.authority_requirements.is_empty() {
        return Err(PureExpressionRefusal::BackAuthority);
    }
    Ok(())
}

fn set_once<T: Copy>(
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

fn required<T: Copy>(
    value: Option<T>,
    fact: PureExpressionFact,
) -> Result<T, PureExpressionRefusal> {
    value.ok_or(PureExpressionRefusal::MissingFact(fact))
}
