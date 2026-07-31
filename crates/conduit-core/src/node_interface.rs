//! Named, reusable node-boundary contracts.
//!
//! A node interface is a semantic contract over an ordinary [`NodeContract`]
//! boundary. It is not a runtime object, implementation ABI, host-service
//! trait, or source-language instance. Version 1 contains complete directional
//! [`PortContract`] members plus a finite set of exact, domain-open non-port
//! requirements. Configuration, lifecycle, effects, and authority remain
//! distinct contracts and are never inferred by this module.

use core::convert::Infallible;
use core::fmt;

use crate::canonical::semantic_hash_with_hash_set;
use crate::{
    CanonicalDescriptor, CanonicalError, CanonicalValue, CompatibilityDecision,
    CompatibilityOutcome, CompatibilityQuery, CompatibilityReason, DescriptorRef, Direction,
    FieldDisposition, Id, MapField, NodeContract, PortCompatibilityDecision, PortContract,
    SemanticHash, assess_port_substitution, assess_type_contract_exact,
};

/// Exact schema of a node-interface descriptor.
pub const NODE_INTERFACE_CONTRACT_SCHEMA_VERSION: u32 = 0;

/// Exact schema of a node-interface satisfaction proof.
pub const NODE_INTERFACE_PROOF_SCHEMA_VERSION: u32 = 0;

/// Portable bound on members in one current interface.
pub const MAX_NODE_INTERFACE_MEMBERS: usize = 64;

/// Portable bound on non-port semantic requirements in one interface.
pub const MAX_NODE_INTERFACE_REQUIREMENTS: usize = 16;

/// Stable reference to one exact named node-interface contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeInterfaceContractRef<'a> {
    /// Stable namespaced semantic identity, independent of a concrete node.
    pub contract_id: Id<'a>,
    /// Exact node-interface descriptor schema.
    pub schema_version: u32,
    /// Exact canonical semantic identity.
    pub semantic_hash: SemanticHash,
}

impl<'a> NodeInterfaceContractRef<'a> {
    /// Validates the portable reference shape.
    pub fn validate(self) -> Result<(), NodeInterfaceContractError<'a>> {
        validate_namespaced_id(self.contract_id)?;
        if self.schema_version != NODE_INTERFACE_CONTRACT_SCHEMA_VERSION {
            return Err(NodeInterfaceContractError::UnsupportedVersion(
                self.schema_version,
            ));
        }
        Ok(())
    }

    /// Converts the interface reference to the common exact descriptor shape.
    #[must_use]
    pub const fn descriptor(self) -> DescriptorRef<'a> {
        DescriptorRef {
            kind: Id("conduit/node-interface-contract"),
            schema_version: self.schema_version,
            semantic_hash: self.semantic_hash,
        }
    }
}

/// Whether a named interface member must exist on a satisfying node.
///
/// Optionality controls only whether the member may be absent. When it is
/// present, its complete `PortContract` must still be satisfied. It does not
/// weaken the nested port's presence, delivery, terminal, sensitivity, or flow
/// guarantees.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterfaceMemberRequirement {
    Required,
    Optional,
}

impl InterfaceMemberRequirement {
    /// Stable descriptor spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Required => "required",
            Self::Optional => "optional",
        }
    }
}

/// One complete named member of an interface boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeInterfaceMember<'a> {
    /// Whether a candidate may omit this entire member.
    pub requirement: InterfaceMemberRequirement,
    /// Complete required directional port contract.
    pub port: PortContract<'a>,
}

/// One exact non-port semantic requirement.
///
/// IDs are namespaced and domain-open. Conventional requirements include
/// configuration, lifecycle, and authority/effect descriptors, but the core
/// does not interpret their domain meaning. Every listed requirement is
/// mandatory and needs a directional `CandidateSubstitutesRequired` decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeInterfaceRequirement<'a> {
    pub id: Id<'a>,
    pub contract: DescriptorRef<'a>,
}

/// Borrowed allocator-free node-interface descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeInterfaceContract<'a> {
    /// Stable namespaced identity.
    pub id: Id<'a>,
    /// Finite named boundary. Source order is not semantic.
    pub members: &'a [NodeInterfaceMember<'a>],
    /// Finite exact configuration, lifecycle, effect, or provider-owned facts.
    ///
    /// An empty slice explicitly defers all such facets in the current schema;
    /// the resulting proof makes no claim about them.
    pub requirements: &'a [NodeInterfaceRequirement<'a>],
}

