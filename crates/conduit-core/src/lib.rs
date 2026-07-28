#![no_std]

//! Allocator-free semantic contracts and execution-plan validation.
//!
//! This crate intentionally contains no parser, registry, dynamic loader,
//! transport, filesystem, or product concepts. A hosted planner can lower rich
//! source and manifests into these borrowed structures; constrained firmware
//! can validate and execute the resulting bounded plan.

use core::fmt;

mod canonical;
mod compatibility;
mod type_contract;

pub use canonical::{
    CANONICAL_FORM_VERSION, CANONICAL_MAGIC, CanonicalDescriptor, CanonicalError, CanonicalSink,
    CanonicalValue, FieldDisposition, MAX_CANONICAL_DEPTH, MapField, SEMANTIC_HASH_DOMAIN,
    SemanticHash,
};
pub use compatibility::{
    CompatibilityClass, CompatibilityDecision, CompatibilityOutcome, CompatibilityQuery,
    CompatibilityReason, DescriptorRef, MigrationRef, RecordField, RecordSchema,
    UnknownFieldPolicy, ValueAcceptance, assess_exact, assess_migration, assess_reader_acceptance,
};
pub use type_contract::{TypeContractRef, TypeContractRefError};

/// A stable identifier borrowed from a descriptor or resolved plan.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Id<'a>(pub &'a str);

impl<'a> Id<'a> {
    /// Creates an identifier after validating the bootstrap ASCII grammar.
    ///
    /// Local identifiers contain lowercase letters, digits, `_`, `-`, and `.`;
    /// they begin with a lowercase letter. Qualified semantic identities may
    /// additionally contain one `/`.
    pub fn new(value: &'a str) -> Result<Self, IdError> {
        if value.is_empty() {
            return Err(IdError::Empty);
        }

        let mut slash_count = 0_u8;
        let mut segment_start = true;
        for (index, byte) in value.bytes().enumerate() {
            let valid = if segment_start {
                byte.is_ascii_lowercase()
            } else {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'_' | b'-' | b'.' | b'/')
            };
            if !valid {
                return Err(IdError::InvalidByte { index, byte });
            }
            if byte == b'/' {
                slash_count = slash_count.saturating_add(1);
                if slash_count > 1 || index + 1 == value.len() {
                    return Err(IdError::InvalidSlash);
                }
            }
            segment_start = matches!(byte, b'.' | b'/');
        }
        if segment_start {
            return Err(IdError::InvalidSlash);
        }

        Ok(Self(value))
    }

    /// Returns the textual identifier.
    #[must_use]
    pub const fn as_str(self) -> &'a str {
        self.0
    }
}

impl fmt::Display for Id<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

/// Identifier validation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdError {
    /// The identifier was empty.
    Empty,
    /// A byte is outside the portable identifier grammar.
    InvalidByte {
        /// Zero-based byte index.
        index: usize,
        /// Invalid byte.
        byte: u8,
    },
    /// A qualified identifier used `/` incorrectly.
    InvalidSlash,
}

impl fmt::Display for IdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("identifier is empty"),
            Self::InvalidByte { index, byte } => {
                write!(formatter, "invalid identifier byte {byte:#04x} at {index}")
            }
            Self::InvalidSlash => formatter.write_str("identifier has an invalid namespace slash"),
        }
    }
}

/// Port direction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    /// Values enter the node.
    Input,
    /// Values leave the node.
    Output,
}

/// Whether a port must be connected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Presence {
    /// At least one connection is required when cardinality permits it.
    Required,
    /// The port may remain disconnected.
    Optional,
}

/// Number of cords permitted at a port.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cardinality {
    /// Exactly one cord.
    ExactlyOne,
    /// Zero or one cord.
    ZeroOrOne,
    /// One or more cords.
    OneOrMore,
    /// Any number of cords.
    ZeroOrMore,
}

impl Cardinality {
    const fn accepts(self, count: usize) -> bool {
        match self {
            Self::ExactlyOne => count == 1,
            Self::ZeroOrOne => count <= 1,
            Self::OneOrMore => count >= 1,
            Self::ZeroOrMore => true,
        }
    }
}

/// Semantic delivery shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Delivery {
    /// Every value is independently meaningful and ordered.
    Stream,
    /// A newer value replaces the previous current state.
    LatestState,
    /// A finite collection is delivered as one logical value.
    FiniteBatch,
    /// A durable or content-addressed artifact reference.
    ArtifactReference,
    /// Lifecycle or authority-bearing control data.
    Control,
}

/// Behavior when a producer reaches a cord's finite capacity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Pressure {
    /// Suspend production until capacity is available.
    Block,
    /// Reject the attempted write.
    Reject,
    /// Replace a prior value under the carried type's replacement relation.
    Coalesce,
    /// Keep values according to an exact sampling policy.
    Sample,
    /// Drop only values explicitly declared disposable.
    DropDisposable,
    /// End the cord connection.
    Disconnect,
    /// Fail the affected execution scope.
    Fail,
}

