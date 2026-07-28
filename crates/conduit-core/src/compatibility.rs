//! Directional compatibility decisions and conservative record-schema rules.
//!
//! Compatibility is never exposed as a bare boolean. A decision records the
//! exact directional query, outcome, class, stable reason, and affected field.
//! Domain providers can use the same algebra while retaining ownership of
//! domain-specific type meaning.

use core::fmt;

use crate::{Id, SemanticHash, TypeContractRef};

/// An exact descriptor revision participating in a compatibility question.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DescriptorRef<'a> {
    /// Descriptor kind.
    pub kind: Id<'a>,
    /// Exact kind-scoped schema revision.
    pub schema_version: u32,
    /// Exact canonical semantic identity.
    pub semantic_hash: SemanticHash,
}

/// The directional question answered by a compatibility decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompatibilityQuery<'a> {
    /// Whether two exact descriptors are identical.
    Exact {
        /// First exact descriptor.
        left: DescriptorRef<'a>,
        /// Second exact descriptor.
        right: DescriptorRef<'a>,
    },
    /// Whether every document emitted by `writer` can be accepted by `reader`.
    ReaderAcceptsWriter {
        /// Consumer of encoded documents or evidence.
        reader: DescriptorRef<'a>,
        /// Producer of encoded documents or evidence.
        writer: DescriptorRef<'a>,
    },
    /// Whether every value emitted by `producer` is accepted by `consumer`.
    ConsumerAcceptsProducer {
        /// Domain-owned type contract at the consuming boundary.
        consumer: TypeContractRef<'a>,
        /// Domain-owned type contract at the producing boundary.
        producer: TypeContractRef<'a>,
    },
    /// Whether `candidate` may replace `required` at a semantic boundary.
    CandidateSubstitutesRequired {
        /// Contract demanded by the surrounding assemblage.
        required: DescriptorRef<'a>,
        /// Proposed replacement contract.
        candidate: DescriptorRef<'a>,
    },
    /// Whether an exact migration transforms `source` into `target`.
    Migration {
        /// Exact source descriptor.
        source: DescriptorRef<'a>,
        /// Exact target descriptor.
        target: DescriptorRef<'a>,
        /// Exact migration descriptor identity.
        migration: SemanticHash,
    },
}

/// Three-valued compatibility outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompatibilityOutcome {
    /// The requested directional relation is proven.
    Compatible,
    /// The requested directional relation is disproven.
    Incompatible,
    /// A named provider or additional fact is required.
    Indeterminate,
}

impl CompatibilityOutcome {
    /// Stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Compatible => "compatible",
            Self::Incompatible => "incompatible",
            Self::Indeterminate => "indeterminate",
        }
    }
}

/// Meaning of a proven compatibility relation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompatibilityClass {
    /// Exact kind, schema revision, and semantic hash.
    Exact,
    /// Same-version but non-identical descriptors are directionally accepted.
    Accepted,
    /// A newer reader accepts an older writer.
    BackwardCompatible,
    /// An older reader accepts a newer writer.
    ForwardCompatible,
    /// A candidate can replace a required semantic contract.
    Substitutable,
    /// An exact explicit migration is available.
    Migratable,
}

impl CompatibilityClass {
    /// Stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Accepted => "accepted",
            Self::BackwardCompatible => "backward-compatible",
            Self::ForwardCompatible => "forward-compatible",
            Self::Substitutable => "substitutable",
            Self::Migratable => "migratable",
        }
    }
}

/// Stable reason for a compatibility outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompatibilityReason {
    /// Exact identities match.
    ExactIdentity,
    /// All producer fields are accepted by the reader.
    FieldsAccepted,
    /// Descriptor kinds differ.
    DescriptorKindMismatch,
    /// Exact schema revisions differ.
    SchemaVersionMismatch,
    /// Canonical semantic identities differ.
    SemanticHashMismatch,
    /// A reader-required field cannot be emitted by the writer.
    MissingRequiredField,
    /// The writer may emit a field the reader rejects.
    UnknownProducerField,
    /// A field's producer value contract is not accepted.
    ValueContractRejected,
    /// A domain provider must decide value-contract compatibility.
    ValueProviderRequired,
    /// Missing values would acquire different meanings.
    DefaultChanged,
    /// A record schema is malformed or ambiguous.
    InvalidSchema,
    /// Migration source does not match the query.
    MigrationSourceMismatch,
    /// Migration target does not match the query.
    MigrationTargetMismatch,
    /// Migration behavior is not deterministic.
    MigrationNotDeterministic,
    /// Migration does not cover every valid source value.
    MigrationNotTotal,
    /// Exact deterministic total migration is available.
    MigrationAccepted,
    /// Both type-contract references have exact identity.
    TypeContractExact,
    /// Both contracts explicitly select the same structural projection.
    TypeStructuralAccepted,
    /// Explicit structural projections differ.
    TypeStructuralMismatch,
    /// The contracts select different comparison strategies.
    TypeStrategyMismatch,
    /// A provider returned a comparison strategy unknown to this registry.
    TypeStrategyUnknown,
    /// A type-contract reference is malformed.
    InvalidTypeReference,
    /// No provider is registered for the required namespace.
    TypeProviderUnavailable,
    /// A provider does not know the exact referenced contract.
    TypeContractUnknown,
    /// A provider descriptor does not match the referenced semantic identity.
    TypeDescriptorInvalid,
    /// A domain provider proved directional acceptance.
    TypeProviderAccepted,
    /// A domain provider disproved directional acceptance.
    TypeProviderRejected,
    /// A domain provider requires an additional named fact.
    TypeProviderIndeterminate,
    /// A provider returned a malformed decision.
    TypeProviderDecisionInvalid,
}

