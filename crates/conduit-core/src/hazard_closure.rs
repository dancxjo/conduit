//! Bounded whole-plan effect closure and policy-selected toxic combinations.
//!
//! Conduit assigns no domain meaning to an effect class or stage transfer.
//! Domains pin those descriptors through ordinary effect constraints. This
//! module proves composition only over resolved plan facts.

use core::convert::Infallible;

use crate::canonical::semantic_hash_with_hash_set;
use crate::{
    AdministrativeProof, AdministrativeSubject, AuthorityConstraintRef, AuthorityTime,
    CanonicalDescriptor, CanonicalError, CanonicalValue, DelegationPolicy, FieldDisposition, Id,
    MapField, PinnedDescriptor, PlanAuthority, ResourceSelector, SemanticHash,
    validate_administrative_proof,
};

pub const HAZARD_CLOSURE_POLICY_SCHEMA_VERSION: u32 = 1;
pub const MAX_HAZARD_CLASSES: usize = 32;
pub const MAX_HAZARD_RULES: usize = 16;
pub const MAX_HAZARD_PATTERNS: usize = 8;
pub const MAX_HAZARD_FLOWS: usize = 32;
pub const MAX_HAZARD_PERMITS: usize = 16;
pub const MAX_HAZARD_EFFECTS: usize = 64;
pub const MAX_HAZARD_PROOF_NODES: usize = 64;

/// Policy-owned interpretation of a domain-owned class descriptor.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EffectClassTraits {
    pub persistence: bool,
    pub delegation: bool,
    pub distributed: bool,
    pub administrative: bool,
}

/// A versioned domain class referenced by an ordinary effect constraint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectClassBinding<'a> {
    pub identity: SemanticHash,
    pub descriptor: PinnedDescriptor<'a>,
    pub constraint: AuthorityConstraintRef<'a>,
    pub traits: EffectClassTraits,
}

/// Three-valued trait selection avoids adding domain categories to core.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraitRequirement {
    Any,
    Required,
    Forbidden,
}

impl TraitRequirement {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::Required => "required",
            Self::Forbidden => "forbidden",
        }
    }

    const fn matches(self, value: bool) -> bool {
        match self {
            Self::Any => true,
            Self::Required => value,
            Self::Forbidden => !value,
        }
    }
}

/// One exact stage in a policy-defined toxic combination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ToxicEffectPattern<'a> {
    pub id: Id<'a>,
    pub class: PinnedDescriptor<'a>,
    pub resource: Option<ResourceSelector<'a>>,
    pub audience: Option<Id<'a>>,
    pub host: Option<Id<'a>>,
    pub realm: Option<Id<'a>>,
    pub budget: Option<PinnedDescriptor<'a>>,
    pub persistence: TraitRequirement,
    pub delegation: TraitRequirement,
    pub distributed: TraitRequirement,
    pub administrative: TraitRequirement,
}

/// A domain-owned transfer connecting two matched stages.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ToxicFlowRequirement<'a> {
    pub from_pattern: u8,
    pub to_pattern: u8,
    pub transfer: PinnedDescriptor<'a>,
}

/// An exact declared transfer between two resolved effects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectFlowBinding<'a> {
    pub from_effect: Id<'a>,
    pub to_effect: Id<'a>,
    pub transfer: PinnedDescriptor<'a>,
}

impl EffectFlowBinding<'_> {
    pub fn computed_semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        flow_hash(*self)
    }
}

/// One forbidden combination. An exact permit may authorize one occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ToxicCombinationRule<'a> {
    pub identity: SemanticHash,
    pub descriptor: PinnedDescriptor<'a>,
    pub patterns: &'a [ToxicEffectPattern<'a>],
    pub flows: &'a [ToxicFlowRequirement<'a>],
}

/// Finite analysis and explanation limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HazardClosureLimits {
    pub maximum_effects: u16,
    pub maximum_classes: u8,
    pub maximum_rules: u8,
    pub maximum_patterns_per_rule: u8,
    pub maximum_flows: u8,
    pub maximum_permits: u8,
    pub maximum_proof_nodes: u8,
    pub maximum_search_steps: u32,
}

/// Domain policy selecting classes and forbidden combinations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HazardClosurePolicy<'a> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub descriptor: PinnedDescriptor<'a>,
    pub permit_class: PinnedDescriptor<'a>,
    pub classes: &'a [EffectClassBinding<'a>],
    pub rules: &'a [ToxicCombinationRule<'a>],
    pub limits: HazardClosureLimits,
}

/// An exact, expiring, independently approved exception.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HazardPermit<'a> {
    pub identity: SemanticHash,
    pub descriptor: PinnedDescriptor<'a>,
    pub policy_identity: SemanticHash,
    pub rule_identity: SemanticHash,
    pub plan_subject: SemanticHash,
    pub epoch: u64,
    pub scope_identity: SemanticHash,
    pub time_basis: Id<'a>,
    pub not_before_tick: u64,
    pub expires_at_tick: u64,
    pub approval: AdministrativeProof<'a>,
}

/// Exact facts supplied at plan validation or transition admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HazardClosureContext<'a> {
    pub plan_subject: SemanticHash,
    pub epoch: u64,
    pub time: AuthorityTime<'a>,
}

/// Exact old/new facts that may coexist during a live transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionEffectClosure<'a> {
    pub old_authorities: &'a [PlanAuthority<'a>],
    pub new_and_rollback_authorities: &'a [PlanAuthority<'a>],
    pub old_flows: &'a [EffectFlowBinding<'a>],
    pub new_and_rollback_flows: &'a [EffectFlowBinding<'a>],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HazardClosureDisposition {
    Accepted,
    Permitted,
}

/// Secret-safe proof nodes. Only validated descriptor and effect identifiers
/// are retained; resource values and protected handles are not copied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HazardProofKind {
    Rule,
    Effect,
    Flow,
    Permit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HazardProofNode<'a> {
    pub parent: Option<u8>,
    pub kind: HazardProofKind,
    pub descriptor: Id<'a>,
    pub effect: Option<Id<'a>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HazardClosureReport {
    pub decision_identity: SemanticHash,
    pub closure_identity: SemanticHash,
    pub disposition: HazardClosureDisposition,
    pub matched_rules: u8,
    pub permits_used: u8,
    pub proof_nodes: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HazardClosureReason {
    UnsupportedVersion,
    InvalidDescriptor,
    IdentityMismatch,
    EffectLimitExceeded,
    FlowInvalid,
    RuleInvalid,
    SearchLimitExceeded,
    ProofStorageExceeded,
    ToxicCombination,
    PermitMissing,
    PermitScopeMismatch,
    PermitExpired,
    PermitApprovalInvalid,
    TransitionSubjectInvalid,
}

