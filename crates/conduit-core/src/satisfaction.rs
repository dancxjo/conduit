//! Language-neutral implicit contract-satisfaction proofs.
//!
//! Satisfaction is implicit only at the authored use site. A hosted provider
//! still emits this exact, reasoned proof and a runnable plan pins its identity.
//! The proof never performs a conversion, grants authority, provisions a host,
//! or mutates a plan.

use core::convert::Infallible;
use core::fmt;

use crate::canonical::semantic_hash_with_hash_set;
use crate::{
    CanonicalDescriptor, CanonicalError, CanonicalValue, CompatibilityOutcome, DescriptorRef,
    FieldDisposition, Id, MapField, PortCompatibilityDecision, PortCompatibilityReason,
    SemanticHash,
};

/// Exact schema of a satisfaction proof descriptor.
pub const SATISFACTION_PROOF_SCHEMA_VERSION: u32 = 0;

/// Directional boundary at which an offered descriptor is being considered.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SatisfactionRole {
    /// An offered output is being connected to a required input.
    PortConnection,
    /// An offered port is being substituted for a required port.
    PortSubstitution,
    /// An implementation is being selected for a semantic node contract.
    Implementation,
    /// A fresh host capability is being matched to a semantic requirement.
    HostCapability,
}

impl SatisfactionRole {
    /// Stable descriptor spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PortConnection => "port-connection",
            Self::PortSubstitution => "port-substitution",
            Self::Implementation => "implementation",
            Self::HostCapability => "host-capability",
        }
    }

    fn required_obligations(self) -> &'static [Id<'static>] {
        match self {
            Self::PortConnection | Self::PortSubstitution => &[
                Id("direction"),
                Id("semantic-type"),
                Id("presence"),
                Id("connection-cardinality"),
                Id("value-cardinality"),
                Id("delivery"),
                Id("temporal"),
                Id("terminal"),
                Id("sensitivity"),
                Id("authority"),
                Id("representation"),
                Id("ownership-lifetime"),
                Id("flow"),
                Id("boundedness"),
            ],
            Self::Implementation => &[
                Id("semantic-contract"),
                Id("ports"),
                Id("configuration"),
                Id("representation"),
                Id("ownership-lifetime"),
                Id("lifecycle"),
                Id("authority"),
                Id("resources"),
                Id("boundedness"),
            ],
            Self::HostCapability => &[
                Id("semantic-capability"),
                Id("observation-freshness"),
                Id("resources"),
                Id("effects"),
                Id("authority"),
                Id("boundedness"),
            ],
        }
    }
}

/// How the relation was established. These are proof strategies, not
/// source-language type mechanisms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SatisfactionMethod {
    /// Required and offered descriptor identities are exactly equal.
    ExactNominal,
    /// A stable provider-owned directional rule discharged the obligations.
    ProviderRule,
    /// Both sides explicitly declared a complete provider-owned facet set.
    StructuralFacets,
}

impl SatisfactionMethod {
    /// Stable descriptor spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExactNominal => "exact-nominal",
            Self::ProviderRule => "provider-rule",
            Self::StructuralFacets => "structural-facets",
        }
    }
}

/// Stable summary reason for the proof's three-valued outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SatisfactionReason {
    /// Every required obligation was discharged.
    Satisfied,
    /// One or more semantic obligations were disproved.
    ObligationRejected,
    /// A named fact needed by an otherwise available provider is missing.
    FactUnavailable,
    /// The responsible provider is unavailable.
    ProviderUnavailable,
    /// The responsible provider descriptor or report is stale.
    ProviderStale,
    /// A host report is stale at the evaluation time.
    HostObservationStale,
    /// A required structural facet is not supported.
    UnsupportedFacet,
    /// More than one candidate remains and no deterministic policy selects one.
    Ambiguous,
    /// The direct relation fails and names an explicit adapter contract.
    ExplicitAdapterRequired,
    /// The direct relation fails and names an explicit migration contract.
    ExplicitMigrationRequired,
}

