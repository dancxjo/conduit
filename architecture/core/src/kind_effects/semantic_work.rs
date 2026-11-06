//! Independent semantic work facts; no realization or execution policy.
use super::*;

/// Independent contract knowledge. Unknown or duplicate laws on one axis do
/// not erase known laws on another. This is not transformation eligibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticWorkKnowledge {
    pub external_effects: Result<ExternalEffectBehavior, PureExpressionRefusal>,
    pub temporal_state: Result<TemporalStateBehavior, PureExpressionRefusal>,
    pub time_dependence: Result<SemanticDependence, PureExpressionRefusal>,
    pub random_dependence: Result<SemanticDependence, PureExpressionRefusal>,
    pub resource_dependence: Result<SemanticDependence, PureExpressionRefusal>,
    pub suspension: Result<SuspensionBehavior, PureExpressionRefusal>,
    pub variability: Result<VariabilityBehavior, PureExpressionRefusal>,
    pub replay: Result<ReplayBehavior, PureExpressionRefusal>,
    first_duplicate: Option<PureExpressionRefusal>,
}
impl SemanticWorkKnowledge {
    /// Existing admission still requires every axis exactly once. Preserve the
    /// original parser's first duplicate refusal before checking missing laws.
    pub fn complete(&self) -> Result<PureExpressionFacts, PureExpressionRefusal> {
        if let Some(refusal) = self.first_duplicate {
            return Err(refusal);
        }
        Ok(PureExpressionFacts {
            external_effects: self.external_effects?,
            temporal_state: self.temporal_state?,
            time_dependence: self.time_dependence?,
            random_dependence: self.random_dependence?,
            resource_dependence: self.resource_dependence?,
            suspension: self.suspension?,
            variability: self.variability?,
            replay: self.replay.clone()?,
        })
    }
}

pub fn semantic_work_knowledge(kind: &Kind) -> SemanticWorkKnowledge {
    knowledge_from_laws(&kind.semantic_laws)
}

/// Missing and duplicate axes refuse; absence never implies purity or safety.
pub fn semantic_work_facts(kind: &Kind) -> Result<PureExpressionFacts, PureExpressionRefusal> {
    semantic_work_knowledge(kind).complete()
}
impl crate::KindSemanticContract {
    pub fn work_knowledge(&self) -> SemanticWorkKnowledge {
        knowledge_from_laws(&self.laws)
    }
    pub fn work_facts(&self) -> Result<PureExpressionFacts, PureExpressionRefusal> {
        self.work_knowledge().complete()
    }
}
fn knowledge_from_laws(laws: &[KindSemanticLaw]) -> SemanticWorkKnowledge {
    let mut result = SemanticWorkKnowledge {
        external_effects: Err(PureExpressionRefusal::MissingFact(
            PureExpressionFact::ExternalEffects,
        )),
        temporal_state: Err(PureExpressionRefusal::MissingFact(
            PureExpressionFact::TemporalState,
        )),
        time_dependence: Err(PureExpressionRefusal::MissingFact(
            PureExpressionFact::TimeDependence,
        )),
        random_dependence: Err(PureExpressionRefusal::MissingFact(
            PureExpressionFact::RandomDependence,
        )),
        resource_dependence: Err(PureExpressionRefusal::MissingFact(
            PureExpressionFact::ResourceDependence,
        )),
        suspension: Err(PureExpressionRefusal::MissingFact(
            PureExpressionFact::Suspension,
        )),
        variability: Err(PureExpressionRefusal::MissingFact(
            PureExpressionFact::Variability,
        )),
        replay: Err(PureExpressionRefusal::MissingFact(
            PureExpressionFact::Replay,
        )),
        first_duplicate: None,
    };
    for law in laws {
        match law {
            KindSemanticLaw::ExternalEffects(value) => record(
                &mut result.external_effects,
                *value,
                PureExpressionFact::ExternalEffects,
                &mut result.first_duplicate,
            ),
            KindSemanticLaw::TemporalState(value) => record(
                &mut result.temporal_state,
                *value,
                PureExpressionFact::TemporalState,
                &mut result.first_duplicate,
            ),
            KindSemanticLaw::TimeDependence(value) => record(
                &mut result.time_dependence,
                *value,
                PureExpressionFact::TimeDependence,
                &mut result.first_duplicate,
            ),
            KindSemanticLaw::RandomDependence(value) => record(
                &mut result.random_dependence,
                *value,
                PureExpressionFact::RandomDependence,
                &mut result.first_duplicate,
            ),
            KindSemanticLaw::ResourceDependence(value) => record(
                &mut result.resource_dependence,
                *value,
                PureExpressionFact::ResourceDependence,
                &mut result.first_duplicate,
            ),
            KindSemanticLaw::Suspension(value) => record(
                &mut result.suspension,
                *value,
                PureExpressionFact::Suspension,
                &mut result.first_duplicate,
            ),
            KindSemanticLaw::Variability(value) => record(
                &mut result.variability,
                *value,
                PureExpressionFact::Variability,
                &mut result.first_duplicate,
            ),
            KindSemanticLaw::Replay(value) => record(
                &mut result.replay,
                value.clone(),
                PureExpressionFact::Replay,
                &mut result.first_duplicate,
            ),
            KindSemanticLaw::RealizationRequirement { .. } => {}
            KindSemanticLaw::ExactRealizationProperty(_) => {}
            KindSemanticLaw::Terminal(_) => {}
            KindSemanticLaw::TerminalTransduction(_) => {}
            KindSemanticLaw::ResourcePorts(_) => {}
            KindSemanticLaw::ValueContracts(_) => {}
            KindSemanticLaw::KeyedJoin(_) => {}
            KindSemanticLaw::BoundedCollect(_) => {}
            KindSemanticLaw::FlowSelect(_) | KindSemanticLaw::InitializedState(_) => {}
            KindSemanticLaw::FlowFold(_)
            | KindSemanticLaw::FlowEach(_)
            | KindSemanticLaw::FlowScan(_) => {}
        }
    }
    result
}
fn record<T>(
    slot: &mut Result<T, PureExpressionRefusal>,
    value: T,
    axis: PureExpressionFact,
    first_duplicate: &mut Option<PureExpressionRefusal>,
) {
    if matches!(slot, Err(PureExpressionRefusal::MissingFact(_))) {
        *slot = Ok(value);
    } else {
        let refusal = PureExpressionRefusal::DuplicateFact(axis);
        *slot = Err(refusal);
        first_duplicate.get_or_insert(refusal);
    }
}