impl<'a> NodeInterfaceContract<'a> {
    /// Validates current interface invariants.
    pub fn validate(&self) -> Result<(), NodeInterfaceContractError<'a>> {
        validate_namespaced_id(self.id)?;
        if self.members.len() > MAX_NODE_INTERFACE_MEMBERS {
            return Err(NodeInterfaceContractError::TooManyMembers);
        }
        if self.requirements.len() > MAX_NODE_INTERFACE_REQUIREMENTS {
            return Err(NodeInterfaceContractError::TooManyRequirements);
        }
        for (index, member) in self.members.iter().enumerate() {
            validate_port(member.port)
                .map_err(|_| NodeInterfaceContractError::InvalidMember(member.port.id))?;
            if self.members[..index].iter().any(|prior| {
                prior.port.id == member.port.id && prior.port.direction == member.port.direction
            }) {
                return Err(NodeInterfaceContractError::DuplicateMember {
                    id: member.port.id,
                    direction: member.port.direction,
                });
            }
        }
        for (index, requirement) in self.requirements.iter().enumerate() {
            validate_namespaced_id(requirement.id)
                .map_err(|_| NodeInterfaceContractError::InvalidRequirement(requirement.id))?;
            if !valid_descriptor(requirement.contract) {
                return Err(NodeInterfaceContractError::InvalidRequirement(
                    requirement.id,
                ));
            }
            if self.requirements[..index]
                .iter()
                .any(|prior| prior.id == requirement.id)
            {
                return Err(NodeInterfaceContractError::DuplicateRequirement(
                    requirement.id,
                ));
            }
        }
        Ok(())
    }

    /// Computes the order-independent canonical interface identity.
    ///
    /// The caller supplies one hash slot per member and non-port requirement.
    /// No allocation or registry access occurs.
    pub fn semantic_hash(
        &self,
        member_hashes: &mut [SemanticHash],
    ) -> Result<SemanticHash, NodeInterfaceIdentityError<'a>> {
        self.validate()
            .map_err(NodeInterfaceIdentityError::InvalidContract)?;
        let needed = self.members.len() + self.requirements.len();
        if member_hashes.len() < needed {
            return Err(NodeInterfaceIdentityError::ScratchTooSmall);
        }
        for (slot, member) in member_hashes.iter_mut().zip(self.members) {
            *slot =
                hash_interface_member(*member).map_err(NodeInterfaceIdentityError::Canonical)?;
        }
        for (slot, requirement) in member_hashes[self.members.len()..]
            .iter_mut()
            .zip(self.requirements)
        {
            *slot = hash_interface_requirement(*requirement)
                .map_err(NodeInterfaceIdentityError::Canonical)?;
        }
        let fields = [semantic("contract_id", CanonicalValue::Identifier(self.id))];
        semantic_hash_with_hash_set(
            Id("conduit/node-interface-contract"),
            NODE_INTERFACE_CONTRACT_SCHEMA_VERSION,
            &fields,
            Id("members"),
            &member_hashes[..needed],
        )
        .map_err(NodeInterfaceIdentityError::Canonical)
    }

    /// Computes and validates the exact reference to this descriptor.
    pub fn validate_reference(
        &self,
        reference: NodeInterfaceContractRef<'a>,
        member_hashes: &mut [SemanticHash],
    ) -> Result<(), NodeInterfaceIdentityError<'a>> {
        reference
            .validate()
            .map_err(NodeInterfaceIdentityError::InvalidContract)?;
        let identity = self.semantic_hash(member_hashes)?;
        if reference.contract_id != self.id || reference.semantic_hash != identity {
            return Err(NodeInterfaceIdentityError::ReferenceMismatch);
        }
        Ok(())
    }
}

