//! Host-independent source lowering over explicitly supplied semantic schemas.

use std::{collections::BTreeMap, convert::Infallible, fmt};

use conduit_core::{
    CanonicalError, CanonicalValue, CompatibilityOutcome, ConfigContract, ConfigFieldContract,
    ConfigIdentity, ConfigMutability, ConfigRequirement, ConnectionCardinality, Delivery,
    DescriptorRef, Direction, FieldDisposition, Id, IdError, InterfaceMemberRequirement,
    LossAcceptance, NodeContract, NodeInterfaceContract, NodeInterfaceContractRef,
    NodeInterfaceMember, PortContract, PortFlowConstraints, Presence, PrincipalPath, SemanticHash,
    Sensitivity, TemporalContract, TerminalContract, TypeContractRef, ValueCardinality,
    assess_node_interface,
};
use conduit_panel::{
    InstancePool, ModuleGraph, Panel, PoolAdmission, PoolCleanup, PoolSupervision, PortGroup,
    PortGroupShape, RootSelectionMode, SourcePressure, SourceSpan, SourceValue,
};
use sha2::{Digest as _, Sha256};

/// Owned exact type-contract identity used by hosted source schemas.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedTypeReference {
    pub id: String,
    pub schema_version: u32,
    pub semantic_hash: SemanticHash,
}

impl From<TypeContractRef<'_>> for OwnedTypeReference {
    fn from(value: TypeContractRef<'_>) -> Self {
        Self {
            id: value.contract_id.as_str().to_owned(),
            schema_version: value.schema_version,
            semantic_hash: value.semantic_hash,
        }
    }
}

/// Owned exact port-contract identity used by compile-time group expansion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedPortReference {
    pub id: String,
    pub direction: conduit_core::Direction,
    pub semantic_hash: SemanticHash,
}

impl OwnedPortReference {
    /// Copies one allocator-free core port contract into a hosted exact reference.
    pub fn from_contract(contract: &PortContract<'_>) -> Result<Self, CanonicalError<Infallible>> {
        Ok(Self {
            id: contract.id.as_str().to_owned(),
            direction: contract.direction,
            semantic_hash: contract.semantic_hash()?,
        })
    }
}

/// Owned canonical semantic value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OwnedSemanticValue {
    Null,
    Boolean(bool),
    Integer(i128),
    Bytes(Vec<u8>),
    Text(String),
    Identifier(String),
    List(Vec<OwnedSemanticValue>),
    Map(Vec<(String, OwnedSemanticValue)>),
    Set(Vec<OwnedSemanticValue>),
}

impl OwnedSemanticValue {
    /// Copies a canonical borrowed core value into hosted storage.
    #[must_use]
    pub fn from_canonical(value: CanonicalValue<'_>) -> Self {
        match value {
            CanonicalValue::Null => Self::Null,
            CanonicalValue::Boolean(value) => Self::Boolean(value),
            CanonicalValue::Integer(value) => Self::Integer(value),
            CanonicalValue::Bytes(value) => Self::Bytes(value.to_vec()),
            CanonicalValue::Text(value) => Self::Text(value.to_owned()),
            CanonicalValue::Identifier(value) => Self::Identifier(value.as_str().to_owned()),
            CanonicalValue::List(values) => {
                Self::List(values.iter().copied().map(Self::from_canonical).collect())
            }
            CanonicalValue::Map(fields) => Self::Map(
                fields
                    .iter()
                    .filter(|field| match field.disposition {
                        FieldDisposition::Semantic => true,
                        FieldDisposition::Defaulted(default) => field.value != *default,
                        FieldDisposition::Annotation => false,
                    })
                    .map(|field| {
                        (
                            field.name.as_str().to_owned(),
                            Self::from_canonical(field.value),
                        )
                    })
                    .collect(),
            ),
            CanonicalValue::Set(values) => {
                Self::Set(values.iter().copied().map(Self::from_canonical).collect())
            }
        }
    }
}

/// Missing-value contract for source lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OwnedConfigRequirement {
    Required,
    Optional,
    Defaulted(OwnedSemanticValue),
}

/// One exact field schema supplied without selecting an implementation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedConfigFieldSchema {
    pub key: String,
    pub value_type: OwnedTypeReference,
    pub requirement: OwnedConfigRequirement,
    pub sensitivity: Sensitivity,
    pub mutability: ConfigMutability,
    pub identity: ConfigIdentity,
    /// Authored origin of a source-defined default, if any.
    pub default_origin: Option<SourceOrigin>,
}

impl OwnedConfigFieldSchema {
    /// Copies a core configuration field without adding source provenance.
    #[must_use]
    pub fn from_contract(field: ConfigFieldContract<'_>) -> Self {
        let requirement = match field.requirement {
            ConfigRequirement::Required => OwnedConfigRequirement::Required,
            ConfigRequirement::Optional => OwnedConfigRequirement::Optional,
            ConfigRequirement::Defaulted(value) => {
                OwnedConfigRequirement::Defaulted(OwnedSemanticValue::from_canonical(value))
            }
        };
        Self {
            key: field.key.as_str().to_owned(),
            value_type: field.value_type.into(),
            requirement,
            sensitivity: field.sensitivity,
            mutability: field.mutability,
            identity: field.identity,
            default_origin: None,
        }
    }
}

/// One semantic node schema used by source lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedNodeSchema {
    pub id: String,
    pub fields: Vec<OwnedConfigFieldSchema>,
}

impl OwnedNodeSchema {
    /// Copies one allocator-free core node contract into hosted schema storage.
    #[must_use]
    pub fn from_contract(contract: &NodeContract<'_>) -> Self {
        Self {
            id: contract.id.as_str().to_owned(),
            fields: contract
                .config
                .fields
                .iter()
                .copied()
                .map(OwnedConfigFieldSchema::from_contract)
                .collect(),
        }
    }

    /// Computes the exact, order-independent hosted schema identity.
    #[must_use]
    pub fn semantic_hash(&self) -> SemanticHash {
        hash_node_schema(self)
    }
}

/// Provider-specific literal rejection without value material.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiteralValidationError {
    WrongKind,
    Overflow,
    InvalidValue,
    ProviderUnavailable,
}

/// Schema boundary for source lowering. Implementations and hosts are absent.
pub trait SourceContractCatalog {
    fn node_schema(&self, id: &str) -> Option<OwnedNodeSchema>;
    fn type_reference(&self, id: &str) -> Option<OwnedTypeReference>;
    fn port_contract(&self, id: &str) -> Option<OwnedPortReference>;
    fn validate_literal(
        &self,
        expected: &OwnedTypeReference,
        source: &SourceValue,
    ) -> Result<OwnedSemanticValue, LiteralValidationError>;
    fn validate_default(
        &self,
        expected: &OwnedTypeReference,
        value: &OwnedSemanticValue,
    ) -> Result<(), LiteralValidationError>;

    fn node_contract(&self, _id: &str) -> Option<OwnedNodeContract> {
        None
    }
    fn interface_contract(&self, _id: &str) -> Option<OwnedInterfaceContract> {
        None
    }
}

/// Owned port contract representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedPortContract {
    pub id: String,
    pub direction: Direction,
    pub value_type: OwnedTypeReference,
    pub presence: Presence,
    pub connections: ConnectionCardinality,
    pub values: ValueCardinality,
    pub delivery: Delivery,
    pub temporal: TemporalContract,
    pub terminal: TerminalContract,
    pub sensitivity: Sensitivity,
    pub loss: LossAcceptance,
}

impl OwnedPortContract {
    #[must_use]
    pub fn from_contract(port: &PortContract<'_>) -> Self {
        Self {
            id: port.id.as_str().to_owned(),
            direction: port.direction,
            value_type: port.value_type.into(),
            presence: port.presence,
            connections: port.connections,
            values: port.values,
            delivery: port.delivery,
            temporal: port.temporal,
            terminal: port.terminal,
            sensitivity: port.sensitivity,
            loss: port.flow.loss,
        }
    }

    pub fn to_core<'a>(&'a self) -> Result<PortContract<'a>, IdError> {
        Ok(PortContract {
            id: Id::new(&self.id)?,
            direction: self.direction,
            value_type: TypeContractRef {
                contract_id: Id::new(&self.value_type.id)?,
                schema_version: self.value_type.schema_version,
                semantic_hash: self.value_type.semantic_hash,
            },
            presence: self.presence,
            connections: self.connections,
            values: self.values,
            delivery: self.delivery,
            temporal: self.temporal,
            terminal: self.terminal,
            sensitivity: self.sensitivity,
            flow: PortFlowConstraints { loss: self.loss },
        })
    }
}

/// Owned node contract representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedNodeContract {
    pub id: String,
    pub inputs: Vec<OwnedPortContract>,
    pub outputs: Vec<OwnedPortContract>,
}

impl OwnedNodeContract {
    #[must_use]
    pub fn from_contract(contract: &NodeContract<'_>) -> Self {
        Self {
            id: contract.id.as_str().to_owned(),
            inputs: contract
                .inputs
                .iter()
                .map(OwnedPortContract::from_contract)
                .collect(),
            outputs: contract
                .outputs
                .iter()
                .map(OwnedPortContract::from_contract)
                .collect(),
        }
    }

    #[must_use]
    pub fn semantic_hash(&self) -> SemanticHash {
        let mut facts = Vec::new();
        facts.push(hash_parts(
            "conduit/owned-node-contract",
            &[self.id.as_str()],
        ));
        for p in &self.inputs {
            if let Ok(core_p) = p.to_core() {
                if let Ok(h) = core_p.semantic_hash() {
                    facts.push(h);
                }
            }
        }
        for p in &self.outputs {
            if let Ok(core_p) = p.to_core() {
                if let Ok(h) = core_p.semantic_hash() {
                    facts.push(h);
                }
            }
        }
        hash_facts("conduit/node-contract", &facts)
    }
}

