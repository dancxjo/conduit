//! Scoped authority contracts, deterministic binding, and redacted evidence.

use core::cmp::Ordering;
use core::convert::Infallible;
use core::fmt;

use crate::{
    CanonicalDescriptor, CanonicalError, CanonicalValue, FieldDisposition, Id, InstancePath,
    MapField, PinnedDescriptor, SemanticHash, Sensitivity, StopPolicy, TerminalCause,
    TerminalCauseCode, TypeContractRef,
};

/// Maximum constraint references in the allocator-free v1 descriptor.
pub const MAX_AUTHORITY_CONSTRAINTS: usize = 8;

/// One concrete host resource.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceRef<'a> {
    pub kind: Id<'a>,
    pub id: Id<'a>,
}

/// Resource set requested by an effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceSelector<'a> {
    Exact(ResourceRef<'a>),
    Kind(Id<'a>),
}

/// Domain-owned immutable constraint descriptor reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthorityConstraintRef<'a> {
    pub id: Id<'a>,
    pub semantic_hash: SemanticHash,
}

/// A semantic effect or authority requirement declared by a node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectRequirement<'a> {
    pub id: Id<'a>,
    /// Domain-owned class requiring administrative containment. `None` is an
    /// ordinary effect and preserves the v1 requirement identity.
    pub administrative_class: Option<PinnedDescriptor<'a>>,
    pub action: Id<'a>,
    pub resource: ResourceSelector<'a>,
    pub requester: InstancePath<'a>,
    pub audience: Id<'a>,
    pub constraints: &'a [AuthorityConstraintRef<'a>],
    pub check_at_use: bool,
}

/// Fresh host observation. Availability never grants permission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostCapability<'a> {
    pub id: Id<'a>,
    pub action: Id<'a>,
    pub resource: ResourceRef<'a>,
    pub host: Id<'a>,
    pub time_basis: Id<'a>,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
}

/// One deterministic observation from a named monotonic time basis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthorityTime<'a> {
    pub basis: Id<'a>,
    pub tick: u64,
}

/// Exact scope of one grant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthorityScope<'a> {
    pub root: InstancePath<'a>,
    pub descendants: bool,
}

/// Whether authority may cross instance or host boundaries.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum DelegationPolicy {
    None,
    SameHostDescendants,
    CrossHostDescendants,
}

impl DelegationPolicy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::SameHostDescendants => "same-host-descendants",
            Self::CrossHostDescendants => "cross-host-descendants",
        }
    }
}

/// Immutable authorization descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthorityGrant<'a> {
    pub id: Id<'a>,
    pub action: Id<'a>,
    pub resource: ResourceRef<'a>,
    pub scope: AuthorityScope<'a>,
    pub audience: Id<'a>,
    pub constraints: &'a [AuthorityConstraintRef<'a>],
    pub time_basis: Id<'a>,
    pub not_before_tick: u64,
    pub expires_at_tick: u64,
    pub issued_for_host: Id<'a>,
    pub delegation: DelegationPolicy,
    pub audit_id: Id<'a>,
    pub terminal_policy: StopPolicy,
}

/// Fresh revocation observation, separate from immutable grant identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrantStatus<'a> {
    Active,
    Revoked { at_tick: u64, reason: Id<'a> },
}

/// One grant plus its current status observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservedGrant<'a> {
    pub grant: AuthorityGrant<'a>,
    pub status: GrantStatus<'a>,
}

/// Exact plan binding of effect, capability, resource, and grant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedAuthorityBinding<'a> {
    pub effect_id: Id<'a>,
    pub capability_id: Id<'a>,
    pub grant_id: Id<'a>,
    pub resource: ResourceRef<'a>,
    pub host: Id<'a>,
    pub audit_id: Id<'a>,
    pub time_basis: Id<'a>,
    pub validated_at_tick: u64,
    pub check_at_use: bool,
}

/// One effect placed on an exact host before authority-plan resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlacedEffect<'a> {
    pub effect: EffectRequirement<'a>,
    pub host: Id<'a>,
}

/// Effects owned by one primitive or already-aggregated composite child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeEffectSet<'a> {
    pub definition: Id<'a>,
    pub instance: InstancePath<'a>,
    pub effects: &'a [EffectRequirement<'a>],
}