impl SatisfactionReason {
    /// Stable descriptor spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Satisfied => "satisfaction-proven",
            Self::ObligationRejected => "satisfaction-obligation-rejected",
            Self::FactUnavailable => "satisfaction-fact-unavailable",
            Self::ProviderUnavailable => "satisfaction-provider-unavailable",
            Self::ProviderStale => "satisfaction-provider-stale",
            Self::HostObservationStale => "satisfaction-host-observation-stale",
            Self::UnsupportedFacet => "satisfaction-unsupported-facet",
            Self::Ambiguous => "satisfaction-ambiguous",
            Self::ExplicitAdapterRequired => "satisfaction-explicit-adapter-required",
            Self::ExplicitMigrationRequired => "satisfaction-explicit-migration-required",
        }
    }
}

/// Exact provider or deterministic selection-policy descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SatisfactionPin<'a> {
    pub descriptor: DescriptorRef<'a>,
}

/// One provider-declared semantic facet. Facet identifiers are open and
/// namespaced; their meaning remains outside the portable core.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SatisfactionFacet<'a> {
    pub id: Id<'a>,
    pub required_hash: SemanticHash,
    pub offered_hash: SemanticHash,
}

/// One exact directional obligation contributing to a proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SatisfactionObligation<'a> {
    /// Stable core obligation name or namespaced provider-owned extension.
    pub id: Id<'a>,
    /// Exact required-side fact.
    pub required_hash: SemanticHash,
    /// Exact offered-side fact.
    pub offered_hash: SemanticHash,
    /// Result of this obligation alone.
    pub outcome: CompatibilityOutcome,
    /// Stable core or provider-owned explanation rule.
    pub reason: Id<'a>,
}

/// An explicit transformation that may be proposed but is never applied by
/// the satisfaction proof itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplicitSatisfactionRequirement<'a> {
    None,
    Adapter(DescriptorRef<'a>),
    Migration(DescriptorRef<'a>),
}

impl<'a> ExplicitSatisfactionRequirement<'a> {
    const fn kind(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Adapter(_) => "adapter",
            Self::Migration(_) => "migration",
        }
    }
}

/// Complete immutable proof of one offered-to-required relation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SatisfactionProof<'a> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub role: SatisfactionRole,
    pub method: SatisfactionMethod,
    pub required: DescriptorRef<'a>,
    pub offered: DescriptorRef<'a>,
    pub provider: Option<SatisfactionPin<'a>>,
    pub provider_rule: Option<Id<'a>>,
    pub policy: Option<SatisfactionPin<'a>>,
    pub facets: &'a [SatisfactionFacet<'a>],
    pub obligations: &'a [SatisfactionObligation<'a>],
    pub outcome: CompatibilityOutcome,
    pub reason: SatisfactionReason,
    pub explanation: Id<'a>,
    pub explicit_requirement: ExplicitSatisfactionRequirement<'a>,
}

