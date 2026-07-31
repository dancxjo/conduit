//! Administrative-plane containment and non-escalation proofs.
//!
//! The core assigns no meaning to administrative operation names. Domains pin
//! the descriptors that require containment, while this module validates exact
//! identities, independent approval structure, freshness, and monotonic scope.

use core::convert::Infallible;

use crate::canonical::semantic_hash_with_hash_set;
use crate::{
    CanonicalDescriptor, CanonicalError, CanonicalValue, FieldDisposition, Id, MapField,
    PinnedDescriptor, ResourceRef, ResourceSelector, SemanticHash,
};

pub const CONTAINMENT_POLICY_SCHEMA_VERSION: u32 = 0;
pub const MAX_ADMINISTRATIVE_APPROVERS: usize = 8;
pub const MAX_ADMINISTRATIVE_BENEFICIARIES: usize = 8;
pub const MAX_ADMINISTRATIVE_APPROVALS: usize = 8;
pub const MAX_CONTAINMENT_REASON_NODES: usize = 16;
pub const MAX_CONTAINMENT_REASON_DEPTH: u8 = 8;

/// Exact realm identity and provenance of an administrative actor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdministrativePrincipal<'a> {
    pub realm: Id<'a>,
    pub entity: Id<'a>,
    pub key: Id<'a>,
    pub profile: PinnedDescriptor<'a>,
    /// Plan from which the authority to act was obtained. Crossing a process,
    /// host, or cord does not alter this provenance.
    pub source_plan: SemanticHash,
    pub source_epoch: u64,
}

/// Exact subject boundary. Optional artifact and budget pins are still exact:
/// absence can only match absence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdministrativeSubject<'a> {
    pub realm: Id<'a>,
    pub entity: Id<'a>,
    pub plan: SemanticHash,
    pub epoch: u64,
    pub artifact: Option<crate::ArtifactDigest>,
    pub budget: Option<PinnedDescriptor<'a>>,
}

/// Scope carried by a delegation or administrative request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DelegationEnvelope<'a> {
    pub action: Id<'a>,
    pub resource: ResourceSelector<'a>,
    pub audience: Id<'a>,
    pub time_basis: Id<'a>,
    pub not_before_tick: u64,
    pub expires_at_tick: u64,
    pub remaining_depth: u8,
}

/// One exact approver named by policy, including its declared failure domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdministrativeApprover<'a> {
    pub realm: Id<'a>,
    pub entity: Id<'a>,
    pub key: Id<'a>,
    pub profile: PinnedDescriptor<'a>,
    pub failure_domain: PinnedDescriptor<'a>,
}

/// Domain-owned policy governing one pinned administrative effect class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContainmentPolicy<'a> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub descriptor: PinnedDescriptor<'a>,
    pub effect_class: PinnedDescriptor<'a>,
    pub approvers: &'a [AdministrativeApprover<'a>],
    pub committer: AdministrativeApprover<'a>,
    pub executor: AdministrativeApprover<'a>,
    pub minimum_approvals: u8,
    pub minimum_failure_domains: u8,
    pub requester_independence: bool,
    pub beneficiary_independence: bool,
    pub successor_independence: bool,
    pub delegation_ceiling: Option<DelegationEnvelope<'a>>,
    /// Governance/root handles remain unavailable unless both the proposal and
    /// this policy pin the same one-operation ceremony.
    pub ceremony: Option<PinnedDescriptor<'a>>,
}

/// Request for one bounded administrative operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdministrativeProposal<'a> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub id: Id<'a>,
    pub effect_class: PinnedDescriptor<'a>,
    pub operation: PinnedDescriptor<'a>,
    pub requester: AdministrativePrincipal<'a>,
    pub subject: AdministrativeSubject<'a>,
    /// Every subject that obtains authority or durable state from the change.
    pub beneficiaries: &'a [AdministrativeSubject<'a>],
    /// Active predecessor when this operation activates a successor plan.
    pub predecessor_plan: Option<SemanticHash>,
    pub delegation: Option<DelegationEnvelope<'a>>,
    pub protected_handle: Option<PinnedDescriptor<'a>>,
    pub ceremony: Option<PinnedDescriptor<'a>>,
    pub time_basis: Id<'a>,
    pub created_at_tick: u64,
    pub expires_at_tick: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdministrativeApprovalStatus {
    Current,
    Revoked,
}

/// One approval. Its origin plan is retained in `approver` and cannot be
/// laundered through another process or transport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdministrativeApproval<'a> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub id: Id<'a>,
    pub proposal_identity: SemanticHash,
    pub policy_identity: SemanticHash,
    pub approver: AdministrativePrincipal<'a>,
    pub failure_domain: PinnedDescriptor<'a>,
    pub time_basis: Id<'a>,
    pub issued_at_tick: u64,
    pub expires_at_tick: u64,
    pub status: AdministrativeApprovalStatus,
}

/// Immutable commit of the exact proposal and approval set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdministrativeCommit<'a> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub id: Id<'a>,
    pub proposal_identity: SemanticHash,
    pub policy_identity: SemanticHash,
    pub approvals: &'a [SemanticHash],
    pub committed_by: AdministrativePrincipal<'a>,
    pub committed_at_tick: u64,
}

/// Single-use execution authorization derived from one exact commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdministrativeExecution<'a> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub id: Id<'a>,
    pub proposal_identity: SemanticHash,
    pub commit_identity: SemanticHash,
    pub executor: AdministrativePrincipal<'a>,
    pub time_basis: Id<'a>,
    pub not_before_tick: u64,
    pub expires_at_tick: u64,
}