/// Stable reason an authority query failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorityReason {
    CapabilityMissing,
    CapabilityStale,
    GrantMissing,
    ActionMismatch,
    ResourceMismatch,
    ScopeMismatch,
    AudienceMismatch,
    ConstraintMismatch,
    TimeBasisMismatch,
    NotYetValid,
    Expired,
    Revoked,
    DelegationDenied,
    InvalidDescriptor,
    BindingMismatch,
    StorageTooSmall,
}

impl AuthorityReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CapabilityMissing => "capability-missing",
            Self::CapabilityStale => "capability-stale",
            Self::GrantMissing => "grant-missing",
            Self::ActionMismatch => "grant-action-mismatch",
            Self::ResourceMismatch => "grant-resource-mismatch",
            Self::ScopeMismatch => "grant-scope-mismatch",
            Self::AudienceMismatch => "grant-audience-mismatch",
            Self::ConstraintMismatch => "grant-constraint-mismatch",
            Self::TimeBasisMismatch => "authority-time-basis-mismatch",
            Self::NotYetValid => "grant-not-yet-valid",
            Self::Expired => "grant-expired",
            Self::Revoked => "grant-revoked",
            Self::DelegationDenied => "grant-delegation-denied",
            Self::InvalidDescriptor => "authority-descriptor-invalid",
            Self::BindingMismatch => "authority-binding-mismatch",
            Self::StorageTooSmall => "authority-storage-too-small",
        }
    }
}

/// Secret-safe structured denial containing the requesting path and missing fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthorityDenial<'a> {
    pub effect_id: Id<'a>,
    pub requester: InstancePath<'a>,
    pub action: Id<'a>,
    pub reason: AuthorityReason,
}

/// Immutable authority evidence payload; issue #12 owns the common envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthorityEvent<'a> {
    pub sequence: u64,
    pub effect_id: Id<'a>,
    pub requester: InstancePath<'a>,
    pub action: Id<'a>,
    pub grant_id: Option<Id<'a>>,
    pub audit_id: Option<Id<'a>>,
    pub kind: AuthorityEventKind,
}

/// Stable authority evidence kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorityEventKind {
    Bound,
    UseAuthorized,
    Denied(AuthorityReason),
    Revoked,
    Expired,
}

/// Build successful binding evidence without carrying protected values.
#[must_use]
pub const fn authority_bound_event<'a>(
    sequence: u64,
    effect: EffectRequirement<'a>,
    binding: ResolvedAuthorityBinding<'a>,
) -> AuthorityEvent<'a> {
    AuthorityEvent {
        sequence,
        effect_id: effect.id,
        requester: effect.requester,
        action: effect.action,
        grant_id: Some(binding.grant_id),
        audit_id: Some(binding.audit_id),
        kind: AuthorityEventKind::Bound,
    }
}

/// Build denial/revocation/expiry evidence without value material.
#[must_use]
pub const fn authority_denial_event<'a>(
    sequence: u64,
    denial: AuthorityDenial<'a>,
    grant: Option<AuthorityGrant<'a>>,
) -> AuthorityEvent<'a> {
    let kind = match denial.reason {
        AuthorityReason::Revoked => AuthorityEventKind::Revoked,
        AuthorityReason::Expired => AuthorityEventKind::Expired,
        reason => AuthorityEventKind::Denied(reason),
    };
    AuthorityEvent {
        sequence,
        effect_id: denial.effect_id,
        requester: denial.requester,
        action: denial.action,
        grant_id: match grant {
            Some(grant) => Some(grant.id),
            None => None,
        },
        audit_id: match grant {
            Some(grant) => Some(grant.audit_id),
            None => None,
        },
        kind,
    }
}

impl AuthorityDenial<'_> {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self.reason {
            AuthorityReason::CapabilityMissing | AuthorityReason::CapabilityStale => "CND-HST-001",
            AuthorityReason::GrantMissing => "CND-AUT-001",
            AuthorityReason::ActionMismatch
            | AuthorityReason::ResourceMismatch
            | AuthorityReason::ScopeMismatch
            | AuthorityReason::AudienceMismatch
            | AuthorityReason::ConstraintMismatch
            | AuthorityReason::TimeBasisMismatch
            | AuthorityReason::NotYetValid
            | AuthorityReason::DelegationDenied => "CND-AUT-002",
            AuthorityReason::Expired | AuthorityReason::Revoked => "CND-AUT-003",
            AuthorityReason::InvalidDescriptor => "CND-AUT-004",
            AuthorityReason::BindingMismatch => "CND-AUT-005",
            AuthorityReason::StorageTooSmall => "CND-AUT-006",
        }
    }
}

