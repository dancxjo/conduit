//! Complete portable port contracts and directional checks.

use core::convert::Infallible;

use crate::{
    CanonicalDescriptor, CanonicalError, CanonicalSink, CanonicalValue, CompatibilityDecision,
    CompatibilityOutcome, CompatibilityQuery, FieldDisposition, Id, MapField, SemanticHash,
    TypeContractRef,
};

/// Port direction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    /// Values enter the node.
    Input,
    /// Values leave the node.
    Output,
}

/// Whether a port must be connected in a complete assemblage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Presence {
    /// At least one connection is required when cardinality permits it.
    Required,
    /// The port may remain disconnected.
    Optional,
}

/// Number of cords permitted at a port.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionCardinality {
    /// Exactly one cord.
    ExactlyOne,
    /// Zero or one cord.
    ZeroOrOne,
    /// One or more cords.
    OneOrMore,
    /// Any number of cords.
    ZeroOrMore,
}

impl ConnectionCardinality {
    pub(crate) const fn accepts_count(self, count: usize) -> bool {
        match self {
            Self::ExactlyOne => count == 1,
            Self::ZeroOrOne => count <= 1,
            Self::OneOrMore => count >= 1,
            Self::ZeroOrMore => true,
        }
    }

    const fn accepts_contract(self, producer: Self) -> bool {
        matches!(
            (self, producer),
            (Self::ZeroOrMore, _)
                | (Self::ZeroOrOne, Self::ExactlyOne | Self::ZeroOrOne)
                | (Self::OneOrMore, Self::ExactlyOne | Self::OneOrMore)
                | (Self::ExactlyOne, Self::ExactlyOne)
        )
    }

    /// Stable descriptor spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExactlyOne => "exactly-one",
            Self::ZeroOrOne => "zero-or-one",
            Self::OneOrMore => "one-or-more",
            Self::ZeroOrMore => "zero-or-more",
        }
    }
}

/// Number of logical values one activation may produce or consume.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueCardinality {
    /// Exactly one value.
    ExactlyOne,
    /// Zero or one value.
    ZeroOrOne,
    /// One or more values.
    OneOrMore,
    /// Any number of values.
    ZeroOrMore,
}

impl ValueCardinality {
    fn accepts(self, producer: Self) -> bool {
        matches!(
            (self, producer),
            (Self::ZeroOrMore, _)
                | (Self::ZeroOrOne, Self::ExactlyOne | Self::ZeroOrOne)
                | (Self::OneOrMore, Self::ExactlyOne | Self::OneOrMore)
                | (Self::ExactlyOne, Self::ExactlyOne)
        )
    }

    /// Stable descriptor spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExactlyOne => "exactly-one",
            Self::ZeroOrOne => "zero-or-one",
            Self::OneOrMore => "one-or-more",
            Self::ZeroOrMore => "zero-or-more",
        }
    }
}

/// Semantic delivery shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Delivery {
    /// Values form an ordered live stream.
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

impl Delivery {
    /// Stable descriptor spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stream => "stream",
            Self::LatestState => "latest-state",
            Self::FiniteBatch => "finite-batch",
            Self::ArtifactReference => "artifact-reference",
            Self::Control => "control",
        }
    }
}

/// Stability of values over semantic time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemporalContract {
    /// The value has no progressive/final distinction.
    Atemporal,
    /// Values may be provisional and superseded by later values.
    Progressive,
    /// Every delivered value is committed and will not be revised.
    Committed,
    /// Every delivered value is committed but comes from an explicit finite
    /// retained-state boundary. This gives a recurrence edge temporal meaning.
    RetainedState,
}

impl TemporalContract {
    fn accepts(self, producer: Self) -> bool {
        self == producer
            || matches!(
                (self, producer),
                (
                    Self::Committed | Self::Progressive,
                    Self::Committed | Self::RetainedState
                )
            )
    }

    /// Stable descriptor spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Atemporal => "atemporal",
            Self::Progressive => "progressive",
            Self::Committed => "committed",
            Self::RetainedState => "retained-state",
        }
    }
}

/// Natural terminal behavior at the port boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalContract {
    /// The producer emits an explicit natural completion after finite values.
    Finite,
    /// No natural completion is promised; cancellation and failure remain explicit.
    OpenEnded,
    /// The consumer accepts either finite completion or an open-ended producer.
    Either,
}