impl SatisfactionProof<'_> {
    /// Number of scratch hashes required to compute the proof identity.
    #[must_use]
    pub const fn identity_fact_count(&self) -> usize {
        self.facets.len() + self.obligations.len()
    }

    /// Computes the proof identity independently of facet or obligation order.
    pub fn semantic_hash(
        &self,
        fact_hashes: &mut [SemanticHash],
    ) -> Result<SemanticHash, SatisfactionIdentityError> {
        let needed = self.identity_fact_count();
        if fact_hashes.len() < needed {
            return Err(SatisfactionIdentityError::ScratchTooSmall);
        }
        let mut cursor = 0;
        for facet in self.facets {
            fact_hashes[cursor] = hash_facet(*facet)?;
            cursor += 1;
        }
        for obligation in self.obligations {
            fact_hashes[cursor] = hash_obligation(*obligation)?;
            cursor += 1;
        }
        let required = descriptor_fields(&self.required);
        let offered = descriptor_fields(&self.offered);
        let provider_fields = self
            .provider
            .as_ref()
            .map(|pin| descriptor_fields(&pin.descriptor));
        let provider = match &provider_fields {
            Some(fields) => CanonicalValue::Map(fields),
            None => CanonicalValue::Null,
        };
        let policy_fields = self
            .policy
            .as_ref()
            .map(|pin| descriptor_fields(&pin.descriptor));
        let policy = match &policy_fields {
            Some(fields) => CanonicalValue::Map(fields),
            None => CanonicalValue::Null,
        };
        let requirement_descriptor = match &self.explicit_requirement {
            ExplicitSatisfactionRequirement::None => None,
            ExplicitSatisfactionRequirement::Adapter(descriptor)
            | ExplicitSatisfactionRequirement::Migration(descriptor) => {
                Some(descriptor_fields(descriptor))
            }
        };
        let requirement = match &requirement_descriptor {
            Some(fields) => CanonicalValue::Map(fields),
            None => CanonicalValue::Null,
        };
        let fields = [
            semantic("role", CanonicalValue::Identifier(Id(self.role.as_str()))),
            semantic(
                "method",
                CanonicalValue::Identifier(Id(self.method.as_str())),
            ),
            semantic("required", CanonicalValue::Map(&required)),
            semantic("offered", CanonicalValue::Map(&offered)),
            semantic("provider", provider),
            semantic(
                "provider_rule",
                self.provider_rule
                    .map_or(CanonicalValue::Null, CanonicalValue::Identifier),
            ),
            semantic("policy", policy),
            semantic(
                "outcome",
                CanonicalValue::Identifier(Id(self.outcome.as_str())),
            ),
            semantic(
                "reason",
                CanonicalValue::Identifier(Id(self.reason.as_str())),
            ),
            semantic("explanation", CanonicalValue::Identifier(self.explanation)),
            semantic(
                "explicit_requirement",
                CanonicalValue::Identifier(Id(self.explicit_requirement.kind())),
            ),
            semantic("explicit_requirement_descriptor", requirement),
        ];
        semantic_hash_with_hash_set(
            Id("conduit/satisfaction-proof"),
            self.schema_version,
            &fields,
            Id("facts"),
            &fact_hashes[..needed],
        )
        .map_err(SatisfactionIdentityError::Canonical)
    }
}

/// Proof identity construction error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SatisfactionIdentityError {
    ScratchTooSmall,
    Canonical(CanonicalError<Infallible>),
}

impl From<CanonicalError<Infallible>> for SatisfactionIdentityError {
    fn from(error: CanonicalError<Infallible>) -> Self {
        Self::Canonical(error)
    }
}

/// Portable proof validation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SatisfactionProofError {
    UnsupportedVersion,
    InvalidDescriptor,
    InvalidProvider,
    InvalidPolicy,
    InvalidFacet,
    DuplicateFacet,
    InvalidObligation,
    DuplicateObligation,
    MissingObligation,
    OutcomeMismatch,
    ReasonMismatch,
    TransformationMismatch,
    CompatibilityMismatch,
    IdentityMismatch,
    ScratchTooSmall,
}

impl fmt::Display for SatisfactionProofError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedVersion => "satisfaction proof version is unsupported",
            Self::InvalidDescriptor => "satisfaction proof operand is invalid",
            Self::InvalidProvider => "satisfaction proof provider is invalid or missing",
            Self::InvalidPolicy => "satisfaction selection policy is invalid",
            Self::InvalidFacet => "satisfaction facet is invalid",
            Self::DuplicateFacet => "satisfaction facet is duplicated",
            Self::InvalidObligation => "satisfaction obligation is invalid",
            Self::DuplicateObligation => "satisfaction obligation is duplicated",
            Self::MissingObligation => "satisfaction proof omits a required obligation",
            Self::OutcomeMismatch => "satisfaction outcome does not match its obligations",
            Self::ReasonMismatch => "satisfaction reason does not match its outcome",
            Self::TransformationMismatch => {
                "explicit adapter or migration requirement is inconsistent"
            }
            Self::CompatibilityMismatch => {
                "satisfaction proof does not match the current port decision"
            }
            Self::IdentityMismatch => "satisfaction proof identity does not match",
            Self::ScratchTooSmall => "satisfaction proof identity scratch is too small",
        })
    }
}