impl fmt::Display for AuthorityDenial<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: effect `{}` at `{}` requires action `{}`: {}",
            self.code(),
            self.effect_id,
            self.requester.as_str(),
            self.action,
            self.reason.as_str()
        )
    }
}

/// Resolve one effect without treating host capability as permission.
pub fn resolve_authority<'a>(
    effect: EffectRequirement<'a>,
    host: Id<'a>,
    time: AuthorityTime<'a>,
    capabilities: &[HostCapability<'a>],
    grants: &[ObservedGrant<'a>],
) -> Result<ResolvedAuthorityBinding<'a>, AuthorityDenial<'a>> {
    validate_effect(effect).map_err(|reason| denial(effect, reason))?;
    if !valid_id(host)
        || !valid_id(time.basis)
        || capabilities
            .iter()
            .any(|capability| validate_capability(*capability).is_err())
        || grants
            .iter()
            .any(|grant| validate_observed_grant(*grant).is_err())
    {
        return Err(denial(effect, AuthorityReason::InvalidDescriptor));
    }

    let mut saw_capability = false;
    let mut saw_fresh_capability = false;
    let mut best: Option<(HostCapability<'a>, AuthorityGrant<'a>)> = None;
    let mut best_denial = AuthorityReason::GrantMissing;

    for capability in capabilities {
        if capability.host != host
            || capability.action != effect.action
            || !selector_matches(effect.resource, capability.resource)
        {
            continue;
        }
        saw_capability = true;
        if capability.time_basis != time.basis
            || capability.observed_at_tick > time.tick
            || time.tick >= capability.valid_until_tick
        {
            continue;
        }
        saw_fresh_capability = true;
        for observed in grants {
            match assess_grant(effect, host, time, capability.resource, *observed) {
                Ok(()) => {
                    let candidate = (*capability, observed.grant);
                    if best
                        .is_none_or(|current| binding_order(candidate, current) == Ordering::Less)
                    {
                        best = Some(candidate);
                    }
                }
                Err(reason) => best_denial = stronger_reason(best_denial, reason),
            }
        }
    }

    let Some((capability, grant)) = best else {
        let reason = if !saw_capability {
            AuthorityReason::CapabilityMissing
        } else if !saw_fresh_capability {
            AuthorityReason::CapabilityStale
        } else {
            best_denial
        };
        return Err(denial(effect, reason));
    };
    Ok(ResolvedAuthorityBinding {
        effect_id: effect.id,
        capability_id: capability.id,
        grant_id: grant.id,
        resource: capability.resource,
        host,
        audit_id: grant.audit_id,
        time_basis: time.basis,
        validated_at_tick: time.tick,
        check_at_use: effect.check_at_use,
    })
}

/// Resolve every placed effect into caller-owned exact-plan bindings.
///
/// Failure clears every binding written by this call, so no partial plan can
/// acquire authority.
pub fn resolve_authority_plan<'a>(
    effects: &[PlacedEffect<'a>],
    time: AuthorityTime<'a>,
    capabilities: &[HostCapability<'a>],
    grants: &[ObservedGrant<'a>],
    bindings: &mut [Option<ResolvedAuthorityBinding<'a>>],
) -> Result<usize, AuthorityDenial<'a>> {
    if effects.is_empty() {
        return Ok(0);
    }
    for slot in bindings.iter_mut().take(effects.len()) {
        *slot = None;
    }
    if bindings.len() < effects.len() {
        return Err(denial(effects[0].effect, AuthorityReason::StorageTooSmall));
    }
    for (index, placed) in effects.iter().enumerate() {
        if effects[..index]
            .iter()
            .any(|prior| prior.effect.id == placed.effect.id)
        {
            return Err(denial(placed.effect, AuthorityReason::InvalidDescriptor));
        }
    }
    for (index, placed) in effects.iter().enumerate() {
        match resolve_authority(placed.effect, placed.host, time, capabilities, grants) {
            Ok(binding) => bindings[index] = Some(binding),
            Err(error) => {
                for slot in &mut bindings[..index] {
                    *slot = None;
                }
                return Err(error);
            }
        }
    }
    Ok(effects.len())
}

fn assess_grant(
    effect: EffectRequirement<'_>,
    host: Id<'_>,
    time: AuthorityTime<'_>,
    resource: ResourceRef<'_>,
    observed: ObservedGrant<'_>,
) -> Result<(), AuthorityReason> {
    let grant = observed.grant;
    validate_grant(grant)?;
    if grant.action != effect.action {
        return Err(AuthorityReason::ActionMismatch);
    }
    if grant.resource != resource {
        return Err(AuthorityReason::ResourceMismatch);
    }
    if grant.audience != effect.audience {
        return Err(AuthorityReason::AudienceMismatch);
    }
    if !same_constraints(effect.constraints, grant.constraints) {
        return Err(AuthorityReason::ConstraintMismatch);
    }
    if grant.time_basis != time.basis {
        return Err(AuthorityReason::TimeBasisMismatch);
    }
    if time.tick < grant.not_before_tick {
        return Err(AuthorityReason::NotYetValid);
    }
    if time.tick >= grant.expires_at_tick {
        return Err(AuthorityReason::Expired);
    }
    if let GrantStatus::Revoked { at_tick, .. } = observed.status {
        if time.tick >= at_tick {
            return Err(AuthorityReason::Revoked);
        }
    }
    let requester = effect.requester.as_str();
    let root = grant.scope.root.as_str();
    let descendant = path_is_descendant(requester, root);
    if requester != root && (!grant.scope.descendants || !descendant) {
        return Err(AuthorityReason::ScopeMismatch);
    }
    if requester != root {
        let needed = if host == grant.issued_for_host {
            DelegationPolicy::SameHostDescendants
        } else {
            DelegationPolicy::CrossHostDescendants
        };
        if grant.delegation < needed {
            return Err(AuthorityReason::DelegationDenied);
        }
    } else if host != grant.issued_for_host
        && grant.delegation != DelegationPolicy::CrossHostDescendants
    {
        return Err(AuthorityReason::DelegationDenied);
    }
    Ok(())
}

/// Revalidate a pinned binding at use time.
pub fn validate_authority_at_use<'a>(
    binding: ResolvedAuthorityBinding<'a>,
    effect: EffectRequirement<'a>,
    time: AuthorityTime<'a>,
    capability: HostCapability<'a>,
    grant: ObservedGrant<'a>,
) -> Result<(), AuthorityDenial<'a>> {
    if validate_effect(effect).is_err()
        || validate_capability(capability).is_err()
        || validate_observed_grant(grant).is_err()
        || !valid_id(time.basis)
    {
        return Err(denial(effect, AuthorityReason::InvalidDescriptor));
    }
    if binding.effect_id != effect.id
        || binding.capability_id != capability.id
        || binding.grant_id != grant.grant.id
        || binding.resource != capability.resource
        || binding.host != capability.host
        || binding.audit_id != grant.grant.audit_id
        || binding.time_basis != time.basis
        || capability.action != effect.action
        || !selector_matches(effect.resource, capability.resource)
    {
        return Err(denial(effect, AuthorityReason::BindingMismatch));
    }
    if capability.time_basis != time.basis {
        return Err(denial(effect, AuthorityReason::TimeBasisMismatch));
    }
    if capability.observed_at_tick > time.tick || time.tick >= capability.valid_until_tick {
        return Err(denial(effect, AuthorityReason::CapabilityStale));
    }
    assess_grant(effect, binding.host, time, binding.resource, grant)
        .map_err(|reason| denial(effect, reason))
}

fn validate_effect(effect: EffectRequirement<'_>) -> Result<(), AuthorityReason> {
    if !valid_id(effect.id) || !valid_id(effect.action) || !valid_id(effect.audience) {
        return Err(AuthorityReason::InvalidDescriptor);
    }
    match effect.resource {
        ResourceSelector::Exact(resource) => validate_resource(resource)?,
        ResourceSelector::Kind(kind) if !valid_id(kind) => {
            return Err(AuthorityReason::InvalidDescriptor);
        }
        ResourceSelector::Kind(_) => {}
    }
    validate_constraints(effect.constraints)?;
    Ok(())
}

fn validate_grant(grant: AuthorityGrant<'_>) -> Result<(), AuthorityReason> {
    if !valid_id(grant.id)
        || !valid_id(grant.action)
        || !valid_id(grant.audience)
        || !valid_id(grant.time_basis)
        || !valid_id(grant.issued_for_host)
        || !valid_id(grant.audit_id)
        || grant.expires_at_tick <= grant.not_before_tick
    {
        return Err(AuthorityReason::InvalidDescriptor);
    }
    validate_resource(grant.resource)?;
    validate_constraints(grant.constraints)
}

fn validate_capability(capability: HostCapability<'_>) -> Result<(), AuthorityReason> {
    if !valid_id(capability.id)
        || !valid_id(capability.action)
        || !valid_id(capability.host)
        || !valid_id(capability.time_basis)
        || capability.valid_until_tick <= capability.observed_at_tick
    {
        return Err(AuthorityReason::InvalidDescriptor);
    }
    validate_resource(capability.resource)
}

fn validate_observed_grant(observed: ObservedGrant<'_>) -> Result<(), AuthorityReason> {
    validate_grant(observed.grant)?;
    if let GrantStatus::Revoked { reason, .. } = observed.status {
        if !valid_id(reason) {
            return Err(AuthorityReason::InvalidDescriptor);
        }
    }
    Ok(())
}

fn validate_resource(resource: ResourceRef<'_>) -> Result<(), AuthorityReason> {
    if valid_id(resource.kind) && valid_id(resource.id) {
        Ok(())
    } else {
        Err(AuthorityReason::InvalidDescriptor)
    }
}

fn validate_constraints(constraints: &[AuthorityConstraintRef<'_>]) -> Result<(), AuthorityReason> {
    if constraints.len() > MAX_AUTHORITY_CONSTRAINTS {
        return Err(AuthorityReason::InvalidDescriptor);
    }
    for (index, constraint) in constraints.iter().enumerate() {
        if !valid_id(constraint.id) {
            return Err(AuthorityReason::InvalidDescriptor);
        }
        if constraints[..index].iter().any(|prior| prior == constraint) {
            return Err(AuthorityReason::InvalidDescriptor);
        }
    }
    Ok(())
}

fn valid_id(id: Id<'_>) -> bool {
    Id::new(id.as_str()).is_ok()
}

fn same_constraints(
    required: &[AuthorityConstraintRef<'_>],
    granted: &[AuthorityConstraintRef<'_>],
) -> bool {
    required.len() == granted.len()
        && required
            .iter()
            .all(|constraint| granted.contains(constraint))
}

fn selector_matches(selector: ResourceSelector<'_>, resource: ResourceRef<'_>) -> bool {
    match selector {
        ResourceSelector::Exact(exact) => exact == resource,
        ResourceSelector::Kind(kind) => kind == resource.kind,
    }
}

fn path_is_descendant(path: &str, root: &str) -> bool {
    path.strip_prefix(root)
        .is_some_and(|suffix| suffix.starts_with('/'))
}

fn binding_order(
    left: (HostCapability<'_>, AuthorityGrant<'_>),
    right: (HostCapability<'_>, AuthorityGrant<'_>),
) -> Ordering {
    left.0
        .resource
        .kind
        .as_str()
        .cmp(right.0.resource.kind.as_str())
        .then_with(|| {
            left.0
                .resource
                .id
                .as_str()
                .cmp(right.0.resource.id.as_str())
        })
        .then_with(|| left.1.id.as_str().cmp(right.1.id.as_str()))
        .then_with(|| left.0.id.as_str().cmp(right.0.id.as_str()))
}

fn stronger_reason(current: AuthorityReason, candidate: AuthorityReason) -> AuthorityReason {
    if reason_rank(candidate) > reason_rank(current) {
        candidate
    } else {
        current
    }
}

const fn reason_rank(reason: AuthorityReason) -> u8 {
    match reason {
        AuthorityReason::Revoked => 11,
        AuthorityReason::Expired => 10,
        AuthorityReason::DelegationDenied => 9,
        AuthorityReason::ScopeMismatch => 8,
        AuthorityReason::AudienceMismatch => 7,
        AuthorityReason::ConstraintMismatch => 6,
        AuthorityReason::TimeBasisMismatch => 6,
        AuthorityReason::ResourceMismatch => 5,
        AuthorityReason::ActionMismatch => 4,
        AuthorityReason::NotYetValid => 3,
        AuthorityReason::InvalidDescriptor => 2,
        AuthorityReason::GrantMissing => 1,
        AuthorityReason::CapabilityMissing
        | AuthorityReason::CapabilityStale
        | AuthorityReason::BindingMismatch
        | AuthorityReason::StorageTooSmall => 0,
    }
}

fn denial(effect: EffectRequirement<'_>, reason: AuthorityReason) -> AuthorityDenial<'_> {
    AuthorityDenial {
        effect_id: effect.id,
        requester: effect.requester,
        action: effect.action,
        reason,
    }
}

/// Convert revocation or expiry denial into the lifecycle cause contract.
#[must_use]
pub fn authority_terminal_cause<'a>(
    denial: AuthorityDenial<'a>,
    grant: AuthorityGrant<'a>,
) -> Option<TerminalCause<'a>> {
    let code = match denial.reason {
        AuthorityReason::Revoked => TerminalCauseCode::AuthorityRevoked,
        AuthorityReason::Expired => TerminalCauseCode::DeadlineExpired,
        _ => return None,
    };
    Some(TerminalCause {
        code,
        subject: denial.effect_id,
        caused_by: None,
        stop: grant.terminal_policy,
    })
}

/// Derive a composite's effects from every immediate child effect set.
///
/// Each effect must name its owning child path or a descendant. Export lists
/// are deliberately absent and therefore cannot hide authority requirements.
pub fn aggregate_composite_effect_sets<'a>(
    children: &[NodeEffectSet<'a>],
    output: &mut [Option<EffectRequirement<'a>>],
) -> Result<usize, AuthorityReason> {
    let needed = children
        .iter()
        .try_fold(0_usize, |total, child| {
            total.checked_add(child.effects.len())
        })
        .ok_or(AuthorityReason::StorageTooSmall)?;
    for slot in output.iter_mut().take(needed) {
        *slot = None;
    }
    if output.len() < needed {
        return Err(AuthorityReason::StorageTooSmall);
    }
    for child in children {
        if !valid_id(child.definition)
            || child.effects.iter().any(|effect| {
                effect.requester != child.instance
                    && !path_is_descendant(effect.requester.as_str(), child.instance.as_str())
            })
        {
            return Err(AuthorityReason::InvalidDescriptor);
        }
    }
    let mut written = 0;
    for child in children {
        for effect in child.effects {
            output[written] = Some(*effect);
            written += 1;
        }
    }
    Ok(written)
}

/// Protected evidence metadata that cannot carry value bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RedactedValueMetadata<'a> {
    pub sensitivity: Sensitivity,
    pub value_type: TypeContractRef<'a>,
    pub present: bool,
}

/// Evidence value representation with secret-safe construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceValue<'a> {
    Public {
        value_type: TypeContractRef<'a>,
        value: CanonicalValue<'a>,
    },
    Redacted(RedactedValueMetadata<'a>),
}

/// Operation attempted on a value with a sensitivity label.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SensitivityUse {
    Connect,
    Record,
    Present,
    Diagnostic,
    Evidence,
}

/// Whether value material may cross the requested observation boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SensitivityDisposition {
    Value,
    Redacted,
    Denied,
}

/// Stable sensitivity-policy explanation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SensitivityReason {
    Accepted,
    DestinationTooWeak,
    GrantRequired,
    ProtectedObservationRedacted,
}

