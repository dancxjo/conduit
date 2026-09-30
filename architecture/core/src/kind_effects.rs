//! Fail-closed derivation for semantic Kind use inside pure expressions.

use crate::{
    Back, ExternalEffectBehavior, Kind, KindId, KindIdentity, KindSemanticLaw, PlannedGear,
    ReplayBehavior, RetainedRetryEvidence, RetainedRetryProof, RetryEvidenceRefusal,
    RetryOperationIdentity, SemanticDependence, SuspensionBehavior, TemporalStateBehavior,
    VariabilityBehavior,
};
use alloc::vec::Vec;

#[derive(Debug, Clone, PartialEq, Eq)]
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
pub enum WorkTransformation {
    Recompute,
    Fusion,
    Replay,
    Retry,
    Memoize,
    Move,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransformationEligibility {
    kind_id: KindId,
    kind_contract_revision: KindIdentity,
    implementation_id: crate::ImplementationId,
    facts: PureExpressionFacts,
    back_has_host_calls: bool,
    back_has_resources: bool,
    back_has_authority: bool,
}

/// A checked authorization for one explicitly requested retry. Consumers must
/// retain this value until dispatch so the exact evidence cannot be silently
/// discarded after a Boolean policy check.
#[derive(Debug, PartialEq, Eq)]
pub enum RetryAuthorization<'a> {
    EffectFree {
        eligibility: &'a TransformationEligibility,
    },
    RetainedEffect {
        eligibility: &'a TransformationEligibility,
        evidence: &'a RetainedRetryEvidence,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransformationRefusal {
    SemanticFacts(PureExpressionRefusal),
    ExternalEffect,
    TemporalState,
    AmbientTime,
    AmbientRandom,
    AmbientResource,
    Suspension,
    Variability,
    ReplayIneligible,
    MissingEffectRetryLaw,
    RetainedRetryEvidenceRequired(ReplayBehavior),
    RetryEvidence(RetryEvidenceRefusal),
    BackHostCall,
    BackResource,
    BackAuthority,
    InvalidFiniteEnvelope,
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
    let facts = semantic_work_facts(kind)?;
    require_recomputable_semantics(&facts)?;
    Ok(facts)
}

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
            KindSemanticLaw::Terminal(_) => {}
            KindSemanticLaw::TerminalTransduction(_) => {}
            KindSemanticLaw::ResourcePorts(_) => {}
            KindSemanticLaw::ValueContracts(_) => {}
            KindSemanticLaw::KeyedJoin(_) => {}
            KindSemanticLaw::FlowSelect(_) => {}
            KindSemanticLaw::FlowFold(_) => {}
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

fn require_recomputable_semantics(
    facts: &PureExpressionFacts,
) -> Result<(), PureExpressionRefusal> {
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
    Ok(())
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

/// Seals transformation decisions to one exact semantic contract and Back.
/// Consumers can inspect identity and ask exact questions, but cannot mint a
/// stronger eligibility value from implementation names or annotations.
pub fn derive_transformation_eligibility(
    kind: &Kind,
    back: &Back,
) -> Result<TransformationEligibility, TransformationRefusal> {
    let facts = semantic_work_facts(kind).map_err(TransformationRefusal::SemanticFacts)?;
    if kind.limits.max_active_instances == 0
        || kind.limits.max_queue_items == 0
        || kind.limits.max_queue_bytes == 0
    {
        return Err(TransformationRefusal::InvalidFiniteEnvelope);
    }
    Ok(TransformationEligibility {
        kind_id: kind.kind_id.clone(),
        kind_contract_revision: kind.kind_contract_revision.clone(),
        implementation_id: back.implementation_id.clone(),
        facts,
        back_has_host_calls: !back.host_calls.is_empty(),
        back_has_resources: !back.resource_requirements.is_empty(),
        back_has_authority: !back.authority_requirements.is_empty(),
    })
}

/// Derive transformation eligibility from the exact realization already
/// sealed into a Plan. This prevents recovery code from describing a
/// same-named Back with weaker Host Call, resource, or authority requirements.
pub fn derive_planned_transformation_eligibility(
    placement: &PlannedGear,
) -> Result<TransformationEligibility, TransformationRefusal> {
    let kind = Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: placement.kind_id.clone(),
        kind_contract_revision: placement.kind_contract_revision.clone(),
        inputs: placement.inputs.clone(),
        outputs: placement.outputs.clone(),
        configuration: placement.semantic_contract.configuration.clone(),
        semantic_laws: placement.semantic_contract.laws.clone(),
        limits: placement.limits.clone(),
    };
    let facts = semantic_work_facts(&kind).map_err(TransformationRefusal::SemanticFacts)?;
    if placement.limits.max_active_instances == 0
        || placement.limits.max_queue_items == 0
        || placement.limits.max_queue_bytes == 0
    {
        return Err(TransformationRefusal::InvalidFiniteEnvelope);
    }
    Ok(TransformationEligibility {
        kind_id: placement.kind_id.clone(),
        kind_contract_revision: placement.kind_contract_revision.clone(),
        implementation_id: placement.implementation_id.clone(),
        facts,
        back_has_host_calls: !placement.host_calls.is_empty(),
        back_has_resources: !placement.resources.is_empty(),
        back_has_authority: !placement.authority.is_empty(),
    })
}

impl TransformationEligibility {
    pub fn kind_id(&self) -> &KindId {
        &self.kind_id
    }