/// Validates a proof against the existing complete directional PortContract
/// decision rather than introducing a second port-compatibility path.
pub fn validate_port_satisfaction_proof(
    proof: &SatisfactionProof<'_>,
    decision: PortCompatibilityDecision<'_>,
    fact_hashes: &mut [SemanticHash],
) -> Result<(), SatisfactionProofError> {
    validate_satisfaction_proof(proof, fact_hashes)?;
    if !matches!(
        proof.role,
        SatisfactionRole::PortConnection | SatisfactionRole::PortSubstitution
    ) {
        return Err(SatisfactionProofError::CompatibilityMismatch);
    }
    let required_hash = decision
        .consumer
        .semantic_hash()
        .map_err(|_| SatisfactionProofError::InvalidDescriptor)?;
    let offered_hash = decision
        .producer
        .semantic_hash()
        .map_err(|_| SatisfactionProofError::InvalidDescriptor)?;
    if proof.required
        != (DescriptorRef {
            kind: Id("conduit/port-contract"),
            schema_version: 0,
            semantic_hash: required_hash,
        })
        || proof.offered
            != (DescriptorRef {
                kind: Id("conduit/port-contract"),
                schema_version: 0,
                semantic_hash: offered_hash,
            })
        || proof.outcome != decision.outcome
    {
        return Err(SatisfactionProofError::CompatibilityMismatch);
    }
    let type_obligation = proof
        .obligations
        .iter()
        .find(|obligation| obligation.id == Id("semantic-type"))
        .ok_or(SatisfactionProofError::MissingObligation)?;
    if type_obligation.required_hash != decision.consumer.value_type.semantic_hash
        || type_obligation.offered_hash != decision.producer.value_type.semantic_hash
        || type_obligation.outcome != decision.type_decision.outcome
    {
        return Err(SatisfactionProofError::CompatibilityMismatch);
    }
    let reason_obligation = match decision.reason {
        PortCompatibilityReason::Accepted => None,
        PortCompatibilityReason::DirectionMismatch => Some(Id("direction")),
        PortCompatibilityReason::TypeMismatch => Some(Id("semantic-type")),
        PortCompatibilityReason::PresenceMismatch => Some(Id("presence")),
        PortCompatibilityReason::ConnectionCardinalityMismatch => {
            Some(Id("connection-cardinality"))
        }
        PortCompatibilityReason::ValueCardinalityMismatch => Some(Id("value-cardinality")),
        PortCompatibilityReason::DeliveryMismatch => Some(Id("delivery")),
        PortCompatibilityReason::TemporalMismatch => Some(Id("temporal")),
        PortCompatibilityReason::TerminalMismatch => Some(Id("terminal")),
        PortCompatibilityReason::SensitivityViolation => Some(Id("sensitivity")),
        PortCompatibilityReason::FlowConstraintMismatch => Some(Id("flow")),
    };
    if reason_obligation.is_none() != (decision.outcome == CompatibilityOutcome::Compatible)
        || reason_obligation.is_some_and(|id| {
            !proof
                .obligations
                .iter()
                .any(|obligation| obligation.id == id && obligation.outcome == decision.outcome)
        })
    {
        return Err(SatisfactionProofError::CompatibilityMismatch);
    }
    Ok(())
}