/// Complete sensitivity-policy result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SensitivityDecision {
    pub disposition: SensitivityDisposition,
    pub reason: SensitivityReason,
}

/// Assess connection, recording, presentation, diagnostic, or evidence use.
///
/// `authorized_action` is the action from an already validated authority
/// binding. It cannot authorize a sensitivity downgrade.
#[must_use]
pub fn assess_sensitivity(
    sensitivity: Sensitivity,
    destination_ceiling: Sensitivity,
    use_kind: SensitivityUse,
    authorized_action: Option<Id<'_>>,
) -> SensitivityDecision {
    if destination_ceiling < sensitivity {
        return SensitivityDecision {
            disposition: SensitivityDisposition::Denied,
            reason: SensitivityReason::DestinationTooWeak,
        };
    }
    match use_kind {
        SensitivityUse::Diagnostic | SensitivityUse::Evidence
            if sensitivity != Sensitivity::Public =>
        {
            SensitivityDecision {
                disposition: SensitivityDisposition::Redacted,
                reason: SensitivityReason::ProtectedObservationRedacted,
            }
        }
        SensitivityUse::Record | SensitivityUse::Present if sensitivity != Sensitivity::Public => {
            let required = match use_kind {
                SensitivityUse::Record => "conduit/data.record",
                SensitivityUse::Present => "conduit/data.present",
                _ => unreachable!(),
            };
            if authorized_action.is_some_and(|action| action.as_str() == required) {
                SensitivityDecision {
                    disposition: SensitivityDisposition::Value,
                    reason: SensitivityReason::Accepted,
                }
            } else {
                SensitivityDecision {
                    disposition: SensitivityDisposition::Denied,
                    reason: SensitivityReason::GrantRequired,
                }
            }
        }
        SensitivityUse::Connect
        | SensitivityUse::Record
        | SensitivityUse::Present
        | SensitivityUse::Diagnostic
        | SensitivityUse::Evidence => SensitivityDecision {
            disposition: SensitivityDisposition::Value,
            reason: SensitivityReason::Accepted,
        },
    }
}