    pub fn kind_contract_revision(&self) -> &KindIdentity {
        &self.kind_contract_revision
    }

    pub fn implementation_id(&self) -> &crate::ImplementationId {
        &self.implementation_id
    }

    pub fn facts(&self) -> &PureExpressionFacts {
        &self.facts
    }

    pub fn require(&self, transformation: WorkTransformation) -> Result<(), TransformationRefusal> {
        match transformation {
            WorkTransformation::Recompute
            | WorkTransformation::Fusion
            | WorkTransformation::Memoize
            | WorkTransformation::Move => self.require_recomputable(),
            WorkTransformation::Replay => self.require_recomputable(),
            WorkTransformation::Retry => self.require_retry(),
        }
    }

    /// Admits an effectful retry only from exact provider-owned evidence
    /// retained beyond the prior Play. This does not schedule or perform a
    /// retry; it only seals the checked permission for an explicit consumer.
    pub fn require_retry_with_evidence<'a>(
        &'a self,
        operation: &RetryOperationIdentity,
        evidence: &'a RetainedRetryEvidence,
    ) -> Result<RetryAuthorization<'a>, TransformationRefusal> {
        if self.facts.external_effects == ExternalEffectBehavior::None {
            self.require_recomputable()?;
            return Ok(RetryAuthorization::EffectFree { eligibility: self });
        }
        let law = self.require_effect_retry_semantics()?;
        crate::retry_evidence::validate_retained_retry_evidence(evidence)
            .map_err(TransformationRefusal::RetryEvidence)?;
        if operation.kind_id != self.kind_id {
            return Err(TransformationRefusal::RetryEvidence(
                RetryEvidenceRefusal::KindMismatch,
            ));
        }
        if operation.kind_contract_revision != self.kind_contract_revision {
            return Err(TransformationRefusal::RetryEvidence(
                RetryEvidenceRefusal::KindRevisionMismatch,
            ));
        }
        if operation.implementation_id != self.implementation_id {
            return Err(TransformationRefusal::RetryEvidence(
                RetryEvidenceRefusal::ImplementationMismatch,
            ));
        }
        if evidence.operation != *operation {
            return Err(TransformationRefusal::RetryEvidence(
                RetryEvidenceRefusal::OperationMismatch,
            ));
        }
        if evidence.replay_law != *law {
            return Err(TransformationRefusal::RetryEvidence(
                RetryEvidenceRefusal::ReplayLawMismatch,
            ));
        }
        if matches!(
            law,
            ReplayBehavior::Idempotent { operation_key_kind }
                if operation_key_kind != &operation.operation_key_kind
        ) {
            return Err(TransformationRefusal::RetryEvidence(
                RetryEvidenceRefusal::ReplayLawMismatch,
            ));
        }
        if !proof_authorizes(law, operation, &evidence.proof) {
            return Err(TransformationRefusal::RetryEvidence(
                RetryEvidenceRefusal::DispositionDoesNotAuthorizeRetry,
            ));
        }
        Ok(RetryAuthorization::RetainedEffect {
            eligibility: self,
            evidence,
        })
    }

    fn require_recomputable(&self) -> Result<(), TransformationRefusal> {
        map_recomputable_refusal(require_recomputable_semantics(&self.facts))?;
        if self.back_has_host_calls {
            return Err(TransformationRefusal::BackHostCall);
        }
        if self.back_has_resources {
            return Err(TransformationRefusal::BackResource);
        }
        if self.back_has_authority {
            return Err(TransformationRefusal::BackAuthority);
        }
        Ok(())
    }

    fn require_retry(&self) -> Result<(), TransformationRefusal> {
        if self.facts.external_effects == ExternalEffectBehavior::None {
            return self.require_recomputable();
        }
        let law = self.require_effect_retry_semantics()?;
        Err(TransformationRefusal::RetainedRetryEvidenceRequired(
            law.clone(),
        ))
    }

    fn require_effect_retry_semantics(&self) -> Result<&ReplayBehavior, TransformationRefusal> {
        // A stronger effect law constrains duplicate effects; it does not erase
        // state, ambient inputs, suspension, or admitted variability.
        if self.facts.temporal_state != TemporalStateBehavior::None {
            return Err(TransformationRefusal::TemporalState);
        }
        if self.facts.time_dependence != SemanticDependence::None {
            return Err(TransformationRefusal::AmbientTime);
        }
        if self.facts.random_dependence != SemanticDependence::None {
            return Err(TransformationRefusal::AmbientRandom);
        }
        if self.facts.resource_dependence != SemanticDependence::None {
            return Err(TransformationRefusal::AmbientResource);
        }
        if self.facts.suspension != SuspensionBehavior::Never {
            return Err(TransformationRefusal::Suspension);
        }
        if self.facts.variability != VariabilityBehavior::DeterministicFromInputs {
            return Err(TransformationRefusal::Variability);
        }
        match &self.facts.replay {
            law @ (ReplayBehavior::Idempotent { .. }
            | ReplayBehavior::Transactional { .. }
            | ReplayBehavior::Compensatable { .. }) => Ok(law),
            ReplayBehavior::Ineligible | ReplayBehavior::Exact => {
                Err(TransformationRefusal::MissingEffectRetryLaw)
            }
        }
    }
}