impl HazardClosureReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "CND-HZD-001",
            Self::InvalidDescriptor => "CND-HZD-002",
            Self::IdentityMismatch => "CND-HZD-003",
            Self::EffectLimitExceeded => "CND-HZD-004",
            Self::FlowInvalid => "CND-HZD-005",
            Self::RuleInvalid => "CND-HZD-006",
            Self::SearchLimitExceeded => "CND-HZD-007",
            Self::ProofStorageExceeded => "CND-HZD-008",
            Self::ToxicCombination => "CND-HZD-009",
            Self::PermitMissing => "CND-HZD-010",
            Self::PermitScopeMismatch => "CND-HZD-011",
            Self::PermitExpired => "CND-HZD-012",
            Self::PermitApprovalInvalid => "CND-HZD-013",
            Self::TransitionSubjectInvalid => "CND-HZD-014",
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "unsupported-hazard-closure-version",
            Self::InvalidDescriptor => "invalid-hazard-closure-descriptor",
            Self::IdentityMismatch => "hazard-closure-identity-mismatch",
            Self::EffectLimitExceeded => "hazard-effect-limit-exceeded",
            Self::FlowInvalid => "hazard-flow-invalid",
            Self::RuleInvalid => "toxic-combination-rule-invalid",
            Self::SearchLimitExceeded => "hazard-search-limit-exceeded",
            Self::ProofStorageExceeded => "hazard-proof-storage-exceeded",
            Self::ToxicCombination => "toxic-effect-combination",
            Self::PermitMissing => "toxic-combination-permit-missing",
            Self::PermitScopeMismatch => "hazard-permit-scope-mismatch",
            Self::PermitExpired => "hazard-permit-expired",
            Self::PermitApprovalInvalid => "hazard-permit-approval-invalid",
            Self::TransitionSubjectInvalid => "hazard-transition-subject-invalid",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HazardClosureDenial<'a> {
    pub reason: HazardClosureReason,
    pub rule: Option<Id<'a>>,
    /// Canonical combined-authority indexes selected by the denied rule.
    ///
    /// The proof tree retains the corresponding validated effect identifiers.
    pub effect_indexes: [u8; MAX_HAZARD_PATTERNS],
    pub effect_count: u8,
}

impl<'a> HazardClosureDenial<'a> {
    const fn new(reason: HazardClosureReason) -> Self {
        Self {
            reason,
            rule: None,
            effect_indexes: [u8::MAX; MAX_HAZARD_PATTERNS],
            effect_count: 0,
        }
    }

    const fn for_match(
        reason: HazardClosureReason,
        rule: Id<'a>,
        effect_indexes: [u8; MAX_HAZARD_PATTERNS],
        effect_count: u8,
    ) -> Self {
        Self {
            reason,
            rule: Some(rule),
            effect_indexes,
            effect_count,
        }
    }
}

/// Analyze one exact runnable plan.
pub fn analyze_effect_closure<'a>(
    policy: HazardClosurePolicy<'a>,
    authorities: &'a [PlanAuthority<'a>],
    flows: &'a [EffectFlowBinding<'a>],
    permits: &'a [HazardPermit<'a>],
    context: HazardClosureContext<'a>,
    proof: &mut [Option<HazardProofNode<'a>>],
) -> Result<HazardClosureReport, HazardClosureDenial<'a>> {
    analyze(
        policy,
        authorities,
        &[],
        flows,
        &[],
        permits,
        context,
        proof,
    )
}

/// Analyze simultaneous old/new generations, including rollback reserve.
pub fn analyze_transition_effect_closure<'a>(
    policy: HazardClosurePolicy<'a>,
    transition: TransitionEffectClosure<'a>,
    permits: &'a [HazardPermit<'a>],
    context: HazardClosureContext<'a>,
    proof: &mut [Option<HazardProofNode<'a>>],
) -> Result<HazardClosureReport, HazardClosureDenial<'a>> {
    if transition.old_authorities.is_empty() || transition.new_and_rollback_authorities.is_empty() {
        return Err(HazardClosureDenial::new(
            HazardClosureReason::TransitionSubjectInvalid,
        ));
    }
    analyze(
        policy,
        transition.old_authorities,
        transition.new_and_rollback_authorities,
        transition.old_flows,
        transition.new_and_rollback_flows,
        permits,
        context,
        proof,
    )
}