impl TerminalContract {
    fn accepts(self, producer: Self) -> bool {
        self == Self::Either || self == producer
    }

    /// Stable descriptor spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Finite => "finite",
            Self::OpenEnded => "open-ended",
            Self::Either => "either",
        }
    }
}

/// Sensitivity ceiling carried by a port or configuration field.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Sensitivity {
    /// Safe for ordinary public observation.
    Public,
    /// Restricted to an authorized execution scope.
    Restricted,
    /// Secret material requiring redaction and explicit grants.
    Secret,
}

impl Sensitivity {
    /// Stable descriptor spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Restricted => "restricted",
            Self::Secret => "secret",
        }
    }
}

/// Loss behavior a port can accept before exact FlowPolicy resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LossAcceptance {
    /// No value may be discarded or coalesced.
    LosslessOnly,
    /// Loss is allowed only when the TypeContract proves the required trait.
    TypeContractDefined,
}

impl LossAcceptance {
    fn accepts(self, producer: Self) -> bool {
        self == Self::TypeContractDefined || self == producer
    }

    /// Stable descriptor spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LosslessOnly => "lossless-only",
            Self::TypeContractDefined => "type-contract-defined",
        }
    }
}

/// Conservative port-level constraints consumed by FlowPolicy resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortFlowConstraints {
    /// Accepted loss semantics. Every resolved cord still requires finite capacity.
    pub loss: LossAcceptance,
}

/// A semantic port contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortContract<'a> {
    /// Stable port identifier within the node contract.
    pub id: Id<'a>,
    /// Direction of value movement.
    pub direction: Direction,
    /// Exact domain-owned semantic type reference.
    pub value_type: TypeContractRef<'a>,
    /// Whether the port must be connected.
    pub presence: Presence,
    /// Permitted number of attached cords.
    pub connections: ConnectionCardinality,
    /// Permitted logical value count.
    pub values: ValueCardinality,
    /// Semantic delivery shape.
    pub delivery: Delivery,
    /// Progressive, committed, or atemporal values.
    pub temporal: TemporalContract,
    /// Natural terminal behavior.
    pub terminal: TerminalContract,
    /// Maximum sensitivity accepted or emitted.
    pub sensitivity: Sensitivity,
    /// Constraints that exact FlowPolicy resolution must satisfy.
    pub flow: PortFlowConstraints,
}

impl Direction {
    /// Stable descriptor spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Output => "output",
        }
    }
}

impl Presence {
    /// Stable descriptor spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Required => "required",
            Self::Optional => "optional",
        }
    }
}

impl PortContract<'_> {
    /// Streams the current port descriptor.
    pub fn write_canonical<S: CanonicalSink>(
        &self,
        sink: &mut S,
    ) -> Result<(), CanonicalError<S::Error>> {
        let type_fields = [
            semantic(
                "contract_id",
                CanonicalValue::Identifier(self.value_type.contract_id),
            ),
            semantic(
                "schema_version",
                CanonicalValue::Integer(i128::from(self.value_type.schema_version)),
            ),
            semantic(
                "semantic_hash",
                CanonicalValue::Bytes(self.value_type.semantic_hash.as_bytes()),
            ),
        ];
        let flow_fields = [semantic(
            "loss",
            CanonicalValue::Identifier(Id(self.flow.loss.as_str())),
        )];
        let fields = [
            semantic("id", CanonicalValue::Identifier(self.id)),
            semantic(
                "direction",
                CanonicalValue::Identifier(Id(self.direction.as_str())),
            ),
            semantic("value_type", CanonicalValue::Map(&type_fields)),
            semantic(
                "presence",
                CanonicalValue::Identifier(Id(self.presence.as_str())),
            ),
            semantic(
                "connections",
                CanonicalValue::Identifier(Id(self.connections.as_str())),
            ),
            semantic(
                "values",
                CanonicalValue::Identifier(Id(self.values.as_str())),
            ),
            semantic(
                "delivery",
                CanonicalValue::Identifier(Id(self.delivery.as_str())),
            ),
            semantic(
                "temporal",
                CanonicalValue::Identifier(Id(self.temporal.as_str())),
            ),
            semantic(
                "terminal",
                CanonicalValue::Identifier(Id(self.terminal.as_str())),
            ),
            semantic(
                "sensitivity",
                CanonicalValue::Identifier(Id(self.sensitivity.as_str())),
            ),
            semantic("flow", CanonicalValue::Map(&flow_fields)),
        ];
        CanonicalDescriptor {
            kind: Id("conduit/port-contract"),
            schema_version: 0,
            body: CanonicalValue::Map(&fields),
        }
        .write_canonical(sink)
    }

    /// Computes the exact canonical port descriptor identity.
    pub fn semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        let mut sink = PortHashSink::new();
        self.write_canonical(&mut sink)?;
        Ok(sink.finish())
    }
}