/// Invalid interface descriptor or reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodeInterfaceContractError<'a> {
    InvalidIdentifier(Id<'a>),
    MissingNamespace(Id<'a>),
    UnsupportedVersion(u32),
    TooManyMembers,
    TooManyRequirements,
    InvalidMember(Id<'a>),
    DuplicateMember { id: Id<'a>, direction: Direction },
    InvalidRequirement(Id<'a>),
    DuplicateRequirement(Id<'a>),
}

impl NodeInterfaceContractError<'_> {
    /// Stable machine-readable reason.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidIdentifier(_) => "interface-invalid-identifier",
            Self::MissingNamespace(_) => "interface-missing-namespace",
            Self::UnsupportedVersion(_) => "interface-unsupported-version",
            Self::TooManyMembers => "interface-too-many-members",
            Self::TooManyRequirements => "interface-too-many-requirements",
            Self::InvalidMember(_) => "interface-invalid-member",
            Self::DuplicateMember { .. } => "interface-duplicate-member",
            Self::InvalidRequirement(_) => "interface-invalid-requirement",
            Self::DuplicateRequirement(_) => "interface-duplicate-requirement",
        }
    }
}

impl fmt::Display for NodeInterfaceContractError<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Interface identity construction error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodeInterfaceIdentityError<'a> {
    InvalidContract(NodeInterfaceContractError<'a>),
    ScratchTooSmall,
    ReferenceMismatch,
    Canonical(CanonicalError<Infallible>),
}

impl NodeInterfaceIdentityError<'_> {
    /// Stable machine-readable reason.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidContract(error) => error.as_str(),
            Self::ScratchTooSmall => "interface-identity-scratch-too-small",
            Self::ReferenceMismatch => "interface-reference-mismatch",
            Self::Canonical(_) => "interface-canonicalization-failed",
        }
    }
}

impl fmt::Display for NodeInterfaceIdentityError<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Caller-supplied provider decision for one interface member.
///
/// Exact type identities need no entry: the portable core derives the exact
/// decision. Non-exact identities remain indeterminate without an explicit
/// provider decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeInterfaceTypeDecision<'a> {
    pub member_id: Id<'a>,
    pub direction: Direction,
    pub decision: CompatibilityDecision<'a>,
}

/// Caller-supplied directional decision for one non-port requirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeInterfaceRequirementDecision<'a> {
    pub requirement_id: Id<'a>,
    pub decision: CompatibilityDecision<'a>,
}

/// Stable member-level satisfaction reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodeInterfaceMemberReason {
    Satisfied,
    OptionalAbsent,
    MissingRequired,
    WrongDirection,
    IncompatiblePort,
    ProviderUnavailable,
    IndeterminatePort,
    AmbiguousTypeDecision,
}

impl NodeInterfaceMemberReason {
    /// Stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Satisfied => "interface-member-satisfied",
            Self::OptionalAbsent => "interface-optional-member-absent",
            Self::MissingRequired => "interface-required-member-missing",
            Self::WrongDirection => "interface-member-wrong-direction",
            Self::IncompatiblePort => "interface-member-incompatible",
            Self::ProviderUnavailable => "interface-member-provider-unavailable",
            Self::IndeterminatePort => "interface-member-indeterminate",
            Self::AmbiguousTypeDecision => "interface-member-type-decision-ambiguous",
        }
    }
}

/// Complete proof for one interface member.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeInterfaceMemberProof<'a> {
    pub required: NodeInterfaceMember<'a>,
    pub offered: Option<PortContract<'a>>,
    pub type_decision: Option<CompatibilityDecision<'a>>,
    pub port_decision: Option<PortCompatibilityDecision<'a>>,
    pub outcome: CompatibilityOutcome,
    pub reason: NodeInterfaceMemberReason,
}

/// Stable non-port requirement proof reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodeInterfaceRequirementReason {
    Satisfied,
    FactUnavailable,
    Incompatible,
    Indeterminate,
    Ambiguous,
    InvalidDecision,
}

impl NodeInterfaceRequirementReason {
    /// Stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Satisfied => "interface-requirement-satisfied",
            Self::FactUnavailable => "interface-requirement-fact-unavailable",
            Self::Incompatible => "interface-requirement-incompatible",
            Self::Indeterminate => "interface-requirement-indeterminate",
            Self::Ambiguous => "interface-requirement-ambiguous",
            Self::InvalidDecision => "interface-requirement-decision-invalid",
        }
    }
}

/// Complete proof for one required non-port semantic facet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeInterfaceRequirementProof<'a> {
    pub required: NodeInterfaceRequirement<'a>,
    pub offered: Option<DescriptorRef<'a>>,
    pub decision: Option<CompatibilityDecision<'a>>,
    pub outcome: CompatibilityOutcome,
    pub reason: NodeInterfaceRequirementReason,
}

