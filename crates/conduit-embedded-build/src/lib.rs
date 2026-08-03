//! Hosted lowering from one validated exact plan into firmware-owned fixed data.
//!
//! The generated module contains no parser, resolver, registry, allocator, or
//! authority source. It binds exact plan, policy package, lock, Conduit
//! revision, implementation, driver, port ordinal, queue, and profile facts.

use std::fmt::{self, Write};

use conduit_core::{
    BlockingFairness, BoundednessProfile, CancellationGuarantee, CanonicalDescriptor,
    CanonicalValue, Direction, ExecutionPlan, FieldDisposition, Id, InstancePath, MapField,
    PinnedDescriptor, PlanAuthority, PlanResourceBinding, PlanValidationContext,
    PlanValidationError, Pressure, ResolvedPlanNode, ResourceRef, SemanticHash,
    ValueRepresentation, validate_execution_plan,
};
use conduit_embedded::{
    EmbeddedError, EmbeddedProfile, STATIC_PLAN_SCHEMA_VERSION, StaticCord, StaticHostOperation,
    StaticNode, StaticPlan, StorageShape, validate_static_plan,
};

pub const GENERATED_EMBEDDED_PLAN_SCHEMA_VERSION: u32 = 0;
const ZERO_HASH: SemanticHash = SemanticHash::from_bytes([0; 32]);

/// Source/package identity that does not belong to the exact execution plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmbeddedProgramIdentity<'a> {
    /// Full lowercase hexadecimal Git commit for the imported Conduit source.
    pub conduit_revision: &'a str,
    pub policy_package_hash: SemanticHash,
    pub policy_lock_hash: SemanticHash,
}

/// Firmware-owned driver and port ordinals for one exact planned instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmbeddedNodeBinding<'a> {
    pub instance: InstancePath<'a>,
    pub driver: PinnedDescriptor<'a>,
    pub input_ports: &'a [Id<'a>],
    pub output_ports: &'a [Id<'a>],
    pub host_operations: &'a [EmbeddedHostOperationBinding<'a>],
}