fn semantic<'a>(name: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    }
}

struct PortHashSink(sha2::Sha256);

impl PortHashSink {
    fn new() -> Self {
        use sha2::Digest as _;

        let mut digest = sha2::Sha256::new();
        digest.update(crate::SEMANTIC_HASH_DOMAIN);
        Self(digest)
    }

    fn finish(self) -> SemanticHash {
        use sha2::Digest as _;

        SemanticHash::from_bytes(self.0.finalize().into())
    }
}

impl CanonicalSink for PortHashSink {
    type Error = Infallible;

    fn write(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        use sha2::Digest as _;

        self.0.update(bytes);
        Ok(())
    }
}

/// Stable reason for a directional port decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortCompatibilityReason {
    /// Every field is directionally accepted.
    Accepted,
    /// A connection was not output to input.
    DirectionMismatch,
    /// The type provider disproved or could not determine acceptance.
    TypeMismatch,
    /// A candidate made an optional port required.
    PresenceMismatch,
    /// Connection-count constraints were narrowed.
    ConnectionCardinalityMismatch,
    /// Producer value counts exceed consumer acceptance.
    ValueCardinalityMismatch,
    /// Delivery shapes differ.
    DeliveryMismatch,
    /// Producer temporal behavior is not accepted.
    TemporalMismatch,
    /// Producer terminal behavior is not accepted.
    TerminalMismatch,
    /// Sensitive data would cross into a weaker boundary.
    SensitivityViolation,
    /// Producer loss semantics are not accepted.
    FlowConstraintMismatch,
}

impl PortCompatibilityReason {
    /// Stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "port-accepted",
            Self::DirectionMismatch => "port-direction-mismatch",
            Self::TypeMismatch => "port-type-mismatch",
            Self::PresenceMismatch => "port-presence-mismatch",
            Self::ConnectionCardinalityMismatch => "port-connection-cardinality-mismatch",
            Self::ValueCardinalityMismatch => "port-value-cardinality-mismatch",
            Self::DeliveryMismatch => "port-delivery-mismatch",
            Self::TemporalMismatch => "port-temporal-mismatch",
            Self::TerminalMismatch => "port-terminal-mismatch",
            Self::SensitivityViolation => "port-sensitivity-violation",
            Self::FlowConstraintMismatch => "port-flow-constraint-mismatch",
        }
    }
}

/// Complete directional port decision retaining the type-provider explanation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortCompatibilityDecision<'a> {
    /// Consumer input or required boundary.
    pub consumer: PortContract<'a>,
    /// Producer output or candidate boundary.
    pub producer: PortContract<'a>,
    /// Compatible, incompatible, or indeterminate.
    pub outcome: CompatibilityOutcome,
    /// Stable port-level reason.
    pub reason: PortCompatibilityReason,
    /// Exact nested type-contract decision.
    pub type_decision: CompatibilityDecision<'a>,
}

