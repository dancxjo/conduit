//! Hosted registry, resolver, explainer, and executor.
//!
//! This first runtime intentionally executes finite, one-shot acyclic panels.
//! The portable core now includes the normative allocator-free bounded queue;
//! a later hosted streaming scheduler can drive it without changing node,
//! port, cord, or flow-policy identity.

use std::collections::BTreeMap;
use std::fmt;
use std::io::{Read, Write};

use conduit_core::{
    BlockingFairness, CanonicalDescriptor, CanonicalValue, CompatibilityOutcome, ConfigContract,
    ConfigFieldContract, ConfigIdentity, ConfigMutability, ConfigRequirement,
    ConnectionCardinality, Delivery, DescriptorRef, Direction, Endpoint as CoreEndpoint,
    FlowCapacity, FlowPolicy, FlowTypeFacts, FlowWatermarks, Id, LossAcceptance, NodeContract,
    PlanCord, PlanGraph, PlanNode, PortContract, PortFlowConstraints, Presence, Pressure,
    SampleSchedule, SemanticHash, Sensitivity, TemporalContract, TerminalContract, TraitProof,
    TypeContractRef, ValueCardinality, validate_plan_graph,
};
use conduit_panel::{
    CompositeDefinition, ConfigEntry, Cord, Endpoint, ExportDirection, Node, Panel, SourcePressure,
};
use serde::Serialize;

mod artifact_verification;
mod config_resolution;
mod evidence_ndjson;
mod host_resolution;
mod implementation_binding;
mod runtime_evidence;
mod scheduler;
mod source_lowering;
mod type_registry;

pub use artifact_verification::{HostedArtifactVerificationError, verify_artifact_bytes};
pub use config_resolution::{
    ConfigAssignment, ConfigResolutionError, ConfigValue, ResolvedConfig, ResolvedConfigEntry,
    SecretValue, resolve_config, validate_config_update,
};
pub use evidence_ndjson::{
    NdjsonError, OwnedEventCorrelation, OwnedEventPayload, OwnedEventRelations,
    OwnedEventTerminality, OwnedEventTime, OwnedExecutionEvent, OwnedPayloadShape, OwnedTypeRef,
    decode_event_ndjson, encode_event_ndjson, encode_owned_event_ndjson,
};
pub use host_resolution::{
    CandidateAuthority, CandidateRejection, CandidateRejectionReason, CapabilityPredicate,
    HostResolverPolicy, PlacementCandidate, PlacementRequest, PlanSealingReason, ResolutionFailure,
    ResolvedPlacement, ResolvedPlacementBinding, ResolverTiePolicy, ResourcePredicate,
    TopologyPredicate, resolve_host_placement, seal_resolved_execution_plan,
};
pub use implementation_binding::{
    ForeignStepReply, ForeignStepRequest, MessageStepBinding, MessageStepEndpoint,
    NativeStepBinding, NativeStepImplementation, OwnedStepOutcome, OwnedStepReply,
    OwnedWakeInterest,
};
pub use runtime_evidence::{
    RuntimeEvidenceContext, RuntimeEvidenceError, record_scheduler_evidence,
};
pub use scheduler::{
    DeterministicExecutor, RuntimeValue, ScheduledNode, SchedulerAllocation, SchedulerError,
    SchedulerEvent, SchedulerEventKind, SchedulerHighWater, SchedulerNode, SchedulerReservation,
    SchedulerStatus, SchedulerStep, SchedulerSubject, SendStatus, StepIo,
};
pub use source_lowering::{
    ConfigProvenance, LOWERED_SOURCE_SCHEMA_V1, LOWERED_SOURCE_SCHEMA_V2, LiteralValidationError,
    LoweredBindingV2, LoweredCompositeChildV2, LoweredCompositeV2, LoweredConfigEntry,
    LoweredConfigValue, LoweredCordV2, LoweredExportV2, LoweredGroupPort, LoweredNode,
    LoweredNodeV2, LoweredPool, LoweredRootSelectionV2, LoweredSource, LoweredSourceV2,
    LoweringDiagnostic, OwnedConfigFieldSchema, OwnedConfigRequirement, OwnedNodeSchema,
    OwnedPortReference, OwnedSemanticValue, OwnedTypeReference, SOURCE_AST_SCHEMA_V2,
    SourceContractCatalog, SourceMapEntry, SourceOrigin, VersionedLoweredSource, lower_source,
    lower_source_v2, lower_source_version, migrate_lowered_source_v1,
};
pub use type_registry::{
    ProviderTypeDecision, TypeComparisonStrategy, TypeContractDescription, TypeContractProvider,
    TypeRegistry, TypeRegistryError, TypeSatisfactionReport,
};

/// Allocator-aware convenience around the core-compatible exact-plan validator.
pub fn validate_hosted_execution_plan(
    plan: &conduit_core::ExecutionPlan<'_>,
    context: conduit_core::PlanValidationContext<'_>,
) -> Result<(), conduit_core::PlanValidationError> {
    let fact_count =
        plan.validation_scratch_count()
            .map_err(|_| conduit_core::PlanValidationError {
                code: conduit_core::PlanDiagnosticCode::InvalidDescriptor,
                collection: conduit_core::PlanCollection::Header,
                subject_index: None,
            })?;
    let mut scratch = vec![SemanticHash::from_bytes([0; 32]); fact_count];
    conduit_core::validate_execution_plan(plan, context, &mut scratch)
}

const TEXT_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("conduit/text.utf8"),
    schema_version: 1,
    semantic_hash: SemanticHash::from_bytes([
        0x23, 0xf6, 0xb8, 0xc6, 0xd7, 0x84, 0x79, 0x9a, 0x10, 0x09, 0xbd, 0x45, 0x32, 0x26, 0x67,
        0x0d, 0xdd, 0x91, 0x80, 0xe0, 0x06, 0xd4, 0xc2, 0x32, 0x70, 0x55, 0xcb, 0xf3, 0x50, 0x77,
        0x6e, 0x9b,
    ]),
};
const EMPTY_CONFIG: ConfigContract<'static> = ConfigContract { fields: &[] };
const LITERAL_CONFIG: ConfigContract<'static> = ConfigContract {
    fields: &[ConfigFieldContract {
        key: Id("value"),
        value_type: TEXT_TYPE,
        requirement: ConfigRequirement::Required,
        sensitivity: Sensitivity::Public,
        mutability: ConfigMutability::PreStart,
        identity: ConfigIdentity::Semantic,
    }],
};
const INPUT_TEXT: PortContract<'static> = PortContract {
    id: Id("in"),
    direction: Direction::Input,
    value_type: TEXT_TYPE,
    presence: Presence::Required,
    connections: ConnectionCardinality::ExactlyOne,
    values: ValueCardinality::ExactlyOne,
    delivery: Delivery::FiniteBatch,
    temporal: TemporalContract::Atemporal,
    terminal: TerminalContract::Finite,
    sensitivity: Sensitivity::Public,
    flow: PortFlowConstraints {
        loss: LossAcceptance::LosslessOnly,
    },
};
const OUTPUT_TEXT: PortContract<'static> = PortContract {
    id: Id("out"),
    direction: Direction::Output,
    value_type: TEXT_TYPE,
    presence: Presence::Required,
    connections: ConnectionCardinality::OneOrMore,
    values: ValueCardinality::ExactlyOne,
    delivery: Delivery::FiniteBatch,
    temporal: TemporalContract::Atemporal,
    terminal: TerminalContract::Finite,
    sensitivity: Sensitivity::Public,
    flow: PortFlowConstraints {
        loss: LossAcceptance::LosslessOnly,
    },
};

const LITERAL_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit/literal"),
    config: LITERAL_CONFIG,
    inputs: &[],
    outputs: &[OUTPUT_TEXT],
};
const STDIN_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit/stdin"),
    config: EMPTY_CONFIG,
    inputs: &[],
    outputs: &[OUTPUT_TEXT],
};
const UPPERCASE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit/uppercase"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[OUTPUT_TEXT],
};
const STDOUT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit/stdout"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[],
};
const STDERR_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("conduit/stderr"),
    config: EMPTY_CONFIG,
    inputs: &[INPUT_TEXT],
    outputs: &[],
};