/// Owned interface member representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedInterfaceMember {
    pub requirement: InterfaceMemberRequirement,
    pub id: String,
    pub direction: Direction,
    pub value_type: OwnedTypeReference,
    pub presence: Presence,
    pub connections: ConnectionCardinality,
    pub values: ValueCardinality,
    pub delivery: Delivery,
    pub temporal: TemporalContract,
    pub terminal: TerminalContract,
    pub sensitivity: Sensitivity,
    pub loss: LossAcceptance,
}

impl OwnedInterfaceMember {
    pub fn to_core<'a>(&'a self) -> Result<NodeInterfaceMember<'a>, IdError> {
        Ok(NodeInterfaceMember {
            requirement: self.requirement,
            port: PortContract {
                id: Id::new(&self.id)?,
                direction: self.direction,
                value_type: TypeContractRef {
                    contract_id: Id::new(&self.value_type.id)?,
                    schema_version: self.value_type.schema_version,
                    semantic_hash: self.value_type.semantic_hash,
                },
                presence: self.presence,
                connections: self.connections,
                values: self.values,
                delivery: self.delivery,
                temporal: self.temporal,
                terminal: self.terminal,
                sensitivity: self.sensitivity,
                flow: PortFlowConstraints { loss: self.loss },
            },
        })
    }
}

/// Owned interface contract representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedPrincipalPath {
    pub receiving: Option<String>,
    pub outgoing: Option<String>,
}

impl OwnedPrincipalPath {
    #[must_use]
    pub const fn none() -> Self {
        Self {
            receiving: None,
            outgoing: None,
        }
    }

    fn to_core(&self) -> Result<PrincipalPath<'_>, IdError> {
        Ok(PrincipalPath {
            receiving: self.receiving.as_deref().map(Id::new).transpose()?,
            outgoing: self.outgoing.as_deref().map(Id::new).transpose()?,
        })
    }
}

/// Owned interface contract representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedInterfaceContract {
    pub id: String,
    pub schema_version: u32,
    pub principal_path: OwnedPrincipalPath,
    pub members: Vec<OwnedInterfaceMember>,
    pub semantic_hash: SemanticHash,
}

/// Hosted principal projection failure with stable core reason spelling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnedPrincipalProjectionError {
    InvalidContract(&'static str),
    Unavailable { direction: Direction },
}

impl OwnedInterfaceContract {
    pub fn compute_semantic_hash(&self) -> Result<SemanticHash, IdError> {
        let core_id = Id::new(&self.id)?;
        let members = self
            .members
            .iter()
            .map(|m| m.to_core())
            .collect::<Result<Vec<_>, IdError>>()?;
        let core_contract = NodeInterfaceContract {
            id: core_id,
            principal_path: self.principal_path.to_core()?,
            members: &members,
            requirements: &[],
        };
        let mut scratch = vec![SemanticHash::from_bytes([0; 32]); members.len()];
        core_contract
            .semantic_hash(&mut scratch)
            .map_err(|_| IdError::InvalidSlash)
    }

    /// Projects a bare endpoint using only this exact semantic descriptor.
    pub fn project_principal(
        &self,
        direction: Direction,
    ) -> Result<&OwnedInterfaceMember, OwnedPrincipalProjectionError> {
        let core_id = Id::new(&self.id).map_err(|_| {
            OwnedPrincipalProjectionError::InvalidContract("interface-invalid-identifier")
        })?;
        let members = self
            .members
            .iter()
            .map(OwnedInterfaceMember::to_core)
            .collect::<Result<Vec<_>, IdError>>()
            .map_err(|_| {
                OwnedPrincipalProjectionError::InvalidContract("interface-invalid-member")
            })?;
        let principal_path = self.principal_path.to_core().map_err(|_| {
            OwnedPrincipalProjectionError::InvalidContract("interface-invalid-identifier")
        })?;
        let contract = NodeInterfaceContract {
            id: core_id,
            principal_path,
            members: &members,
            requirements: &[],
        };
        let projected = contract
            .project_principal(direction)
            .map_err(|error| match error {
                conduit_core::PrincipalProjectionError::InvalidContract(error) => {
                    OwnedPrincipalProjectionError::InvalidContract(error.as_str())
                }
                conduit_core::PrincipalProjectionError::Unavailable { direction } => {
                    OwnedPrincipalProjectionError::Unavailable { direction }
                }
            })?;
        self.members
            .iter()
            .find(|member| member.id == projected.port.id.as_str() && member.direction == direction)
            .ok_or(OwnedPrincipalProjectionError::InvalidContract(
                "interface-principal-member-missing",
            ))
    }
}

/// Authored source location, including its content-identified module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceOrigin {
    pub module_uri: String,
    pub module_hash: String,
    pub span: SourceSpan,
}

/// Provenance of one resolved configuration value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigProvenance {
    Authored,
    SchemaDefault,
    PlanBinding,
}

/// Public semantic data or an unresolved protected binding.
#[derive(Clone, Eq, PartialEq)]
pub enum LoweredConfigValue {
    Public(OwnedSemanticValue),
    SecretReference(String),
}

impl fmt::Debug for LoweredConfigValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Public(value) => formatter.debug_tuple("Public").field(value).finish(),
            Self::SecretReference(_) => formatter.write_str("SecretReference([REDACTED])"),
        }
    }
}

/// One validated and defaulted source configuration field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweredConfigEntry {
    pub field: OwnedConfigFieldSchema,
    pub value: LoweredConfigValue,
    pub provenance: ConfigProvenance,
    pub origin: Option<SourceOrigin>,
}

/// One lowered semantic node descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweredNode {
    pub path: String,
    pub contract_id: String,
    pub contract_hash: SemanticHash,
    pub config: Vec<LoweredConfigEntry>,
    pub semantic_hash: SemanticHash,
}

/// One ordinary port expanded from a finite group.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweredGroupPort {
    pub group_id: String,
    pub member: String,
    pub ordinal: u16,
    pub id: String,
    pub logical_group_path: String,
    pub expanded_port_path: String,
    pub direction: conduit_panel::ExportDirection,
    pub group_maximum: u16,
    pub port_contract: OwnedPortReference,
    pub semantic_hash: SemanticHash,
    pub group_origin: SourceOrigin,
    /// Exact authored key origin. Indexed members are derived and have no
    /// separately authored member token.
    pub member_origin: Option<SourceOrigin>,
    pub origin: SourceOrigin,
}

/// Exact source-level pool specification passed to plan resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweredPool {
    pub path: String,
    pub template_contract_id: String,
    pub template_contract_hash: SemanticHash,
    pub maximum: u16,
    pub admission: PoolAdmission,
    pub deadline_ms: u64,
    pub idle_timeout_ms: u64,
    pub supervision: PoolSupervision,
    pub cleanup: PoolCleanup,
    pub semantic_hash: SemanticHash,
    pub origin: SourceOrigin,
}

/// One source-map relationship for a lowered semantic element.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceMapEntry {
    pub semantic_path: String,
    pub origins: Vec<SourceOrigin>,
}

/// Complete lowered source closure. It is not an ExecutionPlan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweredTopologyBase {
    pub nodes: Vec<LoweredNode>,
    pub group_ports: Vec<LoweredGroupPort>,
    pub pools: Vec<LoweredPool>,
    pub source_map: Vec<SourceMapEntry>,
    pub semantic_hash: SemanticHash,
}

impl LoweredTopologyBase {
    /// Explains provenance without printing configuration value material.
    #[must_use]
    pub fn explain(&self) -> String {
        let mut output = String::new();
        for node in &self.nodes {
            output.push_str("node ");
            output.push_str(&node.path);
            output.push('\n');
            for entry in &node.config {
                output.push_str("  config ");
                output.push_str(&entry.field.key);
                output.push_str(" provenance=");
                output.push_str(match entry.provenance {
                    ConfigProvenance::Authored => "authored",
                    ConfigProvenance::SchemaDefault => "schema-default",
                    ConfigProvenance::PlanBinding => "plan-binding",
                });
                if matches!(&entry.value, LoweredConfigValue::SecretReference(_)) {
                    output.push_str(" value=[REDACTED]");
                }
                output.push('\n');
            }
        }
        output
    }
}

pub const LOWERED_SOURCE_SCHEMA_VERSION: u16 = 0;
pub const SOURCE_AST_SCHEMA_VERSION: u16 = conduit_panel::SOURCE_AST_SCHEMA_VERSION;

/// Corrected root-selection input to lowering. Selection mode is explanatory;
/// equivalent explicit and sole-root selections share semantic identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweredRootSelection {
    pub entry_uri: String,
    pub target: String,
    pub mode: RootSelectionMode,
    pub authored_source_hash: String,
    pub semantic_hash: SemanticHash,
    pub origin: SourceOrigin,
}

/// Version 2 node descriptor retaining its unresolved implementation/capability
/// constraint for later plan resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweredAuthoredNode {
    pub path: String,
    pub contract_id: String,
    pub contract_hash: SemanticHash,
    pub unresolved_constraint: Option<String>,
    pub config: Vec<LoweredConfigEntry>,
    pub semantic_hash: SemanticHash,
    pub origin: SourceOrigin,
}

/// One complete ordinary authored cord retained before plan resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweredCord {
    pub path: String,
    pub from: String,
    pub to: String,
    pub capacity_items: u16,
    pub max_value_bytes: u32,
    pub max_queued_bytes: u64,
    pub low_watermark_items: u16,
    pub high_watermark_items: u16,
    pub pressure: SourcePressure,
    pub semantic_hash: SemanticHash,
    pub origin: SourceOrigin,
}

/// Explicit ownership of one child by one authored composite.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweredCompositeChild {
    pub composite_path: String,
    pub child_path: String,
    pub semantic_hash: SemanticHash,
    pub origin: SourceOrigin,
}

/// One authored composite definition and its exact boundary contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweredComposite {
    pub path: String,
    pub contract_id: String,
    pub contract_hash: SemanticHash,
    pub semantic_hash: SemanticHash,
    pub origin: SourceOrigin,
}