#[allow(clippy::too_many_arguments)]
fn analyze<'a>(
    policy: HazardClosurePolicy<'a>,
    primary: &'a [PlanAuthority<'a>],
    overlap: &'a [PlanAuthority<'a>],
    primary_flows: &'a [EffectFlowBinding<'a>],
    overlap_flows: &'a [EffectFlowBinding<'a>],
    permits: &'a [HazardPermit<'a>],
    context: HazardClosureContext<'a>,
    proof: &mut [Option<HazardProofNode<'a>>],
) -> Result<HazardClosureReport, HazardClosureDenial<'a>> {
    validate_policy(policy).map_err(HazardClosureDenial::new)?;
    let effect_count = primary
        .len()
        .checked_add(overlap.len())
        .ok_or_else(|| HazardClosureDenial::new(HazardClosureReason::EffectLimitExceeded))?;
    let flow_count = primary_flows
        .len()
        .checked_add(overlap_flows.len())
        .ok_or_else(|| HazardClosureDenial::new(HazardClosureReason::FlowInvalid))?;
    if effect_count == 0
        || effect_count > usize::from(policy.limits.maximum_effects)
        || effect_count > MAX_HAZARD_EFFECTS
    {
        return Err(HazardClosureDenial::new(
            HazardClosureReason::EffectLimitExceeded,
        ));
    }
    if flow_count > usize::from(policy.limits.maximum_flows)
        || flow_count > MAX_HAZARD_FLOWS
        || permits.len() > usize::from(policy.limits.maximum_permits)
        || permits.len() > MAX_HAZARD_PERMITS
    {
        return Err(HazardClosureDenial::new(
            HazardClosureReason::InvalidDescriptor,
        ));
    }
    if proof.len() < usize::from(policy.limits.maximum_proof_nodes) {
        return Err(HazardClosureDenial::new(
            HazardClosureReason::ProofStorageExceeded,
        ));
    }
    proof.fill(None);
    validate_effects(primary, overlap).map_err(HazardClosureDenial::new)?;
    validate_flows(primary, overlap, primary_flows, overlap_flows)
        .map_err(HazardClosureDenial::new)?;

    let closure_identity =
        closure_identity(primary, overlap, primary_flows, overlap_flows, context)
            .map_err(|_| HazardClosureDenial::new(HazardClosureReason::IdentityMismatch))?;
    if closure_identity != context.plan_subject {
        return Err(HazardClosureDenial::new(
            HazardClosureReason::TransitionSubjectInvalid,
        ));
    }

    let mut order = [0_u8; MAX_HAZARD_EFFECTS];
    for (index, slot) in order[..effect_count].iter_mut().enumerate() {
        *slot = u8::try_from(index)
            .map_err(|_| HazardClosureDenial::new(HazardClosureReason::EffectLimitExceeded))?;
    }
    order[..effect_count].sort_unstable_by(|left, right| {
        let left = authority_at(primary, overlap, usize::from(*left));
        let right = authority_at(primary, overlap, usize::from(*right));
        left.effect_hash
            .as_bytes()
            .cmp(right.effect_hash.as_bytes())
            .then_with(|| left.effect.id.as_str().cmp(right.effect.id.as_str()))
    });

    let mut written = 0_usize;
    let mut matched_rules = 0_u8;
    let mut permits_used = 0_u8;
    let mut permit_hashes = [SemanticHash::from_bytes([0; 32]); MAX_HAZARD_PERMITS];
    let mut permitted_scopes = [SemanticHash::from_bytes([0; 32]); MAX_HAZARD_PERMITS];
    let mut rule_order = [0_u8; MAX_HAZARD_RULES];
    for (index, slot) in rule_order[..policy.rules.len()].iter_mut().enumerate() {
        *slot = u8::try_from(index)
            .map_err(|_| HazardClosureDenial::new(HazardClosureReason::RuleInvalid))?;
    }
    rule_order[..policy.rules.len()].sort_unstable_by(|left, right| {
        policy.rules[usize::from(*left)]
            .identity
            .as_bytes()
            .cmp(policy.rules[usize::from(*right)].identity.as_bytes())
    });
    for rule_index in &rule_order[..policy.rules.len()] {
        let rule = &policy.rules[usize::from(*rule_index)];
        let mut search_steps = 0_u32;
        let mut prior_selection = [u8::MAX; MAX_HAZARD_PATTERNS];
        let mut has_prior_selection = false;
        let mut rule_matched = false;
        loop {
            let mut selected = [u8::MAX; MAX_HAZARD_PATTERNS];
            if !search_rule(
                policy,
                *rule,
                primary,
                overlap,
                primary_flows,
                overlap_flows,
                &order[..effect_count],
                0,
                if has_prior_selection {
                    Some(&prior_selection)
                } else {
                    None
                },
                &mut selected,
                &mut search_steps,
            )
            .map_err(HazardClosureDenial::new)?
            {
                break;
            }
            prior_selection = selected;
            has_prior_selection = true;
            if !rule_matched {
                matched_rules = matched_rules
                    .checked_add(1)
                    .ok_or_else(|| HazardClosureDenial::new(HazardClosureReason::RuleInvalid))?;
                rule_matched = true;
            }
            let scope = match_scope_identity(*rule, primary, overlap, &selected)
                .map_err(|_| HazardClosureDenial::new(HazardClosureReason::IdentityMismatch))?;
            if permitted_scopes[..usize::from(permits_used)].contains(&scope) {
                continue;
            }
            let permit = permits.iter().find(|permit| {
                permit.policy_identity == policy.identity
                    && permit.rule_identity == rule.identity
                    && permit.plan_subject == context.plan_subject
                    && permit.epoch == context.epoch
                    && permit.scope_identity == scope
            });
            let Some(permit) = permit else {
                write_match_proof(
                    proof,
                    &mut written,
                    *rule,
                    AuthoritySlices { primary, overlap },
                    &selected,
                    None,
                    policy.limits.maximum_proof_nodes,
                )
                .map_err(HazardClosureDenial::new)?;
                return Err(match_denial(
                    HazardClosureReason::PermitMissing,
                    *rule,
                    &selected,
                ));
            };
            validate_permit(*permit, policy, *rule, context, scope)
                .map_err(|reason| match_denial(reason, *rule, &selected))?;
            write_match_proof(
                proof,
                &mut written,
                *rule,
                AuthoritySlices { primary, overlap },
                &selected,
                Some(*permit),
                policy.limits.maximum_proof_nodes,
            )
            .map_err(HazardClosureDenial::new)?;
            let permit_index = usize::from(permits_used);
            if permit_index >= permitted_scopes.len() {
                return Err(HazardClosureDenial::new(
                    HazardClosureReason::InvalidDescriptor,
                ));
            }
            permitted_scopes[permit_index] = scope;
            permit_hashes[permit_index] = permit.identity;
            permits_used = permits_used
                .checked_add(1)
                .ok_or_else(|| HazardClosureDenial::new(HazardClosureReason::InvalidDescriptor))?;
        }
    }

    let disposition = if permits_used == 0 {
        HazardClosureDisposition::Accepted
    } else {
        HazardClosureDisposition::Permitted
    };
    let decision_identity = decision_identity(
        policy.identity,
        closure_identity,
        context,
        disposition,
        matched_rules,
        &permit_hashes[..usize::from(permits_used)],
    )
    .map_err(|_| HazardClosureDenial::new(HazardClosureReason::IdentityMismatch))?;
    Ok(HazardClosureReport {
        decision_identity,
        closure_identity,
        disposition,
        matched_rules,
        permits_used,
        proof_nodes: u8::try_from(written)
            .map_err(|_| HazardClosureDenial::new(HazardClosureReason::ProofStorageExceeded))?,
    })
}

fn validate_policy(policy: HazardClosurePolicy<'_>) -> Result<(), HazardClosureReason> {
    let limits = policy.limits;
    if policy.schema_version != HAZARD_CLOSURE_POLICY_SCHEMA_VERSION {
        return Err(HazardClosureReason::UnsupportedVersion);
    }
    if !valid_pin(policy.descriptor)
        || !valid_pin(policy.permit_class)
        || policy.classes.is_empty()
        || policy.classes.len() > usize::from(limits.maximum_classes)
        || policy.classes.len() > MAX_HAZARD_CLASSES
        || policy.rules.is_empty()
        || policy.rules.len() > usize::from(limits.maximum_rules)
        || policy.rules.len() > MAX_HAZARD_RULES
        || limits.maximum_effects == 0
        || usize::from(limits.maximum_effects) > MAX_HAZARD_EFFECTS
        || limits.maximum_classes == 0
        || usize::from(limits.maximum_classes) > MAX_HAZARD_CLASSES
        || limits.maximum_rules == 0
        || usize::from(limits.maximum_rules) > MAX_HAZARD_RULES
        || limits.maximum_patterns_per_rule == 0
        || usize::from(limits.maximum_patterns_per_rule) > MAX_HAZARD_PATTERNS
        || usize::from(limits.maximum_flows) > MAX_HAZARD_FLOWS
        || usize::from(limits.maximum_permits) > MAX_HAZARD_PERMITS
        || limits.maximum_proof_nodes == 0
        || usize::from(limits.maximum_proof_nodes) > MAX_HAZARD_PROOF_NODES
        || limits.maximum_search_steps == 0
    {
        return Err(HazardClosureReason::InvalidDescriptor);
    }
    for (index, class) in policy.classes.iter().enumerate() {
        if class.constraint.id != class.descriptor.id
            || class.constraint.semantic_hash != class.descriptor.semantic_hash
            || class.identity
                != class
                    .computed_semantic_hash()
                    .map_err(|_| HazardClosureReason::InvalidDescriptor)?
            || policy.classes[..index]
                .iter()
                .any(|prior| prior.descriptor == class.descriptor)
        {
            return Err(HazardClosureReason::IdentityMismatch);
        }
    }
    for (index, rule) in policy.rules.iter().enumerate() {
        validate_rule(*rule, policy)?;
        if rule.identity
            != rule
                .computed_semantic_hash()
                .map_err(|_| HazardClosureReason::RuleInvalid)?
            || policy.rules[..index]
                .iter()
                .any(|prior| prior.identity == rule.identity)
        {
            return Err(HazardClosureReason::IdentityMismatch);
        }
    }
    if policy.identity
        != policy
            .computed_semantic_hash()
            .map_err(|_| HazardClosureReason::InvalidDescriptor)?
    {
        return Err(HazardClosureReason::IdentityMismatch);
    }
    Ok(())
}