impl<'a> EvidenceValue<'a> {
    #[must_use]
    pub const fn public(value_type: TypeContractRef<'a>, value: CanonicalValue<'a>) -> Self {
        Self::Public { value_type, value }
    }

    pub const fn redacted(
        sensitivity: Sensitivity,
        value_type: TypeContractRef<'a>,
        present: bool,
    ) -> Result<Self, AuthorityReason> {
        if matches!(sensitivity, Sensitivity::Public) {
            return Err(AuthorityReason::InvalidDescriptor);
        }
        Ok(Self::Redacted(RedactedValueMetadata {
            sensitivity,
            value_type,
            present,
        }))
    }
}

impl EffectRequirement<'_> {
    /// Computes the exact stable requirement descriptor identity.
    pub fn semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        validate_constraints(self.constraints).map_err(|_| CanonicalError::LengthOverflow)?;
        let constraint_fields = constraint_fields(self.constraints);
        let mut constraint_values = [CanonicalValue::Null; MAX_AUTHORITY_CONSTRAINTS];
        for index in 0..self.constraints.len() {
            constraint_values[index] = CanonicalValue::Map(&constraint_fields[index]);
        }
        let selector_fields = selector_fields(self.resource);
        let administrative_class = self
            .administrative_class
            .map(hash_pinned_descriptor)
            .transpose()?;
        let fields = [
            semantic("id", CanonicalValue::Identifier(self.id)),
            MapField {
                name: Id("administrative_class"),
                value: administrative_class
                    .as_ref()
                    .map_or(CanonicalValue::Null, |hash| {
                        CanonicalValue::Bytes(hash.as_bytes())
                    }),
                disposition: FieldDisposition::Defaulted(&NULL_CANONICAL_VALUE),
            },
            semantic("action", CanonicalValue::Identifier(self.action)),
            semantic("resource", CanonicalValue::Map(&selector_fields)),
            semantic("requester", CanonicalValue::Text(self.requester.as_str())),
            semantic("audience", CanonicalValue::Identifier(self.audience)),
            semantic(
                "constraints",
                CanonicalValue::Set(&constraint_values[..self.constraints.len()]),
            ),
            semantic("check_at_use", CanonicalValue::Boolean(self.check_at_use)),
        ];
        CanonicalDescriptor {
            kind: Id("conduit/effect-requirement"),
            schema_version: 1,
            body: CanonicalValue::Map(&fields),
        }
        .semantic_hash()
    }
}