/// Typed runtime value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Value {
    /// Exact semantic type identity.
    pub value_type: TypeContractRef<'static>,
    /// Canonical or implementation-agreed payload bytes.
    pub bytes: Vec<u8>,
}

impl Value {
    fn text(value: impl Into<Vec<u8>>) -> Self {
        Self {
            value_type: TEXT_TYPE,
            bytes: value.into(),
        }
    }
}

/// Process boundary supplied by the host.
pub struct RunIo<'a> {
    /// Process standard input.
    pub input: &'a mut dyn Read,
    /// Process standard output.
    pub output: &'a mut dyn Write,
    /// Process standard error.
    pub error: &'a mut dyn Write,
}

trait Handler {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError>;
}

type HandlerFactory = fn() -> Box<dyn Handler>;
type ConfigValidator = fn(&Node) -> Result<(), ResolutionError>;

#[derive(Debug)]
struct RegisteredNode {
    contract: &'static NodeContract<'static>,
    factory: HandlerFactory,
    validate_config: ConfigValidator,
}

/// Built-in hosted implementation registry.
///
/// Registry identity and discovery are deliberately above `conduit-core`.
pub struct Registry {
    nodes: BTreeMap<&'static str, RegisteredNode>,
    types: TypeRegistry,
}

impl Default for Registry {
    fn default() -> Self {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            LITERAL_CONTRACT.id.as_str(),
            RegisteredNode {
                contract: &LITERAL_CONTRACT,
                factory: || Box::new(Literal),
                validate_config: validate_literal,
            },
        );
        nodes.insert(
            STDIN_CONTRACT.id.as_str(),
            RegisteredNode {
                contract: &STDIN_CONTRACT,
                factory: || Box::new(Stdin),
                validate_config: validate_empty_config,
            },
        );
        nodes.insert(
            UPPERCASE_CONTRACT.id.as_str(),
            RegisteredNode {
                contract: &UPPERCASE_CONTRACT,
                factory: || Box::new(Uppercase),
                validate_config: validate_empty_config,
            },
        );
        nodes.insert(
            STDOUT_CONTRACT.id.as_str(),
            RegisteredNode {
                contract: &STDOUT_CONTRACT,
                factory: || Box::new(Stdout),
                validate_config: validate_empty_config,
            },
        );
        nodes.insert(
            STDERR_CONTRACT.id.as_str(),
            RegisteredNode {
                contract: &STDERR_CONTRACT,
                factory: || Box::new(Stderr),
                validate_config: validate_empty_config,
            },
        );
        let mut types = TypeRegistry::default();
        types
            .register(BuiltinTypeProvider)
            .expect("built-in type namespace is unique and valid");
        Self { nodes, types }
    }
}

impl Registry {
    /// Resolves semantic source references to concrete hosted implementations.
    pub fn resolve<'a>(&'a self, panel: &'a Panel) -> Result<ResolvedPanel<'a>, ResolutionError> {
        let has_unlowered_source = !panel.imports.is_empty()
            || !panel.roots.is_empty()
            || !panel.port_groups.is_empty()
            || !panel.pools.is_empty()
            || panel.nodes.iter().any(|node| node.constraint.is_some())
            || panel.definitions.iter().any(|definition| {
                !definition.parameters.is_empty()
                    || !definition.port_groups.is_empty()
                    || !definition.pools.is_empty()
                    || definition
                        .nodes
                        .iter()
                        .any(|node| node.constraint.is_some())
            });
        if has_unlowered_source {
            return Err(ResolutionError::new(
                "CND-PLN-005",
                "imports, roots, constraints, port groups, and pools must be explicitly lowered before runtime resolution",
            ));
        }
        let expanded = expand_panel(panel, &self.nodes)?;
        if expanded.nodes.len() > usize::from(u16::MAX) {
            return Err(ResolutionError::new(
                "CND-PLN-003",
                "panel has more nodes than the portable plan can address",
            ));
        }

        let mut nodes = Vec::with_capacity(expanded.nodes.len());
        for source in expanded.nodes {
            Id::new(&source.id).map_err(|error| {
                ResolutionError::new(
                    "CND-ID-001",
                    format!("invalid expanded node id `{}`: {error}", source.id),
                )
            })?;
            let definition = self.nodes.get(source.kind.as_str()).ok_or_else(|| {
                ResolutionError::new(
                    "CND-IMP-001",
                    format!("no ready implementation for `{}`", source.kind),
                )
            })?;
            (definition.validate_config)(&source)?;
            nodes.push(ResolvedNode { source, definition });
        }

        let mut cords = Vec::with_capacity(expanded.cords.len());
        for source in expanded.cords {
            let from_node = node_index(&nodes, &source.from.node)?;
            let to_node = node_index(&nodes, &source.to.node)?;
            let from_port = port_index(
                nodes[from_node].definition.contract.outputs,
                &source.from.port,
                &source.from.node,
            )?;
            let to_port = port_index(
                nodes[to_node].definition.contract.inputs,
                &source.to.port,
                &source.to.node,
            )?;
            cords.push(ResolvedCord {
                source,
                from_node,
                from_port,
                to_node,
                to_port,
            });
        }

        let core_nodes = nodes
            .iter()
            .map(|node| PlanNode {
                id: Id(node.source.id.as_str()),
                contract: node.definition.contract,
            })
            .collect::<Vec<_>>();
        let core_cords = cords
            .iter()
            .map(|cord| {
                let flow = resolve_flow(&cord.source)?;
                let value_type =
                    nodes[cord.from_node].definition.contract.outputs[cord.from_port].value_type;
                let flow_decision = self.types.assess_flow_policy(value_type, flow);
                if flow_decision.outcome != CompatibilityOutcome::Compatible {
                    return Err(ResolutionError::new(
                        "CND-FLW-004",
                        flow_decision.reason.as_str(),
                    ));
                }
                Ok(PlanCord {
                    id: Id(cord.source.id.as_str()),
                    from: CoreEndpoint {
                        node: u16::try_from(cord.from_node).expect("node count checked"),
                        port: u16::try_from(cord.from_port).map_err(|_| {
                            ResolutionError::new("CND-PLN-003", "too many output ports")
                        })?,
                    },
                    to: CoreEndpoint {
                        node: u16::try_from(cord.to_node).expect("node count checked"),
                        port: u16::try_from(cord.to_port).map_err(|_| {
                            ResolutionError::new("CND-PLN-003", "too many input ports")
                        })?,
                    },
                    flow,
                })
            })
            .collect::<Result<Vec<_>, ResolutionError>>()?;
        validate_plan_graph(&PlanGraph {
            nodes: &core_nodes,
            cords: &core_cords,
        })
        .map_err(|error| ResolutionError::new(error.code.as_str(), error.to_string()))?;

        reject_cycles(&nodes, &cords)?;

        Ok(ResolvedPanel {
            source: panel,
            nodes,
            cords,
            logical_composites: expanded.logical_composites,
        })
    }

    /// Returns the semantic contracts available from this registry.
    pub fn contracts(&self) -> impl Iterator<Item = &'static NodeContract<'static>> + '_ {
        self.nodes.values().map(|node| node.contract)
    }

    /// Returns the domain type registry used during flow resolution.
    #[must_use]
    pub const fn type_registry(&self) -> &TypeRegistry {
        &self.types
    }
}

struct BuiltinTypeProvider;

impl TypeContractProvider for BuiltinTypeProvider {
    fn provider_descriptor(&self) -> DescriptorRef<'static> {
        DescriptorRef {
            kind: Id("conduit/builtin-type-provider"),
            schema_version: 1,
            semantic_hash: SemanticHash::from_bytes([0x24; 32]),
        }
    }

    fn namespace(&self) -> &str {
        "conduit"
    }

    fn describe<'a>(
        &'a self,
        reference: TypeContractRef<'a>,
    ) -> Option<TypeContractDescription<'a>> {
        (reference == TEXT_TYPE).then_some(TypeContractDescription {
            human_name: "UTF-8 text",
            descriptor: CanonicalDescriptor {
                kind: TEXT_TYPE.contract_id,
                schema_version: TEXT_TYPE.schema_version,
                body: CanonicalValue::Null,
            },
            strategy: TypeComparisonStrategy::Nominal,
            flow_type_facts: FlowTypeFacts {
                disposable: TraitProof::Disproven,
                coalescers: Some(&[]),
            },
        })
    }

    fn consumer_accepts_producer<'a>(
        &'a self,
        _: TypeContractRef<'a>,
        _: TypeContractRef<'a>,
    ) -> ProviderTypeDecision<'a> {
        ProviderTypeDecision {
            outcome: CompatibilityOutcome::Incompatible,
            rule: Id("conduit/no-type-rule"),
        }
    }
}