/// Validates exact operands, complete obligations, result, and proof identity.
pub fn validate_satisfaction_proof(
    proof: &SatisfactionProof<'_>,
    fact_hashes: &mut [SemanticHash],
) -> Result<(), SatisfactionProofError> {
    if proof.schema_version != SATISFACTION_PROOF_SCHEMA_VERSION {
        return Err(SatisfactionProofError::UnsupportedVersion);
    }
    if !valid_descriptor(proof.required)
        || !valid_descriptor(proof.offered)
        || Id::new(proof.explanation.as_str()).is_err()
    {
        return Err(SatisfactionProofError::InvalidDescriptor);
    }
    if proof
        .provider
        .is_some_and(|pin| !valid_descriptor(pin.descriptor))
        || proof
            .provider_rule
            .is_some_and(|rule| Id::new(rule.as_str()).is_err())
    {
        return Err(SatisfactionProofError::InvalidProvider);
    }
    if proof
        .policy
        .is_some_and(|pin| !valid_descriptor(pin.descriptor))
    {
        return Err(SatisfactionProofError::InvalidPolicy);
    }
    match proof.method {
        SatisfactionMethod::ExactNominal => {
            if proof.required != proof.offered
                || proof.provider.is_some()
                || proof.provider_rule.is_some()
                || !proof.facets.is_empty()
                || !proof.obligations.is_empty()
            {
                return Err(SatisfactionProofError::InvalidProvider);
            }
        }
        SatisfactionMethod::ProviderRule => {
            let unavailable = proof.outcome == CompatibilityOutcome::Indeterminate
                && proof.reason == SatisfactionReason::ProviderUnavailable;
            if unavailable && (proof.provider.is_some() || proof.provider_rule.is_some())
                || !unavailable && (proof.provider.is_none() || proof.provider_rule.is_none())
            {
                return Err(SatisfactionProofError::InvalidProvider);
            }
        }
        SatisfactionMethod::StructuralFacets => {
            let unavailable = proof.outcome == CompatibilityOutcome::Indeterminate
                && proof.reason == SatisfactionReason::ProviderUnavailable;
            if proof.facets.is_empty()
                || unavailable && (proof.provider.is_some() || proof.provider_rule.is_some())
                || !unavailable && (proof.provider.is_none() || proof.provider_rule.is_none())
            {
                return Err(SatisfactionProofError::InvalidProvider);
            }
        }
    }
    for (index, facet) in proof.facets.iter().enumerate() {
        if Id::new(facet.id.as_str()).is_err() {
            return Err(SatisfactionProofError::InvalidFacet);
        }
        if proof.facets[..index]
            .iter()
            .any(|prior| prior.id == facet.id)
        {
            return Err(SatisfactionProofError::DuplicateFacet);
        }
    }
    for (index, obligation) in proof.obligations.iter().enumerate() {
        if Id::new(obligation.id.as_str()).is_err() || Id::new(obligation.reason.as_str()).is_err()
        {
            return Err(SatisfactionProofError::InvalidObligation);
        }
        if proof.obligations[..index]
            .iter()
            .any(|prior| prior.id == obligation.id)
        {
            return Err(SatisfactionProofError::DuplicateObligation);
        }
    }
    if proof.method != SatisfactionMethod::ExactNominal
        && proof.role.required_obligations().iter().any(|required| {
            !proof
                .obligations
                .iter()
                .any(|obligation| obligation.id == *required)
        })
    {
        return Err(SatisfactionProofError::MissingObligation);
    }

    let derived_outcome = if proof.method == SatisfactionMethod::ExactNominal {
        CompatibilityOutcome::Compatible
    } else if proof
        .obligations
        .iter()
        .any(|obligation| obligation.outcome == CompatibilityOutcome::Incompatible)
    {
        CompatibilityOutcome::Incompatible
    } else if proof
        .obligations
        .iter()
        .any(|obligation| obligation.outcome == CompatibilityOutcome::Indeterminate)
    {
        CompatibilityOutcome::Indeterminate
    } else {
        CompatibilityOutcome::Compatible
    };
    if proof.outcome != derived_outcome {
        return Err(SatisfactionProofError::OutcomeMismatch);
    }
    let reason_valid = match proof.outcome {
        CompatibilityOutcome::Compatible => proof.reason == SatisfactionReason::Satisfied,
        CompatibilityOutcome::Incompatible => matches!(
            proof.reason,
            SatisfactionReason::ObligationRejected
                | SatisfactionReason::UnsupportedFacet
                | SatisfactionReason::ExplicitAdapterRequired
                | SatisfactionReason::ExplicitMigrationRequired
        ),
        CompatibilityOutcome::Indeterminate => matches!(
            proof.reason,
            SatisfactionReason::FactUnavailable
                | SatisfactionReason::ProviderUnavailable
                | SatisfactionReason::ProviderStale
                | SatisfactionReason::HostObservationStale
                | SatisfactionReason::UnsupportedFacet
        ),
    };
    if !reason_valid {
        return Err(SatisfactionProofError::ReasonMismatch);
    }
    let transformation_valid = match proof.explicit_requirement {
        ExplicitSatisfactionRequirement::None => !matches!(
            proof.reason,
            SatisfactionReason::ExplicitAdapterRequired
                | SatisfactionReason::ExplicitMigrationRequired
        ),
        ExplicitSatisfactionRequirement::Adapter(descriptor) => {
            valid_descriptor(descriptor)
                && proof.outcome == CompatibilityOutcome::Incompatible
                && proof.reason == SatisfactionReason::ExplicitAdapterRequired
        }
        ExplicitSatisfactionRequirement::Migration(descriptor) => {
            valid_descriptor(descriptor)
                && proof.outcome == CompatibilityOutcome::Incompatible
                && proof.reason == SatisfactionReason::ExplicitMigrationRequired
        }
    };
    if !transformation_valid {
        return Err(SatisfactionProofError::TransformationMismatch);
    }
    match proof.semantic_hash(fact_hashes) {
        Ok(identity) if identity == proof.identity => Ok(()),
        Ok(_) => Err(SatisfactionProofError::IdentityMismatch),
        Err(SatisfactionIdentityError::ScratchTooSmall) => {
            Err(SatisfactionProofError::ScratchTooSmall)
        }
        Err(SatisfactionIdentityError::Canonical(_)) => {
            Err(SatisfactionProofError::InvalidDescriptor)
        }
    }
}