/// Plan-pinned proof. Proposal, approval, commit, and execution identities are
/// deliberately separate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdministrativeProof<'a> {
    pub proposal: AdministrativeProposal<'a>,
    pub policy: ContainmentPolicy<'a>,
    pub approvals: &'a [AdministrativeApproval<'a>],
    pub commit: AdministrativeCommit<'a>,
    pub execution: AdministrativeExecution<'a>,
}

/// Fresh facts at plan validation or operation use.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContainmentContext<'a> {
    pub subject: AdministrativeSubject<'a>,
    pub time_basis: Id<'a>,
    pub now_tick: u64,
}

/// Stable, bounded failure reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainmentReason {
    UnsupportedVersion,
    InvalidDescriptor,
    IdentityMismatch,
    EffectClassMismatch,
    SubjectMismatch,
    ProposalExpired,
    ApprovalMissing,
    ApprovalExpired,
    ApprovalRevoked,
    ApprovalConflict,
    ApprovalReplay,
    ApproverNotAllowed,
    FailureDomainInsufficient,
    SelfSupporting,
    SuccessorSelfAuthorized,
    CyclicSupport,
    DelegationWidened,
    CeremonyRequired,
    CeremonyMismatch,
    CommitMismatch,
    ExecutionMismatch,
    NotYetValid,
    RecoveryWidened,
    ReasonTreeInvalid,
}

impl ContainmentReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "CND-CTN-001",
            Self::InvalidDescriptor => "CND-CTN-002",
            Self::IdentityMismatch => "CND-CTN-003",
            Self::EffectClassMismatch => "CND-CTN-004",
            Self::SubjectMismatch => "CND-CTN-005",
            Self::ProposalExpired => "CND-CTN-006",
            Self::ApprovalMissing => "CND-CTN-007",
            Self::ApprovalExpired => "CND-CTN-008",
            Self::ApprovalRevoked => "CND-CTN-009",
            Self::ApprovalConflict => "CND-CTN-010",
            Self::ApprovalReplay => "CND-CTN-011",
            Self::ApproverNotAllowed => "CND-CTN-012",
            Self::FailureDomainInsufficient => "CND-CTN-013",
            Self::SelfSupporting => "CND-CTN-014",
            Self::SuccessorSelfAuthorized => "CND-CTN-015",
            Self::CyclicSupport => "CND-CTN-016",
            Self::DelegationWidened => "CND-CTN-017",
            Self::CeremonyRequired => "CND-CTN-018",
            Self::CeremonyMismatch => "CND-CTN-019",
            Self::CommitMismatch => "CND-CTN-020",
            Self::ExecutionMismatch => "CND-CTN-021",
            Self::NotYetValid => "CND-CTN-022",
            Self::RecoveryWidened => "CND-CTN-023",
            Self::ReasonTreeInvalid => "CND-CTN-024",
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "unsupported-containment-version",
            Self::InvalidDescriptor => "invalid-containment-descriptor",
            Self::IdentityMismatch => "containment-identity-mismatch",
            Self::EffectClassMismatch => "administrative-effect-class-mismatch",
            Self::SubjectMismatch => "administrative-subject-mismatch",
            Self::ProposalExpired => "administrative-proposal-expired",
            Self::ApprovalMissing => "independent-approval-proof-missing",
            Self::ApprovalExpired => "administrative-approval-expired",
            Self::ApprovalRevoked => "administrative-approval-revoked",
            Self::ApprovalConflict => "administrative-approval-conflict",
            Self::ApprovalReplay => "administrative-approval-replay",
            Self::ApproverNotAllowed => "administrative-approver-not-allowed",
            Self::FailureDomainInsufficient => "approval-failure-domain-insufficient",
            Self::SelfSupporting => "self-supporting-administrative-approval",
            Self::SuccessorSelfAuthorized => "successor-self-authorized",
            Self::CyclicSupport => "cyclic-administrative-support",
            Self::DelegationWidened => "delegation-widened",
            Self::CeremonyRequired => "governance-ceremony-required",
            Self::CeremonyMismatch => "governance-ceremony-mismatch",
            Self::CommitMismatch => "administrative-commit-mismatch",
            Self::ExecutionMismatch => "administrative-execution-mismatch",
            Self::NotYetValid => "administrative-proof-not-yet-valid",
            Self::RecoveryWidened => "recovery-authority-widened",
            Self::ReasonTreeInvalid => "containment-reason-tree-invalid",
        }
    }
}

/// One node in a caller-owned bounded explanation tree.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContainmentReasonNode {
    pub reason: ContainmentReason,
    pub parent: Option<u8>,
    pub depth: u8,
}

/// Immutable control/evidence kind. Payload identities name the exact stage;
/// secrets and protected handles are never embedded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdministrativeControlKind {
    Requested,
    Denied(ContainmentReason),
    Approved,
    Expired,
    Committed,
    Executed,
    Revoked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdministrativeControlRecord<'a> {
    pub identity: SemanticHash,
    pub sequence: u64,
    pub record_id: Id<'a>,
    pub proposal_identity: SemanticHash,
    pub stage_identity: SemanticHash,
    pub realm: Id<'a>,
    pub entity: Id<'a>,
    pub epoch: u64,
    pub kind: AdministrativeControlKind,
}