/// Stable aggregate reason for directional node-interface satisfaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodeInterfaceSatisfactionReason {
    Satisfied,
    MissingRequiredMember,
    WrongDirection,
    IncompatibleMember,
    ProviderUnavailable,
    IndeterminateMember,
    MissingRequirementFact,
    IncompatibleRequirement,
    IndeterminateRequirement,
    Ambiguous,
}

impl NodeInterfaceSatisfactionReason {
    /// Stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Satisfied => "node-interface-satisfied",
            Self::MissingRequiredMember => "node-interface-required-member-missing",
            Self::WrongDirection => "node-interface-member-wrong-direction",
            Self::IncompatibleMember => "node-interface-member-incompatible",
            Self::ProviderUnavailable => "node-interface-provider-unavailable",
            Self::IndeterminateMember => "node-interface-member-indeterminate",
            Self::MissingRequirementFact => "node-interface-requirement-fact-unavailable",
            Self::IncompatibleRequirement => "node-interface-requirement-incompatible",
            Self::IndeterminateRequirement => "node-interface-requirement-indeterminate",
            Self::Ambiguous => "node-interface-ambiguous",
        }
    }
}

/// Complete immutable proof that one concrete node boundary satisfies an
/// interface. Primitive and composite-derived `NodeContract` values use this
/// identical shape and function.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeInterfaceSatisfactionProof<'a, 'proof> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub interface: NodeInterfaceContractRef<'a>,
    pub candidate: DescriptorRef<'a>,
    pub members: &'proof [NodeInterfaceMemberProof<'a>],
    pub requirements: &'proof [NodeInterfaceRequirementProof<'a>],
    pub outcome: CompatibilityOutcome,
    pub reason: NodeInterfaceSatisfactionReason,
}

/// Portable satisfaction validation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodeInterfaceSatisfactionError<'a> {
    InterfaceIdentity(NodeInterfaceIdentityError<'a>),
    InvalidCandidateDescriptor,
    InvalidCandidateContract,
    MemberScratchTooSmall,
    RequirementScratchTooSmall,
    HashScratchTooSmall,
    Canonical(CanonicalError<Infallible>),
}

impl NodeInterfaceSatisfactionError<'_> {
    /// Stable machine-readable reason.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InterfaceIdentity(error) => error.as_str(),
            Self::InvalidCandidateDescriptor => "interface-candidate-descriptor-invalid",
            Self::InvalidCandidateContract => "interface-candidate-contract-invalid",
            Self::MemberScratchTooSmall => "interface-proof-member-scratch-too-small",
            Self::RequirementScratchTooSmall => "interface-proof-requirement-scratch-too-small",
            Self::HashScratchTooSmall => "interface-proof-hash-scratch-too-small",
            Self::Canonical(_) => "interface-proof-canonicalization-failed",
        }
    }
}