/// One explicit composite boundary-to-child port relationship.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweredExport {
    pub path: String,
    pub composite_path: String,
    pub direction: conduit_panel::ExportDirection,
    pub id: String,
    pub target: String,
    pub semantic_hash: SemanticHash,
    pub origin: SourceOrigin,
}

/// One explicit composite parameter-to-child configuration relationship.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweredBinding {
    pub path: String,
    pub composite_path: String,
    pub parameter: String,
    pub target: String,
    pub semantic_hash: SemanticHash,
    pub origin: SourceOrigin,
}

/// Corrected complete lowered source closure. It remains semantic input to
/// planning, never an exact ExecutionPlan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweredTopology {
    pub schema_version: u16,
    pub source_ast_schema_version: u16,
    pub root_selection: Option<LoweredRootSelection>,
    pub nodes: Vec<LoweredAuthoredNode>,
    pub cords: Vec<LoweredCord>,
    pub composites: Vec<LoweredComposite>,
    pub composite_children: Vec<LoweredCompositeChild>,
    pub exports: Vec<LoweredExport>,
    pub bindings: Vec<LoweredBinding>,
    pub group_ports: Vec<LoweredGroupPort>,
    pub pools: Vec<LoweredPool>,
    pub source_map: Vec<SourceMapEntry>,
    pub semantic_hash: SemanticHash,
}

/// One current grammar supervision relationship retained before exact planning.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweredSupervision {
    pub path: String,
    pub subject_path: String,
    pub handler_path: String,
    pub semantic_hash: SemanticHash,
    pub origin: SourceOrigin,
}

/// The current form adds explicit supervisor relationships without changing the
/// current topology representation or turning them into data cords.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweredSupervisedTopology {
    pub schema_version: u16,
    pub source_ast_schema_version: u16,
    pub topology: Box<LoweredTopology>,
    pub supervisions: Vec<LoweredSupervision>,
    pub source_map: Vec<SourceMapEntry>,
    pub semantic_hash: SemanticHash,
}

/// Member-level satisfaction proof retained in lowered source current schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweredInterfaceMemberProof {
    pub member_id: String,
    pub direction: String,
    pub requirement: String,
    pub outcome: String,
    pub reason: String,
}

/// Node interface satisfaction proof retained in lowered source current schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweredInterfaceProof {
    pub path: String,
    pub interface_id: String,
    pub interface_hash: SemanticHash,
    pub candidate_hash: SemanticHash,
    pub outcome: String,
    pub reason: String,
    pub member_proofs: Vec<LoweredInterfaceMemberProof>,
    pub semantic_hash: SemanticHash,
    pub origin: SourceOrigin,
}

/// Current lowered source retaining node-interface satisfaction evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweredSource {
    pub schema_version: u16,
    pub source_ast_schema_version: u16,
    pub supervised: Box<LoweredSupervisedTopology>,
    pub interface_proofs: Vec<LoweredInterfaceProof>,
    pub source_map: Vec<SourceMapEntry>,
    pub semantic_hash: SemanticHash,
}

/// Structured, value-safe lowering diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweringDiagnostic {
    pub code: &'static str,
    pub semantic_path: String,
    pub expected_contract: Option<Box<OwnedTypeReference>>,
    pub origin: Option<Box<SourceOrigin>>,
    pub message: String,
}

impl fmt::Display for LoweringDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at {}: {}",
            self.code, self.semantic_path, self.message
        )
    }
}

impl std::error::Error for LoweringDiagnostic {}

/// Lowers a resolved, content-identified module graph without I/O or probing.
pub fn lower_source_base(
    graph: &ModuleGraph,
    catalog: &impl SourceContractCatalog,
) -> Result<LoweredTopologyBase, LoweringDiagnostic> {
    lower_source_with_identity(graph, catalog, conduit_panel::semantic_source_hash)
}

fn lower_source_with_identity(
    graph: &ModuleGraph,
    catalog: &impl SourceContractCatalog,
    source_identity: fn(&Panel) -> String,
) -> Result<LoweredTopologyBase, LoweringDiagnostic> {
    let mut definitions = BTreeMap::new();
    for module in &graph.modules {
        for definition in &module.panel.definitions {
            definitions.insert(
                (module.canonical_uri.clone(), definition.id.clone()),
                definition_schema(definition, catalog, module, source_identity)?,
            );
        }
    }

    let mut nodes = Vec::new();
    let mut group_ports = Vec::new();
    let mut pools = Vec::new();
    let mut source_map = Vec::new();
    for module in &graph.modules {
        lower_panel(
            module,
            catalog,
            &definitions,
            &mut nodes,
            &mut group_ports,
            &mut pools,
            &mut source_map,
        )?;
    }

    let mut facts = Vec::new();
    facts.extend(nodes.iter().map(|node| node.semantic_hash));
    facts.extend(group_ports.iter().map(|port| port.semantic_hash));
    facts.extend(pools.iter().map(|pool| pool.semantic_hash));
    facts.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    let semantic_hash = hash_facts("conduit/lowered-source", &facts);
    Ok(LoweredTopologyBase {
        nodes,
        group_ports,
        pools,
        source_map,
        semantic_hash,
    })
}

/// Lowers the corrected current schema closure while retaining every
/// plan-relevant authored topology fact and unresolved constraint.
pub fn lower_topology(
    graph: &ModuleGraph,
    catalog: &impl SourceContractCatalog,
) -> Result<LoweredTopology, LoweringDiagnostic> {
    let base = lower_source_with_identity(graph, catalog, conduit_panel::semantic_source_hash)?;
    let mut source_nodes = BTreeMap::new();
    for module in &graph.modules {
        let uri = &module.canonical_uri;
        for node in &module.panel.nodes {
            source_nodes.insert(format!("{uri}/node/{}", node.id), (node, module));
        }
        for definition in &module.panel.definitions {
            for node in &definition.nodes {
                source_nodes.insert(
                    format!("{uri}/definition/{}/node/{}", definition.id, node.id),
                    (node, module),
                );
            }
        }
    }

    let mut source_map = base.source_map.clone();
    let mut nodes = Vec::with_capacity(base.nodes.len());
    for node in base.nodes {
        let (source_node, module) = source_nodes.get(&node.path).ok_or_else(|| {
            diagnostic(
                "CND-LWR-012",
                &node.path,
                None,
                None,
                "current node provenance cannot be reconstructed",
            )
        })?;
        let node_origin = origin(
            &module.canonical_uri,
            &module.content_hash,
            source_node.source_span,
        );
        if let Some(span) = source_node.constraint_span {
            source_map.push(SourceMapEntry {
                semantic_path: format!("{}/constraint", node.path),
                origins: vec![origin(&module.canonical_uri, &module.content_hash, span)],
            });
        }
        let config_hash = hash_config(&node.contract_id, &node.config);
        let semantic_hash = hash_authored_node(
            &node.path,
            &node.contract_id,
            node.contract_hash,
            source_node.constraint.as_deref(),
            config_hash,
        );
        nodes.push(LoweredAuthoredNode {
            path: node.path,
            contract_id: node.contract_id,
            contract_hash: node.contract_hash,
            unresolved_constraint: source_node.constraint.clone(),
            config: node.config,
            semantic_hash,
            origin: node_origin,
        });
    }

    let mut cords = Vec::new();
    let mut composites = Vec::new();
    let mut composite_children = Vec::new();
    let mut exports = Vec::new();
    let mut bindings = Vec::new();
    for module in &graph.modules {
        let uri = &module.canonical_uri;
        for cord in &module.panel.cords {
            let lowered = lower_cord(cord, uri, uri, &module.content_hash);
            source_map.push(SourceMapEntry {
                semantic_path: lowered.path.clone(),
                origins: vec![lowered.origin.clone()],
            });
            cords.push(lowered);
        }
        for definition in &module.panel.definitions {
            let composite_path = format!("{uri}/definition/{}", definition.id);
            let schema = definition_schema(
                definition,
                catalog,
                module,
                conduit_panel::semantic_source_hash,
            )?;
            let contract_hash = schema.semantic_hash();
            let composite_origin = origin(uri, &module.content_hash, definition.source_span);
            let contract_hash_text = contract_hash.to_string();
            composites.push(LoweredComposite {
                path: composite_path.clone(),
                contract_id: schema.id.clone(),
                contract_hash,
                semantic_hash: hash_parts(
                    "conduit/lowered-composite",
                    &[&composite_path, &schema.id, &contract_hash_text],
                ),
                origin: composite_origin.clone(),
            });
            source_map.push(SourceMapEntry {
                semantic_path: composite_path.clone(),
                origins: vec![composite_origin],
            });
            for child in &definition.nodes {
                let child_path = format!("{composite_path}/node/{}", child.id);
                let relationship_path = format!("{composite_path}/child/{}", child.id);
                let child_origin = origin(uri, &module.content_hash, child.source_span);
                composite_children.push(LoweredCompositeChild {
                    composite_path: composite_path.clone(),
                    child_path: child_path.clone(),
                    semantic_hash: hash_parts(
                        "conduit/lowered-composite-child",
                        &[&composite_path, &child_path],
                    ),
                    origin: child_origin.clone(),
                });
                source_map.push(SourceMapEntry {
                    semantic_path: relationship_path,
                    origins: vec![child_origin],
                });
            }
            for cord in &definition.cords {
                let lowered = lower_cord(cord, &composite_path, uri, &module.content_hash);
                source_map.push(SourceMapEntry {
                    semantic_path: lowered.path.clone(),
                    origins: vec![lowered.origin.clone()],
                });
                cords.push(lowered);
            }
            for export in &definition.exports {
                let path = format!("{composite_path}/export/{}", export.id);
                let target = format!(
                    "{composite_path}/node/{}/port/{}",
                    export.target.node, export.target.port
                );
                let direction = match export.direction {
                    conduit_panel::ExportDirection::Input => "input",
                    conduit_panel::ExportDirection::Output => "output",
                };
                let export_origin = origin(uri, &module.content_hash, export.source_span);
                exports.push(LoweredExport {
                    path: path.clone(),
                    composite_path: composite_path.clone(),
                    direction: export.direction,
                    id: export.id.clone(),
                    target: target.clone(),
                    semantic_hash: hash_parts(
                        "conduit/lowered-export",
                        &[&composite_path, direction, &export.id, &target],
                    ),
                    origin: export_origin.clone(),
                });
                source_map.push(SourceMapEntry {
                    semantic_path: path,
                    origins: vec![export_origin],
                });
            }
            for binding in &definition.bindings {
                let path = format!("{composite_path}/binding/{}", binding.parameter);
                let target = format!(
                    "{composite_path}/node/{}/config/{}",
                    binding.target.node, binding.target.port
                );
                let binding_origin = origin(uri, &module.content_hash, binding.source_span);
                bindings.push(LoweredBinding {
                    path: path.clone(),
                    composite_path: composite_path.clone(),
                    parameter: binding.parameter.clone(),
                    target: target.clone(),
                    semantic_hash: hash_parts(
                        "conduit/lowered-binding",
                        &[&composite_path, &binding.parameter, &target],
                    ),
                    origin: binding_origin.clone(),
                });
                source_map.push(SourceMapEntry {
                    semantic_path: path,
                    origins: vec![binding_origin],
                });
            }
        }
    }

    let root_selection = lower_root_selection(graph)?;
    if let Some(selection) = &root_selection {
        source_map.push(SourceMapEntry {
            semantic_path: format!("{}/selected-root/{}", selection.entry_uri, selection.target),
            origins: vec![selection.origin.clone()],
        });
    }

    let mut facts = Vec::new();
    facts.extend(nodes.iter().map(|node| node.semantic_hash));
    facts.extend(cords.iter().map(|cord| cord.semantic_hash));
    facts.extend(composites.iter().map(|composite| composite.semantic_hash));
    facts.extend(composite_children.iter().map(|child| child.semantic_hash));
    facts.extend(exports.iter().map(|export| export.semantic_hash));
    facts.extend(bindings.iter().map(|binding| binding.semantic_hash));
    facts.extend(base.group_ports.iter().map(|port| port.semantic_hash));
    facts.extend(base.pools.iter().map(|pool| pool.semantic_hash));
    facts.extend(
        root_selection
            .iter()
            .map(|selection| selection.semantic_hash),
    );
    facts.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    let semantic_hash = hash_topology_facts(&facts);

    Ok(LoweredTopology {
        schema_version: LOWERED_SOURCE_SCHEMA_VERSION,
        source_ast_schema_version: SOURCE_AST_SCHEMA_VERSION,
        root_selection,
        nodes,
        cords,
        composites,
        composite_children,
        exports,
        bindings,
        group_ports: base.group_ports,
        pools: base.pools,
        source_map,
        semantic_hash,
    })
}