impl AdministrativeControlRecord<'_> {
    pub fn computed_semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        let (kind, reason) = match self.kind {
            AdministrativeControlKind::Requested => ("requested", None),
            AdministrativeControlKind::Denied(reason) => ("denied", Some(reason)),
            AdministrativeControlKind::Approved => ("approved", None),
            AdministrativeControlKind::Expired => ("expired", None),
            AdministrativeControlKind::Committed => ("committed", None),
            AdministrativeControlKind::Executed => ("executed", None),
            AdministrativeControlKind::Revoked => ("revoked", None),
        };
        let fields = [
            semantic(
                "sequence",
                CanonicalValue::Integer(i128::from(self.sequence)),
            ),
            semantic("record_id", CanonicalValue::Identifier(self.record_id)),
            semantic(
                "proposal_identity",
                CanonicalValue::Bytes(self.proposal_identity.as_bytes()),
            ),
            semantic(
                "stage_identity",
                CanonicalValue::Bytes(self.stage_identity.as_bytes()),
            ),
            semantic("realm", CanonicalValue::Identifier(self.realm)),
            semantic("entity", CanonicalValue::Identifier(self.entity)),
            semantic("epoch", CanonicalValue::Integer(i128::from(self.epoch))),
            semantic("kind", CanonicalValue::Identifier(Id(kind))),
            semantic(
                "reason",
                reason.map_or(CanonicalValue::Null, |value| {
                    CanonicalValue::Identifier(Id(value.as_str()))
                }),
            ),
        ];
        descriptor_hash("conduit/administrative-control-record", 1, &fields)
    }
}

pub fn validate_control_record(
    record: AdministrativeControlRecord<'_>,
) -> Result<(), ContainmentReason> {
    let computed = record
        .computed_semantic_hash()
        .map_err(|_| ContainmentReason::InvalidDescriptor)?;
    if record.identity != computed
        || record.identity == record.proposal_identity
        || record.identity == record.stage_identity
    {
        return Err(ContainmentReason::IdentityMismatch);
    }
    Ok(())
}

/// One directed plan-support claim used to reject mutual authorization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdministrativeSupportEdge {
    pub supporter: SemanticHash,
    pub beneficiary: SemanticHash,
}

/// Result distinguishes ordinary execution from a validated administrative
/// effect without assigning a domain meaning to either descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainmentDisposition {
    Ordinary,
    Administrative,
}