/// Exact bounded flow policy for a resolved cord.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlowPolicy {
    /// Maximum number of values resident on the cord.
    pub capacity_items: u16,
    /// Saturation behavior.
    pub pressure: Pressure,
}

/// A semantic port contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortContract<'a> {
    /// Stable port identifier within the node contract.
    pub id: Id<'a>,
    /// Direction of value movement.
    pub direction: Direction,
    /// Stable semantic type identity.
    pub value_type: Id<'a>,
    /// Whether the port must be connected.
    pub presence: Presence,
    /// Permitted connection count.
    pub cardinality: Cardinality,
    /// Semantic delivery shape.
    pub delivery: Delivery,
}

/// A semantic node contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeContract<'a> {
    /// Stable semantic contract identity.
    pub id: Id<'a>,
    /// Input ports in stable contract order.
    pub inputs: &'a [PortContract<'a>],
    /// Output ports in stable contract order.
    pub outputs: &'a [PortContract<'a>],
}

/// A resolved node instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanNode<'a> {
    /// Stable local instance ID.
    pub id: Id<'a>,
    /// Exact semantic contract.
    pub contract: &'a NodeContract<'a>,
}

/// One end of a resolved cord.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Endpoint {
    /// Index into [`ExecutionPlan::nodes`].
    pub node: u16,
    /// Index into the node contract's direction-appropriate port slice.
    pub port: u16,
}

/// A resolved, typed, bounded cord.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanCord<'a> {
    /// Stable local cord ID.
    pub id: Id<'a>,
    /// Output endpoint.
    pub from: Endpoint,
    /// Input endpoint.
    pub to: Endpoint,
    /// Exact flow behavior.
    pub flow: FlowPolicy,
}

/// A borrowed resolved execution plan suitable for constrained runtimes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionPlan<'a> {
    /// Resolved nodes.
    pub nodes: &'a [PlanNode<'a>],
    /// Resolved cords.
    pub cords: &'a [PlanCord<'a>],
}

/// Stable validation diagnostic code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticCode {
    /// `CND-ID-002`: duplicate local identity.
    DuplicateIdentity,
    /// `CND-PRT-001`: endpoint direction or index is invalid.
    InvalidEndpoint,
    /// `CND-PRT-002`: port cardinality is invalid.
    CardinalityViolation,
    /// `CND-TYP-001`: connected port types differ.
    TypeMismatch,
    /// `CND-FLW-001`: a live cord has no finite capacity.
    UnboundedCord,
}

impl DiagnosticCode {
    /// Returns the stable external code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DuplicateIdentity => "CND-ID-002",
            Self::InvalidEndpoint => "CND-PRT-001",
            Self::CardinalityViolation => "CND-PRT-002",
            Self::TypeMismatch => "CND-TYP-001",
            Self::UnboundedCord => "CND-FLW-001",
        }
    }
}

/// One validation failure with an optional subject index.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidationError {
    /// Stable diagnostic code.
    pub code: DiagnosticCode,
    /// Node or cord index when one subject exists.
    pub subject_index: Option<u16>,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.code.as_str())?;
        if let Some(index) = self.subject_index {
            write!(formatter, " at index {index}")?;
        }
        Ok(())
    }
}