const NULL_CANONICAL_VALUE: CanonicalValue<'static> = CanonicalValue::Null;

fn hash_pinned_descriptor(
    pin: PinnedDescriptor<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    if pin.schema_version == 0 || Id::new(pin.id.as_str()).is_err() {
        return Err(CanonicalError::InvalidIdentifier);
    }
    let fields = [
        semantic("id", CanonicalValue::Identifier(pin.id)),
        semantic(
            "schema_version",
            CanonicalValue::Integer(i128::from(pin.schema_version)),
        ),
        semantic(
            "semantic_hash",
            CanonicalValue::Bytes(pin.semantic_hash.as_bytes()),
        ),
    ];
    CanonicalDescriptor {
        kind: Id("conduit/pinned-descriptor"),
        schema_version: 1,
        body: CanonicalValue::Map(&fields),
    }
    .semantic_hash()
}

impl AuthorityGrant<'_> {
    /// Computes the immutable grant descriptor identity; revocation is separate.
    pub fn semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        validate_grant(*self).map_err(|_| CanonicalError::LengthOverflow)?;
        let constraint_fields = constraint_fields(self.constraints);
        let mut constraint_values = [CanonicalValue::Null; MAX_AUTHORITY_CONSTRAINTS];
        for index in 0..self.constraints.len() {
            constraint_values[index] = CanonicalValue::Map(&constraint_fields[index]);
        }
        let resource_fields = resource_fields(self.resource);
        let scope_fields = [
            semantic("root", CanonicalValue::Text(self.scope.root.as_str())),
            semantic(
                "descendants",
                CanonicalValue::Boolean(self.scope.descendants),
            ),
        ];
        let fields = [
            semantic("id", CanonicalValue::Identifier(self.id)),
            semantic("action", CanonicalValue::Identifier(self.action)),
            semantic("resource", CanonicalValue::Map(&resource_fields)),
            semantic("scope", CanonicalValue::Map(&scope_fields)),
            semantic("audience", CanonicalValue::Identifier(self.audience)),
            semantic(
                "constraints",
                CanonicalValue::Set(&constraint_values[..self.constraints.len()]),
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
                "issued_for_host",
                CanonicalValue::Identifier(self.issued_for_host),
            ),
            semantic(
                "delegation",
                CanonicalValue::Identifier(Id(self.delegation.as_str())),
            ),
            semantic("audit_id", CanonicalValue::Identifier(self.audit_id)),
            semantic(
                "terminal_policy",
                CanonicalValue::Identifier(Id(match self.terminal_policy {
                    StopPolicy::Drain => "drain",
                    StopPolicy::Abort => "abort",
                })),
            ),
        ];
        CanonicalDescriptor {
            kind: Id("conduit/authority-grant"),
            schema_version: 1,
            body: CanonicalValue::Map(&fields),
        }
        .semantic_hash()
    }
}