/// Classify and validate one effect. Ordinary effects need no administrative
/// proof; every pinned administrative class fails closed without one.
pub fn validate_effect_containment(
    effect_class: PinnedDescriptor<'_>,
    administrative_classes: &[PinnedDescriptor<'_>],
    proof: Option<AdministrativeProof<'_>>,
    context: ContainmentContext<'_>,
) -> Result<ContainmentDisposition, ContainmentReason> {
    let administrative = administrative_classes.contains(&effect_class);
    if !administrative {
        return if proof.is_none() {
            Ok(ContainmentDisposition::Ordinary)
        } else {
            Err(ContainmentReason::EffectClassMismatch)
        };
    }
    let proof = proof.ok_or(ContainmentReason::ApprovalMissing)?;
    if proof.proposal.effect_class != effect_class || proof.policy.effect_class != effect_class {
        return Err(ContainmentReason::EffectClassMismatch);
    }
    validate_administrative_proof(proof, context)?;
    Ok(ContainmentDisposition::Administrative)
}

pub fn validate_administrative_proof(
    proof: AdministrativeProof<'_>,
    context: ContainmentContext<'_>,
) -> Result<(), ContainmentReason> {
    validate_policy(proof.policy)?;
    validate_proposal(proof.proposal)?;
    if proof.proposal.identity
        != proof
            .proposal
            .computed_semantic_hash()
            .map_err(|_| ContainmentReason::InvalidDescriptor)?
        || proof.policy.identity
            != proof
                .policy
                .computed_semantic_hash()
                .map_err(|_| ContainmentReason::InvalidDescriptor)?
    {
        return Err(ContainmentReason::IdentityMismatch);
    }
    if proof.proposal.effect_class != proof.policy.effect_class {
        return Err(ContainmentReason::EffectClassMismatch);
    }
    if proof.proposal.subject != context.subject {
        return Err(ContainmentReason::SubjectMismatch);
    }
    if proof.proposal.time_basis != context.time_basis
        || context.now_tick < proof.proposal.created_at_tick
    {
        return Err(ContainmentReason::NotYetValid);
    }
    if context.now_tick >= proof.proposal.expires_at_tick {
        return Err(ContainmentReason::ProposalExpired);
    }
    match (proof.proposal.protected_handle, proof.proposal.ceremony) {
        (Some(_), None) => return Err(ContainmentReason::CeremonyRequired),
        (Some(_), Some(ceremony)) if proof.policy.ceremony != Some(ceremony) => {
            return Err(ContainmentReason::CeremonyMismatch);
        }
        (None, Some(_)) => return Err(ContainmentReason::CeremonyMismatch),
        _ => {}
    }
    if let Some(requested) = proof.proposal.delegation {
        let ceiling = proof
            .policy
            .delegation_ceiling
            .ok_or(ContainmentReason::DelegationWidened)?;
        validate_delegation_narrowing(ceiling, requested)?;
    }

    if proof.approvals.is_empty()
        || proof.approvals.len() > MAX_ADMINISTRATIVE_APPROVALS
        || proof.commit.approvals.len() != proof.approvals.len()
    {
        return Err(ContainmentReason::ApprovalMissing);
    }
    let mut valid_approvals = 0_u8;
    let mut failure_domains = 0_u8;
    for (index, approval) in proof.approvals.iter().enumerate() {
        validate_approval(*approval, proof.proposal, proof.policy, context)?;
        if approval.identity
            != approval
                .computed_semantic_hash()
                .map_err(|_| ContainmentReason::InvalidDescriptor)?
        {
            return Err(ContainmentReason::IdentityMismatch);
        }
        if proof.approvals[..index]
            .iter()
            .any(|prior| prior.approver == approval.approver)
        {
            return Err(ContainmentReason::ApprovalConflict);
        }
        if !proof.commit.approvals.contains(&approval.identity) {
            return Err(ContainmentReason::CommitMismatch);
        }
        valid_approvals = valid_approvals.saturating_add(1);
        if !proof.approvals[..index]
            .iter()
            .any(|prior| prior.failure_domain == approval.failure_domain)
        {
            failure_domains = failure_domains.saturating_add(1);
        }
    }
    if valid_approvals < proof.policy.minimum_approvals {
        return Err(ContainmentReason::ApprovalMissing);
    }
    if failure_domains < proof.policy.minimum_failure_domains {
        return Err(ContainmentReason::FailureDomainInsufficient);
    }

    validate_commit(proof.commit, proof.proposal, proof.policy)?;
    if proof.commit.identity
        != proof
            .commit
            .computed_semantic_hash()
            .map_err(|_| ContainmentReason::InvalidDescriptor)?
    {
        return Err(ContainmentReason::IdentityMismatch);
    }
    validate_execution(
        proof.execution,
        proof.proposal,
        proof.commit,
        proof.policy,
        context,
    )?;
    if proof.execution.identity
        != proof
            .execution
            .computed_semantic_hash()
            .map_err(|_| ContainmentReason::InvalidDescriptor)?
    {
        return Err(ContainmentReason::IdentityMismatch);
    }
    Ok(())
}

pub fn validate_delegation_narrowing(
    parent: DelegationEnvelope<'_>,
    child: DelegationEnvelope<'_>,
) -> Result<(), ContainmentReason> {
    let resource_narrows = match (parent.resource, child.resource) {
        (ResourceSelector::Exact(parent), ResourceSelector::Exact(child)) => parent == child,
        (ResourceSelector::Kind(parent), ResourceSelector::Kind(child)) => parent == child,
        (ResourceSelector::Kind(parent), ResourceSelector::Exact(child)) => parent == child.kind,
        (ResourceSelector::Exact(_), ResourceSelector::Kind(_)) => false,
    };
    if parent.action != child.action
        || !resource_narrows
        || parent.audience != child.audience
        || parent.time_basis != child.time_basis
        || child.not_before_tick < parent.not_before_tick
        || child.expires_at_tick > parent.expires_at_tick
        || child.expires_at_tick <= child.not_before_tick
        || child.remaining_depth > parent.remaining_depth
    {
        return Err(ContainmentReason::DelegationWidened);
    }
    Ok(())
}

/// Recovery and emergency authorization may only retain or narrow the scope
/// available in the triggering state.
pub fn validate_recovery_narrowing(
    trigger: DelegationEnvelope<'_>,
    recovered: DelegationEnvelope<'_>,
) -> Result<(), ContainmentReason> {
    validate_delegation_narrowing(trigger, recovered)
        .map_err(|_| ContainmentReason::RecoveryWidened)
}

/// Reject self edges and any directed cycle using caller-owned scratch. The
/// scratch length must cover every edge endpoint.
pub fn validate_support_graph(
    edges: &[AdministrativeSupportEdge],
    visiting: &mut [bool],
) -> Result<(), ContainmentReason> {
    let mut nodes = [SemanticHash::from_bytes([0; 32]); MAX_ADMINISTRATIVE_APPROVALS * 2];
    let mut node_count = 0;
    for edge in edges {
        if edge.supporter == edge.beneficiary {
            return Err(ContainmentReason::SelfSupporting);
        }
        for node in [edge.supporter, edge.beneficiary] {
            if !nodes[..node_count].contains(&node) {
                if node_count == nodes.len() {
                    return Err(ContainmentReason::InvalidDescriptor);
                }
                nodes[node_count] = node;
                node_count += 1;
            }
        }
    }
    if visiting.len() < node_count {
        return Err(ContainmentReason::InvalidDescriptor);
    }
    for candidate in edges {
        visiting[..node_count].fill(false);
        let mut queue = [0_usize; MAX_ADMINISTRATIVE_APPROVALS * 2];
        let start = nodes[..node_count]
            .iter()
            .position(|node| *node == candidate.beneficiary)
            .ok_or(ContainmentReason::InvalidDescriptor)?;
        let target = nodes[..node_count]
            .iter()
            .position(|node| *node == candidate.supporter)
            .ok_or(ContainmentReason::InvalidDescriptor)?;
        queue[0] = start;
        visiting[start] = true;
        let mut read = 0;
        let mut written = 1;
        while read < written {
            let current = queue[read];
            read += 1;
            if current == target {
                return Err(ContainmentReason::CyclicSupport);
            }
            for edge in edges.iter().filter(|edge| edge.supporter == nodes[current]) {
                let next = nodes[..node_count]
                    .iter()
                    .position(|node| *node == edge.beneficiary)
                    .ok_or(ContainmentReason::InvalidDescriptor)?;
                if !visiting[next] {
                    visiting[next] = true;
                    queue[written] = next;
                    written += 1;
                }
            }
        }
    }
    Ok(())
}

pub fn validate_reason_tree(nodes: &[ContainmentReasonNode]) -> Result<(), ContainmentReason> {
    if nodes.is_empty() || nodes.len() > MAX_CONTAINMENT_REASON_NODES {
        return Err(ContainmentReason::ReasonTreeInvalid);
    }
    for (index, node) in nodes.iter().enumerate() {
        if node.depth > MAX_CONTAINMENT_REASON_DEPTH {
            return Err(ContainmentReason::ReasonTreeInvalid);
        }
        match node.parent {
            None if node.depth != 0 => return Err(ContainmentReason::ReasonTreeInvalid),
            Some(parent) => {
                let parent = usize::from(parent);
                if parent >= index || nodes[parent].depth.checked_add(1) != Some(node.depth) {
                    return Err(ContainmentReason::ReasonTreeInvalid);
                }
            }
            None => {}
        }
    }
    Ok(())
}

fn validate_policy(policy: ContainmentPolicy<'_>) -> Result<(), ContainmentReason> {
    if policy.schema_version != CONTAINMENT_POLICY_SCHEMA_VERSION {
        return Err(ContainmentReason::UnsupportedVersion);
    }
    if !valid_pin(policy.descriptor)
        || !valid_pin(policy.effect_class)
        || policy.approvers.is_empty()
        || policy.approvers.len() > MAX_ADMINISTRATIVE_APPROVERS
        || policy.minimum_approvals == 0
        || usize::from(policy.minimum_approvals) > policy.approvers.len()
        || policy.minimum_failure_domains == 0
        || policy.minimum_failure_domains > policy.minimum_approvals
        || policy
            .approvers
            .iter()
            .any(|approver| !valid_approver(*approver))
        || !valid_approver(policy.committer)
        || !valid_approver(policy.executor)
    {
        return Err(ContainmentReason::InvalidDescriptor);
    }
    Ok(())
}

fn validate_proposal(proposal: AdministrativeProposal<'_>) -> Result<(), ContainmentReason> {
    if proposal.schema_version != CONTAINMENT_POLICY_SCHEMA_VERSION {
        return Err(ContainmentReason::UnsupportedVersion);
    }
    if Id::new(proposal.id.as_str()).is_err()
        || !valid_pin(proposal.effect_class)
        || !valid_pin(proposal.operation)
        || !valid_principal(proposal.requester)
        || !valid_subject(proposal.subject)
        || proposal.beneficiaries.is_empty()
        || proposal.beneficiaries.len() > MAX_ADMINISTRATIVE_BENEFICIARIES
        || proposal
            .beneficiaries
            .iter()
            .any(|beneficiary| !valid_subject(*beneficiary))
        || Id::new(proposal.time_basis.as_str()).is_err()
        || proposal.expires_at_tick <= proposal.created_at_tick
    {
        return Err(ContainmentReason::InvalidDescriptor);
    }
    Ok(())
}

fn validate_approval(
    approval: AdministrativeApproval<'_>,
    proposal: AdministrativeProposal<'_>,
    policy: ContainmentPolicy<'_>,
    context: ContainmentContext<'_>,
) -> Result<(), ContainmentReason> {
    if approval.schema_version != CONTAINMENT_POLICY_SCHEMA_VERSION {
        return Err(ContainmentReason::UnsupportedVersion);
    }
    if Id::new(approval.id.as_str()).is_err()
        || !valid_principal(approval.approver)
        || !valid_pin(approval.failure_domain)
        || approval.expires_at_tick <= approval.issued_at_tick
    {
        return Err(ContainmentReason::InvalidDescriptor);
    }
    if approval.proposal_identity != proposal.identity
        || approval.policy_identity != policy.identity
    {
        return Err(ContainmentReason::ApprovalReplay);
    }
    if approval.time_basis != context.time_basis || context.now_tick < approval.issued_at_tick {
        return Err(ContainmentReason::NotYetValid);
    }
    if context.now_tick >= approval.expires_at_tick {
        return Err(ContainmentReason::ApprovalExpired);
    }
    if approval.status == AdministrativeApprovalStatus::Revoked {
        return Err(ContainmentReason::ApprovalRevoked);
    }
    let allowed = policy.approvers.iter().any(|allowed| {
        allowed.realm == approval.approver.realm
            && allowed.entity == approval.approver.entity
            && allowed.key == approval.approver.key
            && allowed.profile == approval.approver.profile
            && allowed.failure_domain == approval.failure_domain
    });
    if !allowed {
        return Err(ContainmentReason::ApproverNotAllowed);
    }
    if policy.successor_independence
        && proposal
            .predecessor_plan
            .is_some_and(|plan| approval.approver.source_plan == plan)
    {
        return Err(ContainmentReason::SuccessorSelfAuthorized);
    }
    if policy.requester_independence && same_provenance(approval.approver, proposal.requester) {
        return Err(ContainmentReason::SelfSupporting);
    }
    if policy.beneficiary_independence
        && proposal
            .beneficiaries
            .iter()
            .any(|beneficiary| principal_benefits(approval.approver, *beneficiary))
    {
        return Err(ContainmentReason::SelfSupporting);
    }
    Ok(())
}

fn validate_commit(
    commit: AdministrativeCommit<'_>,
    proposal: AdministrativeProposal<'_>,
    policy: ContainmentPolicy<'_>,
) -> Result<(), ContainmentReason> {
    if commit.schema_version != CONTAINMENT_POLICY_SCHEMA_VERSION {
        return Err(ContainmentReason::UnsupportedVersion);
    }
    if Id::new(commit.id.as_str()).is_err()
        || !valid_principal(commit.committed_by)
        || !principal_matches(commit.committed_by, policy.committer)
        || commit.proposal_identity != proposal.identity
        || commit.policy_identity != policy.identity
        || commit.committed_at_tick < proposal.created_at_tick
        || commit.committed_at_tick >= proposal.expires_at_tick
    {
        return Err(ContainmentReason::CommitMismatch);
    }
    Ok(())
}

fn validate_execution(
    execution: AdministrativeExecution<'_>,
    proposal: AdministrativeProposal<'_>,
    commit: AdministrativeCommit<'_>,
    policy: ContainmentPolicy<'_>,
    context: ContainmentContext<'_>,
) -> Result<(), ContainmentReason> {
    if execution.schema_version != CONTAINMENT_POLICY_SCHEMA_VERSION {
        return Err(ContainmentReason::UnsupportedVersion);
    }
    if Id::new(execution.id.as_str()).is_err()
        || !valid_principal(execution.executor)
        || !principal_matches(execution.executor, policy.executor)
        || execution.proposal_identity != proposal.identity
        || execution.commit_identity != commit.identity
        || execution.time_basis != context.time_basis
        || execution.expires_at_tick <= execution.not_before_tick
        || context.now_tick < execution.not_before_tick
        || context.now_tick >= execution.expires_at_tick
    {
        return Err(ContainmentReason::ExecutionMismatch);
    }
    Ok(())
}

impl ContainmentPolicy<'_> {
    pub fn computed_semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        if self.approvers.len() > MAX_ADMINISTRATIVE_APPROVERS {
            return Err(CanonicalError::LengthOverflow);
        }
        let mut hashes = [SemanticHash::from_bytes([0; 32]); MAX_ADMINISTRATIVE_APPROVERS];
        for (index, approver) in self.approvers.iter().enumerate() {
            hashes[index] = hash_approver(*approver)?;
        }
        let descriptor = hash_pin(self.descriptor)?;
        let effect = hash_pin(self.effect_class)?;
        let committer = hash_approver(self.committer)?;
        let executor = hash_approver(self.executor)?;
        let ceiling = hash_optional_delegation(self.delegation_ceiling)?;
        let ceremony = hash_optional_pin(self.ceremony)?;
        let fields = [
            semantic("descriptor", CanonicalValue::Bytes(descriptor.as_bytes())),
            semantic("effect_class", CanonicalValue::Bytes(effect.as_bytes())),
            semantic("committer", CanonicalValue::Bytes(committer.as_bytes())),
            semantic("executor", CanonicalValue::Bytes(executor.as_bytes())),
            semantic(
                "minimum_approvals",
                CanonicalValue::Integer(i128::from(self.minimum_approvals)),
            ),
            semantic(
                "minimum_failure_domains",
                CanonicalValue::Integer(i128::from(self.minimum_failure_domains)),
            ),
            semantic(
                "requester_independence",
                CanonicalValue::Boolean(self.requester_independence),
            ),
            semantic(
                "beneficiary_independence",
                CanonicalValue::Boolean(self.beneficiary_independence),
            ),
            semantic(
                "successor_independence",
                CanonicalValue::Boolean(self.successor_independence),
            ),
            semantic("delegation_ceiling", optional_hash_value(ceiling.as_ref())),
            semantic("ceremony", optional_hash_value(ceremony.as_ref())),
        ];
        semantic_hash_with_hash_set(
            Id("conduit/containment-policy"),
            self.schema_version,
            &fields,
            Id("approvers"),
            &hashes[..self.approvers.len()],
        )
    }
}