impl CompatibilityReason {
    /// Stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExactIdentity => "exact-identity",
            Self::FieldsAccepted => "fields-accepted",
            Self::DescriptorKindMismatch => "descriptor-kind-mismatch",
            Self::SchemaVersionMismatch => "schema-version-mismatch",
            Self::SemanticHashMismatch => "semantic-hash-mismatch",
            Self::MissingRequiredField => "missing-required-field",
            Self::UnknownProducerField => "unknown-producer-field",
            Self::ValueContractRejected => "value-contract-rejected",
            Self::ValueProviderRequired => "value-provider-required",
            Self::DefaultChanged => "default-changed",
            Self::InvalidSchema => "invalid-schema",
            Self::MigrationSourceMismatch => "migration-source-mismatch",
            Self::MigrationTargetMismatch => "migration-target-mismatch",
            Self::MigrationNotDeterministic => "migration-not-deterministic",
            Self::MigrationNotTotal => "migration-not-total",
            Self::MigrationAccepted => "migration-accepted",
            Self::TypeContractExact => "type-contract-exact",
            Self::TypeStructuralAccepted => "type-structural-accepted",
            Self::TypeStructuralMismatch => "type-structural-mismatch",
            Self::TypeStrategyMismatch => "type-strategy-mismatch",
            Self::TypeStrategyUnknown => "type-strategy-unknown",
            Self::InvalidTypeReference => "invalid-type-reference",
            Self::TypeProviderUnavailable => "type-provider-unavailable",
            Self::TypeContractUnknown => "type-contract-unknown",
            Self::TypeDescriptorInvalid => "type-descriptor-invalid",
            Self::TypeProviderAccepted => "type-provider-accepted",
            Self::TypeProviderRejected => "type-provider-rejected",
            Self::TypeProviderIndeterminate => "type-provider-indeterminate",
            Self::TypeProviderDecisionInvalid => "type-provider-decision-invalid",
        }
    }
}

/// Complete reasoned answer to a directional compatibility query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompatibilityDecision<'a> {
    /// Exact question that was answered.
    pub query: CompatibilityQuery<'a>,
    /// Compatible, incompatible, or indeterminate.
    pub outcome: CompatibilityOutcome,
    /// Proven class when compatible.
    pub class: Option<CompatibilityClass>,
    /// Stable machine-readable reason.
    pub reason: CompatibilityReason,
    /// Field or other local subject when one exists.
    pub subject: Option<Id<'a>>,
    /// Exact migration identity when migration proves compatibility.
    pub migration: Option<SemanticHash>,
}

impl<'a> CompatibilityDecision<'a> {
    /// Constructs a proven compatible decision.
    #[must_use]
    pub const fn compatible(
        query: CompatibilityQuery<'a>,
        class: CompatibilityClass,
        reason: CompatibilityReason,
        subject: Option<Id<'a>>,
    ) -> Self {
        Self {
            query,
            outcome: CompatibilityOutcome::Compatible,
            class: Some(class),
            reason,
            subject,
            migration: None,
        }
    }

    /// Constructs a proven incompatible decision.
    #[must_use]
    pub const fn incompatible(
        query: CompatibilityQuery<'a>,
        reason: CompatibilityReason,
        subject: Option<Id<'a>>,
    ) -> Self {
        Self {
            query,
            outcome: CompatibilityOutcome::Incompatible,
            class: None,
            reason,
            subject,
            migration: None,
        }
    }

    /// Constructs a decision requiring an external provider or fact.
    #[must_use]
    pub const fn indeterminate(
        query: CompatibilityQuery<'a>,
        reason: CompatibilityReason,
        subject: Option<Id<'a>>,
    ) -> Self {
        Self {
            query,
            outcome: CompatibilityOutcome::Indeterminate,
            class: None,
            reason,
            subject,
            migration: None,
        }
    }
}