/// Lower current grammar supervision bindings into explicit semantic subjects and
/// ordinary handler nodes. Exact actions and resources remain planner inputs.
pub fn lower_supervision(
    graph: &ModuleGraph,
    catalog: &impl SourceContractCatalog,
) -> Result<LoweredSupervisedTopology, LoweringDiagnostic> {
    let topology = lower_topology(graph, catalog)?;
    let mut source_map = topology.source_map.clone();
    let mut supervisions = Vec::new();
    for module in &graph.modules {
        let uri = &module.canonical_uri;
        for binding in &module.panel.supervisions {
            let path = format!("{uri}/supervision/{}", binding.subject);
            let subject_path = format!("{uri}/node/{}", binding.subject);
            let handler_path = format!("{uri}/node/{}", binding.handler);
            let semantic_hash = hash_parts(
                "conduit/lowered-supervision",
                &[&path, &subject_path, &handler_path],
            );
            let binding_origin = origin(uri, &module.content_hash, binding.source_span);
            supervisions.push(LoweredSupervision {
                path: path.clone(),
                subject_path,
                handler_path,
                semantic_hash,
                origin: binding_origin.clone(),
            });
            source_map.push(SourceMapEntry {
                semantic_path: path,
                origins: vec![binding_origin],
            });
        }
        for definition in &module.panel.definitions {
            let composite_path = format!("{uri}/definition/{}", definition.id);
            for binding in &definition.supervisions {
                let path = format!("{composite_path}/supervision/{}", binding.subject);
                let subject_path = format!("{composite_path}/node/{}", binding.subject);
                let handler_path = format!("{composite_path}/node/{}", binding.handler);
                let semantic_hash = hash_parts(
                    "conduit/lowered-supervision",
                    &[&path, &subject_path, &handler_path],
                );
                let binding_origin = origin(uri, &module.content_hash, binding.source_span);
                supervisions.push(LoweredSupervision {
                    path: path.clone(),
                    subject_path,
                    handler_path,
                    semantic_hash,
                    origin: binding_origin.clone(),
                });
                source_map.push(SourceMapEntry {
                    semantic_path: path,
                    origins: vec![binding_origin],
                });
            }
        }
    }
    supervisions.sort_by(|left, right| left.path.cmp(&right.path));
    let mut facts = Vec::with_capacity(1 + supervisions.len());
    facts.push(topology.semantic_hash);
    facts.extend(supervisions.iter().map(|binding| binding.semantic_hash));
    facts.extend(graph.modules.iter().map(|module| {
        let source_hash = conduit_panel::semantic_source_hash(&module.panel);
        hash_parts(
            "conduit/source-ast-reference/supervision",
            &[&module.canonical_uri, &source_hash],
        )
    }));
    facts.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    let semantic_hash = hash_supervision_facts(&facts);
    Ok(LoweredSupervisedTopology {
        schema_version: LOWERED_SOURCE_SCHEMA_VERSION,
        source_ast_schema_version: SOURCE_AST_SCHEMA_VERSION,
        topology: Box::new(topology),
        supervisions,
        source_map,
        semantic_hash,
    })
}

/// Lowers current schema retaining interface satisfaction proofs.
pub fn lower_source(
    graph: &ModuleGraph,
    catalog: &impl SourceContractCatalog,
) -> Result<LoweredSource, LoweringDiagnostic> {
    let supervised = lower_supervision(graph, catalog)?;
    let mut source_map = supervised.source_map.clone();
    let mut interface_proofs = Vec::new();

    for module in &graph.modules {
        let uri = &module.canonical_uri;
        for node in &module.panel.nodes {
            for claim in &node.implements {
                let path = format!("{uri}/node/{}", node.id);
                let claim_origin = origin(uri, &module.content_hash, claim.source_span);
                let proof = prove_interface_claim(
                    path.clone(),
                    &claim.interface,
                    &node.kind,
                    None,
                    module,
                    graph,
                    catalog,
                    claim_origin.clone(),
                )?;
                interface_proofs.push(proof);
                source_map.push(SourceMapEntry {
                    semantic_path: path,
                    origins: vec![claim_origin],
                });
            }
        }
        for definition in &module.panel.definitions {
            for claim in &definition.implements {
                let path = format!("{uri}/definition/{}", definition.id);
                let claim_origin = origin(uri, &module.content_hash, claim.source_span);
                let proof = prove_interface_claim(
                    path.clone(),
                    &claim.interface,
                    &definition.id,
                    Some(definition),
                    module,
                    graph,
                    catalog,
                    claim_origin.clone(),
                )?;
                interface_proofs.push(proof);
                source_map.push(SourceMapEntry {
                    semantic_path: path,
                    origins: vec![claim_origin],
                });
            }
        }
    }

    interface_proofs.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.interface_id.cmp(&right.interface_id))
    });

    let mut facts = Vec::with_capacity(1 + interface_proofs.len());
    facts.push(supervised.semantic_hash);
    facts.extend(interface_proofs.iter().map(|proof| proof.semantic_hash));
    facts.extend(graph.modules.iter().map(|module| {
        let source_hash = conduit_panel::semantic_source_hash(&module.panel);
        hash_parts(
            "conduit/source-ast-reference",
            &[&module.canonical_uri, &source_hash],
        )
    }));
    facts.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    let semantic_hash = hash_source_facts(&facts);

    Ok(LoweredSource {
        schema_version: LOWERED_SOURCE_SCHEMA_VERSION,
        source_ast_schema_version: SOURCE_AST_SCHEMA_VERSION,
        supervised: Box::new(supervised),
        interface_proofs,
        source_map,
        semantic_hash,
    })
}