impl AdministrativeProposal<'_> {
    pub fn computed_semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        if self.beneficiaries.len() > MAX_ADMINISTRATIVE_BENEFICIARIES {
            return Err(CanonicalError::LengthOverflow);
        }
        let mut hashes = [SemanticHash::from_bytes([0; 32]); MAX_ADMINISTRATIVE_BENEFICIARIES];
        for (index, beneficiary) in self.beneficiaries.iter().enumerate() {
            hashes[index] = hash_subject(*beneficiary)?;
        }
        let effect = hash_pin(self.effect_class)?;
        let operation = hash_pin(self.operation)?;
        let requester = hash_principal(self.requester)?;
        let subject = hash_subject(self.subject)?;
        let delegation = hash_optional_delegation(self.delegation)?;
        let handle = hash_optional_pin(self.protected_handle)?;
        let ceremony = hash_optional_pin(self.ceremony)?;
        let fields = [
            semantic("id", CanonicalValue::Identifier(self.id)),
            semantic("effect_class", CanonicalValue::Bytes(effect.as_bytes())),
            semantic("operation", CanonicalValue::Bytes(operation.as_bytes())),
            semantic("requester", CanonicalValue::Bytes(requester.as_bytes())),
            semantic("subject", CanonicalValue::Bytes(subject.as_bytes())),
            semantic(
                "predecessor_plan",
                optional_hash_value(self.predecessor_plan.as_ref()),
            ),
            semantic("delegation", optional_hash_value(delegation.as_ref())),
            semantic("protected_handle", optional_hash_value(handle.as_ref())),
            semantic("ceremony", optional_hash_value(ceremony.as_ref())),
            semantic("time_basis", CanonicalValue::Identifier(self.time_basis)),
            semantic(
                "created_at_tick",
                CanonicalValue::Integer(i128::from(self.created_at_tick)),
            ),
            semantic(
                "expires_at_tick",
                CanonicalValue::Integer(i128::from(self.expires_at_tick)),
            ),
        ];
        semantic_hash_with_hash_set(
            Id("conduit/administrative-proposal"),
            self.schema_version,
            &fields,
            Id("beneficiaries"),
            &hashes[..self.beneficiaries.len()],
        )
    }
}