impl fmt::Display for NodeInterfaceSatisfactionError<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Directionally proves that a complete ordinary node boundary satisfies a
/// named interface.
///
/// Extra concrete ports are allowed, but every concrete port slice must be
/// well formed and unambiguous. An optional interface member may be absent; if
/// present, it is checked exactly like a required member. The function inserts
/// no adapter and grants no authority.
#[allow(clippy::too_many_arguments)]
pub fn assess_node_interface<'a, 'proof>(
    interface: &NodeInterfaceContract<'a>,
    interface_ref: NodeInterfaceContractRef<'a>,
    candidate_ref: DescriptorRef<'a>,
    candidate: &NodeContract<'a>,
    type_decisions: &[NodeInterfaceTypeDecision<'a>],
    requirement_decisions: &[NodeInterfaceRequirementDecision<'a>],
    member_scratch: &'proof mut [NodeInterfaceMemberProof<'a>],
    requirement_scratch: &'proof mut [NodeInterfaceRequirementProof<'a>],
    interface_hash_scratch: &mut [SemanticHash],
    proof_hash_scratch: &mut [SemanticHash],
) -> Result<NodeInterfaceSatisfactionProof<'a, 'proof>, NodeInterfaceSatisfactionError<'a>> {
    interface
        .validate_reference(interface_ref, interface_hash_scratch)
        .map_err(NodeInterfaceSatisfactionError::InterfaceIdentity)?;
    if candidate_ref.kind != Id("conduit/node-contract") || candidate_ref.schema_version != 0 {
        return Err(NodeInterfaceSatisfactionError::InvalidCandidateDescriptor);
    }
    validate_candidate(candidate)
        .map_err(|()| NodeInterfaceSatisfactionError::InvalidCandidateContract)?;
    if member_scratch.len() < interface.members.len() {
        return Err(NodeInterfaceSatisfactionError::MemberScratchTooSmall);
    }
    if requirement_scratch.len() < interface.requirements.len() {
        return Err(NodeInterfaceSatisfactionError::RequirementScratchTooSmall);
    }
    let fact_count = interface.members.len() + interface.requirements.len();
    if proof_hash_scratch.len() < fact_count {
        return Err(NodeInterfaceSatisfactionError::HashScratchTooSmall);
    }

    for (index, required) in interface.members.iter().copied().enumerate() {
        member_scratch[index] = assess_member(required, candidate, type_decisions);
        proof_hash_scratch[index] = hash_member_proof(member_scratch[index])
            .map_err(NodeInterfaceSatisfactionError::Canonical)?;
    }
    let members = &member_scratch[..interface.members.len()];
    for (index, required) in interface.requirements.iter().copied().enumerate() {
        requirement_scratch[index] = assess_requirement(required, requirement_decisions);
        proof_hash_scratch[interface.members.len() + index] =
            hash_requirement_proof(requirement_scratch[index])
                .map_err(NodeInterfaceSatisfactionError::Canonical)?;
    }
    let requirements = &requirement_scratch[..interface.requirements.len()];
    let outcome = aggregate_outcome(members, requirements);
    let reason = aggregate_reason(members, requirements, outcome);
    let identity = hash_interface_proof(
        interface_ref,
        candidate_ref,
        outcome,
        reason,
        &proof_hash_scratch[..fact_count],
    )
    .map_err(NodeInterfaceSatisfactionError::Canonical)?;

    Ok(NodeInterfaceSatisfactionProof {
        schema_version: NODE_INTERFACE_PROOF_SCHEMA_VERSION,
        identity,
        interface: interface_ref,
        candidate: candidate_ref,
        members,
        requirements,
        outcome,
        reason,
    })
}

fn assess_requirement<'a>(
    required: NodeInterfaceRequirement<'a>,
    decisions: &[NodeInterfaceRequirementDecision<'a>],
) -> NodeInterfaceRequirementProof<'a> {
    let mut matching = decisions
        .iter()
        .filter(|entry| entry.requirement_id == required.id);
    let supplied = matching.next().copied();
    if matching.next().is_some() {
        return requirement_result(
            required,
            None,
            None,
            CompatibilityOutcome::Indeterminate,
            NodeInterfaceRequirementReason::Ambiguous,
        );
    }
    let Some(supplied) = supplied else {
        return requirement_result(
            required,
            None,
            None,
            CompatibilityOutcome::Indeterminate,
            NodeInterfaceRequirementReason::FactUnavailable,
        );
    };
    let offered = match supplied.decision.query {
        CompatibilityQuery::CandidateSubstitutesRequired {
            required: decision_required,
            candidate,
        } if decision_required == required.contract => candidate,
        _ => {
            return requirement_result(
                required,
                None,
                Some(supplied.decision),
                CompatibilityOutcome::Indeterminate,
                NodeInterfaceRequirementReason::InvalidDecision,
            );
        }
    };
    let reason = match supplied.decision.outcome {
        CompatibilityOutcome::Compatible => NodeInterfaceRequirementReason::Satisfied,
        CompatibilityOutcome::Incompatible => NodeInterfaceRequirementReason::Incompatible,
        CompatibilityOutcome::Indeterminate => NodeInterfaceRequirementReason::Indeterminate,
    };
    requirement_result(
        required,
        Some(offered),
        Some(supplied.decision),
        supplied.decision.outcome,
        reason,
    )
}

const fn requirement_result<'a>(
    required: NodeInterfaceRequirement<'a>,
    offered: Option<DescriptorRef<'a>>,
    decision: Option<CompatibilityDecision<'a>>,
    outcome: CompatibilityOutcome,
    reason: NodeInterfaceRequirementReason,
) -> NodeInterfaceRequirementProof<'a> {
    NodeInterfaceRequirementProof {
        required,
        offered,
        decision,
        outcome,
        reason,
    }
}