#[allow(clippy::too_many_arguments)]
fn prove_interface_claim(
    path: String,
    interface_id: &str,
    candidate_kind: &str,
    definition: Option<&conduit_panel::CompositeDefinition>,
    module: &conduit_panel::ResolvedModule,
    graph: &ModuleGraph,
    catalog: &impl SourceContractCatalog,
    claim_origin: SourceOrigin,
) -> Result<LoweredInterfaceProof, LoweringDiagnostic> {
    let interface =
        resolve_interface_contract(interface_id, module, graph, catalog, &path, &claim_origin)?;
    let candidate = if let Some(def) = definition {
        resolve_composite_candidate_contract(def, module, graph, catalog, &path, &claim_origin)?
    } else {
        resolve_primitive_candidate_contract(candidate_kind, catalog, &path, &claim_origin)?
    };

    let core_interface_id = Id::new(&interface.id).map_err(|_| {
        diagnostic(
            "CND-LWR-013",
            &path,
            None,
            Some(claim_origin.clone()),
            format!("invalid interface id `{}`", interface.id),
        )
    })?;
    let core_members = interface
        .members
        .iter()
        .map(|m| m.to_core())
        .collect::<Result<Vec<_>, IdError>>()
        .map_err(|_| {
            diagnostic(
                "CND-LWR-013",
                &path,
                None,
                Some(claim_origin.clone()),
                "invalid member port in interface contract",
            )
        })?;
    let core_interface_contract = NodeInterfaceContract {
        id: core_interface_id,
        principal_path: interface.principal_path.to_core().map_err(|_| {
            diagnostic(
                "CND-LWR-013",
                &path,
                None,
                Some(claim_origin.clone()),
                "invalid principal path in interface contract",
            )
        })?,
        members: &core_members,
        requirements: &[],
    };

    let core_candidate_id = Id::new(&candidate.id).map_err(|_| {
        diagnostic(
            "CND-LWR-013",
            &path,
            None,
            Some(claim_origin.clone()),
            format!("invalid candidate id `{}`", candidate.id),
        )
    })?;
    let core_inputs = candidate
        .inputs
        .iter()
        .map(|p| p.to_core())
        .collect::<Result<Vec<_>, IdError>>()
        .map_err(|_| {
            diagnostic(
                "CND-LWR-013",
                &path,
                None,
                Some(claim_origin.clone()),
                "invalid input port in candidate node contract",
            )
        })?;
    let core_outputs = candidate
        .outputs
        .iter()
        .map(|p| p.to_core())
        .collect::<Result<Vec<_>, IdError>>()
        .map_err(|_| {
            diagnostic(
                "CND-LWR-013",
                &path,
                None,
                Some(claim_origin.clone()),
                "invalid output port in candidate node contract",
            )
        })?;
    let core_candidate_contract = NodeContract {
        id: core_candidate_id,
        config: ConfigContract { fields: &[] },
        inputs: &core_inputs,
        outputs: &core_outputs,
    };

    let candidate_hash = candidate.semantic_hash();
    let interface_ref = NodeInterfaceContractRef {
        contract_id: core_interface_id,
        schema_version: interface.schema_version,
        semantic_hash: interface.semantic_hash,
    };
    let candidate_ref = DescriptorRef {
        kind: Id("conduit/node-contract"),
        schema_version: 0,
        semantic_hash: candidate_hash,
    };

    let mut member_scratch = vec![
        conduit_core::NodeInterfaceMemberProof {
            required: NodeInterfaceMember {
                requirement: InterfaceMemberRequirement::Required,
                port: PortContract {
                    id: Id(""),
                    direction: Direction::Input,
                    value_type: TypeContractRef {
                        contract_id: Id(""),
                        schema_version: 0,
                        semantic_hash: SemanticHash::from_bytes([0; 32]),
                    },
                    presence: Presence::Required,
                    connections: ConnectionCardinality::ZeroOrMore,
                    values: ValueCardinality::ExactlyOne,
                    delivery: Delivery::Stream,
                    temporal: TemporalContract::Committed,
                    terminal: TerminalContract::Either,
                    sensitivity: Sensitivity::Public,
                    flow: PortFlowConstraints {
                        loss: LossAcceptance::LosslessOnly,
                    },
                },
            },
            offered: None,
            type_decision: None,
            port_decision: None,
            outcome: CompatibilityOutcome::Incompatible,
            reason: conduit_core::NodeInterfaceMemberReason::MissingRequired,
        };
        interface.members.len()
    ];
    let mut req_scratch = Vec::new();
    let fact_count = interface.members.len();
    let mut hash_scratch1 = vec![SemanticHash::from_bytes([0; 32]); fact_count];
    let mut hash_scratch2 = vec![SemanticHash::from_bytes([0; 32]); fact_count];

    let proof_result = assess_node_interface(
        &core_interface_contract,
        interface_ref,
        candidate_ref,
        &core_candidate_contract,
        &[],
        &[],
        &mut member_scratch,
        &mut req_scratch,
        &mut hash_scratch1,
        &mut hash_scratch2,
    );

    let proof = match proof_result {
        Ok(p) => p,
        Err(err) => {
            return Err(diagnostic(
                "CND-LWR-013",
                &path,
                None,
                Some(claim_origin),
                format!("node interface assessment error: {}", err.as_str()),
            ));
        }
    };

    if proof.outcome != CompatibilityOutcome::Compatible {
        return Err(diagnostic(
            "CND-LWR-013",
            &path,
            None,
            Some(claim_origin),
            format!(
                "interface contract `{interface_id}` satisfaction failed: {}",
                proof.reason.as_str()
            ),
        ));
    }

    let member_proofs = proof
        .members
        .iter()
        .map(|m| LoweredInterfaceMemberProof {
            member_id: m.required.port.id.as_str().to_owned(),
            direction: match m.required.port.direction {
                Direction::Input => "input".to_owned(),
                Direction::Output => "output".to_owned(),
            },
            requirement: match m.required.requirement {
                InterfaceMemberRequirement::Required => "required".to_owned(),
                InterfaceMemberRequirement::Optional => "optional".to_owned(),
            },
            outcome: match m.outcome {
                CompatibilityOutcome::Compatible => "compatible".to_owned(),
                CompatibilityOutcome::Incompatible => "incompatible".to_owned(),
                CompatibilityOutcome::Indeterminate => "indeterminate".to_owned(),
            },
            reason: m.reason.as_str().to_owned(),
        })
        .collect();

    Ok(LoweredInterfaceProof {
        path,
        interface_id: interface_id.to_owned(),
        interface_hash: interface.semantic_hash,
        candidate_hash,
        outcome: "compatible".to_owned(),
        reason: proof.reason.as_str().to_owned(),
        member_proofs,
        semantic_hash: proof.identity,
        origin: claim_origin,
    })
}

fn resolve_interface_contract(
    interface_id: &str,
    module: &conduit_panel::ResolvedModule,
    graph: &ModuleGraph,
    catalog: &impl SourceContractCatalog,
    path: &str,
    origin: &SourceOrigin,
) -> Result<OwnedInterfaceContract, LoweringDiagnostic> {
    if let Some((alias, member_name)) = interface_id.split_once('.') {
        if let Some(import) = module.panel.imports.iter().find(|imp| imp.alias == alias) {
            if let Some(imported_mod) = graph
                .modules
                .iter()
                .find(|m| m.canonical_uri == import.target)
            {
                if let Some(decl) = imported_mod
                    .panel
                    .interfaces
                    .iter()
                    .find(|iface| iface.id == member_name)
                {
                    return interface_decl_to_owned(decl, catalog);
                }
            }
        }
    } else if let Some(decl) = module
        .panel
        .interfaces
        .iter()
        .find(|iface| iface.id == interface_id)
    {
        return interface_decl_to_owned(decl, catalog);
    }

    if let Some(contract) = catalog.interface_contract(interface_id) {
        return Ok(contract);
    }

    Err(diagnostic(
        "CND-LWR-013",
        path,
        None,
        Some(origin.clone()),
        format!("interface contract `{interface_id}` unavailable in catalog or module graph"),
    ))
}

fn resolve_primitive_candidate_contract(
    kind: &str,
    catalog: &impl SourceContractCatalog,
    path: &str,
    origin: &SourceOrigin,
) -> Result<OwnedNodeContract, LoweringDiagnostic> {
    if let Some(contract) = catalog.node_contract(kind) {
        return Ok(contract);
    }

    if let Some(schema) = catalog.node_schema(kind) {
        return Ok(OwnedNodeContract {
            id: schema.id,
            inputs: Vec::new(),
            outputs: Vec::new(),
        });
    }

    Err(diagnostic(
        "CND-LWR-013",
        path,
        None,
        Some(origin.clone()),
        format!("candidate node contract `{kind}` unavailable in catalog"),
    ))
}

#[allow(clippy::only_used_in_recursion)]
fn resolve_composite_candidate_contract(
    definition: &conduit_panel::CompositeDefinition,
    module: &conduit_panel::ResolvedModule,
    graph: &ModuleGraph,
    catalog: &impl SourceContractCatalog,
    path: &str,
    origin: &SourceOrigin,
) -> Result<OwnedNodeContract, LoweringDiagnostic> {
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();

    for export in &definition.exports {
        let child = definition
            .nodes
            .iter()
            .find(|n| n.id == export.target.node)
            .ok_or_else(|| {
                diagnostic(
                    "CND-LWR-013",
                    path,
                    None,
                    Some(origin.clone()),
                    format!(
                        "export `{}` targets unknown child node `{}`",
                        export.id, export.target.node
                    ),
                )
            })?;

        let child_contract = if let Some(child_def) =
            module.panel.definitions.iter().find(|d| d.id == child.kind)
        {
            resolve_composite_candidate_contract(child_def, module, graph, catalog, path, origin)?
        } else {
            resolve_primitive_candidate_contract(&child.kind, catalog, path, origin)?
        };

        let matching_port = match export.direction {
            conduit_panel::ExportDirection::Input => child_contract
                .inputs
                .iter()
                .find(|p| p.id == export.target.port),
            conduit_panel::ExportDirection::Output => child_contract
                .outputs
                .iter()
                .find(|p| p.id == export.target.port),
        }
        .ok_or_else(|| {
            diagnostic(
                "CND-LWR-013",
                path,
                None,
                Some(origin.clone()),
                format!(
                    "export `{}` targets unknown child port `{}.{}`",
                    export.id, export.target.node, export.target.port
                ),
            )
        })?;

        let mut exported_port = matching_port.clone();
        exported_port.id = export.id.clone();
        match export.direction {
            conduit_panel::ExportDirection::Input => inputs.push(exported_port),
            conduit_panel::ExportDirection::Output => outputs.push(exported_port),
        }
    }

    Ok(OwnedNodeContract {
        id: definition.id.clone(),
        inputs,
        outputs,
    })
}