impl AdministrativeApproval<'_> {
    pub fn computed_semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        let approver = hash_principal(self.approver)?;
        let domain = hash_pin(self.failure_domain)?;
        let fields = [
            semantic("id", CanonicalValue::Identifier(self.id)),
            semantic(
                "proposal_identity",
                CanonicalValue::Bytes(self.proposal_identity.as_bytes()),
            ),
            semantic(
                "policy_identity",
                CanonicalValue::Bytes(self.policy_identity.as_bytes()),
            ),
            semantic("approver", CanonicalValue::Bytes(approver.as_bytes())),
            semantic("failure_domain", CanonicalValue::Bytes(domain.as_bytes())),
            semantic("time_basis", CanonicalValue::Identifier(self.time_basis)),
            semantic(
                "issued_at_tick",
                CanonicalValue::Integer(i128::from(self.issued_at_tick)),
            ),
            semantic(
                "expires_at_tick",
                CanonicalValue::Integer(i128::from(self.expires_at_tick)),
            ),
            semantic(
                "status",
                CanonicalValue::Identifier(Id(match self.status {
                    AdministrativeApprovalStatus::Current => "current",
                    AdministrativeApprovalStatus::Revoked => "revoked",
                })),
            ),
        ];
        descriptor_hash(
            "conduit/administrative-approval",
            self.schema_version,
            &fields,
        )
    }
}