#[derive(Debug)]
struct ExpandedSource {
    nodes: Vec<Node>,
    cords: Vec<Cord>,
    logical_composites: Vec<LogicalComposite>,
}

#[derive(Debug)]
struct LogicalComposite {
    path: String,
    definition: String,
    children: Vec<(String, String)>,
    cords: Vec<(String, String)>,
    exports: Vec<(ExportDirection, String, Endpoint)>,
    bindings: Vec<(String, String)>,
}

type BoundaryMap = BTreeMap<(u8, String), Endpoint>;

fn expand_panel(
    panel: &Panel,
    primitives: &BTreeMap<&'static str, RegisteredNode>,
) -> Result<ExpandedSource, ResolutionError> {
    validate_definition_names(panel, primitives)?;
    validate_definition_shapes(panel, primitives)?;
    validate_definition_cycles(panel)?;
    let mut expanded = ExpandedSource {
        nodes: Vec::new(),
        cords: Vec::new(),
        logical_composites: Vec::new(),
    };
    let mut roots = BTreeMap::<String, BoundaryMap>::new();
    for node in &panel.nodes {
        validate_instance_id(&node.id)?;
        if roots.contains_key(&node.id) {
            return Err(ResolutionError::new(
                "CND-ID-002",
                format!("duplicate node id `{}`", node.id),
            ));
        }
        let boundary = expand_instance(
            panel,
            primitives,
            node,
            &node.id,
            &mut Vec::new(),
            &mut expanded,
        )?;
        roots.insert(node.id.clone(), boundary);
    }
    for cord in &panel.cords {
        let from = resolve_boundary_endpoint(&roots, &cord.from, ExportDirection::Output)?;
        let to = resolve_boundary_endpoint(&roots, &cord.to, ExportDirection::Input)?;
        push_expanded_cord(&mut expanded, cord, from, to);
    }
    Ok(expanded)
}