/// Firmware ordinal assigned to one exact effect and resource from the plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmbeddedHostOperationBinding<'a> {
    pub ordinal: u16,
    pub effect_hash: SemanticHash,
    pub resource_binding: Id<'a>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedPin {
    pub id: String,
    pub schema_version: u32,
    pub semantic_hash: SemanticHash,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedStaticNode {
    pub semantic_path: String,
    pub implementation: GeneratedPin,
    pub driver: GeneratedPin,
    pub input_port_ids: Vec<String>,
    pub output_port_ids: Vec<String>,
    pub host_operations: Vec<GeneratedHostOperation>,
    pub input_ports: u8,
    pub output_ports: u8,
    pub maximum_step_work: u16,
    pub nesting_depth: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedHostOperation {
    pub ordinal: u16,
    pub operation: String,
    pub resource_binding: String,
    pub resource_kind: String,
    pub resource_id: String,
    pub effect_hash: SemanticHash,
    pub grant_hash: SemanticHash,
    pub resource_lease_hash: SemanticHash,
    pub commit_profile_hash: SemanticHash,
    pub capability_id: String,
    pub grant_id: String,
    pub host: String,
    pub check_at_use: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedStaticCord {
    pub semantic_id: String,
    pub producer_node: u16,
    pub producer_port: u8,
    pub consumer_node: u16,
    pub consumer_port: u8,
    pub slot_start: u16,
    pub capacity: u16,
    pub maximum_value_bytes: u16,
}

/// Owned build output that can be inspected, rendered, or borrowed as a static plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedEmbeddedPlan {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub conduit_revision: String,
    pub policy_package_hash: SemanticHash,
    pub policy_lock_hash: SemanticHash,
    pub source_semantic_hash: SemanticHash,
    pub full_plan_hash: SemanticHash,
    pub profile: EmbeddedProfile,
    pub nodes: Vec<GeneratedStaticNode>,
    pub cords: Vec<GeneratedStaticCord>,
}

impl GeneratedEmbeddedPlan {
    /// Borrow the owned output through the allocator-free executor representation.
    pub fn with_static_plan<R>(&self, action: impl FnOnce(StaticPlan<'_>) -> R) -> R {
        let host_operations = self
            .nodes
            .iter()
            .map(|node| {
                node.host_operations
                    .iter()
                    .map(borrowed_host_operation)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let nodes = self
            .nodes
            .iter()
            .zip(&host_operations)
            .map(|(node, host_operations)| StaticNode {
                semantic_path: Id(&node.semantic_path),
                implementation: borrowed_pin(&node.implementation),
                driver: borrowed_pin(&node.driver),
                host_operations,
                input_ports: node.input_ports,
                output_ports: node.output_ports,
                maximum_step_work: node.maximum_step_work,
                nesting_depth: node.nesting_depth,
            })
            .collect::<Vec<_>>();
        let cords = self
            .cords
            .iter()
            .map(|cord| StaticCord {
                semantic_id: Id(&cord.semantic_id),
                producer_node: cord.producer_node,
                producer_port: cord.producer_port,
                consumer_node: cord.consumer_node,
                consumer_port: cord.consumer_port,
                slot_start: cord.slot_start,
                capacity: cord.capacity,
                maximum_value_bytes: cord.maximum_value_bytes,
            })
            .collect::<Vec<_>>();
        action(StaticPlan {
            schema_version: STATIC_PLAN_SCHEMA_VERSION,
            generated_plan_hash: self.identity,
            full_plan_hash: self.full_plan_hash,
            profile_hash: self.profile.identity,
            nodes: &nodes,
            cords: &cords,
        })
    }

    /// Render one current Rust module for inclusion from firmware `OUT_DIR`.
    #[must_use]
    pub fn render_rust_module(&self) -> String {
        let mut output = String::from(
            "// Generated by conduit-embedded-build. Do not edit.\n\
             pub const GENERATED_EMBEDDED_PLAN_SCHEMA_VERSION: u32 = 0;\n",
        );
        writeln!(
            output,
            "pub const CONDUIT_REVISION: &str = {:?};",
            self.conduit_revision
        )
        .expect("String writes cannot fail");
        write_hash_constant(
            &mut output,
            "GENERATED_EMBEDDED_PLAN_IDENTITY",
            self.identity,
        );
        write_hash_constant(&mut output, "POLICY_PACKAGE_HASH", self.policy_package_hash);
        write_hash_constant(&mut output, "POLICY_LOCK_HASH", self.policy_lock_hash);
        write_hash_constant(
            &mut output,
            "SOURCE_SEMANTIC_HASH",
            self.source_semantic_hash,
        );
        write_hash_constant(&mut output, "FULL_PLAN_HASH", self.full_plan_hash);
        render_profile(&mut output, self.profile);
        writeln!(
            output,
            "pub const GENERATED_NODE_PORT_BINDINGS: [(&[conduit_core::Id<'static>], &[conduit_core::Id<'static>]); {}] = [",
            self.nodes.len()
        )
        .expect("String writes cannot fail");
        for node in &self.nodes {
            output.push_str("    (&[");
            render_id_list(&mut output, &node.input_port_ids);
            output.push_str("], &[");
            render_id_list(&mut output, &node.output_port_ids);
            output.push_str("]),\n");
        }
        output.push_str("];\n");
        writeln!(
            output,
            "pub const GENERATED_NODES: [conduit_embedded::StaticNode<'static>; {}] = [",
            self.nodes.len()
        )
        .expect("String writes cannot fail");
        for node in &self.nodes {
            output.push_str("    conduit_embedded::StaticNode {\n");
            writeln!(
                output,
                "        semantic_path: conduit_core::Id({:?}),",
                node.semantic_path
            )
            .expect("String writes cannot fail");
            render_pin(&mut output, "implementation", &node.implementation, 8);
            render_pin(&mut output, "driver", &node.driver, 8);
            output.push_str("        host_operations: &[\n");
            for binding in &node.host_operations {
                render_host_operation(&mut output, binding, 12);
            }
            output.push_str("        ],\n");
            writeln!(output, "        input_ports: {},", node.input_ports)
                .expect("String writes cannot fail");
            writeln!(output, "        output_ports: {},", node.output_ports)
                .expect("String writes cannot fail");
            writeln!(
                output,
                "        maximum_step_work: {},",
                node.maximum_step_work
            )
            .expect("String writes cannot fail");
            writeln!(output, "        nesting_depth: {},", node.nesting_depth)
                .expect("String writes cannot fail");
            output.push_str("    },\n");
        }
        output.push_str("];\n");
        writeln!(
            output,
            "pub const GENERATED_CORDS: [conduit_embedded::StaticCord<'static>; {}] = [",
            self.cords.len()
        )
        .expect("String writes cannot fail");
        for cord in &self.cords {
            output.push_str("    conduit_embedded::StaticCord {\n");
            writeln!(
                output,
                "        semantic_id: conduit_core::Id({:?}),",
                cord.semantic_id
            )
            .expect("String writes cannot fail");
            writeln!(output, "        producer_node: {},", cord.producer_node)
                .expect("String writes cannot fail");
            writeln!(output, "        producer_port: {},", cord.producer_port)
                .expect("String writes cannot fail");
            writeln!(output, "        consumer_node: {},", cord.consumer_node)
                .expect("String writes cannot fail");
            writeln!(output, "        consumer_port: {},", cord.consumer_port)
                .expect("String writes cannot fail");
            writeln!(output, "        slot_start: {},", cord.slot_start)
                .expect("String writes cannot fail");
            writeln!(output, "        capacity: {},", cord.capacity)
                .expect("String writes cannot fail");
            writeln!(
                output,
                "        maximum_value_bytes: {},",
                cord.maximum_value_bytes
            )
            .expect("String writes cannot fail");
            output.push_str("    },\n");
        }
        output.push_str("];\n");
        output.push_str(
            "pub const GENERATED_STATIC_PLAN: conduit_embedded::StaticPlan<'static> = \
             conduit_embedded::StaticPlan {\n\
                 schema_version: conduit_embedded::STATIC_PLAN_SCHEMA_VERSION,\n\
                 generated_plan_hash: GENERATED_EMBEDDED_PLAN_IDENTITY,\n\
                 full_plan_hash: FULL_PLAN_HASH,\n\
                 profile_hash: GENERATED_EMBEDDED_PROFILE.identity,\n\
                 nodes: &GENERATED_NODES,\n\
                 cords: &GENERATED_CORDS,\n\
             };\n",
        );
        output
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsupportedPlanFeature {
    Workloads,
    ValueEnvelopes,
    ClockConversions,
    FeedbackBoundaries,
    DistributedCords,
    FanOuts,
    Merges,
    EventStreams,
    RuntimeEvidence,
    EvidenceProvider,
    WatchAdmissions,
    Jobs,
    SatisfactionProofs,
    AuthorityConstraints,
    AuthorityAdministrativeContainment,
    AuthorityPolicyBudgets,
    HazardClosure,
    InstancePools,
    Supervisions,
    NonHardBoundedNode,
    NonBoundedCancellation,
    UnenforcedStepBound,
    NonBlockingFifoPressure,
    NonExactQueueByteCapacity,
    NonFullPressureWatermarks,
    SharedPort,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BindingError {
    Count,
    MissingNode,
    DuplicateNode,
    InvalidDriver,
    PortCount,
    InvalidPort,
    DuplicatePort,
    PortRepresentation,
    HostOperationCount,
    InvalidHostOperation,
    DuplicateHostOperation,
    HostEffect,
    HostResource,
    UnusedHostAuthority,
    CordEndpoint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenerationError {
    InvalidConduitRevision,
    InvalidProgramIdentity,
    Plan(PlanValidationError),
    Profile(EmbeddedError),
    Unsupported(UnsupportedPlanFeature),
    Binding(BindingError),
    RangeExceeded,
    ArithmeticOverflow,
    Identity,
}

impl fmt::Display for GenerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConduitRevision => {
                formatter.write_str("Conduit revision must be one full lowercase Git commit")
            }
            Self::InvalidProgramIdentity => {
                formatter.write_str("policy package and lock identities must be exact")
            }
            Self::Plan(error) => write!(formatter, "exact plan is not valid: {error}"),
            Self::Profile(error) => write!(formatter, "embedded profile is not valid: {error}"),
            Self::Unsupported(feature) => {
                write!(
                    formatter,
                    "exact plan feature is not supported by firmware: {feature:?}"
                )
            }
            Self::Binding(error) => write!(formatter, "embedded binding is invalid: {error:?}"),
            Self::RangeExceeded => formatter.write_str("exact plan exceeds fixed integer ranges"),
            Self::ArithmeticOverflow => formatter.write_str("fixed plan accounting overflowed"),
            Self::Identity => formatter.write_str("generated plan identity could not be computed"),
        }
    }
}

impl std::error::Error for GenerationError {}

/// Validate and lower one exact plan. Unsupported semantics fail closed rather
/// than being approximated by the constrained executor.
pub fn generate_embedded_plan(
    plan: &ExecutionPlan<'_>,
    context: PlanValidationContext<'_>,
    profile: EmbeddedProfile,
    program: EmbeddedProgramIdentity<'_>,
    bindings: &[EmbeddedNodeBinding<'_>],
) -> Result<GeneratedEmbeddedPlan, GenerationError> {
    validate_program_identity(program)?;
    let scratch_len = plan
        .identity_fact_count()
        .map_err(|_| GenerationError::Identity)?;
    let mut scratch = vec![ZERO_HASH; scratch_len];
    validate_execution_plan(plan, context, &mut scratch).map_err(GenerationError::Plan)?;
    profile.validate().map_err(GenerationError::Profile)?;
    reject_unrepresented_features(plan)?;
    if bindings.len() != plan.nodes.len() {
        return Err(GenerationError::Binding(BindingError::Count));
    }

    let mut nodes = Vec::with_capacity(plan.nodes.len());
    for node in plan.nodes {
        let binding = unique_binding(bindings, node.instance)?;
        validate_node_binding(node, binding)?;
        let execution = node.execution_profile.ok_or(GenerationError::Unsupported(
            UnsupportedPlanFeature::NonHardBoundedNode,
        ))?;
        if execution.boundedness != BoundednessProfile::Hard {
            return Err(GenerationError::Unsupported(
                UnsupportedPlanFeature::NonHardBoundedNode,
            ));
        }
        if execution.cancellation != CancellationGuarantee::Bounded {
            return Err(GenerationError::Unsupported(
                UnsupportedPlanFeature::NonBoundedCancellation,
            ));
        }
        if !execution.step_bound_enforced {
            return Err(GenerationError::Unsupported(
                UnsupportedPlanFeature::UnenforcedStepBound,
            ));
        }
        let maximum_step_work = u16::try_from(execution.limits.max_step_work)
            .map_err(|_| GenerationError::RangeExceeded)?;
        if maximum_step_work == 0 {
            return Err(GenerationError::RangeExceeded);
        }
        let host_operations = generate_host_operations(plan, node, binding)?;
        nodes.push(GeneratedStaticNode {
            semantic_path: node.instance.as_str().to_owned(),
            implementation: owned_pin(node.implementation),
            driver: owned_pin(binding.driver),
            input_port_ids: binding
                .input_ports
                .iter()
                .map(|port| port.as_str().to_owned())
                .collect(),
            output_port_ids: binding
                .output_ports
                .iter()
                .map(|port| port.as_str().to_owned())
                .collect(),
            host_operations,
            input_ports: u8::try_from(binding.input_ports.len())
                .map_err(|_| GenerationError::RangeExceeded)?,
            output_ports: u8::try_from(binding.output_ports.len())
                .map_err(|_| GenerationError::RangeExceeded)?,
            maximum_step_work,
            // The exact plan is already flattened to primitive nodes.
            nesting_depth: 1,
        });
    }

    let represented_authorities = nodes
        .iter()
        .map(|node| node.host_operations.len())
        .sum::<usize>();
    if represented_authorities != plan.authorities.len() {
        return Err(GenerationError::Binding(BindingError::UnusedHostAuthority));
    }

    let mut cords = Vec::with_capacity(plan.cords.len());
    let mut slot_start = 0_u16;
    for cord in plan.cords {
        if !matches!(cord.flow.pressure, Pressure::Block(BlockingFairness::Fifo)) {
            return Err(GenerationError::Unsupported(
                UnsupportedPlanFeature::NonBlockingFifoPressure,
            ));
        }
        let capacity = cord.flow.capacity.items();
        let maximum_value_bytes = u16::try_from(cord.flow.capacity.max_value_bytes())
            .map_err(|_| GenerationError::RangeExceeded)?;
        let exact_queue_bytes = u64::from(capacity)
            .checked_mul(u64::from(maximum_value_bytes))
            .ok_or(GenerationError::ArithmeticOverflow)?;
        if exact_queue_bytes != cord.flow.capacity.max_queued_bytes() {
            return Err(GenerationError::Unsupported(
                UnsupportedPlanFeature::NonExactQueueByteCapacity,
            ));
        }
        if cord.flow.watermarks.high_items() != capacity
            || cord.flow.watermarks.low_items() != capacity.saturating_sub(1)
        {
            return Err(GenerationError::Unsupported(
                UnsupportedPlanFeature::NonFullPressureWatermarks,
            ));
        }
        let producer_node = node_ordinal(plan, cord.from.node)?;
        let consumer_node = node_ordinal(plan, cord.to.node)?;
        let producer_binding = unique_binding(bindings, cord.from.node)?;
        let consumer_binding = unique_binding(bindings, cord.to.node)?;
        let producer_port = port_ordinal(producer_binding.output_ports, cord.from.port)?;
        let consumer_port = port_ordinal(consumer_binding.input_ports, cord.to.port)?;
        validate_endpoint_representation(
            plan.nodes[usize::from(producer_node)],
            Direction::Output,
            cord.from.port,
            u32::from(maximum_value_bytes),
        )?;
        validate_endpoint_representation(
            plan.nodes[usize::from(consumer_node)],
            Direction::Input,
            cord.to.port,
            u32::from(maximum_value_bytes),
        )?;
        if cords.iter().any(|existing: &GeneratedStaticCord| {
            (existing.producer_node, existing.producer_port) == (producer_node, producer_port)
                || (existing.consumer_node, existing.consumer_port)
                    == (consumer_node, consumer_port)
        }) {
            return Err(GenerationError::Unsupported(
                UnsupportedPlanFeature::SharedPort,
            ));
        }
        cords.push(GeneratedStaticCord {
            semantic_id: cord.id.as_str().to_owned(),
            producer_node,
            producer_port,
            consumer_node,
            consumer_port,
            slot_start,
            capacity,
            maximum_value_bytes,
        });
        slot_start = slot_start
            .checked_add(capacity)
            .ok_or(GenerationError::ArithmeticOverflow)?;
    }

    let mut generated = GeneratedEmbeddedPlan {
        schema_version: GENERATED_EMBEDDED_PLAN_SCHEMA_VERSION,
        identity: ZERO_HASH,
        conduit_revision: program.conduit_revision.to_owned(),
        policy_package_hash: program.policy_package_hash,
        policy_lock_hash: program.policy_lock_hash,
        source_semantic_hash: plan.source_semantic_hash,
        full_plan_hash: plan.identity,
        profile,
        nodes,
        cords,
    };
    generated.identity = generated_identity(&generated)?;
    validate_generated_shape(&generated)?;
    Ok(generated)
}

fn validate_program_identity(program: EmbeddedProgramIdentity<'_>) -> Result<(), GenerationError> {
    if program.conduit_revision.len() != 40
        || !program
            .conduit_revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(GenerationError::InvalidConduitRevision);
    }
    if program.policy_package_hash == ZERO_HASH || program.policy_lock_hash == ZERO_HASH {
        return Err(GenerationError::InvalidProgramIdentity);
    }
    Ok(())
}

fn reject_unrepresented_features(plan: &ExecutionPlan<'_>) -> Result<(), GenerationError> {
    let checks = [
        (
            !plan.workloads.is_empty(),
            UnsupportedPlanFeature::Workloads,
        ),
        (
            !plan.value_envelopes.is_empty(),
            UnsupportedPlanFeature::ValueEnvelopes,
        ),
        (
            !plan.clock_conversions.is_empty(),
            UnsupportedPlanFeature::ClockConversions,
        ),
        (
            !plan.feedback_boundaries.is_empty(),
            UnsupportedPlanFeature::FeedbackBoundaries,
        ),
        (
            !plan.distributed_cords.is_empty(),
            UnsupportedPlanFeature::DistributedCords,
        ),
        (!plan.fanouts.is_empty(), UnsupportedPlanFeature::FanOuts),
        (!plan.merges.is_empty(), UnsupportedPlanFeature::Merges),
        (
            !plan.event_streams.is_empty(),
            UnsupportedPlanFeature::EventStreams,
        ),
        (
            plan.runtime_evidence.is_some(),
            UnsupportedPlanFeature::RuntimeEvidence,
        ),
        (
            plan.evidence_provider.is_some(),
            UnsupportedPlanFeature::EvidenceProvider,
        ),
        (
            !plan.watch_admissions.is_empty(),
            UnsupportedPlanFeature::WatchAdmissions,
        ),
        (!plan.jobs.is_empty(), UnsupportedPlanFeature::Jobs),
        (
            !plan.satisfaction_proofs.is_empty(),
            UnsupportedPlanFeature::SatisfactionProofs,
        ),
        (
            plan.hazard_closure.is_some(),
            UnsupportedPlanFeature::HazardClosure,
        ),
        (
            !plan.instance_pools.is_empty(),
            UnsupportedPlanFeature::InstancePools,
        ),
        (
            !plan.supervisions.is_empty(),
            UnsupportedPlanFeature::Supervisions,
        ),
    ];
    if let Some((_, feature)) = checks.into_iter().find(|(present, _)| *present) {
        return Err(GenerationError::Unsupported(feature));
    }
    Ok(())
}

fn generate_host_operations(
    plan: &ExecutionPlan<'_>,
    node: &ResolvedPlanNode<'_>,
    binding: &EmbeddedNodeBinding<'_>,
) -> Result<Vec<GeneratedHostOperation>, GenerationError> {
    if binding.host_operations.len() != node.required_effects.len()
        || node.required_resources.iter().any(|required| {
            !binding
                .host_operations
                .iter()
                .any(|operation| operation.resource_binding == *required)
        })
    {
        return Err(GenerationError::Binding(BindingError::HostOperationCount));
    }
    let mut generated = Vec::with_capacity(binding.host_operations.len());
    for (index, requested) in binding.host_operations.iter().enumerate() {
        if requested.effect_hash == ZERO_HASH
            || Id::new(requested.resource_binding.as_str()).is_err()
        {
            return Err(GenerationError::Binding(BindingError::InvalidHostOperation));
        }
        if binding.host_operations[..index].iter().any(|prior| {
            prior.ordinal == requested.ordinal || prior.effect_hash == requested.effect_hash
        }) {
            return Err(GenerationError::Binding(
                BindingError::DuplicateHostOperation,
            ));
        }
        if !node.required_effects.contains(&requested.effect_hash) {
            return Err(GenerationError::Binding(BindingError::HostEffect));
        }
        if !node
            .required_resources
            .contains(&requested.resource_binding)
        {
            return Err(GenerationError::Binding(BindingError::HostResource));
        }
        let authority = unique_authority(plan.authorities, node.instance, requested.effect_hash)?;
        if !authority.effect.constraints.is_empty() || !authority.grant.constraints.is_empty() {
            return Err(GenerationError::Unsupported(
                UnsupportedPlanFeature::AuthorityConstraints,
            ));
        }
        if authority.effect.administrative_class.is_some()
            || authority.administrative_subject.is_some()
            || authority.containment.is_some()
        {
            return Err(GenerationError::Unsupported(
                UnsupportedPlanFeature::AuthorityAdministrativeContainment,
            ));
        }
        if !authority.policy_budgets.is_empty() {
            return Err(GenerationError::Unsupported(
                UnsupportedPlanFeature::AuthorityPolicyBudgets,
            ));
        }
        let resource = unique_resource(plan.resources, node.instance, requested.resource_binding)?;
        let resource_lease = resource
            .lease
            .ok_or(GenerationError::Binding(BindingError::HostResource))?;
        let commit_profile = authority
            .commit_profile
            .ok_or(GenerationError::Binding(BindingError::HostEffect))?;
        if resource.resource != authority.binding.resource
            || authority.effect.id != authority.binding.effect_id
            || authority.capability.id != authority.binding.capability_id
            || authority.grant.id != authority.binding.grant_id
            || authority.binding.host != node.host
            || authority.binding.check_at_use != authority.effect.check_at_use
        {
            return Err(GenerationError::Binding(BindingError::HostEffect));
        }
        generated.push(GeneratedHostOperation {
            ordinal: requested.ordinal,
            operation: authority.effect.action.as_str().to_owned(),
            resource_binding: resource.id.as_str().to_owned(),
            resource_kind: resource.resource.kind.as_str().to_owned(),
            resource_id: resource.resource.id.as_str().to_owned(),
            effect_hash: requested.effect_hash,
            grant_hash: authority.grant_hash,
            resource_lease_hash: resource_lease
                .semantic_hash()
                .map_err(|_| GenerationError::Identity)?,
            commit_profile_hash: commit_profile
                .semantic_hash()
                .map_err(|_| GenerationError::Identity)?,
            capability_id: authority.capability.id.as_str().to_owned(),
            grant_id: authority.grant.id.as_str().to_owned(),
            host: authority.binding.host.as_str().to_owned(),
            check_at_use: authority.binding.check_at_use,
        });
    }
    Ok(generated)
}

fn unique_authority<'a>(
    authorities: &'a [PlanAuthority<'a>],
    instance: InstancePath<'_>,
    effect_hash: SemanticHash,
) -> Result<&'a PlanAuthority<'a>, GenerationError> {
    let mut matches = authorities
        .iter()
        .filter(|authority| authority.node == instance && authority.effect_hash == effect_hash);
    let authority = matches
        .next()
        .ok_or(GenerationError::Binding(BindingError::HostEffect))?;
    if matches.next().is_some() {
        return Err(GenerationError::Binding(BindingError::HostEffect));
    }
    Ok(authority)
}

fn unique_resource<'a>(
    resources: &'a [PlanResourceBinding<'a>],
    instance: InstancePath<'_>,
    id: Id<'_>,
) -> Result<&'a PlanResourceBinding<'a>, GenerationError> {
    let mut matches = resources
        .iter()
        .filter(|resource| resource.node == instance && resource.id == id);
    let resource = matches
        .next()
        .ok_or(GenerationError::Binding(BindingError::HostResource))?;
    if matches.next().is_some() {
        return Err(GenerationError::Binding(BindingError::HostResource));
    }
    Ok(resource)
}

fn unique_binding<'a>(
    bindings: &'a [EmbeddedNodeBinding<'a>],
    instance: InstancePath<'_>,
) -> Result<&'a EmbeddedNodeBinding<'a>, GenerationError> {
    let mut matches = bindings
        .iter()
        .filter(|binding| binding.instance.as_str() == instance.as_str());
    let binding = matches
        .next()
        .ok_or(GenerationError::Binding(BindingError::MissingNode))?;
    if matches.next().is_some() {
        return Err(GenerationError::Binding(BindingError::DuplicateNode));
    }
    Ok(binding)
}