impl AdministrativeCommit<'_> {
    pub fn computed_semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        if self.approvals.len() > MAX_ADMINISTRATIVE_APPROVALS {
            return Err(CanonicalError::LengthOverflow);
        }
        let committer = hash_principal(self.committed_by)?;
        let fields = [
            semantic("id", CanonicalValue::Identifier(self.id)),
            semantic(
                "proposal_identity",
                CanonicalValue::Bytes(self.proposal_identity.as_bytes()),
            ),
            semantic(
                "policy_identity",
                CanonicalValue::Bytes(self.policy_identity.as_bytes()),
            ),
            semantic("committed_by", CanonicalValue::Bytes(committer.as_bytes())),
            semantic(
                "committed_at_tick",
                CanonicalValue::Integer(i128::from(self.committed_at_tick)),
            ),
        ];
        semantic_hash_with_hash_set(
            Id("conduit/administrative-commit"),
            self.schema_version,
            &fields,
            Id("approvals"),
            self.approvals,
        )
    }
}

impl AdministrativeExecution<'_> {
    pub fn computed_semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        let executor = hash_principal(self.executor)?;
        let fields = [
            semantic("id", CanonicalValue::Identifier(self.id)),
            semantic(
                "proposal_identity",
                CanonicalValue::Bytes(self.proposal_identity.as_bytes()),
            ),
            semantic(
                "commit_identity",
                CanonicalValue::Bytes(self.commit_identity.as_bytes()),
            ),
            semantic("executor", CanonicalValue::Bytes(executor.as_bytes())),
            semantic("time_basis", CanonicalValue::Identifier(self.time_basis)),
            semantic(
                "not_before_tick",
                CanonicalValue::Integer(i128::from(self.not_before_tick)),
            ),
            semantic(
                "expires_at_tick",
                CanonicalValue::Integer(i128::from(self.expires_at_tick)),
            ),
        ];
        descriptor_hash(
            "conduit/administrative-execution",
            self.schema_version,
            &fields,
        )
    }
}

fn hash_principal(
    principal: AdministrativePrincipal<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let profile = hash_pin(principal.profile)?;
    let fields = [
        semantic("realm", CanonicalValue::Identifier(principal.realm)),
        semantic("entity", CanonicalValue::Identifier(principal.entity)),
        semantic("key", CanonicalValue::Identifier(principal.key)),
        semantic("profile", CanonicalValue::Bytes(profile.as_bytes())),
        semantic(
            "source_plan",
            CanonicalValue::Bytes(principal.source_plan.as_bytes()),
        ),
        semantic(
            "source_epoch",
            CanonicalValue::Integer(i128::from(principal.source_epoch)),
        ),
    ];
    descriptor_hash("conduit/administrative-principal", 1, &fields)
}

fn hash_subject(
    subject: AdministrativeSubject<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let budget = hash_optional_pin(subject.budget)?;
    let fields = [
        semantic("realm", CanonicalValue::Identifier(subject.realm)),
        semantic("entity", CanonicalValue::Identifier(subject.entity)),
        semantic("plan", CanonicalValue::Bytes(subject.plan.as_bytes())),
        semantic("epoch", CanonicalValue::Integer(i128::from(subject.epoch))),
        semantic(
            "artifact",
            match subject.artifact.as_ref() {
                Some(value) => CanonicalValue::Bytes(value.as_bytes()),
                None => CanonicalValue::Null,
            },
        ),
        semantic("budget", optional_hash_value(budget.as_ref())),
    ];
    descriptor_hash("conduit/administrative-subject", 1, &fields)
}