/// Validates the portable structural invariants of a resolved execution plan.
///
/// Rich hosted validation may produce several diagnostics. This allocator-free
/// boundary returns the first failure in deterministic plan order.
pub fn validate_plan(plan: &ExecutionPlan<'_>) -> Result<(), ValidationError> {
    for (index, node) in plan.nodes.iter().enumerate() {
        if plan.nodes[..index].iter().any(|prior| prior.id == node.id) {
            return Err(ValidationError {
                code: DiagnosticCode::DuplicateIdentity,
                subject_index: u16::try_from(index).ok(),
            });
        }
    }

    for (index, cord) in plan.cords.iter().enumerate() {
        if plan.cords[..index].iter().any(|prior| prior.id == cord.id) {
            return Err(ValidationError {
                code: DiagnosticCode::DuplicateIdentity,
                subject_index: u16::try_from(index).ok(),
            });
        }
        if cord.flow.capacity_items == 0 {
            return Err(ValidationError {
                code: DiagnosticCode::UnboundedCord,
                subject_index: u16::try_from(index).ok(),
            });
        }

        let Some(source_node) = plan.nodes.get(usize::from(cord.from.node)) else {
            return Err(ValidationError {
                code: DiagnosticCode::InvalidEndpoint,
                subject_index: u16::try_from(index).ok(),
            });
        };
        let Some(source_port) = source_node
            .contract
            .outputs
            .get(usize::from(cord.from.port))
        else {
            return Err(ValidationError {
                code: DiagnosticCode::InvalidEndpoint,
                subject_index: u16::try_from(index).ok(),
            });
        };
        let Some(target_node) = plan.nodes.get(usize::from(cord.to.node)) else {
            return Err(ValidationError {
                code: DiagnosticCode::InvalidEndpoint,
                subject_index: u16::try_from(index).ok(),
            });
        };
        let Some(target_port) = target_node.contract.inputs.get(usize::from(cord.to.port)) else {
            return Err(ValidationError {
                code: DiagnosticCode::InvalidEndpoint,
                subject_index: u16::try_from(index).ok(),
            });
        };

        if source_port.direction != Direction::Output || target_port.direction != Direction::Input {
            return Err(ValidationError {
                code: DiagnosticCode::InvalidEndpoint,
                subject_index: u16::try_from(index).ok(),
            });
        }
        if source_port.value_type != target_port.value_type {
            return Err(ValidationError {
                code: DiagnosticCode::TypeMismatch,
                subject_index: u16::try_from(index).ok(),
            });
        }
    }

    for (node_index, node) in plan.nodes.iter().enumerate() {
        for (port_index, port) in node.contract.inputs.iter().enumerate() {
            let count = plan
                .cords
                .iter()
                .filter(|cord| {
                    usize::from(cord.to.node) == node_index
                        && usize::from(cord.to.port) == port_index
                })
                .count();
            if !port.cardinality.accepts(count)
                || (port.presence == Presence::Required && count == 0)
            {
                return Err(ValidationError {
                    code: DiagnosticCode::CardinalityViolation,
                    subject_index: u16::try_from(node_index).ok(),
                });
            }
        }
        for (port_index, port) in node.contract.outputs.iter().enumerate() {
            let count = plan
                .cords
                .iter()
                .filter(|cord| {
                    usize::from(cord.from.node) == node_index
                        && usize::from(cord.from.port) == port_index
                })
                .count();
            if !port.cardinality.accepts(count)
                || (port.presence == Presence::Required && count == 0)
            {
                return Err(ValidationError {
                    code: DiagnosticCode::CardinalityViolation,
                    subject_index: u16::try_from(node_index).ok(),
                });
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: Id<'static> = Id("conduit/text.utf8");
    const OUT: PortContract<'static> = PortContract {
        id: Id("out"),
        direction: Direction::Output,
        value_type: TEXT,
        presence: Presence::Required,
        cardinality: Cardinality::OneOrMore,
        delivery: Delivery::FiniteBatch,
    };
    const INPUT: PortContract<'static> = PortContract {
        id: Id("in"),
        direction: Direction::Input,
        value_type: TEXT,
        presence: Presence::Required,
        cardinality: Cardinality::ExactlyOne,
        delivery: Delivery::FiniteBatch,
    };
    const SOURCE: NodeContract<'static> = NodeContract {
        id: Id("conduit/source"),
        inputs: &[],
        outputs: &[OUT],
    };
    const SINK: NodeContract<'static> = NodeContract {
        id: Id("conduit/sink"),
        inputs: &[INPUT],
        outputs: &[],
    };

    #[test]
    fn validates_a_typed_bounded_plan() {
        let nodes = [
            PlanNode {
                id: Id("source"),
                contract: &SOURCE,
            },
            PlanNode {
                id: Id("sink"),
                contract: &SINK,
            },
        ];
        let cords = [PlanCord {
            id: Id("speech"),
            from: Endpoint { node: 0, port: 0 },
            to: Endpoint { node: 1, port: 0 },
            flow: FlowPolicy {
                capacity_items: 1,
                pressure: Pressure::Block,
            },
        }];
        let plan = ExecutionPlan {
            nodes: &nodes,
            cords: &cords,
        };

        assert_eq!(validate_plan(&plan), Ok(()));
    }

    #[test]
    fn rejects_zero_capacity() {
        let nodes = [
            PlanNode {
                id: Id("source"),
                contract: &SOURCE,
            },
            PlanNode {
                id: Id("sink"),
                contract: &SINK,
            },
        ];
        let cords = [PlanCord {
            id: Id("speech"),
            from: Endpoint { node: 0, port: 0 },
            to: Endpoint { node: 1, port: 0 },
            flow: FlowPolicy {
                capacity_items: 0,
                pressure: Pressure::Block,
            },
        }];
        let plan = ExecutionPlan {
            nodes: &nodes,
            cords: &cords,
        };

        assert_eq!(
            validate_plan(&plan),
            Err(ValidationError {
                code: DiagnosticCode::UnboundedCord,
                subject_index: Some(0),
            })
        );
    }

    #[test]
    fn identifier_validation_is_portable() {
        assert!(Id::new("tongues/audio.stream").is_ok());
        assert!(Id::new("Hello").is_err());
        assert!(Id::new("two/slashes/here").is_err());
        assert!(Id::new("namespace/1invalid").is_err());
        assert!(Id::new("namespace/trailing.").is_err());
    }
}