fn interface_decl_to_owned(
    decl: &conduit_panel::InterfaceDeclaration,
    catalog: &impl SourceContractCatalog,
) -> Result<OwnedInterfaceContract, LoweringDiagnostic> {
    let mut members = Vec::new();
    for m in &decl.members {
        let direction = match m.direction {
            conduit_panel::ExportDirection::Input => Direction::Input,
            conduit_panel::ExportDirection::Output => Direction::Output,
        };
        let value_type = catalog.type_reference(&m.port_contract).ok_or_else(|| {
            diagnostic(
                "CND-LWR-013",
                &decl.id,
                None,
                None,
                format!(
                    "type contract `{}` for interface member `{}` unavailable in catalog",
                    m.port_contract, m.id
                ),
            )
        })?;
        let requirement = if m.optional {
            InterfaceMemberRequirement::Optional
        } else {
            InterfaceMemberRequirement::Required
        };
        members.push(OwnedInterfaceMember {
            requirement,
            id: m.id.clone(),
            direction,
            value_type,
            presence: Presence::Required,
            connections: ConnectionCardinality::ZeroOrMore,
            values: ValueCardinality::ExactlyOne,
            delivery: Delivery::Stream,
            temporal: TemporalContract::Committed,
            terminal: TerminalContract::Either,
            sensitivity: Sensitivity::Public,
            loss: LossAcceptance::LosslessOnly,
        });
    }

    let mut owned = OwnedInterfaceContract {
        id: decl.id.clone(),
        schema_version: 0,
        principal_path: OwnedPrincipalPath::none(),
        members,
        semantic_hash: SemanticHash::from_bytes([0; 32]),
    };
    owned.semantic_hash = owned.compute_semantic_hash().map_err(|_| {
        diagnostic(
            "CND-LWR-013",
            &decl.id,
            None,
            None,
            format!("invalid interface identifier `{}`", decl.id),
        )
    })?;
    Ok(owned)
}

fn hash_source_facts(facts: &[SemanticHash]) -> SemanticHash {
    let mut digest = Sha256::new();
    digest.update(b"conduit.lowered-source\0");
    for fact in facts {
        digest.update(fact.as_bytes());
    }
    SemanticHash::from_bytes(digest.finalize().into())
}

fn lower_root_selection(
    graph: &ModuleGraph,
) -> Result<Option<LoweredRootSelection>, LoweringDiagnostic> {
    let Some(selection) = &graph.root_selection else {
        return Ok(None);
    };
    let module = graph
        .modules
        .iter()
        .find(|module| module.canonical_uri == graph.entry_uri)
        .ok_or_else(|| {
            diagnostic(
                "CND-LWR-012",
                &graph.entry_uri,
                None,
                None,
                "entry module is absent from the resolved graph",
            )
        })?;
    let declared = module
        .panel
        .roots
        .iter()
        .find(|root| root.target == selection.target)
        .ok_or_else(|| {
            diagnostic(
                "CND-LWR-012",
                &graph.entry_uri,
                None,
                None,
                "resolved root is absent from the authored entry module",
            )
        })?;
    let authored_source_hash = conduit_panel::semantic_source_hash(&module.panel);
    let semantic_hash = hash_parts(
        "conduit/lowered-root-selection",
        &[&graph.entry_uri, &selection.target, &authored_source_hash],
    );
    Ok(Some(LoweredRootSelection {
        entry_uri: graph.entry_uri.clone(),
        target: selection.target.clone(),
        mode: selection.mode,
        authored_source_hash,
        semantic_hash,
        origin: origin(
            &module.canonical_uri,
            &module.content_hash,
            declared.source_span,
        ),
    }))
}

fn lower_cord(
    cord: &conduit_panel::Cord,
    scope: &str,
    uri: &str,
    module_hash: &str,
) -> LoweredCord {
    let path = format!("{scope}/cord/{}", cord.id);
    let from = format!("{scope}/node/{}/port/{}", cord.from.node, cord.from.port);
    let to = format!("{scope}/node/{}/port/{}", cord.to.node, cord.to.port);
    let pressure = pressure_identity(&cord.pressure);
    let numeric = [
        cord.capacity_items.to_string(),
        cord.max_value_bytes.to_string(),
        cord.max_queued_bytes.to_string(),
        cord.low_watermark_items.to_string(),
        cord.high_watermark_items.to_string(),
    ];
    let semantic_hash = hash_parts(
        "conduit/lowered-cord",
        &[
            &path,
            &from,
            &to,
            &numeric[0],
            &numeric[1],
            &numeric[2],
            &numeric[3],
            &numeric[4],
            &pressure,
        ],
    );
    LoweredCord {
        path,
        from,
        to,
        capacity_items: cord.capacity_items,
        max_value_bytes: cord.max_value_bytes,
        max_queued_bytes: cord.max_queued_bytes,
        low_watermark_items: cord.low_watermark_items,
        high_watermark_items: cord.high_watermark_items,
        pressure: cord.pressure.clone(),
        semantic_hash,
        origin: origin(uri, module_hash, cord.source_span),
    }
}

fn pressure_identity(pressure: &SourcePressure) -> String {
    match pressure {
        SourcePressure::Block => "block".to_owned(),
        SourcePressure::Reject => "reject".to_owned(),
        SourcePressure::Coalesce { relation } => format!("coalesce/{relation}"),
        SourcePressure::Sample { every, offset } => format!("sample/{every}/{offset}"),
        SourcePressure::DropDisposable => "drop-disposable".to_owned(),
        SourcePressure::Disconnect => "disconnect".to_owned(),
        SourcePressure::Fail => "fail".to_owned(),
    }
}

fn definition_schema(
    definition: &conduit_panel::CompositeDefinition,
    catalog: &impl SourceContractCatalog,
    module: &conduit_panel::ResolvedModule,
    source_identity: fn(&Panel) -> String,
) -> Result<OwnedNodeSchema, LoweringDiagnostic> {
    let mut fields = Vec::new();
    for parameter in &definition.parameters {
        let value_type = catalog
            .type_reference(&parameter.value_type)
            .ok_or_else(|| {
                diagnostic(
                    "CND-LWR-001",
                    format!(
                        "{}/definition/{}/{}",
                        module.canonical_uri, definition.id, parameter.id
                    ),
                    None,
                    Some(origin(
                        &module.canonical_uri,
                        &module.content_hash,
                        parameter.source_span,
                    )),
                    format!("type contract `{}` is unavailable", parameter.value_type),
                )
            })?;
        let requirement = match &parameter.default {
            Some(default) => {
                let default_origin = parameter
                    .default_span
                    .map(|span| origin(&module.canonical_uri, &module.content_hash, span));
                let value = catalog
                    .validate_literal(&value_type, default)
                    .map_err(|_| {
                        diagnostic(
                            "CND-LWR-007",
                            format!(
                                "{}/definition/{}/{}",
                                module.canonical_uri, definition.id, parameter.id
                            ),
                            Some(value_type.clone()),
                            default_origin.clone(),
                            "authored schema default is invalid",
                        )
                    })?;
                catalog.validate_default(&value_type, &value).map_err(|_| {
                    diagnostic(
                        "CND-LWR-007",
                        format!(
                            "{}/definition/{}/{}",
                            module.canonical_uri, definition.id, parameter.id
                        ),
                        Some(value_type.clone()),
                        default_origin.clone(),
                        "schema default is invalid",
                    )
                })?;
                OwnedConfigRequirement::Defaulted(value)
            }
            None => OwnedConfigRequirement::Required,
        };
        fields.push(OwnedConfigFieldSchema {
            key: parameter.id.clone(),
            value_type,
            requirement,
            sensitivity: Sensitivity::Public,
            mutability: ConfigMutability::PreStart,
            identity: ConfigIdentity::Semantic,
            default_origin: parameter
                .default_span
                .map(|span| origin(&module.canonical_uri, &module.content_hash, span)),
        });
    }
    Ok(OwnedNodeSchema {
        id: format!("{}#{}", source_identity(&module.panel), definition.id),
        fields,
    })
}