fn validate_node_binding(
    node: &ResolvedPlanNode<'_>,
    binding: &EmbeddedNodeBinding<'_>,
) -> Result<(), GenerationError> {
    if !valid_pin(binding.driver) {
        return Err(GenerationError::Binding(BindingError::InvalidDriver));
    }
    let execution = node
        .execution_profile
        .ok_or(GenerationError::Binding(BindingError::PortRepresentation))?;
    validate_port_order(
        binding.input_ports,
        execution.representations,
        Direction::Input,
    )?;
    validate_port_order(
        binding.output_ports,
        execution.representations,
        Direction::Output,
    )
}

fn validate_port_order(
    ports: &[Id<'_>],
    representations: &[ValueRepresentation<'_>],
    direction: Direction,
) -> Result<(), GenerationError> {
    for (index, port) in ports.iter().enumerate() {
        if Id::new(port.as_str()).is_err() {
            return Err(GenerationError::Binding(BindingError::InvalidPort));
        }
        if ports[..index].contains(port) {
            return Err(GenerationError::Binding(BindingError::DuplicatePort));
        }
        if !representations.iter().any(|representation| {
            representation.direction == direction && representation.port == *port
        }) {
            return Err(GenerationError::Binding(BindingError::PortRepresentation));
        }
    }
    Ok(())
}

fn validate_endpoint_representation(
    node: ResolvedPlanNode<'_>,
    direction: Direction,
    port: Id<'_>,
    maximum_value_bytes: u32,
) -> Result<(), GenerationError> {
    let matches = node
        .execution_profile
        .ok_or(GenerationError::Binding(BindingError::PortRepresentation))?
        .representations
        .iter()
        .filter(|representation| {
            representation.direction == direction && representation.port == port
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 || matches[0].max_bytes < maximum_value_bytes {
        return Err(GenerationError::Binding(BindingError::PortRepresentation));
    }
    Ok(())
}

fn node_ordinal(
    plan: &ExecutionPlan<'_>,
    instance: InstancePath<'_>,
) -> Result<u16, GenerationError> {
    let mut matches = plan
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.instance.as_str() == instance.as_str());
    let (index, _) = matches
        .next()
        .ok_or(GenerationError::Binding(BindingError::CordEndpoint))?;
    if matches.next().is_some() {
        return Err(GenerationError::Binding(BindingError::DuplicateNode));
    }
    u16::try_from(index).map_err(|_| GenerationError::RangeExceeded)
}

fn port_ordinal(ports: &[Id<'_>], port: Id<'_>) -> Result<u8, GenerationError> {
    let index = ports
        .iter()
        .position(|candidate| *candidate == port)
        .ok_or(GenerationError::Binding(BindingError::CordEndpoint))?;
    u8::try_from(index).map_err(|_| GenerationError::RangeExceeded)
}

fn validate_generated_shape(generated: &GeneratedEmbeddedPlan) -> Result<(), GenerationError> {
    generated.with_static_plan(|plan| {
        validate_static_plan(
            &plan,
            &generated.profile,
            StorageShape {
                nodes: generated.profile.maximum_nodes,
                cords: generated.profile.maximum_cords,
                ports: generated.profile.maximum_ports,
                queue_slots: generated.profile.maximum_queue_slots,
                value_bytes: generated.profile.maximum_value_bytes,
                evidence_records: generated.profile.maximum_evidence_records,
                timers: generated.profile.maximum_timers,
                interests_per_node: generated.profile.maximum_interests_per_node,
                static_bytes: 1,
            },
        )
        .map(|_| ())
        .map_err(GenerationError::Profile)
    })
}

fn generated_identity(plan: &GeneratedEmbeddedPlan) -> Result<SemanticHash, GenerationError> {
    let node_hashes = plan
        .nodes
        .iter()
        .enumerate()
        .map(|(ordinal, node)| generated_node_identity(ordinal, node))
        .collect::<Result<Vec<_>, _>>()?;
    let cord_hashes = plan
        .cords
        .iter()
        .enumerate()
        .map(|(ordinal, cord)| generated_cord_identity(ordinal, cord))
        .collect::<Result<Vec<_>, _>>()?;
    let node_values = node_hashes
        .iter()
        .map(|hash| CanonicalValue::Bytes(hash.as_bytes()))
        .collect::<Vec<_>>();
    let cord_values = cord_hashes
        .iter()
        .map(|hash| CanonicalValue::Bytes(hash.as_bytes()))
        .collect::<Vec<_>>();
    CanonicalDescriptor {
        kind: Id("conduit/generated-embedded-plan"),
        schema_version: GENERATED_EMBEDDED_PLAN_SCHEMA_VERSION,
        body: CanonicalValue::Map(&[
            semantic(
                "conduit_revision",
                CanonicalValue::Text(&plan.conduit_revision),
            ),
            semantic(
                "policy_package_hash",
                CanonicalValue::Bytes(plan.policy_package_hash.as_bytes()),
            ),
            semantic(
                "policy_lock_hash",
                CanonicalValue::Bytes(plan.policy_lock_hash.as_bytes()),
            ),
            semantic(
                "source_semantic_hash",
                CanonicalValue::Bytes(plan.source_semantic_hash.as_bytes()),
            ),
            semantic(
                "full_plan_hash",
                CanonicalValue::Bytes(plan.full_plan_hash.as_bytes()),
            ),
            semantic(
                "profile_hash",
                CanonicalValue::Bytes(plan.profile.identity.as_bytes()),
            ),
            semantic("nodes", CanonicalValue::List(&node_values)),
            semantic("cords", CanonicalValue::List(&cord_values)),
        ]),
    }
    .semantic_hash()
    .map_err(|_| GenerationError::Identity)
}

fn generated_node_identity(
    ordinal: usize,
    node: &GeneratedStaticNode,
) -> Result<SemanticHash, GenerationError> {
    let input_ports = node
        .input_port_ids
        .iter()
        .map(|port| CanonicalValue::Identifier(Id(port)))
        .collect::<Vec<_>>();
    let output_ports = node
        .output_port_ids
        .iter()
        .map(|port| CanonicalValue::Identifier(Id(port)))
        .collect::<Vec<_>>();
    let host_operation_hashes = node
        .host_operations
        .iter()
        .map(generated_host_operation_identity)
        .collect::<Result<Vec<_>, _>>()?;
    let host_operations = host_operation_hashes
        .iter()
        .map(|hash| CanonicalValue::Bytes(hash.as_bytes()))
        .collect::<Vec<_>>();
    CanonicalDescriptor {
        kind: Id("conduit/generated-embedded-node"),
        schema_version: GENERATED_EMBEDDED_PLAN_SCHEMA_VERSION,
        body: CanonicalValue::Map(&[
            semantic("ordinal", CanonicalValue::Integer(ordinal as i128)),
            semantic(
                "semantic_path",
                CanonicalValue::Identifier(Id(&node.semantic_path)),
            ),
            semantic(
                "implementation_id",
                CanonicalValue::Identifier(Id(&node.implementation.id)),
            ),
            semantic(
                "implementation_schema_version",
                CanonicalValue::Integer(i128::from(node.implementation.schema_version)),
            ),
            semantic(
                "implementation_hash",
                CanonicalValue::Bytes(node.implementation.semantic_hash.as_bytes()),
            ),
            semantic("driver_id", CanonicalValue::Identifier(Id(&node.driver.id))),
            semantic(
                "driver_schema_version",
                CanonicalValue::Integer(i128::from(node.driver.schema_version)),
            ),
            semantic(
                "driver_hash",
                CanonicalValue::Bytes(node.driver.semantic_hash.as_bytes()),
            ),
            semantic(
                "input_ports",
                CanonicalValue::Integer(i128::from(node.input_ports)),
            ),
            semantic("input_port_ids", CanonicalValue::List(&input_ports)),
            semantic(
                "output_ports",
                CanonicalValue::Integer(i128::from(node.output_ports)),
            ),
            semantic("output_port_ids", CanonicalValue::List(&output_ports)),
            semantic("host_operations", CanonicalValue::List(&host_operations)),
            semantic(
                "maximum_step_work",
                CanonicalValue::Integer(i128::from(node.maximum_step_work)),
            ),
            semantic(
                "nesting_depth",
                CanonicalValue::Integer(i128::from(node.nesting_depth)),
            ),
        ]),
    }
    .semantic_hash()
    .map_err(|_| GenerationError::Identity)
}

fn generated_host_operation_identity(
    binding: &GeneratedHostOperation,
) -> Result<SemanticHash, GenerationError> {
    CanonicalDescriptor {
        kind: Id("conduit/generated-embedded-host-operation"),
        schema_version: GENERATED_EMBEDDED_PLAN_SCHEMA_VERSION,
        body: CanonicalValue::Map(&[
            semantic(
                "ordinal",
                CanonicalValue::Integer(i128::from(binding.ordinal)),
            ),
            semantic(
                "operation",
                CanonicalValue::Identifier(Id(&binding.operation)),
            ),
            semantic(
                "resource_binding",
                CanonicalValue::Identifier(Id(&binding.resource_binding)),
            ),
            semantic(
                "resource_kind",
                CanonicalValue::Identifier(Id(&binding.resource_kind)),
            ),
            semantic(
                "resource_id",
                CanonicalValue::Identifier(Id(&binding.resource_id)),
            ),
            semantic(
                "effect_hash",
                CanonicalValue::Bytes(binding.effect_hash.as_bytes()),
            ),
            semantic(
                "grant_hash",
                CanonicalValue::Bytes(binding.grant_hash.as_bytes()),
            ),
            semantic(
                "resource_lease_hash",
                CanonicalValue::Bytes(binding.resource_lease_hash.as_bytes()),
            ),
            semantic(
                "commit_profile_hash",
                CanonicalValue::Bytes(binding.commit_profile_hash.as_bytes()),
            ),
            semantic(
                "capability_id",
                CanonicalValue::Identifier(Id(&binding.capability_id)),
            ),
            semantic(
                "grant_id",
                CanonicalValue::Identifier(Id(&binding.grant_id)),
            ),
            semantic("host", CanonicalValue::Identifier(Id(&binding.host))),
            semantic(
                "check_at_use",
                CanonicalValue::Boolean(binding.check_at_use),
            ),
        ]),
    }
    .semantic_hash()
    .map_err(|_| GenerationError::Identity)
}

fn generated_cord_identity(
    ordinal: usize,
    cord: &GeneratedStaticCord,
) -> Result<SemanticHash, GenerationError> {
    CanonicalDescriptor {
        kind: Id("conduit/generated-embedded-cord"),
        schema_version: GENERATED_EMBEDDED_PLAN_SCHEMA_VERSION,
        body: CanonicalValue::Map(&[
            semantic("ordinal", CanonicalValue::Integer(ordinal as i128)),
            semantic(
                "semantic_id",
                CanonicalValue::Identifier(Id(&cord.semantic_id)),
            ),
            semantic(
                "producer_node",
                CanonicalValue::Integer(i128::from(cord.producer_node)),
            ),
            semantic(
                "producer_port",
                CanonicalValue::Integer(i128::from(cord.producer_port)),
            ),
            semantic(
                "consumer_node",
                CanonicalValue::Integer(i128::from(cord.consumer_node)),
            ),
            semantic(
                "consumer_port",
                CanonicalValue::Integer(i128::from(cord.consumer_port)),
            ),
            semantic(
                "slot_start",
                CanonicalValue::Integer(i128::from(cord.slot_start)),
            ),
            semantic(
                "capacity",
                CanonicalValue::Integer(i128::from(cord.capacity)),
            ),
            semantic(
                "maximum_value_bytes",
                CanonicalValue::Integer(i128::from(cord.maximum_value_bytes)),
            ),
        ]),
    }
    .semantic_hash()
    .map_err(|_| GenerationError::Identity)
}

fn semantic<'a>(name: &'static str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    }
}

fn valid_pin(pin: PinnedDescriptor<'_>) -> bool {
    Id::new(pin.id.as_str()).is_ok() && pin.semantic_hash != ZERO_HASH
}

fn owned_pin(pin: PinnedDescriptor<'_>) -> GeneratedPin {
    GeneratedPin {
        id: pin.id.as_str().to_owned(),
        schema_version: pin.schema_version,
        semantic_hash: pin.semantic_hash,
    }
}

fn borrowed_pin(pin: &GeneratedPin) -> PinnedDescriptor<'_> {
    PinnedDescriptor {
        id: Id(&pin.id),
        schema_version: pin.schema_version,
        semantic_hash: pin.semantic_hash,
    }
}

fn borrowed_host_operation(binding: &GeneratedHostOperation) -> StaticHostOperation<'_> {
    StaticHostOperation {
        ordinal: binding.ordinal,
        operation: Id(&binding.operation),
        resource_binding: Id(&binding.resource_binding),
        resource: ResourceRef {
            kind: Id(&binding.resource_kind),
            id: Id(&binding.resource_id),
        },
        effect_hash: binding.effect_hash,
        grant_hash: binding.grant_hash,
        resource_lease_hash: binding.resource_lease_hash,
        commit_profile_hash: binding.commit_profile_hash,
        capability_id: Id(&binding.capability_id),
        grant_id: Id(&binding.grant_id),
        host: Id(&binding.host),
        check_at_use: binding.check_at_use,
    }
}