/// Assesses one output-to-input connection.
#[must_use]
pub fn assess_port_connection<'a>(
    consumer: PortContract<'a>,
    producer: PortContract<'a>,
    type_decision: CompatibilityDecision<'a>,
) -> PortCompatibilityDecision<'a> {
    let mut outcome = CompatibilityOutcome::Incompatible;
    let reason =
        if consumer.direction != Direction::Input || producer.direction != Direction::Output {
            PortCompatibilityReason::DirectionMismatch
        } else if !type_operands_match(type_decision, consumer.value_type, producer.value_type)
            || type_decision.outcome != CompatibilityOutcome::Compatible
        {
            outcome = type_decision.outcome;
            if outcome == CompatibilityOutcome::Compatible {
                outcome = CompatibilityOutcome::Indeterminate;
            }
            PortCompatibilityReason::TypeMismatch
        } else if !consumer.values.accepts(producer.values) {
            PortCompatibilityReason::ValueCardinalityMismatch
        } else if consumer.delivery != producer.delivery {
            PortCompatibilityReason::DeliveryMismatch
        } else if !consumer.temporal.accepts(producer.temporal) {
            PortCompatibilityReason::TemporalMismatch
        } else if !consumer.terminal.accepts(producer.terminal) {
            PortCompatibilityReason::TerminalMismatch
        } else if consumer.sensitivity < producer.sensitivity {
            PortCompatibilityReason::SensitivityViolation
        } else if !consumer.flow.loss.accepts(producer.flow.loss) {
            PortCompatibilityReason::FlowConstraintMismatch
        } else {
            outcome = CompatibilityOutcome::Compatible;
            PortCompatibilityReason::Accepted
        };
    PortCompatibilityDecision {
        consumer,
        producer,
        outcome,
        reason,
        type_decision,
    }
}

/// Assesses whether a candidate same-direction port preserves a required port.
///
/// The caller supplies the type decision in the variance direction required by
/// `COM-008`. Other current semantic fields use conservative directional rules.
#[must_use]
pub fn assess_port_substitution<'a>(
    required: PortContract<'a>,
    candidate: PortContract<'a>,
    type_decision: CompatibilityDecision<'a>,
) -> PortCompatibilityDecision<'a> {
    let mut outcome = CompatibilityOutcome::Incompatible;
    let reason = if required.direction != candidate.direction {
        PortCompatibilityReason::DirectionMismatch
    } else if required.presence == Presence::Optional && candidate.presence == Presence::Required {
        PortCompatibilityReason::PresenceMismatch
    } else if !candidate.connections.accepts_contract(required.connections) {
        PortCompatibilityReason::ConnectionCardinalityMismatch
    } else {
        let (type_consumer, type_producer, consumer, producer) =
            if required.direction == Direction::Input {
                (
                    candidate.value_type,
                    required.value_type,
                    candidate,
                    required,
                )
            } else {
                (
                    required.value_type,
                    candidate.value_type,
                    required,
                    candidate,
                )
            };
        if !type_operands_match(type_decision, type_consumer, type_producer)
            || type_decision.outcome != CompatibilityOutcome::Compatible
        {
            outcome = type_decision.outcome;
            if outcome == CompatibilityOutcome::Compatible {
                outcome = CompatibilityOutcome::Indeterminate;
            }
            PortCompatibilityReason::TypeMismatch
        } else {
            let connection = assess_semantics(consumer, producer);
            outcome = connection.0;
            connection.1
        }
    };
    PortCompatibilityDecision {
        consumer: required,
        producer: candidate,
        outcome,
        reason,
        type_decision,
    }
}

fn type_operands_match(
    decision: CompatibilityDecision<'_>,
    consumer: TypeContractRef<'_>,
    producer: TypeContractRef<'_>,
) -> bool {
    matches!(
        decision.query,
        CompatibilityQuery::ConsumerAcceptsProducer {
            consumer: decision_consumer,
            producer: decision_producer,
        } if decision_consumer == consumer && decision_producer == producer
    )
}

fn assess_semantics(
    consumer: PortContract<'_>,
    producer: PortContract<'_>,
) -> (CompatibilityOutcome, PortCompatibilityReason) {
    let reason = if !consumer.values.accepts(producer.values) {
        PortCompatibilityReason::ValueCardinalityMismatch
    } else if consumer.delivery != producer.delivery {
        PortCompatibilityReason::DeliveryMismatch
    } else if !consumer.temporal.accepts(producer.temporal) {
        PortCompatibilityReason::TemporalMismatch
    } else if !consumer.terminal.accepts(producer.terminal) {
        PortCompatibilityReason::TerminalMismatch
    } else if consumer.sensitivity < producer.sensitivity {
        PortCompatibilityReason::SensitivityViolation
    } else if !consumer.flow.loss.accepts(producer.flow.loss) {
        PortCompatibilityReason::FlowConstraintMismatch
    } else {
        return (
            CompatibilityOutcome::Compatible,
            PortCompatibilityReason::Accepted,
        );
    };
    (CompatibilityOutcome::Incompatible, reason)
}