fn assess_member<'a>(
    required: NodeInterfaceMember<'a>,
    candidate: &NodeContract<'a>,
    type_decisions: &[NodeInterfaceTypeDecision<'a>],
) -> NodeInterfaceMemberProof<'a> {
    let same = ports_for_direction(candidate, required.port.direction)
        .iter()
        .copied()
        .find(|port| port.id == required.port.id);
    if same.is_none()
        && ports_for_direction(candidate, opposite(required.port.direction))
            .iter()
            .any(|port| port.id == required.port.id)
    {
        return member_result(
            required,
            None,
            None,
            None,
            CompatibilityOutcome::Incompatible,
            NodeInterfaceMemberReason::WrongDirection,
        );
    }
    let Some(offered) = same else {
        return if required.requirement == InterfaceMemberRequirement::Optional {
            member_result(
                required,
                None,
                None,
                None,
                CompatibilityOutcome::Compatible,
                NodeInterfaceMemberReason::OptionalAbsent,
            )
        } else {
            member_result(
                required,
                None,
                None,
                None,
                CompatibilityOutcome::Incompatible,
                NodeInterfaceMemberReason::MissingRequired,
            )
        };
    };

    let mut matching = type_decisions.iter().filter(|entry| {
        entry.member_id == required.port.id && entry.direction == required.port.direction
    });
    let supplied = matching.next().copied();
    if matching.next().is_some() {
        return member_result(
            required,
            Some(offered),
            None,
            None,
            CompatibilityOutcome::Indeterminate,
            NodeInterfaceMemberReason::AmbiguousTypeDecision,
        );
    }
    let type_decision = supplied.map_or_else(
        || {
            let (consumer, producer) = if required.port.direction == Direction::Input {
                (offered.value_type, required.port.value_type)
            } else {
                (required.port.value_type, offered.value_type)
            };
            assess_type_contract_exact(consumer, producer)
        },
        |entry| entry.decision,
    );
    let port_decision = assess_port_substitution(required.port, offered, type_decision);
    let reason = match port_decision.outcome {
        CompatibilityOutcome::Compatible => NodeInterfaceMemberReason::Satisfied,
        CompatibilityOutcome::Incompatible => NodeInterfaceMemberReason::IncompatiblePort,
        CompatibilityOutcome::Indeterminate
            if type_decision.reason == CompatibilityReason::TypeProviderUnavailable
                || type_decision.reason == CompatibilityReason::ValueProviderRequired =>
        {
            NodeInterfaceMemberReason::ProviderUnavailable
        }
        CompatibilityOutcome::Indeterminate => NodeInterfaceMemberReason::IndeterminatePort,
    };
    member_result(
        required,
        Some(offered),
        Some(type_decision),
        Some(port_decision),
        port_decision.outcome,
        reason,
    )
}

const fn member_result<'a>(
    required: NodeInterfaceMember<'a>,
    offered: Option<PortContract<'a>>,
    type_decision: Option<CompatibilityDecision<'a>>,
    port_decision: Option<PortCompatibilityDecision<'a>>,
    outcome: CompatibilityOutcome,
    reason: NodeInterfaceMemberReason,
) -> NodeInterfaceMemberProof<'a> {
    NodeInterfaceMemberProof {
        required,
        offered,
        type_decision,
        port_decision,
        outcome,
        reason,
    }
}