impl fmt::Display for CompatibilityDecision<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {}",
            self.outcome.as_str(),
            self.reason.as_str()
        )?;
        if let Some(subject) = self.subject {
            write!(formatter, " ({subject})")?;
        }
        Ok(())
    }
}

/// Reader behavior for semantic fields it does not recognize.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnknownFieldPolicy {
    /// Reject any unknown semantic field.
    Reject,
    /// Preserve the canonical field without claiming to interpret it.
    Preserve,
}

/// How a reader field accepts a writer's value contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueAcceptance<'a> {
    /// Accept only the field's exact declared value contract.
    Exact,
    /// Accept the exact contract or one of these provider-approved producers.
    ExactOr(&'a [SemanticHash]),
    /// Ask the domain provider when exact identity does not match.
    ProviderRequired,
}

/// One field in a conservative record schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecordField<'a> {
    /// Stable field identifier.
    pub id: Id<'a>,
    /// Whether a valid record must carry this field.
    pub required: bool,
    /// Exact value contract declared by this schema.
    pub value_contract: SemanticHash,
    /// Directional producer contracts accepted by a reader.
    pub accepts: ValueAcceptance<'a>,
    /// Canonical identity of missing-value semantics, if any.
    pub default: Option<SemanticHash>,
}

/// Generic record boundary used by descriptor and evidence schemas.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecordSchema<'a> {
    /// Exact schema descriptor.
    pub descriptor: DescriptorRef<'a>,
    /// Fields in arbitrary source order.
    pub fields: &'a [RecordField<'a>],
    /// Reader behavior for unknown semantic fields.
    pub unknown_fields: UnknownFieldPolicy,
}

/// Exact migration between exact descriptor revisions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MigrationRef<'a> {
    /// Stable migration identifier.
    pub id: Id<'a>,
    /// Exact migration descriptor identity.
    pub semantic_hash: SemanticHash,
    /// Exact accepted source descriptor.
    pub source: DescriptorRef<'a>,
    /// Exact produced target descriptor.
    pub target: DescriptorRef<'a>,
    /// Same source value and context always produce the same result.
    pub deterministic: bool,
    /// Every valid source value has a defined result or typed failure.
    pub total: bool,
}

/// Assesses exact descriptor equality.
#[must_use]
pub fn assess_exact<'a>(
    left: DescriptorRef<'a>,
    right: DescriptorRef<'a>,
) -> CompatibilityDecision<'a> {
    let query = CompatibilityQuery::Exact { left, right };
    if Id::new(left.kind.as_str()).is_err() {
        return CompatibilityDecision::indeterminate(
            query,
            CompatibilityReason::InvalidSchema,
            Some(left.kind),
        );
    }
    if Id::new(right.kind.as_str()).is_err() {
        return CompatibilityDecision::indeterminate(
            query,
            CompatibilityReason::InvalidSchema,
            Some(right.kind),
        );
    }
    if left.kind != right.kind {
        return CompatibilityDecision::incompatible(
            query,
            CompatibilityReason::DescriptorKindMismatch,
            None,
        );
    }
    if left.schema_version != right.schema_version {
        return CompatibilityDecision::incompatible(
            query,
            CompatibilityReason::SchemaVersionMismatch,
            None,
        );
    }
    if left.semantic_hash != right.semantic_hash {
        return CompatibilityDecision::incompatible(
            query,
            CompatibilityReason::SemanticHashMismatch,
            None,
        );
    }
    CompatibilityDecision::compatible(
        query,
        CompatibilityClass::Exact,
        CompatibilityReason::ExactIdentity,
        None,
    )
}