fn hash_approver(
    approver: AdministrativeApprover<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let profile = hash_pin(approver.profile)?;
    let domain = hash_pin(approver.failure_domain)?;
    let fields = [
        semantic("realm", CanonicalValue::Identifier(approver.realm)),
        semantic("entity", CanonicalValue::Identifier(approver.entity)),
        semantic("key", CanonicalValue::Identifier(approver.key)),
        semantic("profile", CanonicalValue::Bytes(profile.as_bytes())),
        semantic("failure_domain", CanonicalValue::Bytes(domain.as_bytes())),
    ];
    descriptor_hash("conduit/administrative-approver", 1, &fields)
}

fn hash_optional_delegation(
    delegation: Option<DelegationEnvelope<'_>>,
) -> Result<Option<SemanticHash>, CanonicalError<Infallible>> {
    delegation.map(hash_delegation).transpose()
}

fn hash_delegation(
    delegation: DelegationEnvelope<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let (mode, kind, id) = match delegation.resource {
        ResourceSelector::Exact(ResourceRef { kind, id }) => ("exact", kind, Some(id)),
        ResourceSelector::Kind(kind) => ("kind", kind, None),
    };
    let fields = [
        semantic("action", CanonicalValue::Identifier(delegation.action)),
        semantic("resource_mode", CanonicalValue::Identifier(Id(mode))),
        semantic("resource_kind", CanonicalValue::Identifier(kind)),
        semantic(
            "resource_id",
            id.map_or(CanonicalValue::Null, CanonicalValue::Identifier),
        ),
        semantic("audience", CanonicalValue::Identifier(delegation.audience)),
        semantic(
            "time_basis",
            CanonicalValue::Identifier(delegation.time_basis),
        ),
        semantic(
            "not_before_tick",
            CanonicalValue::Integer(i128::from(delegation.not_before_tick)),
        ),
        semantic(
            "expires_at_tick",
            CanonicalValue::Integer(i128::from(delegation.expires_at_tick)),
        ),
        semantic(
            "remaining_depth",
            CanonicalValue::Integer(i128::from(delegation.remaining_depth)),
        ),
    ];
    descriptor_hash("conduit/delegation-envelope", 1, &fields)
}

fn hash_optional_pin(
    pin: Option<PinnedDescriptor<'_>>,
) -> Result<Option<SemanticHash>, CanonicalError<Infallible>> {
    pin.map(hash_pin).transpose()
}

fn hash_pin(pin: PinnedDescriptor<'_>) -> Result<SemanticHash, CanonicalError<Infallible>> {
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
    descriptor_hash("conduit/pinned-descriptor", 1, &fields)
}

fn descriptor_hash(
    kind: &'static str,
    schema_version: u32,
    fields: &[MapField<'_>],
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    CanonicalDescriptor {
        kind: Id(kind),
        schema_version,
        body: CanonicalValue::Map(fields),
    }
    .semantic_hash()
}

fn optional_hash_value(hash: Option<&SemanticHash>) -> CanonicalValue<'_> {
    hash.map_or(CanonicalValue::Null, |value| {
        CanonicalValue::Bytes(value.as_bytes())
    })
}

fn valid_pin(pin: PinnedDescriptor<'_>) -> bool {
    pin.schema_version == 0 && Id::new(pin.id.as_str()).is_ok()
}

fn valid_principal(principal: AdministrativePrincipal<'_>) -> bool {
    Id::new(principal.realm.as_str()).is_ok()
        && Id::new(principal.entity.as_str()).is_ok()
        && Id::new(principal.key.as_str()).is_ok()
        && valid_pin(principal.profile)
}

fn valid_subject(subject: AdministrativeSubject<'_>) -> bool {
    Id::new(subject.realm.as_str()).is_ok()
        && Id::new(subject.entity.as_str()).is_ok()
        && subject.budget.is_none_or(valid_pin)
}

fn valid_approver(approver: AdministrativeApprover<'_>) -> bool {
    Id::new(approver.realm.as_str()).is_ok()
        && Id::new(approver.entity.as_str()).is_ok()
        && Id::new(approver.key.as_str()).is_ok()
        && valid_pin(approver.profile)
        && valid_pin(approver.failure_domain)
}

fn same_provenance(left: AdministrativePrincipal<'_>, right: AdministrativePrincipal<'_>) -> bool {
    left.realm == right.realm
        && (left.entity == right.entity
            || (left.source_plan == right.source_plan && left.source_epoch == right.source_epoch))
}

fn principal_benefits(
    principal: AdministrativePrincipal<'_>,
    beneficiary: AdministrativeSubject<'_>,
) -> bool {
    principal.realm == beneficiary.realm
        && (principal.entity == beneficiary.entity
            || (principal.source_plan == beneficiary.plan
                && principal.source_epoch == beneficiary.epoch))
}

fn principal_matches(
    principal: AdministrativePrincipal<'_>,
    allowed: AdministrativeApprover<'_>,
) -> bool {
    principal.realm == allowed.realm
        && principal.entity == allowed.entity
        && principal.key == allowed.key
        && principal.profile == allowed.profile
}

const fn semantic<'a>(name: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    }
}