fn render_profile(output: &mut String, profile: EmbeddedProfile) {
    output.push_str(
        "pub const GENERATED_EMBEDDED_PROFILE: conduit_embedded::EmbeddedProfile = \
         conduit_embedded::EmbeddedProfile {\n",
    );
    output.push_str("    identity: ");
    write_hash_expression(output, profile.identity);
    output.push_str(",\n");
    writeln!(output, "    maximum_nodes: {},", profile.maximum_nodes)
        .expect("String writes cannot fail");
    writeln!(output, "    maximum_cords: {},", profile.maximum_cords)
        .expect("String writes cannot fail");
    writeln!(output, "    maximum_ports: {},", profile.maximum_ports)
        .expect("String writes cannot fail");
    writeln!(
        output,
        "    maximum_host_operations: {},",
        profile.maximum_host_operations
    )
    .expect("String writes cannot fail");
    writeln!(
        output,
        "    maximum_queue_slots: {},",
        profile.maximum_queue_slots
    )
    .expect("String writes cannot fail");
    writeln!(
        output,
        "    maximum_value_bytes: {},",
        profile.maximum_value_bytes
    )
    .expect("String writes cannot fail");
    writeln!(
        output,
        "    maximum_evidence_records: {},",
        profile.maximum_evidence_records
    )
    .expect("String writes cannot fail");
    writeln!(output, "    maximum_timers: {},", profile.maximum_timers)
        .expect("String writes cannot fail");
    writeln!(
        output,
        "    maximum_interests_per_node: {},",
        profile.maximum_interests_per_node
    )
    .expect("String writes cannot fail");
    writeln!(output, "    maximum_nesting: {},", profile.maximum_nesting)
        .expect("String writes cannot fail");
    writeln!(
        output,
        "    maximum_timer_delay: {},",
        profile.maximum_timer_delay
    )
    .expect("String writes cannot fail");
    writeln!(
        output,
        "    static_ram_budget_bytes: {},",
        profile.static_ram_budget_bytes
    )
    .expect("String writes cannot fail");
    writeln!(
        output,
        "    stack_budget_bytes: {},",
        profile.stack_budget_bytes
    )
    .expect("String writes cannot fail");
    writeln!(
        output,
        "    flash_budget_bytes: {},",
        profile.flash_budget_bytes
    )
    .expect("String writes cannot fail");
    output.push_str("};\n");
}

