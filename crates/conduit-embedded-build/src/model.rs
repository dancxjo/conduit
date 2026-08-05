use std::error::Error;
use std::fmt;

use conduit_core::{CancellationPolicy, TerminalPolicy};
use conduit_runtime::lowering::MAXIMUM_KERNEL_PORTS_PER_NODE;

/// Reviewed finite ceilings for one generated fixed image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmbeddedImageBounds {
    pub maximum_nodes: usize,
    pub maximum_cords: usize,
    pub maximum_routes: usize,
    pub maximum_route_targets: usize,
    pub maximum_host_operations: usize,
    pub maximum_resources: usize,
    pub maximum_evidence_expectations: usize,
    pub maximum_configuration_entries: usize,
    pub maximum_ports_per_node: usize,
    pub maximum_cord_value_slots: u16,
    pub maximum_cord_value_bytes: u32,
    pub maximum_evidence_items: u16,
    pub maximum_evidence_bytes: u32,
}

impl EmbeddedImageBounds {
    /// Broad host-tooling limits. Board builds should provide a narrower,
    /// reviewed profile rather than silently inheriting these ceilings.
    pub const HOST_TOOLING: Self = Self {
        maximum_nodes: u16::MAX as usize,
        maximum_cords: u16::MAX as usize,
        maximum_routes: u16::MAX as usize,
        maximum_route_targets: u16::MAX as usize,
        maximum_host_operations: u16::MAX as usize,
        maximum_resources: u16::MAX as usize,
        maximum_evidence_expectations: u16::MAX as usize,
        maximum_configuration_entries: u16::MAX as usize,
        maximum_ports_per_node: MAXIMUM_KERNEL_PORTS_PER_NODE,
        maximum_cord_value_slots: u16::MAX,
        maximum_cord_value_bytes: u32::MAX,
        maximum_evidence_items: u16::MAX,
        maximum_evidence_bytes: u32::MAX,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsupportedPlanFeature {
    RemoteConnection,
    RemoteRouteTarget,
    WiderKernelPortTable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GenerationError {
    InvalidFragment,
    EmptyFragment,
    IdentityMismatch,
    Unsupported(UnsupportedPlanFeature),
    InconsistentLowering(&'static str),
    BoundExceeded {
        table: &'static str,
        actual: u64,
        maximum: u64,
    },
    InvalidRange {
        table: &'static str,
        start: u64,
        length: u64,
        limit: u64,
    },
    ArithmeticOverflow(&'static str),
}

impl fmt::Display for GenerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFragment => formatter.write_str("current plan fragment is invalid"),
            Self::EmptyFragment => formatter.write_str("cannot generate an empty plan fragment"),
            Self::IdentityMismatch => formatter.write_str(
                "current plan fragment and lowered fragment do not share exact identity",
            ),
            Self::Unsupported(feature) => {
                write!(formatter, "fixed image does not support {feature:?}")
            }
            Self::InconsistentLowering(subject) => {
                write!(formatter, "lowered fragment is inconsistent: {subject}")
            }
            Self::BoundExceeded {
                table,
                actual,
                maximum,
            } => write!(
                formatter,
                "fixed-image {table} bound exceeded: {actual} > {maximum}"
            ),
            Self::InvalidRange {
                table,
                start,
                length,
                limit,
            } => write!(
                formatter,
                "invalid {table} range: start={start} length={length} limit={limit}"
            ),
            Self::ArithmeticOverflow(subject) => {
                write!(formatter, "fixed-image arithmetic overflow: {subject}")
            }
        }
    }
}

impl Error for GenerationError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedPort {
    pub node: u16,
    pub port: u16,
    pub port_id: String,
    pub value_kind: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GeneratedConfigurationValue {
    Bool(bool),
    U64(u64),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedConfigurationEntry {
    pub node: u16,
    pub key: String,
    pub value: GeneratedConfigurationValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedStaticNode {
    pub node: u16,
    pub placement_id: String,
    pub kind_id: String,
    pub implementation_id: String,
    pub artifact_id: String,
    pub input_cords: [Option<u16>; MAXIMUM_KERNEL_PORTS_PER_NODE],
    pub maximum_step_work: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedStaticCord {
    pub cord: u16,
    pub connection_id: String,
    pub source_node: u16,
    pub source_port: u16,
    pub sink_node: u16,
    pub sink_port: u16,
    pub slot_start: u16,
    pub item_capacity: u16,
    pub byte_capacity: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneratedStaticRoute {
    pub source_node: u16,
    pub source_port: u16,
    pub target_start: u16,
    pub target_len: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneratedStaticRouteTarget {
    pub cord: u16,
    pub sink_node: u16,
    pub sink_port: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedHostOperation {
    pub node: u16,
    pub operation: u16,
    pub contract_id: String,
    pub target_kind: Option<String>,
    pub maximum_in_flight: u16,
    pub maximum_input_bytes: u32,
    pub maximum_output_bytes: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneratedStaticResource {
    pub node: u16,
    pub resource: u16,
    pub units: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneratedEvidenceTarget {
    Fragment,
    Node(u16),
    Cord(u16),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedStaticEvidence {
    pub expectation: u16,
    pub kind: &'static str,
    pub subject: Option<String>,
    pub target: GeneratedEvidenceTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedExpectedTerminal {
    pub kind: &'static str,
    pub subject: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneratedStartupDependency {
    pub prerequisite_node: u16,
    pub dependent_node: u16,
}

/// Owned build output. Firmware may consume only the rendered module and does
/// not need to link this host-only crate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedEmbeddedPlan {
    pub schema_version: u32,
    pub plan_id: String,
    pub fragment_id: String,
    pub host_id: String,
    pub boot_id: String,
    pub offer_generation: u64,
    pub cancellation_policy: CancellationPolicy,
    pub terminal_policy: TerminalPolicy,
    pub nodes: Vec<GeneratedStaticNode>,
    pub input_ports: Vec<GeneratedPort>,
    pub output_ports: Vec<GeneratedPort>,
    pub configuration: Vec<GeneratedConfigurationEntry>,
    pub cords: Vec<GeneratedStaticCord>,
    pub routes: Vec<GeneratedStaticRoute>,
    pub route_targets: Vec<GeneratedStaticRouteTarget>,
    pub host_operations: Vec<GeneratedHostOperation>,
    pub resources: Vec<GeneratedStaticResource>,
    pub evidence: Vec<GeneratedStaticEvidence>,
    pub startup_dependencies: Vec<GeneratedStartupDependency>,
    pub startup_order: Vec<u16>,
    pub expected_terminals: Vec<GeneratedExpectedTerminal>,
    pub cord_value_slots: u16,
    pub cord_value_bytes: u32,
    pub evidence_items: u16,
    pub evidence_bytes: u32,
}
