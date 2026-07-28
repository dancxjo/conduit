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
    ConnectionCardinality, Delivery, Direction, Endpoint as CoreEndpoint, ExecutionPlan,
    FlowCapacity, FlowPolicy, FlowTypeFacts, FlowWatermarks, Id, LossAcceptance, NodeContract,
    PlanCord, PlanNode, PortContract, PortFlowConstraints, Presence, Pressure, SampleSchedule,
    SemanticHash, Sensitivity, TemporalContract, TerminalContract, TraitProof, TypeContractRef,
    ValueCardinality, validate_plan,
};
use conduit_panel::{Node, Panel, SourcePressure};

mod config_resolution;
mod type_registry;

pub use config_resolution::{
    ConfigAssignment, ConfigResolutionError, ConfigValue, ResolvedConfig, ResolvedConfigEntry,
    SecretValue, resolve_config, validate_config_update,
};
pub use type_registry::{
    ProviderTypeDecision, TypeComparisonStrategy, TypeContractDescription, TypeContractProvider,
    TypeRegistry, TypeRegistryError,
};

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
        if panel.nodes.len() > usize::from(u16::MAX) {
            return Err(ResolutionError::new(
                "CND-PLN-003",
                "panel has more nodes than the portable plan can address",
            ));
        }

        let mut nodes = Vec::with_capacity(panel.nodes.len());
        for source in &panel.nodes {
            Id::new(&source.id).map_err(|error| {
                ResolutionError::new(
                    "CND-ID-001",
                    format!("invalid node id `{}`: {error}", source.id),
                )
            })?;
            let definition = self.nodes.get(source.kind.as_str()).ok_or_else(|| {
                ResolutionError::new(
                    "CND-IMP-001",
                    format!("no ready implementation for `{}`", source.kind),
                )
            })?;
            (definition.validate_config)(source)?;
            nodes.push(ResolvedNode { source, definition });
        }

        let mut cords = Vec::with_capacity(panel.cords.len());
        for source in &panel.cords {
            let from_node = node_index(panel, &source.from.node)?;
            let to_node = node_index(panel, &source.to.node)?;
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
                let flow = resolve_flow(cord.source)?;
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
        validate_plan(&ExecutionPlan {
            nodes: &core_nodes,
            cords: &core_cords,
        })
        .map_err(|error| ResolutionError::new(error.code.as_str(), error.to_string()))?;

        reject_cycles(&nodes, &cords)?;

        Ok(ResolvedPanel {
            source: panel,
            nodes,
            cords,
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

/// A source node paired with its selected implementation.
#[derive(Debug)]
struct ResolvedNode<'a> {
    source: &'a Node,
    definition: &'a RegisteredNode,
}

/// A source cord with resolved numeric endpoints.
#[derive(Debug)]
struct ResolvedCord<'a> {
    source: &'a conduit_panel::Cord,
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
    cords: Vec<ResolvedCord<'a>>,
}

impl ResolvedPanel<'_> {
    /// Produces deterministic human-readable resolution output.
    #[must_use]
    pub fn explain(&self) -> String {
        use std::fmt::Write as _;

        let mut explanation = String::new();
        writeln!(
            explanation,
            "panel v{}: {} nodes, {} cords",
            self.source.version,
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
                let node_outputs = handler.run(resolved.source, &inputs, io)?;
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

fn node_index(panel: &Panel, id: &str) -> Result<usize, ResolutionError> {
    let matches = panel
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.id == id)
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
    cords: &[ResolvedCord<'_>],
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
}