fn render_pin(output: &mut String, field: &str, pin: &GeneratedPin, indent: usize) {
    let padding = " ".repeat(indent);
    writeln!(
        output,
        "{padding}{field}: conduit_core::PinnedDescriptor {{"
    )
    .expect("String writes cannot fail");
    writeln!(output, "{padding}    id: conduit_core::Id({:?}),", pin.id)
        .expect("String writes cannot fail");
    writeln!(
        output,
        "{padding}    schema_version: {},",
        pin.schema_version
    )
    .expect("String writes cannot fail");
    output.push_str(&padding);
    output.push_str("    semantic_hash: ");
    write_hash_expression(output, pin.semantic_hash);
    output.push_str(",\n");
    writeln!(output, "{padding}}},").expect("String writes cannot fail");
}

fn render_host_operation(output: &mut String, binding: &GeneratedHostOperation, indent: usize) {
    let padding = " ".repeat(indent);
    writeln!(output, "{padding}conduit_embedded::StaticHostOperation {{")
        .expect("String writes cannot fail");
    writeln!(output, "{padding}    ordinal: {},", binding.ordinal)
        .expect("String writes cannot fail");
    writeln!(
        output,
        "{padding}    operation: conduit_core::Id({:?}),",
        binding.operation
    )
    .expect("String writes cannot fail");
    writeln!(
        output,
        "{padding}    resource_binding: conduit_core::Id({:?}),",
        binding.resource_binding
    )
    .expect("String writes cannot fail");
    writeln!(
        output,
        "{padding}    resource: conduit_core::ResourceRef {{"
    )
    .expect("String writes cannot fail");
    writeln!(
        output,
        "{padding}        kind: conduit_core::Id({:?}),",
        binding.resource_kind
    )
    .expect("String writes cannot fail");
    writeln!(
        output,
        "{padding}        id: conduit_core::Id({:?}),",
        binding.resource_id
    )
    .expect("String writes cannot fail");
    writeln!(output, "{padding}    }},").expect("String writes cannot fail");
    write!(output, "{padding}    effect_hash: ").expect("String writes cannot fail");
    write_hash_expression(output, binding.effect_hash);
    output.push_str(",\n");
    write!(output, "{padding}    grant_hash: ").expect("String writes cannot fail");
    write_hash_expression(output, binding.grant_hash);
    output.push_str(",\n");
    write!(output, "{padding}    resource_lease_hash: ").expect("String writes cannot fail");
    write_hash_expression(output, binding.resource_lease_hash);
    output.push_str(",\n");
    write!(output, "{padding}    commit_profile_hash: ").expect("String writes cannot fail");
    write_hash_expression(output, binding.commit_profile_hash);
    output.push_str(",\n");
    writeln!(
        output,
        "{padding}    capability_id: conduit_core::Id({:?}),",
        binding.capability_id
    )
    .expect("String writes cannot fail");
    writeln!(
        output,
        "{padding}    grant_id: conduit_core::Id({:?}),",
        binding.grant_id
    )
    .expect("String writes cannot fail");
    writeln!(
        output,
        "{padding}    host: conduit_core::Id({:?}),",
        binding.host
    )
    .expect("String writes cannot fail");
    writeln!(
        output,
        "{padding}    check_at_use: {},",
        binding.check_at_use
    )
    .expect("String writes cannot fail");
    writeln!(output, "{padding}}},").expect("String writes cannot fail");
}

fn write_hash_constant(output: &mut String, name: &str, hash: SemanticHash) {
    write!(output, "pub const {name}: conduit_core::SemanticHash = ")
        .expect("String writes cannot fail");
    write_hash_expression(output, hash);
    output.push_str(";\n");
}

fn write_hash_expression(output: &mut String, hash: SemanticHash) {
    output.push_str("conduit_core::SemanticHash::from_bytes([");
    for (index, byte) in hash.as_bytes().iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        write!(output, "{byte}").expect("String writes cannot fail");
    }
    output.push_str("])");
}

fn render_id_list(output: &mut String, ids: &[String]) {
    for (index, id) in ids.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        write!(output, "conduit_core::Id({id:?})").expect("String writes cannot fail");
    }
}