/// Candidate considered by deterministic satisfaction selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SatisfactionCandidate<'a> {
    pub id: Id<'a>,
    pub proof: &'a SatisfactionProof<'a>,
    /// Policy-owned rank. Lower values win; equal best ranks are ambiguous.
    pub policy_rank: u32,
}

/// Stable result of selecting among proven candidates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SatisfactionSelection<'a> {
    pub outcome: CompatibilityOutcome,
    pub selected: Option<Id<'a>>,
    pub proof_identity: Option<SemanticHash>,
    pub policy: Option<SatisfactionPin<'a>>,
    pub reason: SatisfactionReason,
}

/// Selects a candidate independently of discovery order.
///
/// Without an exact policy, precisely one compatible candidate is required.
/// With a policy, the uniquely lowest rank wins. A tie is an ambiguity rather
/// than a registry-order tiebreak.
#[must_use]
pub fn select_satisfaction_candidate<'a>(
    candidates: &'a [SatisfactionCandidate<'a>],
    policy: Option<SatisfactionPin<'a>>,
) -> SatisfactionSelection<'a> {
    let invalid_input = policy.is_some_and(|pin| !valid_descriptor(pin.descriptor))
        || candidates.iter().enumerate().any(|(index, candidate)| {
            Id::new(candidate.id.as_str()).is_err()
                || candidates[..index]
                    .iter()
                    .any(|prior| prior.id == candidate.id)
        });
    if invalid_input {
        return SatisfactionSelection {
            outcome: CompatibilityOutcome::Indeterminate,
            selected: None,
            proof_identity: None,
            policy,
            reason: SatisfactionReason::FactUnavailable,
        };
    }
    let compatible_count = candidates
        .iter()
        .filter(|candidate| candidate.proof.outcome == CompatibilityOutcome::Compatible)
        .count();
    if compatible_count == 0 {
        let indeterminate = candidates
            .iter()
            .any(|candidate| candidate.proof.outcome == CompatibilityOutcome::Indeterminate);
        return SatisfactionSelection {
            outcome: if indeterminate {
                CompatibilityOutcome::Indeterminate
            } else {
                CompatibilityOutcome::Incompatible
            },
            selected: None,
            proof_identity: None,
            policy,
            reason: if indeterminate {
                SatisfactionReason::FactUnavailable
            } else {
                SatisfactionReason::ObligationRejected
            },
        };
    }
    let selected = if policy.is_none() {
        if compatible_count != 1 {
            None
        } else {
            candidates
                .iter()
                .find(|candidate| candidate.proof.outcome == CompatibilityOutcome::Compatible)
        }
    } else {
        let best_rank = candidates
            .iter()
            .filter(|candidate| candidate.proof.outcome == CompatibilityOutcome::Compatible)
            .map(|candidate| candidate.policy_rank)
            .min();
        best_rank.and_then(|rank| {
            let mut best = candidates.iter().filter(|candidate| {
                candidate.proof.outcome == CompatibilityOutcome::Compatible
                    && candidate.policy_rank == rank
            });
            let candidate = best.next()?;
            best.next().is_none().then_some(candidate)
        })
    };
    selected.map_or(
        SatisfactionSelection {
            outcome: CompatibilityOutcome::Indeterminate,
            selected: None,
            proof_identity: None,
            policy,
            reason: SatisfactionReason::Ambiguous,
        },
        |candidate| SatisfactionSelection {
            outcome: CompatibilityOutcome::Compatible,
            selected: Some(candidate.id),
            proof_identity: Some(candidate.proof.identity),
            policy,
            reason: SatisfactionReason::Satisfied,
        },
    )
}