fn aggregate_outcome(
    members: &[NodeInterfaceMemberProof<'_>],
    requirements: &[NodeInterfaceRequirementProof<'_>],
) -> CompatibilityOutcome {
    if members
        .iter()
        .any(|member| member.outcome == CompatibilityOutcome::Incompatible)
        || requirements
            .iter()
            .any(|requirement| requirement.outcome == CompatibilityOutcome::Incompatible)
    {
        CompatibilityOutcome::Incompatible
    } else if members
        .iter()
        .any(|member| member.outcome == CompatibilityOutcome::Indeterminate)
        || requirements
            .iter()
            .any(|requirement| requirement.outcome == CompatibilityOutcome::Indeterminate)
    {
        CompatibilityOutcome::Indeterminate
    } else {
        CompatibilityOutcome::Compatible
    }
}

fn aggregate_reason(
    members: &[NodeInterfaceMemberProof<'_>],
    requirements: &[NodeInterfaceRequirementProof<'_>],
    outcome: CompatibilityOutcome,
) -> NodeInterfaceSatisfactionReason {
    if outcome == CompatibilityOutcome::Compatible {
        return NodeInterfaceSatisfactionReason::Satisfied;
    }
    let contains = |reason| members.iter().any(|member| member.reason == reason);
    let requirement_contains = |reason| {
        requirements
            .iter()
            .any(|requirement| requirement.reason == reason)
    };
    if outcome == CompatibilityOutcome::Incompatible {
        if contains(NodeInterfaceMemberReason::WrongDirection) {
            NodeInterfaceSatisfactionReason::WrongDirection
        } else if contains(NodeInterfaceMemberReason::MissingRequired) {
            NodeInterfaceSatisfactionReason::MissingRequiredMember
        } else if contains(NodeInterfaceMemberReason::IncompatiblePort) {
            NodeInterfaceSatisfactionReason::IncompatibleMember
        } else {
            NodeInterfaceSatisfactionReason::IncompatibleRequirement
        }
    } else if contains(NodeInterfaceMemberReason::AmbiguousTypeDecision)
        || requirement_contains(NodeInterfaceRequirementReason::Ambiguous)
    {
        NodeInterfaceSatisfactionReason::Ambiguous
    } else if contains(NodeInterfaceMemberReason::ProviderUnavailable) {
        NodeInterfaceSatisfactionReason::ProviderUnavailable
    } else if requirement_contains(NodeInterfaceRequirementReason::FactUnavailable) {
        NodeInterfaceSatisfactionReason::MissingRequirementFact
    } else if requirement_contains(NodeInterfaceRequirementReason::Indeterminate)
        || requirement_contains(NodeInterfaceRequirementReason::InvalidDecision)
    {
        NodeInterfaceSatisfactionReason::IndeterminateRequirement
    } else {
        NodeInterfaceSatisfactionReason::IndeterminateMember
    }
}

fn valid_descriptor(descriptor: DescriptorRef<'_>) -> bool {
    Id::new(descriptor.kind.as_str()).is_ok() && descriptor.schema_version == 0
}

fn validate_namespaced_id(id: Id<'_>) -> Result<(), NodeInterfaceContractError<'_>> {
    Id::new(id.as_str()).map_err(|_| NodeInterfaceContractError::InvalidIdentifier(id))?;
    if id.as_str().split_once('/').is_none() {
        return Err(NodeInterfaceContractError::MissingNamespace(id));
    }
    Ok(())
}

fn validate_port(port: PortContract<'_>) -> Result<(), ()> {
    Id::new(port.id.as_str()).map_err(|_| ())?;
    port.value_type.validate().map_err(|_| ())
}

fn validate_candidate(candidate: &NodeContract<'_>) -> Result<(), ()> {
    Id::new(candidate.id.as_str()).map_err(|_| ())?;
    candidate.config.validate().map_err(|_| ())?;
    for (expected, ports) in [
        (Direction::Input, candidate.inputs),
        (Direction::Output, candidate.outputs),
    ] {
        for (index, port) in ports.iter().copied().enumerate() {
            validate_port(port)?;
            if port.direction != expected || ports[..index].iter().any(|prior| prior.id == port.id)
            {
                return Err(());
            }
        }
    }
    Ok(())
}

const fn ports_for_direction<'a>(
    candidate: &NodeContract<'a>,
    direction: Direction,
) -> &'a [PortContract<'a>] {
    match direction {
        Direction::Input => candidate.inputs,
        Direction::Output => candidate.outputs,
    }
}

const fn opposite(direction: Direction) -> Direction {
    match direction {
        Direction::Input => Direction::Output,
        Direction::Output => Direction::Input,
    }
}

fn hash_interface_member(
    member: NodeInterfaceMember<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let port_hash = member.port.semantic_hash()?;
    CanonicalDescriptor {
        kind: Id("conduit/node-interface-member"),
        schema_version: 0,
        body: CanonicalValue::Map(&[
            semantic(
                "requirement",
                CanonicalValue::Identifier(Id(member.requirement.as_str())),
            ),
            semantic("port", CanonicalValue::Bytes(port_hash.as_bytes())),
        ]),
    }
    .semantic_hash()
}

fn hash_interface_requirement(
    requirement: NodeInterfaceRequirement<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let contract_fields = descriptor_fields(&requirement.contract);
    CanonicalDescriptor {
        kind: Id("conduit/node-interface-requirement"),
        schema_version: 0,
        body: CanonicalValue::Map(&[
            semantic("id", CanonicalValue::Identifier(requirement.id)),
            semantic("contract", CanonicalValue::Map(&contract_fields)),
        ]),
    }
    .semantic_hash()
}

fn hash_member_proof(
    proof: NodeInterfaceMemberProof<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let required_hash = hash_interface_member(proof.required)?;
    let offered_hash = match proof.offered {
        Some(port) => Some(port.semantic_hash()?),
        None => None,
    };
    let type_outcome = proof
        .type_decision
        .map(|decision| Id(decision.outcome.as_str()));
    let type_reason = proof
        .type_decision
        .map(|decision| Id(decision.reason.as_str()));
    CanonicalDescriptor {
        kind: Id("conduit/node-interface-member-proof"),
        schema_version: 0,
        body: CanonicalValue::Map(&[
            semantic(
                "required_member",
                CanonicalValue::Bytes(required_hash.as_bytes()),
            ),
            semantic(
                "offered_port",
                offered_hash.as_ref().map_or(CanonicalValue::Null, |hash| {
                    CanonicalValue::Bytes(hash.as_bytes())
                }),
            ),
            semantic(
                "type_outcome",
                type_outcome.map_or(CanonicalValue::Null, CanonicalValue::Identifier),
            ),
            semantic(
                "type_reason",
                type_reason.map_or(CanonicalValue::Null, CanonicalValue::Identifier),
            ),
            semantic(
                "outcome",
                CanonicalValue::Identifier(Id(proof.outcome.as_str())),
            ),
            semantic(
                "reason",
                CanonicalValue::Identifier(Id(proof.reason.as_str())),
            ),
        ]),
    }
    .semantic_hash()
}

fn hash_requirement_proof(
    proof: NodeInterfaceRequirementProof<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let required_hash = hash_interface_requirement(proof.required)?;
    let offered_fields = proof.offered.as_ref().map(descriptor_fields);
    let offered = offered_fields
        .as_ref()
        .map_or(CanonicalValue::Null, |fields| CanonicalValue::Map(fields));
    let decision_reason = proof
        .decision
        .map(|decision| Id(decision.reason.as_str()))
        .map_or(CanonicalValue::Null, CanonicalValue::Identifier);
    CanonicalDescriptor {
        kind: Id("conduit/node-interface-requirement-proof"),
        schema_version: 0,
        body: CanonicalValue::Map(&[
            semantic("required", CanonicalValue::Bytes(required_hash.as_bytes())),
            semantic("offered", offered),
            semantic("decision_reason", decision_reason),
            semantic(
                "outcome",
                CanonicalValue::Identifier(Id(proof.outcome.as_str())),
            ),
            semantic(
                "reason",
                CanonicalValue::Identifier(Id(proof.reason.as_str())),
            ),
        ]),
    }
    .semantic_hash()
}

fn hash_interface_proof(
    interface: NodeInterfaceContractRef<'_>,
    candidate: DescriptorRef<'_>,
    outcome: CompatibilityOutcome,
    reason: NodeInterfaceSatisfactionReason,
    member_hashes: &[SemanticHash],
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let interface_descriptor = interface.descriptor();
    let interface_fields = descriptor_fields(&interface_descriptor);
    let candidate_fields = descriptor_fields(&candidate);
    let fields = [
        semantic("interface", CanonicalValue::Map(&interface_fields)),
        semantic("candidate", CanonicalValue::Map(&candidate_fields)),
        semantic("outcome", CanonicalValue::Identifier(Id(outcome.as_str()))),
        semantic("reason", CanonicalValue::Identifier(Id(reason.as_str()))),
    ];
    semantic_hash_with_hash_set(
        Id("conduit/node-interface-satisfaction-proof"),
        NODE_INTERFACE_PROOF_SCHEMA_VERSION,
        &fields,
        Id("members"),
        member_hashes,
    )
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

const fn semantic<'a>(name: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    }
}