fn validate_rule(
    rule: ToxicCombinationRule<'_>,
    policy: HazardClosurePolicy<'_>,
) -> Result<(), HazardClosureReason> {
    if !valid_pin(rule.descriptor)
        || rule.patterns.is_empty()
        || rule.patterns.len() > usize::from(policy.limits.maximum_patterns_per_rule)
        || rule.patterns.len() > MAX_HAZARD_PATTERNS
        || rule.flows.len() > usize::from(policy.limits.maximum_flows)
    {
        return Err(HazardClosureReason::RuleInvalid);
    }
    for (index, pattern) in rule.patterns.iter().enumerate() {
        if Id::new(pattern.id.as_str()).is_err()
            || !valid_pin(pattern.class)
            || !policy
                .classes
                .iter()
                .any(|class| class.descriptor == pattern.class)
            || pattern
                .audience
                .is_some_and(|value| Id::new(value.as_str()).is_err())
            || pattern
                .host
                .is_some_and(|value| Id::new(value.as_str()).is_err())
            || pattern
                .realm
                .is_some_and(|value| Id::new(value.as_str()).is_err())
            || pattern.budget.is_some_and(|value| !valid_pin(value))
            || rule.patterns[..index]
                .iter()
                .any(|prior| prior.id == pattern.id)
        {
            return Err(HazardClosureReason::RuleInvalid);
        }
    }
    for flow in rule.flows {
        if usize::from(flow.from_pattern) >= rule.patterns.len()
            || usize::from(flow.to_pattern) >= rule.patterns.len()
            || flow.from_pattern == flow.to_pattern
            || !valid_pin(flow.transfer)
        {
            return Err(HazardClosureReason::RuleInvalid);
        }
    }
    Ok(())
}