fn constraint_fields<'a>(
    constraints: &'a [AuthorityConstraintRef<'a>],
) -> [[MapField<'a>; 2]; MAX_AUTHORITY_CONSTRAINTS] {
    let empty = [
        MapField {
            name: Id("id"),
            value: CanonicalValue::Null,
            disposition: FieldDisposition::Annotation,
        },
        MapField {
            name: Id("semantic_hash"),
            value: CanonicalValue::Null,
            disposition: FieldDisposition::Annotation,
        },
    ];
    let mut fields = [empty; MAX_AUTHORITY_CONSTRAINTS];
    for (index, constraint) in constraints.iter().enumerate() {
        fields[index] = [
            semantic("id", CanonicalValue::Identifier(constraint.id)),
            semantic(
                "semantic_hash",
                CanonicalValue::Bytes(constraint.semantic_hash.as_bytes()),
            ),
        ];
    }
    fields
}

fn selector_fields(selector: ResourceSelector<'_>) -> [MapField<'_>; 3] {
    match selector {
        ResourceSelector::Exact(resource) => [
            semantic("mode", CanonicalValue::Identifier(Id("exact"))),
            semantic("kind", CanonicalValue::Identifier(resource.kind)),
            semantic("id", CanonicalValue::Identifier(resource.id)),
        ],
        ResourceSelector::Kind(kind) => [
            semantic("mode", CanonicalValue::Identifier(Id("kind"))),
            semantic("kind", CanonicalValue::Identifier(kind)),
            MapField {
                name: Id("id"),
                value: CanonicalValue::Null,
                disposition: FieldDisposition::Annotation,
            },
        ],
    }
}

fn resource_fields(resource: ResourceRef<'_>) -> [MapField<'_>; 2] {
    [
        semantic("kind", CanonicalValue::Identifier(resource.kind)),
        semantic("id", CanonicalValue::Identifier(resource.id)),
    ]
}

const fn semantic<'a>(name: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    }
}