fn validate_definition_shapes(
    panel: &Panel,
    primitives: &BTreeMap<&'static str, RegisteredNode>,
) -> Result<(), ResolutionError> {
    for definition in &panel.definitions {
        for (index, child) in definition.nodes.iter().enumerate() {
            validate_instance_id(&child.id)?;
            if definition.nodes[..index]
                .iter()
                .any(|prior| prior.id == child.id)
            {
                return Err(ResolutionError::new(
                    "CND-ID-002",
                    format!("duplicate child `{}` in `{}`", child.id, definition.id),
                ));
            }
        }
        for cord in &definition.cords {
            for (endpoint, direction) in [
                (&cord.from, ExportDirection::Output),
                (&cord.to, ExportDirection::Input),
            ] {
                let child = definition
                    .nodes
                    .iter()
                    .find(|child| child.id == endpoint.node)
                    .ok_or_else(|| {
                        ResolutionError::new(
                            "CND-CMP-003",
                            format!(
                                "cord in `{}` targets missing child `{}`",
                                definition.id, endpoint.node
                            ),
                        )
                    })?;
                if !kind_has_port(panel, primitives, &child.kind, direction, &endpoint.port) {
                    return Err(ResolutionError::new(
                        "CND-CMP-003",
                        format!(
                            "cord in `{}` targets missing or wrong-direction port `{}.{}`",
                            definition.id, endpoint.node, endpoint.port
                        ),
                    ));
                }
            }
        }
        for (index, export) in definition.exports.iter().enumerate() {
            if definition.exports[..index].iter().any(|prior| {
                prior.direction == export.direction
                    && (prior.id == export.id || prior.target == export.target)
            }) {
                return Err(ResolutionError::new(
                    "CND-CMP-002",
                    format!("duplicate export `{}` in `{}`", export.id, definition.id),
                ));
            }
            let child = definition
                .nodes
                .iter()
                .find(|child| child.id == export.target.node)
                .ok_or_else(|| {
                    ResolutionError::new(
                        "CND-CMP-003",
                        format!(
                            "export `{}` targets missing child `{}`",
                            export.id, export.target.node
                        ),
                    )
                })?;
            if !kind_has_port(
                panel,
                primitives,
                &child.kind,
                export.direction,
                &export.target.port,
            ) {
                return Err(ResolutionError::new(
                    "CND-CMP-003",
                    format!(
                        "export `{}` targets missing or wrong-direction port `{}.{}`",
                        export.id, export.target.node, export.target.port
                    ),
                ));
            }
        }
        for (index, binding) in definition.bindings.iter().enumerate() {
            if definition.bindings[..index]
                .iter()
                .any(|prior| prior.parameter == binding.parameter && prior.target == binding.target)
            {
                return Err(ResolutionError::new(
                    "CND-CMP-002",
                    format!(
                        "duplicate binding `{}` to `{}.{}`",
                        binding.parameter, binding.target.node, binding.target.port
                    ),
                ));
            }
            let child = definition
                .nodes
                .iter()
                .find(|child| child.id == binding.target.node)
                .ok_or_else(|| {
                    ResolutionError::new(
                        "CND-CMP-003",
                        format!(
                            "binding `{}` targets missing child `{}`",
                            binding.parameter, binding.target.node
                        ),
                    )
                })?;
            if !kind_has_parameter(panel, primitives, &child.kind, &binding.target.port) {
                return Err(ResolutionError::new(
                    "CND-CMP-003",
                    format!(
                        "binding `{}` targets missing field `{}.{}`",
                        binding.parameter, binding.target.node, binding.target.port
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn kind_has_port(
    panel: &Panel,
    primitives: &BTreeMap<&'static str, RegisteredNode>,
    kind: &str,
    direction: ExportDirection,
    port: &str,
) -> bool {
    if let Some(primitive) = primitives.get(kind) {
        let ports = match direction {
            ExportDirection::Input => primitive.contract.inputs,
            ExportDirection::Output => primitive.contract.outputs,
        };
        return ports.iter().any(|candidate| candidate.id.as_str() == port);
    }
    panel
        .definitions
        .iter()
        .find(|definition| definition.id == kind)
        .is_some_and(|definition| {
            definition
                .exports
                .iter()
                .any(|export| export.direction == direction && export.id == port)
        })
}

fn kind_has_parameter(
    panel: &Panel,
    primitives: &BTreeMap<&'static str, RegisteredNode>,
    kind: &str,
    parameter: &str,
) -> bool {
    if let Some(primitive) = primitives.get(kind) {
        return primitive
            .contract
            .config
            .fields
            .iter()
            .any(|field| field.key.as_str() == parameter);
    }
    panel
        .definitions
        .iter()
        .find(|definition| definition.id == kind)
        .is_some_and(|definition| {
            definition
                .bindings
                .iter()
                .any(|binding| binding.parameter == parameter)
        })
}

fn validate_definition_names(
    panel: &Panel,
    primitives: &BTreeMap<&'static str, RegisteredNode>,
) -> Result<(), ResolutionError> {
    for (index, definition) in panel.definitions.iter().enumerate() {
        Id::new(&definition.id).map_err(|error| {
            ResolutionError::new(
                "CND-CMP-001",
                format!("invalid composite id `{}`: {error}", definition.id),
            )
        })?;
        if primitives.contains_key(definition.id.as_str())
            || panel.definitions[..index]
                .iter()
                .any(|prior| prior.id == definition.id)
        {
            return Err(ResolutionError::new(
                "CND-CMP-001",
                format!("duplicate node definition `{}`", definition.id),
            ));
        }
    }
    for definition in &panel.definitions {
        for child in &definition.nodes {
            if !primitives.contains_key(child.kind.as_str())
                && !panel
                    .definitions
                    .iter()
                    .any(|candidate| candidate.id == child.kind)
            {
                return Err(ResolutionError::new(
                    "CND-CMP-005",
                    format!(
                        "composite `{}` references unknown definition `{}`",
                        definition.id, child.kind
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn validate_definition_cycles(panel: &Panel) -> Result<(), ResolutionError> {
    fn visit<'a>(
        panel: &'a Panel,
        definition: &'a CompositeDefinition,
        visiting: &mut Vec<&'a str>,
        visited: &mut Vec<&'a str>,
    ) -> Result<(), ResolutionError> {
        if visiting.contains(&definition.id.as_str()) {
            let mut cycle = visiting.join(" -> ");
            cycle.push_str(" -> ");
            cycle.push_str(&definition.id);
            return Err(ResolutionError::new(
                "CND-CMP-005",
                format!("recursive composite definition: {cycle}"),
            ));
        }
        if visited.contains(&definition.id.as_str()) {
            return Ok(());
        }
        visiting.push(&definition.id);
        for child in &definition.nodes {
            if let Some(nested) = panel
                .definitions
                .iter()
                .find(|candidate| candidate.id == child.kind)
            {
                visit(panel, nested, visiting, visited)?;
            }
        }
        visiting.pop();
        visited.push(&definition.id);
        Ok(())
    }

    let mut visiting = Vec::new();
    let mut visited = Vec::new();
    for definition in &panel.definitions {
        visit(panel, definition, &mut visiting, &mut visited)?;
    }
    Ok(())
}

fn expand_instance(
    panel: &Panel,
    primitives: &BTreeMap<&'static str, RegisteredNode>,
    source: &Node,
    path: &str,
    stack: &mut Vec<String>,
    expanded: &mut ExpandedSource,
) -> Result<BoundaryMap, ResolutionError> {
    if let Some(primitive) = primitives.get(source.kind.as_str()) {
        let id = expanded_id(path);
        let mut boundary = BoundaryMap::new();
        for port in primitive.contract.inputs {
            boundary.insert(
                (
                    direction_key(ExportDirection::Input),
                    port.id.as_str().to_owned(),
                ),
                Endpoint {
                    node: id.clone(),
                    port: port.id.as_str().to_owned(),
                },
            );
        }
        for port in primitive.contract.outputs {
            boundary.insert(
                (
                    direction_key(ExportDirection::Output),
                    port.id.as_str().to_owned(),
                ),
                Endpoint {
                    node: id.clone(),
                    port: port.id.as_str().to_owned(),
                },
            );
        }
        let mut node = source.clone();
        node.id = id;
        expanded.nodes.push(node);
        return Ok(boundary);
    }

    let definition = panel
        .definitions
        .iter()
        .find(|definition| definition.id == source.kind)
        .ok_or_else(|| {
            ResolutionError::new(
                "CND-IMP-001",
                format!("no ready implementation or composite for `{}`", source.kind),
            )
        })?;
    if stack.contains(&definition.id) {
        return Err(ResolutionError::new(
            "CND-CMP-005",
            format!("recursive composite `{}`", definition.id),
        ));
    }
    stack.push(definition.id.clone());

    validate_instance_config(source, definition)?;
    let mut children = BTreeMap::<String, BoundaryMap>::new();
    for child in &definition.nodes {
        if children.contains_key(&child.id) {
            return Err(ResolutionError::new(
                "CND-ID-002",
                format!("duplicate child `{}` in `{}`", child.id, definition.id),
            ));
        }
        let mut bound = child.clone();
        apply_bindings(source, definition, &mut bound)?;
        let child_path = format!("{path}/{}", child.id);
        let boundary = expand_instance(panel, primitives, &bound, &child_path, stack, expanded)?;
        children.insert(child.id.clone(), boundary);
    }
    for cord in &definition.cords {
        let from = resolve_boundary_endpoint(&children, &cord.from, ExportDirection::Output)?;
        let to = resolve_boundary_endpoint(&children, &cord.to, ExportDirection::Input)?;
        push_expanded_cord(expanded, cord, from, to);
    }

    let mut boundary = BoundaryMap::new();
    let mut logical_exports = Vec::new();
    for export in &definition.exports {
        let key = (direction_key(export.direction), export.id.clone());
        if boundary.contains_key(&key) {
            return Err(ResolutionError::new(
                "CND-CMP-002",
                format!("duplicate export `{}` in `{}`", export.id, definition.id),
            ));
        }
        let target = resolve_boundary_endpoint(&children, &export.target, export.direction)?;
        boundary.insert(key, target.clone());
        logical_exports.push((export.direction, export.id.clone(), target));
    }
    expanded.logical_composites.push(LogicalComposite {
        path: path.to_owned(),
        definition: definition.id.clone(),
        children: definition
            .nodes
            .iter()
            .map(|child| (format!("{path}/{}", child.id), child.kind.clone()))
            .collect(),
        cords: definition
            .cords
            .iter()
            .map(|cord| {
                (
                    format!("{}.{}", cord.from.node, cord.from.port),
                    format!("{}.{}", cord.to.node, cord.to.port),
                )
            })
            .collect(),
        exports: logical_exports,
        bindings: definition
            .bindings
            .iter()
            .map(|binding| {
                (
                    binding.parameter.clone(),
                    format!("{path}/{}.{}", binding.target.node, binding.target.port),
                )
            })
            .collect(),
    });
    stack.pop();
    Ok(boundary)
}

fn validate_instance_config(
    source: &Node,
    definition: &CompositeDefinition,
) -> Result<(), ResolutionError> {
    for entry in &source.config {
        let count = definition
            .bindings
            .iter()
            .filter(|binding| binding.parameter == entry.key)
            .count();
        if count == 0 {
            return Err(ResolutionError::new(
                "CND-CMP-007",
                format!(
                    "composite `{}` has no parameter `{}`",
                    definition.id, entry.key
                ),
            ));
        }
        if source
            .config
            .iter()
            .filter(|candidate| candidate.key == entry.key)
            .count()
            != 1
        {
            return Err(ResolutionError::new(
                "CND-CFG-002",
                format!("duplicate composite parameter `{}`", entry.key),
            ));
        }
    }
    Ok(())
}

fn apply_bindings(
    source: &Node,
    definition: &CompositeDefinition,
    child: &mut Node,
) -> Result<(), ResolutionError> {
    for binding in definition
        .bindings
        .iter()
        .filter(|binding| binding.target.node == child.id)
    {
        let source_entry = source
            .config
            .iter()
            .find(|entry| entry.key == binding.parameter)
            .ok_or_else(|| {
                ResolutionError::new(
                    "CND-CMP-007",
                    format!(
                        "composite `{}` requires parameter `{}`",
                        definition.id, binding.parameter
                    ),
                )
            })?;
        if child
            .config
            .iter()
            .any(|entry| entry.key == binding.target.port)
        {
            return Err(ResolutionError::new(
                "CND-CMP-007",
                format!(
                    "binding for `{}.{}` conflicts with child configuration",
                    child.id, binding.target.port
                ),
            ));
        }
        child.config.push(ConfigEntry {
            key: binding.target.port.clone(),
            value: source_entry.value.clone(),
            source_span: source_entry.source_span,
        });
    }
    for binding in &definition.bindings {
        if !definition
            .nodes
            .iter()
            .any(|candidate| candidate.id == binding.target.node)
        {
            return Err(ResolutionError::new(
                "CND-CMP-003",
                format!(
                    "binding `{}` targets missing child `{}`",
                    binding.parameter, binding.target.node
                ),
            ));
        }
    }
    Ok(())
}

fn resolve_boundary_endpoint(
    instances: &BTreeMap<String, BoundaryMap>,
    endpoint: &Endpoint,
    direction: ExportDirection,
) -> Result<Endpoint, ResolutionError> {
    let boundary = instances.get(&endpoint.node).ok_or_else(|| {
        ResolutionError::new(
            "CND-CMP-006",
            format!(
                "endpoint `{}` bypasses an instance boundary or names no child",
                endpoint.node
            ),
        )
    })?;
    boundary
        .get(&(direction_key(direction), endpoint.port.clone()))
        .cloned()
        .ok_or_else(|| {
            ResolutionError::new(
                "CND-CMP-003",
                format!(
                    "dangling or wrong-direction port mapping `{}.{}`",
                    endpoint.node, endpoint.port
                ),
            )
        })
}

fn push_expanded_cord(expanded: &mut ExpandedSource, source: &Cord, from: Endpoint, to: Endpoint) {
    let mut cord = source.clone();
    cord.id = format!("cord-{}", expanded.cords.len());
    cord.from = from;
    cord.to = to;
    expanded.cords.push(cord);
}

const fn direction_key(direction: ExportDirection) -> u8 {
    match direction {
        ExportDirection::Input => 0,
        ExportDirection::Output => 1,
    }
}

fn expanded_id(path: &str) -> String {
    path.replace('/', ".")
}

fn validate_instance_id(id: &str) -> Result<(), ResolutionError> {
    if id.contains('/') || id.contains('.') || Id::new(id).is_err() {
        return Err(ResolutionError::new(
            "CND-CMP-001",
            format!("`{id}` is not a valid local instance id"),
        ));
    }
    Ok(())
}

/// A source node paired with its selected implementation.
#[derive(Debug)]
struct ResolvedNode<'a> {
    source: Node,
    definition: &'a RegisteredNode,
}

/// A source cord with resolved numeric endpoints.
#[derive(Debug)]
struct ResolvedCord {
    source: conduit_panel::Cord,
    from_node: usize,
    from_port: usize,
    to_node: usize,
    to_port: usize,
}

/// A validated, implementation-resolved hosted panel.
#[derive(Debug)]
pub struct ResolvedPanel<'a> {
    source: &'a Panel,
    nodes: Vec<ResolvedNode<'a>>,
    cords: Vec<ResolvedCord>,
    logical_composites: Vec<LogicalComposite>,
}

/// Presentation-neutral structured view of one validated hosted resolution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResolvedPanelView {
    pub panel_version: u16,
    pub root_nodes: usize,
    pub root_cords: usize,
    pub composites: Vec<ResolvedCompositeView>,
    pub nodes: Vec<ResolvedNodeView>,
    pub cords: Vec<ResolvedCordView>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResolvedCompositeView {
    pub path: String,
    pub definition: String,
    pub children: Vec<ResolvedChildView>,
    pub cords: Vec<ResolvedLogicalCordView>,
    pub exports: Vec<ResolvedExportView>,
    pub bindings: Vec<ResolvedBindingView>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResolvedChildView {
    pub path: String,
    pub contract_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResolvedLogicalCordView {
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResolvedExportView {
    pub direction: &'static str,
    pub id: String,
    pub target_node: String,
    pub target_port: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResolvedBindingView {
    pub parameter: String,
    pub target: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResolvedNodeView {
    pub index: usize,
    pub id: String,
    pub contract_id: String,
    pub inputs: Vec<ResolvedPortView>,
    pub outputs: Vec<ResolvedPortView>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResolvedPortView {
    pub id: String,
    pub type_id: String,
    pub delivery: &'static str,
    pub connections: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResolvedCordView {
    pub index: usize,
    pub id: String,
    pub from_node: String,
    pub from_port: String,
    pub to_node: String,
    pub to_port: String,
    pub capacity_items: u16,
    pub max_value_bytes: u32,
    pub max_queued_bytes: u64,
    pub low_watermark_items: u16,
    pub high_watermark_items: u16,
    pub pressure: String,
}

/// Exact source-derived topology facts consumed by hosted plan compilation.
///
/// This is not another plan type: it contains no implementation, artifact,
/// host, authority, resource, or resolver selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactTopologyView {
    pub source_semantic_hash: SemanticHash,
    pub nodes: Vec<ExactTopologyNode>,
    pub cords: Vec<ExactTopologyCord>,
    pub logical_composites: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactTopologyNode {
    pub instance: String,
    pub contract_id: String,
    pub contract_hash: SemanticHash,
    pub inputs: Vec<ExactTopologyPort>,
    pub outputs: Vec<ExactTopologyPort>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactTopologyPort {
    pub id: String,
    pub direction: Direction,
    pub contract_hash: SemanticHash,
    pub value_type: TypeContractRef<'static>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactTopologyCord {
    pub id: String,
    pub from_node: String,
    pub from_port: ExactTopologyPort,
    pub to_node: String,
    pub to_port: ExactTopologyPort,
    pub capacity_items: u16,
    pub max_value_bytes: u32,
    pub max_queued_bytes: u64,
    pub low_watermark_items: u16,
    pub high_watermark_items: u16,
    pub pressure: SourcePressure,
}

impl ResolvedPanel<'_> {
    /// Returns only semantic/source topology needed before exact host binding.
    pub fn exact_topology(&self) -> Result<ExactTopologyView, ResolutionError> {
        let source_semantic_hash = semantic_hash_text(&conduit_panel::semantic_source_hash_v2(
            self.source,
        ))
        .ok_or_else(|| ResolutionError::new("CND-CMP-002", "semantic source hash is malformed"))?;
        let nodes = self
            .nodes
            .iter()
            .map(|node| {
                let contract_hash =
                    OwnedNodeSchema::from_contract(node.definition.contract).semantic_hash();
                let inputs = node
                    .definition
                    .contract
                    .inputs
                    .iter()
                    .map(exact_topology_port)
                    .collect::<Result<Vec<_>, _>>()?;
                let outputs = node
                    .definition
                    .contract
                    .outputs
                    .iter()
                    .map(exact_topology_port)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(ExactTopologyNode {
                    instance: format!("root/{}", node.source.id),
                    contract_id: node.definition.contract.id.as_str().to_owned(),
                    contract_hash,
                    inputs,
                    outputs,
                })
            })
            .collect::<Result<Vec<_>, ResolutionError>>()?;
        let cords = self
            .cords
            .iter()
            .map(|cord| {
                let from = &self.nodes[cord.from_node];
                let to = &self.nodes[cord.to_node];
                Ok(ExactTopologyCord {
                    id: cord.source.id.clone(),
                    from_node: format!("root/{}", from.source.id),
                    from_port: exact_topology_port(
                        &from.definition.contract.outputs[cord.from_port],
                    )?,
                    to_node: format!("root/{}", to.source.id),
                    to_port: exact_topology_port(&to.definition.contract.inputs[cord.to_port])?,
                    capacity_items: cord.source.capacity_items,
                    max_value_bytes: cord.source.max_value_bytes,
                    max_queued_bytes: cord.source.max_queued_bytes,
                    low_watermark_items: cord.source.low_watermark_items,
                    high_watermark_items: cord.source.high_watermark_items,
                    pressure: cord.source.pressure.clone(),
                })
            })
            .collect::<Result<Vec<_>, ResolutionError>>()?;
        Ok(ExactTopologyView {
            source_semantic_hash,
            nodes,
            cords,
            logical_composites: self.logical_composites.len(),
        })
    }

    /// Returns structured resolution facts without choosing a CLI encoding.
    #[must_use]
    pub fn view(&self) -> ResolvedPanelView {
        let mut composites = self
            .logical_composites
            .iter()
            .map(|composite| ResolvedCompositeView {
                path: composite.path.clone(),
                definition: composite.definition.clone(),
                children: composite
                    .children
                    .iter()
                    .map(|(path, contract_id)| ResolvedChildView {
                        path: path.clone(),
                        contract_id: contract_id.clone(),
                    })
                    .collect(),
                cords: composite
                    .cords
                    .iter()
                    .map(|(from, to)| ResolvedLogicalCordView {
                        from: from.clone(),
                        to: to.clone(),
                    })
                    .collect(),
                exports: composite
                    .exports
                    .iter()
                    .map(|(direction, id, target)| ResolvedExportView {
                        direction: match direction {
                            ExportDirection::Input => "input",
                            ExportDirection::Output => "output",
                        },
                        id: id.clone(),
                        target_node: target.node.clone(),
                        target_port: target.port.clone(),
                    })
                    .collect(),
                bindings: composite
                    .bindings
                    .iter()
                    .map(|(parameter, target)| ResolvedBindingView {
                        parameter: parameter.clone(),
                        target: target.clone(),
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();
        composites.sort_by(|left, right| left.path.cmp(&right.path));
        let nodes = self
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| ResolvedNodeView {
                index,
                id: node.source.id.clone(),
                contract_id: node.definition.contract.id.as_str().to_owned(),
                inputs: node
                    .definition
                    .contract
                    .inputs
                    .iter()
                    .map(resolved_port_view)
                    .collect(),
                outputs: node
                    .definition
                    .contract
                    .outputs
                    .iter()
                    .map(resolved_port_view)
                    .collect(),
            })
            .collect();
        let cords = self
            .cords
            .iter()
            .enumerate()
            .map(|(index, cord)| ResolvedCordView {
                index,
                id: cord.source.id.clone(),
                from_node: self.nodes[cord.from_node].source.id.clone(),
                from_port: self.nodes[cord.from_node].definition.contract.outputs[cord.from_port]
                    .id
                    .as_str()
                    .to_owned(),
                to_node: self.nodes[cord.to_node].source.id.clone(),
                to_port: self.nodes[cord.to_node].definition.contract.inputs[cord.to_port]
                    .id
                    .as_str()
                    .to_owned(),
                capacity_items: cord.source.capacity_items,
                max_value_bytes: cord.source.max_value_bytes,
                max_queued_bytes: cord.source.max_queued_bytes,
                low_watermark_items: cord.source.low_watermark_items,
                high_watermark_items: cord.source.high_watermark_items,
                pressure: cord.source.pressure.to_string(),
            })
            .collect();
        ResolvedPanelView {
            panel_version: self.source.version,
            root_nodes: self.source.nodes.len(),
            root_cords: self.source.cords.len(),
            composites,
            nodes,
            cords,
        }
    }

    /// Produces deterministic logical and expanded resolution output.
    #[must_use]
    pub fn explain(&self) -> String {
        format!("{}\n{}", self.explain_logical(), self.explain_expanded())
    }

    /// Shows authored instances and composite boundary provenance.
    #[must_use]
    pub fn explain_logical(&self) -> String {
        use std::fmt::Write as _;

        let mut explanation = String::new();
        writeln!(
            explanation,
            "logical panel v{}: {} root nodes, {} root cords",
            self.source.version,
            self.source.nodes.len(),
            self.source.cords.len()
        )
        .expect("writing to String cannot fail");
        for node in &self.source.nodes {
            writeln!(explanation, "  instance {} : {}", node.id, node.kind)
                .expect("writing to String cannot fail");
        }
        let mut composites = self.logical_composites.iter().collect::<Vec<_>>();
        composites.sort_by(|left, right| left.path.cmp(&right.path));
        for composite in composites {
            writeln!(
                explanation,
                "  composite {} : {}",
                composite.path, composite.definition
            )
            .expect("writing to String cannot fail");
            for (child_path, definition) in &composite.children {
                writeln!(explanation, "    child {child_path} : {definition}")
                    .expect("writing to String cannot fail");
            }
            for (from, to) in &composite.cords {
                writeln!(explanation, "    cord {from} -> {to}")
                    .expect("writing to String cannot fail");
            }
            for (direction, id, target) in &composite.exports {
                let direction = match direction {
                    ExportDirection::Input => "input",
                    ExportDirection::Output => "output",
                };
                writeln!(
                    explanation,
                    "    export {direction} {id} -> {}.{}",
                    target.node, target.port
                )
                .expect("writing to String cannot fail");
            }
            for (parameter, target) in &composite.bindings {
                writeln!(explanation, "    bind {parameter} -> {target}")
                    .expect("writing to String cannot fail");
            }
        }
        explanation
    }

    /// Shows the exact flattened primitive execution topology.
    #[must_use]
    pub fn explain_expanded(&self) -> String {
        use std::fmt::Write as _;

        let mut explanation = String::new();
        writeln!(
            explanation,
            "expanded plan: {} nodes, {} cords",
            self.nodes.len(),
            self.cords.len()
        )
        .expect("writing to String cannot fail");
        for (index, node) in self.nodes.iter().enumerate() {
            writeln!(
                explanation,
                "  node {index}: {} : {} -> hosted builtin",
                node.source.id, node.definition.contract.id
            )
            .expect("writing to String cannot fail");
            for port in node.definition.contract.inputs {
                writeln!(
                    explanation,
                    "    input  {} : {} {:?} {:?}",
                    port.id, port.value_type.contract_id, port.delivery, port.connections
                )
                .expect("writing to String cannot fail");
            }
            for port in node.definition.contract.outputs {
                writeln!(
                    explanation,
                    "    output {} : {} {:?} {:?}",
                    port.id, port.value_type.contract_id, port.delivery, port.connections
                )
                .expect("writing to String cannot fail");
            }
        }
        for (index, cord) in self.cords.iter().enumerate() {
            writeln!(
                explanation,
                "  cord {index}: {}.{} -> {}.{} capacity={} max_value_bytes={} max_queued_bytes={} watermarks={}..{} pressure={}",
                self.nodes[cord.from_node].source.id,
                self.nodes[cord.from_node].definition.contract.outputs[cord.from_port].id,
                self.nodes[cord.to_node].source.id,
                self.nodes[cord.to_node].definition.contract.inputs[cord.to_port].id,
                cord.source.capacity_items,
                cord.source.max_value_bytes,
                cord.source.max_queued_bytes,
                cord.source.low_watermark_items,
                cord.source.high_watermark_items,
                cord.source.pressure
            )
            .expect("writing to String cannot fail");
        }
        explanation
    }

    /// Executes the finite acyclic proof runtime.
    pub fn run(&self, io: &mut RunIo<'_>) -> Result<ExecutionSummary, RuntimeError> {
        let mut outputs: Vec<Option<Vec<Value>>> = vec![None; self.nodes.len()];
        let mut remaining = self.nodes.len();

        while remaining > 0 {
            let mut progress = false;
            for node_index in 0..self.nodes.len() {
                if outputs[node_index].is_some() {
                    continue;
                }
                let incoming = self
                    .cords
                    .iter()
                    .filter(|cord| cord.to_node == node_index)
                    .collect::<Vec<_>>();
                if incoming
                    .iter()
                    .any(|cord| outputs[cord.from_node].is_none())
                {
                    continue;
                }

                let mut inputs = Vec::with_capacity(incoming.len());
                for input_port in 0..self.nodes[node_index].definition.contract.inputs.len() {
                    for cord in incoming.iter().filter(|cord| cord.to_port == input_port) {
                        let value = outputs[cord.from_node]
                            .as_ref()
                            .and_then(|values| values.get(cord.from_port))
                            .ok_or_else(|| {
                                RuntimeError::new(
                                    "CND-RUN-004",
                                    format!(
                                        "node `{}` did not emit required port {}",
                                        self.nodes[cord.from_node].source.id, cord.from_port
                                    ),
                                )
                            })?
                            .clone();
                        inputs.push(value);
                    }
                }

                let resolved = &self.nodes[node_index];
                let mut handler = (resolved.definition.factory)();
                let node_outputs = handler.run(&resolved.source, &inputs, io)?;
                if node_outputs.len() != resolved.definition.contract.outputs.len() {
                    return Err(RuntimeError::new(
                        "CND-RUN-004",
                        format!(
                            "node `{}` emitted {} ports; contract requires {}",
                            resolved.source.id,
                            node_outputs.len(),
                            resolved.definition.contract.outputs.len()
                        ),
                    ));
                }
                for (value, port) in node_outputs
                    .iter()
                    .zip(resolved.definition.contract.outputs)
                {
                    if value.value_type != port.value_type {
                        return Err(RuntimeError::new(
                            "CND-RUN-004",
                            format!(
                                "node `{}` emitted `{}` on `{}`; expected `{}`",
                                resolved.source.id,
                                value.value_type.contract_id,
                                port.id,
                                port.value_type.contract_id
                            ),
                        ));
                    }
                }
                outputs[node_index] = Some(node_outputs);
                remaining -= 1;
                progress = true;
            }
            if !progress {
                return Err(RuntimeError::new(
                    "CND-RUN-001",
                    "execution made no progress; the plan contains a dependency cycle",
                ));
            }
        }

        Ok(ExecutionSummary {
            nodes_completed: self.nodes.len(),
            cords_conducted: self.cords.len(),
        })
    }
}

fn exact_topology_port(port: &PortContract<'static>) -> Result<ExactTopologyPort, ResolutionError> {
    Ok(ExactTopologyPort {
        id: port.id.as_str().to_owned(),
        direction: port.direction,
        contract_hash: port
            .semantic_hash()
            .map_err(|_| ResolutionError::new("CND-CMP-002", "port contract is malformed"))?,
        value_type: port.value_type,
    })
}

fn semantic_hash_text(value: &str) -> Option<SemanticHash> {
    let value = value.strip_prefix("sha256:")?;
    if value.len() != 64 {
        return None;
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(pair[0])?;
        let low = hex_nibble(pair[1])?;
        bytes[index] = (high << 4) | low;
    }
    Some(SemanticHash::from_bytes(bytes))
}

const fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn resolved_port_view(port: &PortContract<'_>) -> ResolvedPortView {
    ResolvedPortView {
        id: port.id.as_str().to_owned(),
        type_id: port.value_type.contract_id.as_str().to_owned(),
        delivery: port.delivery.as_str(),
        connections: port.connections.as_str(),
    }
}

fn resolve_flow(source: &conduit_panel::Cord) -> Result<FlowPolicy<'_>, ResolutionError> {
    let capacity = FlowCapacity::new(
        source.capacity_items,
        source.max_value_bytes,
        source.max_queued_bytes,
    )
    .map_err(|error| ResolutionError::new(error.code(), error.to_string()))?;
    let watermarks = FlowWatermarks::new(
        source.low_watermark_items,
        source.high_watermark_items,
        capacity,
    )
    .map_err(|error| ResolutionError::new(error.code(), error.to_string()))?;
    let pressure = match &source.pressure {
        SourcePressure::Block => Pressure::Block(BlockingFairness::Fifo),
        SourcePressure::Reject => Pressure::Reject,
        SourcePressure::Coalesce { relation } => Pressure::Coalesce {
            relation: Id(relation),
        },
        SourcePressure::Sample { every, offset } => Pressure::Sample(
            SampleSchedule::new(*every, *offset)
                .map_err(|error| ResolutionError::new(error.code(), error.to_string()))?,
        ),
        SourcePressure::DropDisposable => Pressure::DropDisposable,
        SourcePressure::Disconnect => Pressure::Disconnect,
        SourcePressure::Fail => Pressure::Fail,
    };
    FlowPolicy::new(capacity, pressure, watermarks)
        .map_err(|error| ResolutionError::new(error.code(), error.to_string()))
}

/// Successful execution counts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionSummary {
    /// Nodes that reached completion.
    pub nodes_completed: usize,
    /// Resolved cords in the conducted plan.
    pub cords_conducted: usize,
}

/// Resolution failure with a stable diagnostic code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionError {
    /// Stable code.
    pub code: &'static str,
    /// Human-readable detail.
    pub message: String,
}

impl ResolutionError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ResolutionError {}

/// Runtime failure with a stable diagnostic code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeError {
    /// Stable code.
    pub code: &'static str,
    /// Human-readable detail.
    pub message: String,
}

impl RuntimeError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for RuntimeError {}

fn node_index(nodes: &[ResolvedNode<'_>], id: &str) -> Result<usize, ResolutionError> {
    let matches = nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.source.id == id)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [index] => Ok(*index),
        [] => Err(ResolutionError::new(
            "CND-ID-003",
            format!("unknown node `{id}`"),
        )),
        _ => Err(ResolutionError::new(
            "CND-ID-002",
            format!("duplicate node id `{id}`"),
        )),
    }
}

fn port_index(ports: &[PortContract<'_>], id: &str, node: &str) -> Result<usize, ResolutionError> {
    ports
        .iter()
        .position(|port| port.id.as_str() == id)
        .ok_or_else(|| ResolutionError::new("CND-ID-003", format!("unknown port `{node}.{id}`")))
}

fn reject_cycles(
    nodes: &[ResolvedNode<'_>],
    cords: &[ResolvedCord],
) -> Result<(), ResolutionError> {
    let mut completed = vec![false; nodes.len()];
    let mut remaining = nodes.len();
    while remaining > 0 {
        let mut progress = false;
        for node in 0..nodes.len() {
            if completed[node] {
                continue;
            }
            if cords
                .iter()
                .filter(|cord| cord.to_node == node)
                .all(|cord| completed[cord.from_node])
            {
                completed[node] = true;
                remaining -= 1;
                progress = true;
            }
        }
        if !progress {
            return Err(ResolutionError::new(
                "CND-CMP-001",
                "panel contains a dependency cycle",
            ));
        }
    }
    Ok(())
}

fn validate_empty_config(node: &Node) -> Result<(), ResolutionError> {
    if let Some(entry) = node.config.first() {
        return Err(ResolutionError::new(
            "CND-SRC-002",
            format!(
                "node `{}` does not accept configuration field `{}`",
                node.id, entry.key
            ),
        ));
    }
    Ok(())
}

fn validate_literal(node: &Node) -> Result<(), ResolutionError> {
    if node.config("value").is_none() {
        return Err(ResolutionError::new(
            "CND-SRC-002",
            format!("literal node `{}` requires `value`", node.id),
        ));
    }
    if let Some(entry) = node.config.iter().find(|entry| entry.key != "value") {
        return Err(ResolutionError::new(
            "CND-SRC-002",
            format!(
                "literal node `{}` has unknown field `{}`",
                node.id, entry.key
            ),
        ));
    }
    Ok(())
}

struct Literal;

impl Handler for Literal {
    fn run(
        &mut self,
        node: &Node,
        _inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let value = node
            .config("value")
            .ok_or_else(|| RuntimeError::new("CND-RUN-004", "literal value disappeared"))?;
        Ok(vec![Value::text(value.as_bytes())])
    }
}

struct Stdin;

impl Handler for Stdin {
    fn run(
        &mut self,
        _node: &Node,
        _inputs: &[Value],
        io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let mut bytes = Vec::new();
        io.input
            .read_to_end(&mut bytes)
            .map_err(|error| RuntimeError::new("CND-RUN-005", error.to_string()))?;
        std::str::from_utf8(&bytes)
            .map_err(|error| RuntimeError::new("CND-RUN-005", error.to_string()))?;
        Ok(vec![Value::text(bytes)])
    }
}

struct Uppercase;

impl Handler for Uppercase {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let input = inputs
            .first()
            .ok_or_else(|| RuntimeError::new("CND-RUN-004", "uppercase input missing"))?;
        let text = std::str::from_utf8(&input.bytes)
            .map_err(|error| RuntimeError::new("CND-RUN-005", error.to_string()))?;
        Ok(vec![Value::text(text.to_uppercase().into_bytes())])
    }
}

struct Stdout;

impl Handler for Stdout {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let input = inputs
            .first()
            .ok_or_else(|| RuntimeError::new("CND-RUN-004", "stdout input missing"))?;
        io.output
            .write_all(&input.bytes)
            .map_err(|error| RuntimeError::new("CND-RUN-005", error.to_string()))?;
        Ok(Vec::new())
    }
}

struct Stderr;

impl Handler for Stderr {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let input = inputs
            .first()
            .ok_or_else(|| RuntimeError::new("CND-RUN-004", "stderr input missing"))?;
        io.error
            .write_all(&input.bytes)
            .map_err(|error| RuntimeError::new("CND-RUN-005", error.to_string()))?;
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_panel::parse;

    #[test]
    fn resolves_explains_and_runs_a_panel() {
        let panel = parse(
            r#"
                panel 1
                node greeting : conduit/literal {
                    value = "Hello from Conduit.\n"
                }
                node shout : conduit/uppercase
                node output : conduit/stdout
                cord greeting.out -> shout.in
                cord shout.out -> output.in
            "#,
        )
        .expect("panel parses");
        let registry = Registry::default();
        let resolved = registry.resolve(&panel).expect("panel resolves");
        let explanation = resolved.explain();
        assert!(explanation.contains("capacity=8"));
        assert!(explanation.contains("max_value_bytes=65536"));
        assert!(explanation.contains("watermarks=7..8"));
        assert!(explanation.contains("pressure=block(fifo)"));

        let mut input = &b""[..];
        let mut output = Vec::new();
        let mut error = Vec::new();
        let summary = resolved
            .run(&mut RunIo {
                input: &mut input,
                output: &mut output,
                error: &mut error,
            })
            .expect("panel runs");

        assert_eq!(output, b"HELLO FROM CONDUIT.\n");
        assert!(error.is_empty());
        assert_eq!(summary.nodes_completed, 3);
        assert_eq!(summary.cords_conducted, 2);
    }

    #[test]
    fn rejects_unknown_implementations() {
        let panel = parse("panel 1\nnode mystery : example/missing").expect("panel parses");
        let error = Registry::default()
            .resolve(&panel)
            .expect_err("missing implementation");
        assert_eq!(error.code, "CND-IMP-001");
    }

    #[test]
    fn source_only_module_group_and_pool_forms_require_explicit_lowering() {
        for source in [
            "panel 1\nimport \"./child.panel\" as child",
            "panel 1\nport-group routes input : fixture/request indexed max 8",
            "panel 1\npool sessions : fixture/handler { maximum = 8 admission = reject deadline_ms = 1000 idle_timeout_ms = 5000 supervision = isolate cleanup = abort }",
            "panel 1\nnode app { node child : conduit/literal }\nroot app",
            "panel 1\nnode source : conduit/literal using ready",
        ] {
            let panel = parse(source).expect("source form parses");
            let error = Registry::default()
                .resolve(&panel)
                .expect_err("source-only construct must not be ignored");
            assert_eq!(error.code, "CND-PLN-005");
        }
    }

    #[test]
    fn rejects_loss_and_missing_type_traits_before_execution() {
        let sample = parse(
            "panel 1\nnode a : conduit/stdin\nnode b : conduit/stdout\n\
             cord a.out -> b.in {\n\
               pressure = sample\n\
               sample_every = 2\n\
             }",
        )
        .unwrap();
        let error = Registry::default()
            .resolve(&sample)
            .expect_err("lossless ports reject sampling");
        assert_eq!(error.code, "CND-FLW-002");

        let coalesce = parse(
            "panel 1\nnode a : conduit/stdin\nnode b : conduit/stdout\n\
             cord a.out -> b.in {\n\
               pressure = coalesce\n\
               coalescer = conduit/replace-latest\n\
             }",
        )
        .unwrap();
        let error = Registry::default()
            .resolve(&coalesce)
            .expect_err("text type does not declare coalescing");
        assert_eq!(error.code, "CND-FLW-004");
        assert_eq!(error.message, "coalescing-relation-unavailable");
    }

    #[test]
    fn stdin_is_an_explicit_source_node() {
        let panel = parse(
            r#"
                panel 1
                node input : conduit/stdin
                node output : conduit/stdout
                cord input.out -> output.in
            "#,
        )
        .expect("panel parses");
        let registry = Registry::default();
        let resolved = registry.resolve(&panel).expect("panel resolves");
        let mut input = &b"pipe friendly"[..];
        let mut output = Vec::new();
        let mut error = Vec::new();

        resolved
            .run(&mut RunIo {
                input: &mut input,
                output: &mut output,
                error: &mut error,
            })
            .expect("panel runs");

        assert_eq!(output, b"pipe friendly");
    }

    #[test]
    fn nested_composites_bind_parameters_export_ports_and_preserve_views() {
        let panel = parse(
            r#"
                panel 1
                composite example/literal-line {
                    node source : conduit/literal
                    export output text = source.out
                    bind value = source.value
                }
                composite example/upper-line {
                    node source : example/literal-line
                    node upper : conduit/uppercase
                    cord source.text -> upper.in
                    export output text = upper.out
                    bind value = source.value
                }
                node line : example/upper-line { value = "mixed Case" }
                node stdout : conduit/stdout
                node stderr : conduit/stderr
                cord line.text -> stdout.in
                cord line.text -> stderr.in
            "#,
        )
        .expect("nested composite parses");
        let registry = Registry::default();
        let resolved = registry.resolve(&panel).expect("composite resolves");
        let logical = resolved.explain_logical();
        let expanded = resolved.explain_expanded();
        assert!(logical.contains("composite line : example/upper-line"));
        assert!(logical.contains("composite line/source : example/literal-line"));
        assert!(logical.contains("child line/upper : conduit/uppercase"));
        assert!(logical.contains("export output text -> line.upper.out"));
        assert!(logical.contains("bind value -> line/source.value"));
        assert!(expanded.contains("line.source.source : conduit/literal"));
        assert!(expanded.contains("line.upper : conduit/uppercase"));
        assert!(!expanded.contains("example/upper-line -> hosted builtin"));

        let mut input = &b""[..];
        let mut output = Vec::new();
        let mut error = Vec::new();
        let summary = resolved
            .run(&mut RunIo {
                input: &mut input,
                output: &mut output,
                error: &mut error,
            })
            .expect("flattened composite runs");
        assert_eq!(summary.nodes_completed, 4);
        assert_eq!(output, b"MIXED CASE");
        assert_eq!(error, b"MIXED CASE");
    }

    #[test]
    fn composite_boundary_is_substitutable_for_primitive_inputs_and_outputs() {
        let panel = parse(
            r#"
                panel 1
                composite example/uppercase {
                    node worker : conduit/uppercase
                    export input in = worker.in
                    export output out = worker.out
                }
                node source : conduit/literal { value = "boundary" }
                node transform : example/uppercase
                node sink : conduit/stdout
                cord source.out -> transform.in
                cord transform.out -> sink.in
            "#,
        )
        .expect("transparent composite parses");
        let registry = Registry::default();
        let resolved = registry
            .resolve(&panel)
            .expect("transparent boundary resolves");
        let mut input = &b""[..];
        let mut output = Vec::new();
        let mut error = Vec::new();
        resolved
            .run(&mut RunIo {
                input: &mut input,
                output: &mut output,
                error: &mut error,
            })
            .expect("same primitive implementation runs");
        assert_eq!(output, b"BOUNDARY");
    }

    #[test]
    fn rejects_recursive_duplicate_dangling_and_boundary_bypass() {
        let registry = Registry::default();
        for (source, source_code, runtime_code) in [
            (
                "panel 1\ncomposite example/a { node b : example/b }\n\
                 composite example/b { node a : example/a }\n\
                 node root : example/a",
                None,
                Some("CND-CMP-005"),
            ),
            (
                "panel 1\ncomposite example/a {\n\
                   node source : conduit/stdin\n\
                   export output out = source.out\n\
                   export output out = source.out\n\
                 }\nnode root : example/a",
                Some("CND-SRC-002"),
                None,
            ),
            (
                "panel 1\ncomposite example/a {\n\
                   node source : conduit/stdin\n\
                   export output out = missing.out\n\
                 }\nnode root : example/a",
                Some("CND-SRC-009"),
                None,
            ),
            (
                "panel 1\ncomposite example/a {\n\
                   node source : conduit/stdin\n\
                   export input in = source.out\n\
                 }\nnode root : example/a",
                None,
                Some("CND-CMP-003"),
            ),
            (
                "panel 1\ncomposite example/a {\n\
                   node source : conduit/literal\n\
                   export output out = source.out\n\
                   bind value = source.missing\n\
                 }\nnode root : example/a { value = x }",
                None,
                Some("CND-CMP-003"),
            ),
            (
                "panel 1\ncomposite example/a {\n\
                   node source : conduit/stdin\n\
                   export output out = source.out\n\
                 }\nnode root : example/a\nnode sink : conduit/stdout\n\
                 cord root.source.out -> sink.in",
                Some("CND-SRC-009"),
                None,
            ),
        ] {
            match parse(source) {
                Err(error) => {
                    assert_eq!(Some(error.code), source_code, "{}", error.message);
                }
                Ok(panel) => {
                    assert!(source_code.is_none(), "expected source rejection");
                    let error = registry.resolve(&panel).expect_err("must reject");
                    assert_eq!(Some(error.code), runtime_code, "{}", error.message);
                }
            }
        }
    }
}