fn validate_effects(
    primary: &[PlanAuthority<'_>],
    overlap: &[PlanAuthority<'_>],
) -> Result<(), HazardClosureReason> {
    let count = primary.len() + overlap.len();
    for index in 0..count {
        let authority = authority_at(primary, overlap, index);
        if Id::new(authority.effect.id.as_str()).is_err()
            || authority.effect.constraints.len() > crate::MAX_AUTHORITY_CONSTRAINTS
        {
            return Err(HazardClosureReason::InvalidDescriptor);
        }
        for prior in 0..index {
            if authority_at(primary, overlap, prior).effect.id == authority.effect.id {
                return Err(HazardClosureReason::InvalidDescriptor);
            }
        }
    }
    Ok(())
}

fn validate_flows(
    primary: &[PlanAuthority<'_>],
    overlap: &[PlanAuthority<'_>],
    primary_flows: &[EffectFlowBinding<'_>],
    overlap_flows: &[EffectFlowBinding<'_>],
) -> Result<(), HazardClosureReason> {
    let flow_count = primary_flows.len() + overlap_flows.len();
    for index in 0..flow_count {
        let flow = flow_at(primary_flows, overlap_flows, index);
        if flow.from_effect == flow.to_effect
            || !valid_pin(flow.transfer)
            || !contains_effect(primary, overlap, flow.from_effect)
            || !contains_effect(primary, overlap, flow.to_effect)
        {
            return Err(HazardClosureReason::FlowInvalid);
        }
        for prior in 0..index {
            if flow_at(primary_flows, overlap_flows, prior) == flow {
                return Err(HazardClosureReason::FlowInvalid);
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn search_rule(
    policy: HazardClosurePolicy<'_>,
    rule: ToxicCombinationRule<'_>,
    primary: &[PlanAuthority<'_>],
    overlap: &[PlanAuthority<'_>],
    primary_flows: &[EffectFlowBinding<'_>],
    overlap_flows: &[EffectFlowBinding<'_>],
    order: &[u8],
    pattern_index: usize,
    after: Option<&[u8; MAX_HAZARD_PATTERNS]>,
    selected: &mut [u8; MAX_HAZARD_PATTERNS],
    search_steps: &mut u32,
) -> Result<bool, HazardClosureReason> {
    if pattern_index == rule.patterns.len() {
        return Ok(after.is_none_or(|prior| {
            canonical_selection_after(selected, prior, order, rule.patterns.len())
        }) && flows_match(
            rule,
            primary,
            overlap,
            primary_flows,
            overlap_flows,
            selected,
        ));
    }
    for candidate in order {
        *search_steps = search_steps
            .checked_add(1)
            .ok_or(HazardClosureReason::SearchLimitExceeded)?;
        if *search_steps > policy.limits.maximum_search_steps {
            return Err(HazardClosureReason::SearchLimitExceeded);
        }
        if selected[..pattern_index].contains(candidate) {
            continue;
        }
        let authority = authority_at(primary, overlap, usize::from(*candidate));
        if !pattern_matches(policy, rule.patterns[pattern_index], authority) {
            continue;
        }
        selected[pattern_index] = *candidate;
        if search_rule(
            policy,
            rule,
            primary,
            overlap,
            primary_flows,
            overlap_flows,
            order,
            pattern_index + 1,
            after,
            selected,
            search_steps,
        )? {
            return Ok(true);
        }
        selected[pattern_index] = u8::MAX;
    }
    Ok(false)
}

fn canonical_selection_after(
    selected: &[u8; MAX_HAZARD_PATTERNS],
    prior: &[u8; MAX_HAZARD_PATTERNS],
    order: &[u8],
    pattern_count: usize,
) -> bool {
    for index in 0..pattern_count {
        let selected_rank = order.iter().position(|value| value == &selected[index]);
        let prior_rank = order.iter().position(|value| value == &prior[index]);
        match selected_rank.cmp(&prior_rank) {
            core::cmp::Ordering::Greater => return true,
            core::cmp::Ordering::Less => return false,
            core::cmp::Ordering::Equal => {}
        }
    }
    false
}

fn pattern_matches(
    policy: HazardClosurePolicy<'_>,
    pattern: ToxicEffectPattern<'_>,
    authority: &PlanAuthority<'_>,
) -> bool {
    let Some(class) = policy
        .classes
        .iter()
        .find(|class| class.descriptor == pattern.class)
    else {
        return false;
    };
    if !authority.effect.constraints.contains(&class.constraint)
        || pattern
            .resource
            .is_some_and(|selector| !resource_matches(selector, authority.effect.resource))
        || pattern
            .audience
            .is_some_and(|audience| audience != authority.effect.audience)
        || pattern
            .host
            .is_some_and(|host| host != authority.binding.host)
        || pattern.realm.is_some_and(|realm| {
            authority
                .administrative_subject
                .is_none_or(|subject| subject.realm != realm)
        })
        || pattern
            .budget
            .is_some_and(|budget| authority.effect.policy_budget_class != Some(budget))
    {
        return false;
    }
    let delegation = class.traits.delegation
        || authority.grant.delegation != DelegationPolicy::None
        || authority
            .containment
            .is_some_and(|proof| proof.proposal.delegation.is_some());
    pattern.persistence.matches(class.traits.persistence)
        && pattern.delegation.matches(delegation)
        && pattern.distributed.matches(class.traits.distributed)
        && pattern
            .administrative
            .matches(class.traits.administrative || authority.effect.administrative_class.is_some())
}

fn flows_match(
    rule: ToxicCombinationRule<'_>,
    primary: &[PlanAuthority<'_>],
    overlap: &[PlanAuthority<'_>],
    primary_flows: &[EffectFlowBinding<'_>],
    overlap_flows: &[EffectFlowBinding<'_>],
    selected: &[u8; MAX_HAZARD_PATTERNS],
) -> bool {
    rule.flows.iter().all(|required| {
        let from = authority_at(
            primary,
            overlap,
            usize::from(selected[usize::from(required.from_pattern)]),
        )
        .effect
        .id;
        let to = authority_at(
            primary,
            overlap,
            usize::from(selected[usize::from(required.to_pattern)]),
        )
        .effect
        .id;
        (0..primary_flows.len() + overlap_flows.len()).any(|index| {
            let flow = flow_at(primary_flows, overlap_flows, index);
            flow.from_effect == from && flow.to_effect == to && flow.transfer == required.transfer
        })
    })
}

fn validate_permit(
    permit: HazardPermit<'_>,
    policy: HazardClosurePolicy<'_>,
    rule: ToxicCombinationRule<'_>,
    context: HazardClosureContext<'_>,
    scope: SemanticHash,
) -> Result<(), HazardClosureReason> {
    if !valid_pin(permit.descriptor)
        || permit.policy_identity != policy.identity
        || permit.rule_identity != rule.identity
        || permit.plan_subject != context.plan_subject
        || permit.epoch != context.epoch
        || permit.scope_identity != scope
        || permit.time_basis != context.time.basis
    {
        return Err(HazardClosureReason::PermitScopeMismatch);
    }
    if permit.expires_at_tick <= permit.not_before_tick
        || context.time.tick < permit.not_before_tick
        || context.time.tick >= permit.expires_at_tick
    {
        return Err(HazardClosureReason::PermitExpired);
    }
    let scope_pin = PinnedDescriptor {
        id: permit.descriptor.id,
        schema_version: permit.descriptor.schema_version,
        semantic_hash: permit.scope_identity,
    };
    let subject = AdministrativeSubject {
        realm: permit.approval.proposal.subject.realm,
        entity: permit.approval.proposal.subject.entity,
        plan: permit.plan_subject,
        epoch: permit.epoch,
        artifact: permit.approval.proposal.subject.artifact,
        budget: Some(scope_pin),
    };
    if permit.approval.proposal.effect_class != policy.permit_class
        || permit.approval.policy.effect_class != policy.permit_class
        || permit.approval.proposal.operation != permit.descriptor
        || permit.approval.proposal.subject != subject
        || validate_administrative_proof(
            permit.approval,
            crate::ContainmentContext {
                subject,
                time_basis: context.time.basis,
                now_tick: context.time.tick,
            },
        )
        .is_err()
    {
        return Err(HazardClosureReason::PermitApprovalInvalid);
    }
    if permit.identity
        != permit
            .computed_semantic_hash()
            .map_err(|_| HazardClosureReason::PermitApprovalInvalid)?
    {
        return Err(HazardClosureReason::IdentityMismatch);
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct AuthoritySlices<'a> {
    primary: &'a [PlanAuthority<'a>],
    overlap: &'a [PlanAuthority<'a>],
}

fn write_match_proof<'a>(
    proof: &mut [Option<HazardProofNode<'a>>],
    written: &mut usize,
    rule: ToxicCombinationRule<'a>,
    authorities: AuthoritySlices<'a>,
    selected: &[u8; MAX_HAZARD_PATTERNS],
    permit: Option<HazardPermit<'a>>,
    maximum_proof_nodes: u8,
) -> Result<(), HazardClosureReason> {
    let needed = 1_usize
        .checked_add(rule.patterns.len())
        .and_then(|value| value.checked_add(rule.flows.len()))
        .and_then(|value| value.checked_add(usize::from(permit.is_some())))
        .ok_or(HazardClosureReason::ProofStorageExceeded)?;
    let end = written
        .checked_add(needed)
        .ok_or(HazardClosureReason::ProofStorageExceeded)?;
    if end > usize::from(maximum_proof_nodes) || end > proof.len() {
        return Err(HazardClosureReason::ProofStorageExceeded);
    }
    let root = u8::try_from(*written).map_err(|_| HazardClosureReason::ProofStorageExceeded)?;
    proof[*written] = Some(HazardProofNode {
        parent: None,
        kind: HazardProofKind::Rule,
        descriptor: rule.descriptor.id,
        effect: None,
    });
    *written += 1;
    for (pattern_index, pattern) in rule.patterns.iter().enumerate() {
        let authority = authority_at(
            authorities.primary,
            authorities.overlap,
            usize::from(selected[pattern_index]),
        );
        proof[*written] = Some(HazardProofNode {
            parent: Some(root),
            kind: HazardProofKind::Effect,
            descriptor: pattern.id,
            effect: Some(authority.effect.id),
        });
        *written += 1;
    }
    for flow in rule.flows {
        proof[*written] = Some(HazardProofNode {
            parent: Some(root),
            kind: HazardProofKind::Flow,
            descriptor: flow.transfer.id,
            effect: None,
        });
        *written += 1;
    }
    if let Some(permit) = permit {
        proof[*written] = Some(HazardProofNode {
            parent: Some(root),
            kind: HazardProofKind::Permit,
            descriptor: permit.descriptor.id,
            effect: None,
        });
        *written += 1;
    }
    Ok(())
}

fn match_denial<'a>(
    reason: HazardClosureReason,
    rule: ToxicCombinationRule<'a>,
    selected: &[u8; MAX_HAZARD_PATTERNS],
) -> HazardClosureDenial<'a> {
    HazardClosureDenial::for_match(
        reason,
        rule.descriptor.id,
        *selected,
        u8::try_from(rule.patterns.len()).unwrap_or(u8::MAX),
    )
}

fn match_scope_identity(
    rule: ToxicCombinationRule<'_>,
    primary: &[PlanAuthority<'_>],
    overlap: &[PlanAuthority<'_>],
    selected: &[u8; MAX_HAZARD_PATTERNS],
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let mut hashes = [SemanticHash::from_bytes([0; 32]); MAX_HAZARD_PATTERNS];
    for (index, selected) in selected[..rule.patterns.len()].iter().enumerate() {
        hashes[index] =
            authority_scope_hash(authority_at(primary, overlap, usize::from(*selected)))?;
    }
    semantic_hash_with_hash_set(
        Id("conduit/hazard-permit-scope"),
        1,
        &[semantic(
            "rule_identity",
            CanonicalValue::Bytes(rule.identity.as_bytes()),
        )],
        Id("effects"),
        &hashes[..rule.patterns.len()],
    )
}

/// Compute an exact permit scope for a policy-selected match.
pub fn effect_combination_scope(
    rule: ToxicCombinationRule<'_>,
    authorities: &[PlanAuthority<'_>],
    effects: &[Id<'_>],
) -> Result<SemanticHash, HazardClosureReason> {
    if effects.len() != rule.patterns.len() || effects.len() > MAX_HAZARD_PATTERNS {
        return Err(HazardClosureReason::PermitScopeMismatch);
    }
    let mut selected = [u8::MAX; MAX_HAZARD_PATTERNS];
    for (index, effect) in effects.iter().enumerate() {
        let position = authorities
            .iter()
            .position(|authority| authority.effect.id == *effect)
            .ok_or(HazardClosureReason::PermitScopeMismatch)?;
        selected[index] =
            u8::try_from(position).map_err(|_| HazardClosureReason::EffectLimitExceeded)?;
    }
    match_scope_identity(rule, authorities, &[], &selected)
        .map_err(|_| HazardClosureReason::IdentityMismatch)
}

fn closure_identity(
    primary: &[PlanAuthority<'_>],
    overlap: &[PlanAuthority<'_>],
    primary_flows: &[EffectFlowBinding<'_>],
    overlap_flows: &[EffectFlowBinding<'_>],
    context: HazardClosureContext<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let effect_count = primary.len() + overlap.len();
    let flow_count = primary_flows.len() + overlap_flows.len();
    if effect_count > MAX_HAZARD_EFFECTS || flow_count > MAX_HAZARD_FLOWS {
        return Err(CanonicalError::LengthOverflow);
    }
    let mut effects = [SemanticHash::from_bytes([0; 32]); MAX_HAZARD_EFFECTS];
    for (index, slot) in effects[..effect_count].iter_mut().enumerate() {
        *slot = authority_scope_hash(authority_at(primary, overlap, index))?;
    }
    let mut flows = [SemanticHash::from_bytes([0; 32]); MAX_HAZARD_FLOWS];
    for (index, slot) in flows[..flow_count].iter_mut().enumerate() {
        *slot = flow_hash(flow_at(primary_flows, overlap_flows, index))?;
    }
    let effect_set = semantic_hash_with_hash_set(
        Id("conduit/hazard-effect-set"),
        1,
        &[],
        Id("effects"),
        &effects[..effect_count],
    )?;
    semantic_hash_with_hash_set(
        Id("conduit/hazard-closure-subject"),
        1,
        &[
            semantic("effects", CanonicalValue::Bytes(effect_set.as_bytes())),
            semantic("epoch", CanonicalValue::Integer(i128::from(context.epoch))),
            semantic("time_basis", CanonicalValue::Identifier(context.time.basis)),
        ],
        Id("flows"),
        &flows[..flow_count],
    )
}

/// Compute the exact permit/plan subject before evaluating policy.
pub fn effect_closure_subject(
    authorities: &[PlanAuthority<'_>],
    flows: &[EffectFlowBinding<'_>],
    epoch: u64,
    time_basis: Id<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    closure_identity(
        authorities,
        &[],
        flows,
        &[],
        HazardClosureContext {
            plan_subject: SemanticHash::from_bytes([0; 32]),
            epoch,
            time: AuthorityTime {
                basis: time_basis,
                tick: 0,
            },
        },
    )
}

/// Compute the exact old/new/rollback overlap subject.
pub fn transition_effect_closure_subject(
    old_authorities: &[PlanAuthority<'_>],
    new_and_rollback_authorities: &[PlanAuthority<'_>],
    old_flows: &[EffectFlowBinding<'_>],
    new_and_rollback_flows: &[EffectFlowBinding<'_>],
    epoch: u64,
    time_basis: Id<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    closure_identity(
        old_authorities,
        new_and_rollback_authorities,
        old_flows,
        new_and_rollback_flows,
        HazardClosureContext {
            plan_subject: SemanticHash::from_bytes([0; 32]),
            epoch,
            time: AuthorityTime {
                basis: time_basis,
                tick: 0,
            },
        },
    )
}

fn decision_identity(
    policy: SemanticHash,
    closure: SemanticHash,
    context: HazardClosureContext<'_>,
    disposition: HazardClosureDisposition,
    matched_rules: u8,
    permits: &[SemanticHash],
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    semantic_hash_with_hash_set(
        Id("conduit/hazard-closure-decision"),
        1,
        &[
            semantic("policy", CanonicalValue::Bytes(policy.as_bytes())),
            semantic("closure", CanonicalValue::Bytes(closure.as_bytes())),
            semantic("epoch", CanonicalValue::Integer(i128::from(context.epoch))),
            semantic("time_basis", CanonicalValue::Identifier(context.time.basis)),
            semantic(
                "disposition",
                CanonicalValue::Identifier(Id(match disposition {
                    HazardClosureDisposition::Accepted => "accepted",
                    HazardClosureDisposition::Permitted => "permitted",
                })),
            ),
            semantic(
                "matched_rules",
                CanonicalValue::Integer(i128::from(matched_rules)),
            ),
        ],
        Id("permits"),
        permits,
    )
}

fn authority_scope_hash(
    authority: &PlanAuthority<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let (resource_kind, resource_id) = match authority.effect.resource {
        ResourceSelector::Exact(resource) => (resource.kind, Some(resource.id)),
        ResourceSelector::Kind(kind) => (kind, None),
    };
    let realm = authority
        .administrative_subject
        .map(|subject| subject.realm);
    let artifact = authority
        .administrative_subject
        .and_then(|subject| subject.artifact);
    let budget = authority
        .effect
        .policy_budget_class
        .map(hash_pin)
        .transpose()?;
    descriptor_hash(
        Id("conduit/hazard-effect-fact"),
        &[
            semantic(
                "effect_hash",
                CanonicalValue::Bytes(authority.effect_hash.as_bytes()),
            ),
            semantic("effect", CanonicalValue::Identifier(authority.effect.id)),
            semantic("node", CanonicalValue::Text(authority.node.as_str())),
            semantic(
                "action",
                CanonicalValue::Identifier(authority.effect.action),
            ),
            semantic("resource_kind", CanonicalValue::Identifier(resource_kind)),
            semantic(
                "resource_id",
                resource_id.map_or(CanonicalValue::Null, CanonicalValue::Identifier),
            ),
            semantic(
                "audience",
                CanonicalValue::Identifier(authority.effect.audience),
            ),
            semantic("host", CanonicalValue::Identifier(authority.binding.host)),
            semantic(
                "realm",
                realm.map_or(CanonicalValue::Null, CanonicalValue::Identifier),
            ),
            semantic(
                "artifact",
                artifact.as_ref().map_or(CanonicalValue::Null, |digest| {
                    CanonicalValue::Bytes(digest.as_bytes())
                }),
            ),
            semantic(
                "budget",
                budget.as_ref().map_or(CanonicalValue::Null, |hash| {
                    CanonicalValue::Bytes(hash.as_bytes())
                }),
            ),
            semantic(
                "grant",
                CanonicalValue::Bytes(authority.grant_hash.as_bytes()),
            ),
        ],
    )
}

fn flow_hash(flow: EffectFlowBinding<'_>) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let transfer = hash_pin(flow.transfer)?;
    descriptor_hash(
        Id("conduit/hazard-effect-flow"),
        &[
            semantic("from", CanonicalValue::Identifier(flow.from_effect)),
            semantic("to", CanonicalValue::Identifier(flow.to_effect)),
            semantic("transfer", CanonicalValue::Bytes(transfer.as_bytes())),
        ],
    )
}

impl EffectClassBinding<'_> {
    pub fn computed_semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        let descriptor = hash_pin(self.descriptor)?;
        descriptor_hash(
            Id("conduit/hazard-effect-class-binding"),
            &[
                semantic("descriptor", CanonicalValue::Bytes(descriptor.as_bytes())),
                semantic(
                    "constraint_id",
                    CanonicalValue::Identifier(self.constraint.id),
                ),
                semantic(
                    "constraint_hash",
                    CanonicalValue::Bytes(self.constraint.semantic_hash.as_bytes()),
                ),
                semantic(
                    "persistence",
                    CanonicalValue::Boolean(self.traits.persistence),
                ),
                semantic(
                    "delegation",
                    CanonicalValue::Boolean(self.traits.delegation),
                ),
                semantic(
                    "distributed",
                    CanonicalValue::Boolean(self.traits.distributed),
                ),
                semantic(
                    "administrative",
                    CanonicalValue::Boolean(self.traits.administrative),
                ),
            ],
        )
    }
}

impl ToxicCombinationRule<'_> {
    pub fn computed_semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        if self.patterns.len() > MAX_HAZARD_PATTERNS || self.flows.len() > MAX_HAZARD_FLOWS {
            return Err(CanonicalError::LengthOverflow);
        }
        let mut patterns = [SemanticHash::from_bytes([0; 32]); MAX_HAZARD_PATTERNS];
        for (index, pattern) in self.patterns.iter().enumerate() {
            patterns[index] = pattern_hash(*pattern)?;
        }
        let mut flows = [SemanticHash::from_bytes([0; 32]); MAX_HAZARD_FLOWS];
        for (index, flow) in self.flows.iter().enumerate() {
            flows[index] = required_flow_hash(*flow)?;
        }
        let descriptor = hash_pin(self.descriptor)?;
        let mut pattern_values = [CanonicalValue::Null; MAX_HAZARD_PATTERNS];
        for (index, hash) in patterns[..self.patterns.len()].iter().enumerate() {
            pattern_values[index] = CanonicalValue::Bytes(hash.as_bytes());
        }
        let pattern_sequence = descriptor_hash(
            Id("conduit/toxic-pattern-sequence"),
            &[semantic(
                "patterns",
                CanonicalValue::List(&pattern_values[..self.patterns.len()]),
            )],
        )?;
        semantic_hash_with_hash_set(
            Id("conduit/toxic-combination-rule"),
            1,
            &[
                semantic("descriptor", CanonicalValue::Bytes(descriptor.as_bytes())),
                semantic(
                    "patterns",
                    CanonicalValue::Bytes(pattern_sequence.as_bytes()),
                ),
            ],
            Id("flows"),
            &flows[..self.flows.len()],
        )
    }
}

impl HazardClosurePolicy<'_> {
    pub fn computed_semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        if self.classes.len() > MAX_HAZARD_CLASSES || self.rules.len() > MAX_HAZARD_RULES {
            return Err(CanonicalError::LengthOverflow);
        }
        let mut classes = [SemanticHash::from_bytes([0; 32]); MAX_HAZARD_CLASSES];
        for (index, class) in self.classes.iter().enumerate() {
            classes[index] = class.identity;
        }
        let mut rules = [SemanticHash::from_bytes([0; 32]); MAX_HAZARD_RULES];
        for (index, rule) in self.rules.iter().enumerate() {
            rules[index] = rule.identity;
        }
        let descriptor = hash_pin(self.descriptor)?;
        let permit_class = hash_pin(self.permit_class)?;
        let rule_set = semantic_hash_with_hash_set(
            Id("conduit/toxic-rule-set"),
            1,
            &[],
            Id("rules"),
            &rules[..self.rules.len()],
        )?;
        semantic_hash_with_hash_set(
            Id("conduit/hazard-closure-policy"),
            self.schema_version,
            &[
                semantic("descriptor", CanonicalValue::Bytes(descriptor.as_bytes())),
                semantic(
                    "permit_class",
                    CanonicalValue::Bytes(permit_class.as_bytes()),
                ),
                semantic("rules", CanonicalValue::Bytes(rule_set.as_bytes())),
                semantic(
                    "maximum_effects",
                    CanonicalValue::Integer(i128::from(self.limits.maximum_effects)),
                ),
                semantic(
                    "maximum_classes",
                    CanonicalValue::Integer(i128::from(self.limits.maximum_classes)),
                ),
                semantic(
                    "maximum_rules",
                    CanonicalValue::Integer(i128::from(self.limits.maximum_rules)),
                ),
                semantic(
                    "maximum_patterns_per_rule",
                    CanonicalValue::Integer(i128::from(self.limits.maximum_patterns_per_rule)),
                ),
                semantic(
                    "maximum_flows",
                    CanonicalValue::Integer(i128::from(self.limits.maximum_flows)),
                ),
                semantic(
                    "maximum_permits",
                    CanonicalValue::Integer(i128::from(self.limits.maximum_permits)),
                ),
                semantic(
                    "maximum_proof_nodes",
                    CanonicalValue::Integer(i128::from(self.limits.maximum_proof_nodes)),
                ),
                semantic(
                    "maximum_search_steps",
                    CanonicalValue::Integer(i128::from(self.limits.maximum_search_steps)),
                ),
            ],
            Id("classes"),
            &classes[..self.classes.len()],
        )
    }
}

impl HazardPermit<'_> {
    pub fn computed_semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        let descriptor = hash_pin(self.descriptor)?;
        descriptor_hash(
            Id("conduit/hazard-permit"),
            &[
                semantic("descriptor", CanonicalValue::Bytes(descriptor.as_bytes())),
                semantic(
                    "policy_identity",
                    CanonicalValue::Bytes(self.policy_identity.as_bytes()),
                ),
                semantic(
                    "rule_identity",
                    CanonicalValue::Bytes(self.rule_identity.as_bytes()),
                ),
                semantic(
                    "plan_subject",
                    CanonicalValue::Bytes(self.plan_subject.as_bytes()),
                ),
                semantic("epoch", CanonicalValue::Integer(i128::from(self.epoch))),
                semantic(
                    "scope_identity",
                    CanonicalValue::Bytes(self.scope_identity.as_bytes()),
                ),
                semantic("time_basis", CanonicalValue::Identifier(self.time_basis)),
                semantic(
                    "not_before_tick",
                    CanonicalValue::Integer(i128::from(self.not_before_tick)),
                ),
                semantic(
                    "expires_at_tick",
                    CanonicalValue::Integer(i128::from(self.expires_at_tick)),
                ),
                semantic(
                    "approval_execution",
                    CanonicalValue::Bytes(self.approval.execution.identity.as_bytes()),
                ),
            ],
        )
    }
}

fn pattern_hash(
    pattern: ToxicEffectPattern<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let class = hash_pin(pattern.class)?;
    let resource = match pattern.resource {
        None => None,
        Some(ResourceSelector::Kind(kind)) => Some(descriptor_hash(
            Id("conduit/hazard-resource-selector"),
            &[
                semantic("kind", CanonicalValue::Identifier(kind)),
                semantic("id", CanonicalValue::Null),
            ],
        )?),
        Some(ResourceSelector::Exact(resource)) => Some(descriptor_hash(
            Id("conduit/hazard-resource-selector"),
            &[
                semantic("kind", CanonicalValue::Identifier(resource.kind)),
                semantic("id", CanonicalValue::Identifier(resource.id)),
            ],
        )?),
    };
    let budget = pattern.budget.map(hash_pin).transpose()?;
    descriptor_hash(
        Id("conduit/toxic-effect-pattern"),
        &[
            semantic("id", CanonicalValue::Identifier(pattern.id)),
            semantic("class", CanonicalValue::Bytes(class.as_bytes())),
            semantic(
                "resource",
                resource.as_ref().map_or(CanonicalValue::Null, |hash| {
                    CanonicalValue::Bytes(hash.as_bytes())
                }),
            ),
            semantic(
                "audience",
                pattern
                    .audience
                    .map_or(CanonicalValue::Null, CanonicalValue::Identifier),
            ),
            semantic(
                "host",
                pattern
                    .host
                    .map_or(CanonicalValue::Null, CanonicalValue::Identifier),
            ),
            semantic(
                "realm",
                pattern
                    .realm
                    .map_or(CanonicalValue::Null, CanonicalValue::Identifier),
            ),
            semantic(
                "budget",
                budget.as_ref().map_or(CanonicalValue::Null, |hash| {
                    CanonicalValue::Bytes(hash.as_bytes())
                }),
            ),
            semantic(
                "persistence",
                CanonicalValue::Identifier(Id(pattern.persistence.as_str())),
            ),
            semantic(
                "delegation",
                CanonicalValue::Identifier(Id(pattern.delegation.as_str())),
            ),
            semantic(
                "distributed",
                CanonicalValue::Identifier(Id(pattern.distributed.as_str())),
            ),
            semantic(
                "administrative",
                CanonicalValue::Identifier(Id(pattern.administrative.as_str())),
            ),
        ],
    )
}

fn required_flow_hash(
    flow: ToxicFlowRequirement<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let transfer = hash_pin(flow.transfer)?;
    descriptor_hash(
        Id("conduit/toxic-flow-requirement"),
        &[
            semantic(
                "from_pattern",
                CanonicalValue::Integer(i128::from(flow.from_pattern)),
            ),
            semantic(
                "to_pattern",
                CanonicalValue::Integer(i128::from(flow.to_pattern)),
            ),
            semantic("transfer", CanonicalValue::Bytes(transfer.as_bytes())),
        ],
    )
}

fn hash_pin(pin: PinnedDescriptor<'_>) -> Result<SemanticHash, CanonicalError<Infallible>> {
    descriptor_hash(
        Id("conduit/pinned-descriptor"),
        &[
            semantic("id", CanonicalValue::Identifier(pin.id)),
            semantic(
                "schema_version",
                CanonicalValue::Integer(i128::from(pin.schema_version)),
            ),
            semantic(
                "semantic_hash",
                CanonicalValue::Bytes(pin.semantic_hash.as_bytes()),
            ),
        ],
    )
}

fn resource_matches(required: ResourceSelector<'_>, actual: ResourceSelector<'_>) -> bool {
    match (required, actual) {
        (ResourceSelector::Exact(required), ResourceSelector::Exact(actual)) => required == actual,
        (ResourceSelector::Kind(required), ResourceSelector::Kind(actual)) => required == actual,
        (ResourceSelector::Kind(required), ResourceSelector::Exact(actual)) => {
            required == actual.kind
        }
        (ResourceSelector::Exact(_), ResourceSelector::Kind(_)) => false,
    }
}

fn valid_pin(pin: PinnedDescriptor<'_>) -> bool {
    Id::new(pin.id.as_str()).is_ok()
        && pin.schema_version > 0
        && pin.semantic_hash != SemanticHash::from_bytes([0; 32])
}

fn contains_effect(
    primary: &[PlanAuthority<'_>],
    overlap: &[PlanAuthority<'_>],
    effect: Id<'_>,
) -> bool {
    primary
        .iter()
        .chain(overlap)
        .any(|authority| authority.effect.id == effect)
}

fn authority_at<'a>(
    primary: &'a [PlanAuthority<'a>],
    overlap: &'a [PlanAuthority<'a>],
    index: usize,
) -> &'a PlanAuthority<'a> {
    if index < primary.len() {
        &primary[index]
    } else {
        &overlap[index - primary.len()]
    }
}

fn flow_at<'a>(
    primary: &'a [EffectFlowBinding<'a>],
    overlap: &'a [EffectFlowBinding<'a>],
    index: usize,
) -> EffectFlowBinding<'a> {
    if index < primary.len() {
        primary[index]
    } else {
        overlap[index - primary.len()]
    }
}

const fn semantic<'a>(name: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    }
}

fn descriptor_hash(
    kind: Id<'_>,
    fields: &[MapField<'_>],
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    CanonicalDescriptor {
        kind,
        schema_version: 1,
        body: CanonicalValue::Map(fields),
    }
    .semantic_hash()
}