impl RetryAuthorization<'_> {
    pub fn eligibility(&self) -> &TransformationEligibility {
        match self {
            Self::EffectFree { eligibility } | Self::RetainedEffect { eligibility, .. } => {
                eligibility
            }
        }
    }

    pub fn retained_evidence(&self) -> Option<&RetainedRetryEvidence> {
        match self {
            Self::EffectFree { .. } => None,
            Self::RetainedEffect { evidence, .. } => Some(evidence),
        }
    }
}

fn proof_authorizes(
    law: &ReplayBehavior,
    operation: &RetryOperationIdentity,
    proof: &RetainedRetryProof,
) -> bool {
    if matches!(proof, RetainedRetryProof::EffectNotCommitted { .. }) {
        return true;
    }
    match (law, proof) {
        (
            ReplayBehavior::Idempotent { operation_key_kind },
            RetainedRetryProof::IdempotencyKeyRetained { .. },
        ) => operation_key_kind == &operation.operation_key_kind,
        (
            ReplayBehavior::Transactional {
                transaction_contract,
            },
            RetainedRetryProof::TransactionRetained {
                transaction_contract: retained_contract,
                disposition: crate::RetainedTransactionDisposition::NotCommitted,
                ..
            },
        ) => transaction_contract == retained_contract,
        (
            ReplayBehavior::Compensatable {
                compensation_contract,
            },
            RetainedRetryProof::CompensationCommitted {
                compensation_contract: retained_contract,
                ..
            },
        ) => compensation_contract == retained_contract,
        _ => false,
    }
}

fn map_recomputable_refusal(
    result: Result<(), PureExpressionRefusal>,
) -> Result<(), TransformationRefusal> {
    result.map_err(|refusal| match refusal {
        PureExpressionRefusal::ExternalEffect => TransformationRefusal::ExternalEffect,
        PureExpressionRefusal::TemporalState => TransformationRefusal::TemporalState,
        PureExpressionRefusal::TimeDependence => TransformationRefusal::AmbientTime,
        PureExpressionRefusal::RandomDependence => TransformationRefusal::AmbientRandom,
        PureExpressionRefusal::ResourceDependence => TransformationRefusal::AmbientResource,
        PureExpressionRefusal::Suspension => TransformationRefusal::Suspension,
        PureExpressionRefusal::Variability => TransformationRefusal::Variability,
        PureExpressionRefusal::ReplayIneligible => TransformationRefusal::ReplayIneligible,
        other => TransformationRefusal::SemanticFacts(other),
    })
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