/// Assesses whether every writer record can be accepted by the reader.
///
/// This conservative reference rule is suitable for descriptor and evidence
/// record schemas. Domain value compatibility remains delegated through
/// [`ValueAcceptance`].
#[must_use]
pub fn assess_reader_acceptance<'a>(
    reader: &RecordSchema<'a>,
    writer: &RecordSchema<'a>,
) -> CompatibilityDecision<'a> {
    let query = CompatibilityQuery::ReaderAcceptsWriter {
        reader: reader.descriptor,
        writer: writer.descriptor,
    };

    if let Some(subject) = invalid_schema_subject(reader).or_else(|| invalid_schema_subject(writer))
    {
        return CompatibilityDecision::indeterminate(
            query,
            CompatibilityReason::InvalidSchema,
            Some(subject),
        );
    }
    if reader.descriptor.kind != writer.descriptor.kind {
        return CompatibilityDecision::incompatible(
            query,
            CompatibilityReason::DescriptorKindMismatch,
            None,
        );
    }
    if reader.descriptor == writer.descriptor {
        return CompatibilityDecision::compatible(
            query,
            CompatibilityClass::Exact,
            CompatibilityReason::ExactIdentity,
            None,
        );
    }

    for reader_field in reader.fields {
        let Some(writer_field) = find_field(writer.fields, reader_field.id) else {
            if reader_field.required && reader_field.default.is_none() {
                return CompatibilityDecision::incompatible(
                    query,
                    CompatibilityReason::MissingRequiredField,
                    Some(reader_field.id),
                );
            }
            continue;
        };

        if reader_field.value_contract != writer_field.value_contract {
            match reader_field.accepts {
                ValueAcceptance::Exact => {
                    return CompatibilityDecision::incompatible(
                        query,
                        CompatibilityReason::ValueContractRejected,
                        Some(reader_field.id),
                    );
                }
                ValueAcceptance::ExactOr(accepted)
                    if !accepted.contains(&writer_field.value_contract) =>
                {
                    return CompatibilityDecision::incompatible(
                        query,
                        CompatibilityReason::ValueContractRejected,
                        Some(reader_field.id),
                    );
                }
                ValueAcceptance::ProviderRequired => {
                    return CompatibilityDecision::indeterminate(
                        query,
                        CompatibilityReason::ValueProviderRequired,
                        Some(reader_field.id),
                    );
                }
                ValueAcceptance::ExactOr(_) => {}
            }
        }

        if !writer_field.required {
            if reader_field.required && reader_field.default.is_none() {
                return CompatibilityDecision::incompatible(
                    query,
                    CompatibilityReason::MissingRequiredField,
                    Some(reader_field.id),
                );
            }
            if reader_field.default != writer_field.default {
                return CompatibilityDecision::incompatible(
                    query,
                    CompatibilityReason::DefaultChanged,
                    Some(reader_field.id),
                );
            }
        }
    }

    if reader.unknown_fields == UnknownFieldPolicy::Reject {
        for writer_field in writer.fields {
            if find_field(reader.fields, writer_field.id).is_none() {
                return CompatibilityDecision::incompatible(
                    query,
                    CompatibilityReason::UnknownProducerField,
                    Some(writer_field.id),
                );
            }
        }
    }

    let class = match reader
        .descriptor
        .schema_version
        .cmp(&writer.descriptor.schema_version)
    {
        core::cmp::Ordering::Greater => CompatibilityClass::BackwardCompatible,
        core::cmp::Ordering::Less => CompatibilityClass::ForwardCompatible,
        core::cmp::Ordering::Equal => CompatibilityClass::Accepted,
    };
    CompatibilityDecision::compatible(query, class, CompatibilityReason::FieldsAccepted, None)
}

/// Assesses an exact deterministic total migration.
#[must_use]
pub fn assess_migration<'a>(
    source: DescriptorRef<'a>,
    target: DescriptorRef<'a>,
    migration: MigrationRef<'a>,
) -> CompatibilityDecision<'a> {
    let query = CompatibilityQuery::Migration {
        source,
        target,
        migration: migration.semantic_hash,
    };
    for subject in [
        migration.id,
        source.kind,
        target.kind,
        migration.source.kind,
        migration.target.kind,
    ] {
        if Id::new(subject.as_str()).is_ok() {
            continue;
        }
        return CompatibilityDecision::indeterminate(
            query,
            CompatibilityReason::InvalidSchema,
            Some(subject),
        );
    }
    if migration.source != source {
        return CompatibilityDecision::incompatible(
            query,
            CompatibilityReason::MigrationSourceMismatch,
            None,
        );
    }
    if migration.target != target {
        return CompatibilityDecision::incompatible(
            query,
            CompatibilityReason::MigrationTargetMismatch,
            None,
        );
    }
    if !migration.deterministic {
        return CompatibilityDecision::incompatible(
            query,
            CompatibilityReason::MigrationNotDeterministic,
            None,
        );
    }
    if !migration.total {
        return CompatibilityDecision::incompatible(
            query,
            CompatibilityReason::MigrationNotTotal,
            None,
        );
    }

    let mut decision = CompatibilityDecision::compatible(
        query,
        CompatibilityClass::Migratable,
        CompatibilityReason::MigrationAccepted,
        None,
    );
    decision.migration = Some(migration.semantic_hash);
    decision
}

fn invalid_schema_subject<'a>(schema: &RecordSchema<'a>) -> Option<Id<'a>> {
    if Id::new(schema.descriptor.kind.as_str()).is_err() {
        return Some(schema.descriptor.kind);
    }
    for (index, field) in schema.fields.iter().enumerate() {
        if Id::new(field.id.as_str()).is_err()
            || schema.fields[..index]
                .iter()
                .any(|prior| prior.id == field.id)
        {
            return Some(field.id);
        }
    }
    None
}

fn find_field<'a>(fields: &'a [RecordField<'a>], id: Id<'_>) -> Option<&'a RecordField<'a>> {
    fields.iter().find(|field| field.id == id)
}