fn valid_descriptor(descriptor: DescriptorRef<'_>) -> bool {
    Id::new(descriptor.kind.as_str()).is_ok() && descriptor.schema_version == 0
}

fn descriptor_fields<'a>(descriptor: &'a DescriptorRef<'a>) -> [MapField<'a>; 3] {
    [
        semantic("kind", CanonicalValue::Identifier(descriptor.kind)),
        semantic(
            "schema_version",
            CanonicalValue::Integer(i128::from(descriptor.schema_version)),
        ),
        semantic(
            "semantic_hash",
            CanonicalValue::Bytes(descriptor.semantic_hash.as_bytes()),
        ),
    ]
}

fn hash_facet(facet: SatisfactionFacet<'_>) -> Result<SemanticHash, CanonicalError<Infallible>> {
    CanonicalDescriptor {
        kind: Id("conduit/satisfaction-facet"),
        schema_version: 0,
        body: CanonicalValue::Map(&[
            semantic("id", CanonicalValue::Identifier(facet.id)),
            semantic(
                "required_hash",
                CanonicalValue::Bytes(facet.required_hash.as_bytes()),
            ),
            semantic(
                "offered_hash",
                CanonicalValue::Bytes(facet.offered_hash.as_bytes()),
            ),
        ]),
    }
    .semantic_hash()
}

fn hash_obligation(
    obligation: SatisfactionObligation<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    CanonicalDescriptor {
        kind: Id("conduit/satisfaction-obligation"),
        schema_version: 0,
        body: CanonicalValue::Map(&[
            semantic("id", CanonicalValue::Identifier(obligation.id)),
            semantic(
                "required_hash",
                CanonicalValue::Bytes(obligation.required_hash.as_bytes()),
            ),
            semantic(
                "offered_hash",
                CanonicalValue::Bytes(obligation.offered_hash.as_bytes()),
            ),
            semantic(
                "outcome",
                CanonicalValue::Identifier(Id(obligation.outcome.as_str())),
            ),
            semantic("reason", CanonicalValue::Identifier(obligation.reason)),
        ]),
    }
    .semantic_hash()
}

const fn semantic<'a>(name: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    }
}