#[allow(clippy::too_many_arguments)]
fn lower_panel(
    module: &conduit_panel::ResolvedModule,
    catalog: &impl SourceContractCatalog,
    definitions: &BTreeMap<(String, String), OwnedNodeSchema>,
    nodes: &mut Vec<LoweredNode>,
    group_ports: &mut Vec<LoweredGroupPort>,
    pools: &mut Vec<LoweredPool>,
    source_map: &mut Vec<SourceMapEntry>,
) -> Result<(), LoweringDiagnostic> {
    let panel: &Panel = &module.panel;
    let uri = &module.canonical_uri;
    let module_hash = &module.content_hash;
    for node in &panel.nodes {
        lower_node(
            node,
            &format!("{uri}/node/{}", node.id),
            uri,
            module_hash,
            module,
            catalog,
            definitions,
            nodes,
            source_map,
        )?;
    }
    for definition in &panel.definitions {
        for node in &definition.nodes {
            lower_node(
                node,
                &format!("{uri}/definition/{}/node/{}", definition.id, node.id),
                uri,
                module_hash,
                module,
                catalog,
                definitions,
                nodes,
                source_map,
            )?;
        }
        for group in &definition.port_groups {
            lower_group(
                group,
                &format!("{uri}/definition/{}/group/{}", definition.id, group.id),
                uri,
                module_hash,
                catalog,
                group_ports,
                source_map,
            )?;
        }
        for pool in &definition.pools {
            lower_pool(
                pool,
                &format!("{uri}/definition/{}/pool/{}", definition.id, pool.id),
                uri,
                module_hash,
                module,
                catalog,
                definitions,
                pools,
                source_map,
            )?;
        }
    }
    for group in &panel.port_groups {
        lower_group(
            group,
            &format!("{uri}/group/{}", group.id),
            uri,
            module_hash,
            catalog,
            group_ports,
            source_map,
        )?;
    }
    for pool in &panel.pools {
        lower_pool(
            pool,
            &format!("{uri}/pool/{}", pool.id),
            uri,
            module_hash,
            module,
            catalog,
            definitions,
            pools,
            source_map,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn lower_node(
    node: &conduit_panel::Node,
    path: &str,
    uri: &str,
    module_hash: &str,
    module: &conduit_panel::ResolvedModule,
    catalog: &impl SourceContractCatalog,
    definitions: &BTreeMap<(String, String), OwnedNodeSchema>,
    nodes: &mut Vec<LoweredNode>,
    source_map: &mut Vec<SourceMapEntry>,
) -> Result<(), LoweringDiagnostic> {
    let schema =
        resolve_node_schema(&node.kind, module, catalog, definitions).ok_or_else(|| {
            diagnostic(
                "CND-LWR-001",
                path,
                None,
                Some(origin(uri, module_hash, node.source_span)),
                format!("semantic node schema `{}` is unavailable", node.kind),
            )
        })?;
    let config = lower_config(node, &schema, path, uri, module_hash, catalog)?;
    let contract_hash = schema.semantic_hash();
    let config_hash = hash_config(&schema.id, &config);
    let semantic_hash = hash_node(path, &schema.id, contract_hash, config_hash);
    source_map.push(SourceMapEntry {
        semantic_path: path.to_owned(),
        origins: vec![origin(uri, module_hash, node.source_span)],
    });
    for entry in &config {
        source_map.push(SourceMapEntry {
            semantic_path: format!("{path}/config/{}", entry.field.key),
            origins: vec![
                entry
                    .origin
                    .clone()
                    .unwrap_or_else(|| origin(uri, module_hash, node.source_span)),
            ],
        });
    }
    nodes.push(LoweredNode {
        path: path.to_owned(),
        contract_id: schema.id,
        contract_hash,
        config,
        semantic_hash,
    });
    Ok(())
}

fn resolve_node_schema(
    id: &str,
    module: &conduit_panel::ResolvedModule,
    catalog: &impl SourceContractCatalog,
    definitions: &BTreeMap<(String, String), OwnedNodeSchema>,
) -> Option<OwnedNodeSchema> {
    let qualified_schema = id.split_once('.').and_then(|(alias, symbol)| {
        module
            .imports
            .iter()
            .find(|import| import.alias == alias)
            .and_then(|import| {
                definitions
                    .get(&(import.canonical_uri.clone(), symbol.to_owned()))
                    .cloned()
            })
    });
    catalog
        .node_schema(id)
        .or_else(|| {
            definitions
                .get(&(module.canonical_uri.clone(), id.to_owned()))
                .cloned()
        })
        .or(qualified_schema)
}

fn lower_config(
    node: &conduit_panel::Node,
    schema: &OwnedNodeSchema,
    path: &str,
    uri: &str,
    module_hash: &str,
    catalog: &impl SourceContractCatalog,
) -> Result<Vec<LoweredConfigEntry>, LoweringDiagnostic> {
    for entry in &node.config {
        if !schema.fields.iter().any(|field| field.key == entry.key) {
            return Err(diagnostic(
                "CND-LWR-002",
                format!("{path}/config/{}", entry.key),
                None,
                Some(origin(uri, module_hash, entry.source_span)),
                format!("unknown configuration field `{}`", entry.key),
            ));
        }
    }
    let mut lowered = Vec::new();
    for field in &schema.fields {
        let assignment = node.config.iter().find(|entry| entry.key == field.key);
        let (value, provenance, value_origin) = match assignment {
            Some(entry) => {
                let value_origin = Some(origin(uri, module_hash, entry.source_span));
                if let SourceValue::SecretReference(reference) = &entry.value {
                    if field.sensitivity == Sensitivity::Public
                        || field.identity != ConfigIdentity::Plan
                    {
                        return Err(diagnostic(
                            "CND-LWR-009",
                            format!("{path}/config/{}", field.key),
                            Some(field.value_type.clone()),
                            value_origin,
                            "secret reference is not permitted by this field contract",
                        ));
                    }
                    (
                        LoweredConfigValue::SecretReference(reference.clone()),
                        ConfigProvenance::PlanBinding,
                        value_origin,
                    )
                } else if field.sensitivity != Sensitivity::Public {
                    return Err(diagnostic(
                        "CND-LWR-009",
                        format!("{path}/config/{}", field.key),
                        Some(field.value_type.clone()),
                        value_origin,
                        "protected source configuration requires an unresolved secret reference",
                    ));
                } else if contains_secret_reference(&entry.value) {
                    return Err(diagnostic(
                        "CND-LWR-009",
                        format!("{path}/config/{}", field.key),
                        Some(field.value_type.clone()),
                        value_origin,
                        "nested secret references are not a public semantic value",
                    ));
                } else {
                    let value = catalog
                        .validate_literal(&field.value_type, &entry.value)
                        .map_err(|error| {
                            literal_diagnostic(
                                error,
                                format!("{path}/config/{}", field.key),
                                field.value_type.clone(),
                                value_origin.clone(),
                            )
                        })?;
                    (
                        LoweredConfigValue::Public(value),
                        ConfigProvenance::Authored,
                        value_origin,
                    )
                }
            }
            None => match &field.requirement {
                OwnedConfigRequirement::Required => {
                    return Err(diagnostic(
                        "CND-LWR-004",
                        format!("{path}/config/{}", field.key),
                        Some(field.value_type.clone()),
                        None,
                        "required configuration field is absent",
                    ));
                }
                OwnedConfigRequirement::Optional => continue,
                OwnedConfigRequirement::Defaulted(default) => {
                    catalog
                        .validate_default(&field.value_type, default)
                        .map_err(|_| {
                            diagnostic(
                                "CND-LWR-007",
                                format!("{path}/config/{}", field.key),
                                Some(field.value_type.clone()),
                                None,
                                "schema default is invalid",
                            )
                        })?;
                    (
                        LoweredConfigValue::Public(default.clone()),
                        ConfigProvenance::SchemaDefault,
                        field.default_origin.clone(),
                    )
                }
            },
        };
        lowered.push(LoweredConfigEntry {
            field: field.clone(),
            value,
            provenance,
            origin: value_origin,
        });
    }
    Ok(lowered)
}

fn contains_secret_reference(value: &SourceValue) -> bool {
    match value {
        SourceValue::SecretReference(_) => true,
        SourceValue::List(values) => values.iter().any(contains_secret_reference),
        SourceValue::Record(fields) => fields
            .iter()
            .any(|(_, value)| contains_secret_reference(value)),
        SourceValue::Boolean(_)
        | SourceValue::Integer(_)
        | SourceValue::Text(_)
        | SourceValue::Bytes(_)
        | SourceValue::Reference(_)
        | SourceValue::ContractReference(_)
        | SourceValue::ExactDecimal(_) => false,
    }
}

fn lower_group(
    group: &PortGroup,
    path: &str,
    uri: &str,
    module_hash: &str,
    catalog: &impl SourceContractCatalog,
    output: &mut Vec<LoweredGroupPort>,
    source_map: &mut Vec<SourceMapEntry>,
) -> Result<(), LoweringDiagnostic> {
    let port_contract = catalog.port_contract(&group.port_contract).ok_or_else(|| {
        diagnostic(
            "CND-LWR-001",
            path,
            None,
            Some(origin(uri, module_hash, group.source_span)),
            format!(
                "semantic port contract `{}` is unavailable",
                group.port_contract
            ),
        )
    })?;
    let authored_direction = match group.direction {
        conduit_panel::ExportDirection::Input => conduit_core::Direction::Input,
        conduit_panel::ExportDirection::Output => conduit_core::Direction::Output,
    };
    if port_contract.direction != authored_direction {
        return Err(diagnostic(
            "CND-LWR-010",
            path,
            None,
            Some(origin(uri, module_hash, group.source_span)),
            format!(
                "port-group direction `{}` does not match complete port contract `{}` direction `{}`",
                authored_direction.as_str(),
                port_contract.id,
                port_contract.direction.as_str()
            ),
        ));
    }
    let members: Vec<(String, Option<SourceSpan>)> = match &group.shape {
        PortGroupShape::Keyed(members) => members
            .iter()
            .map(|member| (member.key.clone(), Some(member.source_span)))
            .collect(),
        PortGroupShape::Indexed => (0..group.maximum)
            .map(|index| (index.to_string(), None))
            .collect(),
    };
    let group_origin = origin(uri, module_hash, group.source_span);
    for (ordinal, (member, member_span)) in members.into_iter().enumerate() {
        let id = format!("{}[{member}]", group.id);
        let member_origin = member_span.map(|span| origin(uri, module_hash, span));
        let origin = member_origin
            .clone()
            .unwrap_or_else(|| group_origin.clone());
        let semantic_path = format!("{path}/member/{member}");
        let direction = match group.direction {
            conduit_panel::ExportDirection::Input => "input",
            conduit_panel::ExportDirection::Output => "output",
        };
        let hash = hash_parts(
            "conduit/lowered-group-port",
            &[
                path,
                &id,
                &ordinal.to_string(),
                direction,
                &port_contract.id,
                &port_contract.semantic_hash.to_string(),
                &group.maximum.to_string(),
            ],
        );
        output.push(LoweredGroupPort {
            group_id: group.id.clone(),
            member,
            ordinal: u16::try_from(ordinal).expect("group maximum is u16"),
            id,
            logical_group_path: path.to_owned(),
            expanded_port_path: semantic_path.clone(),
            direction: group.direction,
            group_maximum: group.maximum,
            port_contract: port_contract.clone(),
            semantic_hash: hash,
            group_origin: group_origin.clone(),
            member_origin,
            origin: origin.clone(),
        });
        source_map.push(SourceMapEntry {
            semantic_path,
            origins: vec![origin],
        });
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn lower_pool(
    pool: &InstancePool,
    path: &str,
    uri: &str,
    module_hash: &str,
    module: &conduit_panel::ResolvedModule,
    catalog: &impl SourceContractCatalog,
    definitions: &BTreeMap<(String, String), OwnedNodeSchema>,
    output: &mut Vec<LoweredPool>,
    source_map: &mut Vec<SourceMapEntry>,
) -> Result<(), LoweringDiagnostic> {
    let template =
        resolve_node_schema(&pool.template, module, catalog, definitions).ok_or_else(|| {
            diagnostic(
                "CND-LWR-001",
                path,
                None,
                Some(origin(uri, module_hash, pool.source_span)),
                format!("pool template contract `{}` is unavailable", pool.template),
            )
        })?;
    let template_contract_hash = template.semantic_hash();
    let mut parts = vec![
        path.to_owned(),
        template.id.clone(),
        template_contract_hash.to_string(),
        pool.maximum.to_string(),
        pool.deadline_ms.to_string(),
        pool.idle_timeout_ms.to_string(),
    ];
    match pool.admission {
        PoolAdmission::Reject => parts.push("admission/reject".to_owned()),
        PoolAdmission::Block => parts.push("admission/block".to_owned()),
        PoolAdmission::QueueBounded(capacity) => {
            parts.push("admission/queue-bounded".to_owned());
            parts.push(capacity.to_string());
        }
        PoolAdmission::Fail => parts.push("admission/fail".to_owned()),
    }
    match &pool.supervision {
        PoolSupervision::FailTogether => parts.push("supervision/fail-together".to_owned()),
        PoolSupervision::Isolate => parts.push("supervision/isolate".to_owned()),
        PoolSupervision::RestartBounded {
            attempts,
            backoff_ms,
        } => {
            parts.push("supervision/restart-bounded".to_owned());
            parts.push(attempts.to_string());
            parts.push(backoff_ms.to_string());
        }
        PoolSupervision::Fallback(target) => {
            parts.push("supervision/fallback".to_owned());
            parts.push(target.clone());
        }
        PoolSupervision::Escalate => parts.push("supervision/escalate".to_owned()),
    }
    parts.push(
        match pool.cleanup {
            PoolCleanup::Drain => "cleanup/drain",
            PoolCleanup::Abort => "cleanup/abort",
        }
        .to_owned(),
    );
    let part_refs: Vec<&str> = parts.iter().map(String::as_str).collect();
    let hash = hash_parts("conduit/lowered-pool", &part_refs);
    let origin = origin(uri, module_hash, pool.source_span);
    output.push(LoweredPool {
        path: path.to_owned(),
        template_contract_id: template.id,
        template_contract_hash,
        maximum: pool.maximum,
        admission: pool.admission.clone(),
        deadline_ms: pool.deadline_ms,
        idle_timeout_ms: pool.idle_timeout_ms,
        supervision: pool.supervision.clone(),
        cleanup: pool.cleanup,
        semantic_hash: hash,
        origin: origin.clone(),
    });
    source_map.push(SourceMapEntry {
        semantic_path: path.to_owned(),
        origins: vec![origin],
    });
    Ok(())
}

fn origin(uri: &str, module_hash: &str, span: SourceSpan) -> SourceOrigin {
    SourceOrigin {
        module_uri: uri.to_owned(),
        module_hash: module_hash.to_owned(),
        span,
    }
}

fn literal_diagnostic(
    error: LiteralValidationError,
    path: String,
    expected: OwnedTypeReference,
    origin: Option<SourceOrigin>,
) -> LoweringDiagnostic {
    let (code, message) = match error {
        LiteralValidationError::WrongKind => ("CND-LWR-005", "literal kind does not match"),
        LiteralValidationError::Overflow => ("CND-LWR-006", "integer is outside contract bounds"),
        LiteralValidationError::InvalidValue => ("CND-LWR-005", "literal value is invalid"),
        LiteralValidationError::ProviderUnavailable => {
            ("CND-LWR-008", "required type provider is unavailable")
        }
    };
    diagnostic(code, path, Some(expected), origin, message)
}

fn diagnostic(
    code: &'static str,
    path: impl Into<String>,
    expected_contract: Option<OwnedTypeReference>,
    origin: Option<SourceOrigin>,
    message: impl Into<String>,
) -> LoweringDiagnostic {
    LoweringDiagnostic {
        code,
        semantic_path: path.into(),
        expected_contract: expected_contract.map(Box::new),
        origin: origin.map(Box::new),
        message: message.into(),
    }
}

fn hash_config(contract_id: &str, config: &[LoweredConfigEntry]) -> SemanticHash {
    let mut fields = Vec::new();
    for entry in config {
        if entry.field.identity != ConfigIdentity::Semantic {
            continue;
        }
        let LoweredConfigValue::Public(value) = &entry.value else {
            continue;
        };
        if matches!(
            &entry.field.requirement,
            OwnedConfigRequirement::Defaulted(default) if default == value
        ) {
            continue;
        }
        fields.push((entry.field.key.as_str(), value));
    }
    fields.sort_by(|left, right| left.0.cmp(right.0));
    let mut bytes = Vec::new();
    write_text(contract_id, &mut bytes);
    for (key, value) in fields {
        write_text(key, &mut bytes);
        write_value(value, &mut bytes);
    }
    SemanticHash::from_bytes(
        Sha256::digest([b"conduit.lowered-config\0".as_slice(), &bytes].concat()).into(),
    )
}

fn hash_node_schema(schema: &OwnedNodeSchema) -> SemanticHash {
    let mut fields: Vec<_> = schema.fields.iter().collect();
    fields.sort_by(|left, right| left.key.cmp(&right.key));
    let mut bytes = Vec::new();
    write_text(&schema.id, &mut bytes);
    for field in fields {
        write_text(&field.key, &mut bytes);
        write_text(&field.value_type.id, &mut bytes);
        bytes.extend_from_slice(&field.value_type.schema_version.to_be_bytes());
        bytes.extend_from_slice(field.value_type.semantic_hash.as_bytes());
        match &field.requirement {
            OwnedConfigRequirement::Required => bytes.push(0),
            OwnedConfigRequirement::Optional => bytes.push(1),
            OwnedConfigRequirement::Defaulted(value) => {
                bytes.push(2);
                write_value(value, &mut bytes);
            }
        }
        write_text(field.sensitivity.as_str(), &mut bytes);
        write_text(field.mutability.as_str(), &mut bytes);
        write_text(field.identity.as_str(), &mut bytes);
    }
    SemanticHash::from_bytes(
        Sha256::digest([b"conduit.source-node-schema\0".as_slice(), bytes.as_slice()].concat())
            .into(),
    )
}

fn write_value(value: &OwnedSemanticValue, output: &mut Vec<u8>) {
    match value {
        OwnedSemanticValue::Null => output.push(0),
        OwnedSemanticValue::Boolean(value) => output.push(if *value { 2 } else { 1 }),
        OwnedSemanticValue::Integer(value) => {
            output.push(0x10);
            output.extend_from_slice(&value.to_be_bytes());
        }
        OwnedSemanticValue::Bytes(value) => {
            output.push(0x20);
            output.extend_from_slice(&(value.len() as u64).to_be_bytes());
            output.extend_from_slice(value);
        }
        OwnedSemanticValue::Text(value) => {
            output.push(0x21);
            write_text(value, output);
        }
        OwnedSemanticValue::Identifier(value) => {
            output.push(0x22);
            write_text(value, output);
        }
        OwnedSemanticValue::List(values) => {
            output.push(0x30);
            output.extend_from_slice(&(values.len() as u64).to_be_bytes());
            for value in values {
                write_value(value, output);
            }
        }
        OwnedSemanticValue::Map(fields) => {
            output.push(0x31);
            let mut fields: Vec<_> = fields.iter().collect();
            fields.sort_by(|left, right| left.0.cmp(&right.0));
            output.extend_from_slice(&(fields.len() as u64).to_be_bytes());
            for (key, value) in fields {
                write_text(key, output);
                write_value(value, output);
            }
        }
        OwnedSemanticValue::Set(values) => {
            output.push(0x32);
            let mut encoded: Vec<Vec<u8>> = values
                .iter()
                .map(|value| {
                    let mut bytes = Vec::new();
                    write_value(value, &mut bytes);
                    bytes
                })
                .collect();
            encoded.sort();
            output.extend_from_slice(&(encoded.len() as u64).to_be_bytes());
            for value in encoded {
                output.extend_from_slice(&value);
            }
        }
    }
}

fn write_text(value: &str, output: &mut Vec<u8>) {
    output.extend_from_slice(&(value.len() as u64).to_be_bytes());
    output.extend_from_slice(value.as_bytes());
}

fn hash_parts(domain: &str, parts: &[&str]) -> SemanticHash {
    let mut digest = Sha256::new();
    digest.update(domain.as_bytes());
    digest.update([0]);
    for part in parts {
        digest.update((part.len() as u64).to_be_bytes());
        digest.update(part.as_bytes());
    }
    SemanticHash::from_bytes(digest.finalize().into())
}

fn hash_facts(kind: &str, facts: &[SemanticHash]) -> SemanticHash {
    let mut digest = Sha256::new();
    digest.update(b"conduit.lowered-source\0");
    digest.update(kind.as_bytes());
    for fact in facts {
        digest.update(fact.as_bytes());
    }
    SemanticHash::from_bytes(digest.finalize().into())
}

fn hash_topology_facts(facts: &[SemanticHash]) -> SemanticHash {
    let mut digest = Sha256::new();
    digest.update(b"conduit.lowered-source\0");
    for fact in facts {
        digest.update(fact.as_bytes());
    }
    SemanticHash::from_bytes(digest.finalize().into())
}

fn hash_supervision_facts(facts: &[SemanticHash]) -> SemanticHash {
    let mut digest = Sha256::new();
    digest.update(b"conduit.lowered-source/supervised\0");
    for fact in facts {
        digest.update(fact.as_bytes());
    }
    SemanticHash::from_bytes(digest.finalize().into())
}

fn hash_node(
    path: &str,
    contract: &str,
    contract_hash: SemanticHash,
    config: SemanticHash,
) -> SemanticHash {
    let mut digest = Sha256::new();
    digest.update(b"conduit.lowered-node\0");
    digest.update((path.len() as u64).to_be_bytes());
    digest.update(path.as_bytes());
    digest.update((contract.len() as u64).to_be_bytes());
    digest.update(contract.as_bytes());
    digest.update(contract_hash.as_bytes());
    digest.update(config.as_bytes());
    SemanticHash::from_bytes(digest.finalize().into())
}

fn hash_authored_node(
    path: &str,
    contract: &str,
    contract_hash: SemanticHash,
    unresolved_constraint: Option<&str>,
    config: SemanticHash,
) -> SemanticHash {
    let constraint = unresolved_constraint.unwrap_or("");
    let constraint_presence = if unresolved_constraint.is_some() {
        "present"
    } else {
        "absent"
    };
    let contract_hash = contract_hash.to_string();
    let config = config.to_string();
    hash_parts(
        "conduit/lowered-node",
        &[
            path,
            contract,
            &contract_hash,
            constraint_presence,
            constraint,
            &config,
        ],
    )
}
