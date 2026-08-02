//! Exact temporal modality contracts for ordinary values, flows, and current observation.
//!
//! These contracts describe semantic value delivery only. They grant no mutation authority,
//! allocate no buffer, imply no item/byte/time maximum, and make no scheduler-progress claim.

use core::convert::Infallible;

use crate::{
    CanonicalDescriptor, CanonicalError, CanonicalSink, CanonicalValue, CompatibilityOutcome,
    DescriptorRef, FieldDisposition, Id, MapField, SemanticHash, TypeContractRef,
};

/// Exact current temporal-modality descriptor schema.
pub const TEMPORAL_MODALITY_SCHEMA_VERSION: u32 = 0;

/// Logical item cardinality carried by one temporal surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemporalCardinality {
    ExactlyOne,
    ZeroOrMore,
    CurrentAndReplacements,
}

impl TemporalCardinality {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExactlyOne => "exactly-one",
            Self::ZeroOrMore => "zero-or-more",
            Self::CurrentAndReplacements => "current-and-replacements",
        }
    }
}

/// Normal closing-boundary semantics, separate from resource boundedness or progress.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClosingBoundary {
    AfterValue,
    Available,
    Absent,
}

impl ClosingBoundary {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AfterValue => "after-value",
            Self::Available => "available",
            Self::Absent => "absent",
        }
    }
}

/// Whether a subscriber is promised a value immediately on observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitialAvailability {
    NotPromised,
    ImmediateCurrent,
}

impl InitialAvailability {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotPromised => "not-promised",
            Self::ImmediateCurrent => "immediate-current",
        }
    }
}

/// Retention guaranteed by the semantic surface itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModalityRetention {
    None,
    LatestReplacement,
}

impl ModalityRetention {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::LatestReplacement => "latest-replacement",
        }
    }
}

/// Replay promised to a subscriber by the semantic surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModalityReplay {
    None,
    CurrentOnly,
}

impl ModalityReplay {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::CurrentOnly => "current-only",
        }
    }
}

/// Whether a later value replaces an earlier current value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplacementBehavior {
    NoReplacement,
    ReplaceLatest,
}

impl ReplacementBehavior {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoReplacement => "no-replacement",
            Self::ReplaceLatest => "replace-latest",
        }
    }
}

/// The four published common temporal surfaces.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemporalSurface {
    Value,
    ClosingFlow,
    OpenFlow,
    Current,
}

impl TemporalSurface {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Value => "value",
            Self::ClosingFlow => "closing-flow",
            Self::OpenFlow => "open-flow",
            Self::Current => "current",
        }
    }
}

/// Explicit semantic fields produced when a temporal surface is lowered.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TemporalModalityContract<'a> {
    pub item_type: TypeContractRef<'a>,
    pub cardinality: TemporalCardinality,
    pub closing: ClosingBoundary,
    pub initial: InitialAvailability,
    pub retention: ModalityRetention,
    pub replay: ModalityReplay,
    pub replacement: ReplacementBehavior,
}

impl<'a> TemporalModalityContract<'a> {
    #[must_use]
    pub const fn value(item_type: TypeContractRef<'a>) -> Self {
        Self {
            item_type,
            cardinality: TemporalCardinality::ExactlyOne,
            closing: ClosingBoundary::AfterValue,
            initial: InitialAvailability::NotPromised,
            retention: ModalityRetention::None,
            replay: ModalityReplay::None,
            replacement: ReplacementBehavior::NoReplacement,
        }
    }

    #[must_use]
    pub const fn closing_flow(item_type: TypeContractRef<'a>) -> Self {
        Self {
            item_type,
            cardinality: TemporalCardinality::ZeroOrMore,
            closing: ClosingBoundary::Available,
            initial: InitialAvailability::NotPromised,
            retention: ModalityRetention::None,
            replay: ModalityReplay::None,
            replacement: ReplacementBehavior::NoReplacement,
        }
    }

    #[must_use]
    pub const fn open_flow(item_type: TypeContractRef<'a>) -> Self {
        Self {
            item_type,
            cardinality: TemporalCardinality::ZeroOrMore,
            closing: ClosingBoundary::Absent,
            initial: InitialAvailability::NotPromised,
            retention: ModalityRetention::None,
            replay: ModalityReplay::None,
            replacement: ReplacementBehavior::NoReplacement,
        }
    }

    #[must_use]
    pub const fn current(item_type: TypeContractRef<'a>) -> Self {
        Self {
            item_type,
            cardinality: TemporalCardinality::CurrentAndReplacements,
            closing: ClosingBoundary::Absent,
            initial: InitialAvailability::ImmediateCurrent,
            retention: ModalityRetention::LatestReplacement,
            replay: ModalityReplay::CurrentOnly,
            replacement: ReplacementBehavior::ReplaceLatest,
        }
    }

    /// Validates that the explicit fields name exactly one published surface.
    pub fn surface(self) -> Result<TemporalSurface, TemporalModalityError> {
        self.item_type
            .validate()
            .map_err(|_| TemporalModalityError::InvalidItemType)?;
        match (
            self.cardinality,
            self.closing,
            self.initial,
            self.retention,
            self.replay,
            self.replacement,
        ) {
            (
                TemporalCardinality::ExactlyOne,
                ClosingBoundary::AfterValue,
                InitialAvailability::NotPromised,
                ModalityRetention::None,
                ModalityReplay::None,
                ReplacementBehavior::NoReplacement,
            ) => Ok(TemporalSurface::Value),
            (
                TemporalCardinality::ZeroOrMore,
                ClosingBoundary::Available,
                InitialAvailability::NotPromised,
                ModalityRetention::None,
                ModalityReplay::None,
                ReplacementBehavior::NoReplacement,
            ) => Ok(TemporalSurface::ClosingFlow),
            (
                TemporalCardinality::ZeroOrMore,
                ClosingBoundary::Absent,
                InitialAvailability::NotPromised,
                ModalityRetention::None,
                ModalityReplay::None,
                ReplacementBehavior::NoReplacement,
            ) => Ok(TemporalSurface::OpenFlow),
            (
                TemporalCardinality::CurrentAndReplacements,
                ClosingBoundary::Absent,
                InitialAvailability::ImmediateCurrent,
                ModalityRetention::LatestReplacement,
                ModalityReplay::CurrentOnly,
                ReplacementBehavior::ReplaceLatest,
            ) => Ok(TemporalSurface::Current),
            _ => Err(TemporalModalityError::InvalidCombination),
        }
    }

    /// Streams the exact current descriptor without allocation.
    pub fn write_canonical<S: CanonicalSink>(
        &self,
        sink: &mut S,
    ) -> Result<(), TemporalModalityIdentityError<S::Error>> {
        self.surface()
            .map_err(TemporalModalityIdentityError::InvalidContract)?;
        let item_type_fields = [
            semantic(
                "contract_id",
                CanonicalValue::Identifier(self.item_type.contract_id),
            ),
            semantic(
                "schema_version",
                CanonicalValue::Integer(i128::from(self.item_type.schema_version)),
            ),
            semantic(
                "semantic_hash",
                CanonicalValue::Bytes(self.item_type.semantic_hash.as_bytes()),
            ),
        ];
        let fields = [
            semantic("item_type", CanonicalValue::Map(&item_type_fields)),
            semantic(
                "cardinality",
                CanonicalValue::Identifier(Id(self.cardinality.as_str())),
            ),
            semantic(
                "closing",
                CanonicalValue::Identifier(Id(self.closing.as_str())),
            ),
            semantic(
                "initial",
                CanonicalValue::Identifier(Id(self.initial.as_str())),
            ),
            semantic(
                "retention",
                CanonicalValue::Identifier(Id(self.retention.as_str())),
            ),
            semantic(
                "replay",
                CanonicalValue::Identifier(Id(self.replay.as_str())),
            ),
            semantic(
                "replacement",
                CanonicalValue::Identifier(Id(self.replacement.as_str())),
            ),
        ];
        CanonicalDescriptor {
            kind: Id("conduit/temporal-modality-contract"),
            schema_version: TEMPORAL_MODALITY_SCHEMA_VERSION,
            body: CanonicalValue::Map(&fields),
        }
        .write_canonical(sink)
        .map_err(TemporalModalityIdentityError::Canonical)
    }

    /// Computes the exact identity of all modality fields and the item type.
    pub fn semantic_hash(&self) -> Result<SemanticHash, TemporalModalityIdentityError<Infallible>> {
        let mut sink = ModalityHashSink::new();
        self.write_canonical(&mut sink)?;
        Ok(sink.finish())
    }
}

/// Invalid modality descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemporalModalityError {
    InvalidItemType,
    InvalidCombination,
}

impl TemporalModalityError {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidItemType => "temporal-modality-invalid-item-type",
            Self::InvalidCombination => "temporal-modality-invalid-combination",
        }
    }
}

/// Canonical identity construction error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemporalModalityIdentityError<E> {
    InvalidContract(TemporalModalityError),
    Canonical(CanonicalError<E>),
}

/// Stable reason for exact temporal-modality compatibility.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemporalModalityCompatibilityReason {
    Exact,
    InvalidRequired,
    InvalidCandidate,
    ItemTypeMismatch,
    CardinalityMismatch,
    ClosingMismatch,
    InitialAvailabilityMismatch,
    RetentionMismatch,
    ReplayMismatch,
    ReplacementMismatch,
}

/// Exact temporal-modality compatibility decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TemporalModalityCompatibility {
    pub outcome: CompatibilityOutcome,
    pub reason: TemporalModalityCompatibilityReason,
}

/// Compares temporal modalities without inserting a conversion or consulting a provider.
#[must_use]
pub fn assess_temporal_modality_exact(
    required: TemporalModalityContract<'_>,
    candidate: TemporalModalityContract<'_>,
) -> TemporalModalityCompatibility {
    if required.surface().is_err() {
        return incompatible(TemporalModalityCompatibilityReason::InvalidRequired);
    }
    if candidate.surface().is_err() {
        return incompatible(TemporalModalityCompatibilityReason::InvalidCandidate);
    }
    let reason = if required.item_type != candidate.item_type {
        TemporalModalityCompatibilityReason::ItemTypeMismatch
    } else if required.cardinality != candidate.cardinality {
        TemporalModalityCompatibilityReason::CardinalityMismatch
    } else if required.closing != candidate.closing {
        TemporalModalityCompatibilityReason::ClosingMismatch
    } else if required.initial != candidate.initial {
        TemporalModalityCompatibilityReason::InitialAvailabilityMismatch
    } else if required.retention != candidate.retention {
        TemporalModalityCompatibilityReason::RetentionMismatch
    } else if required.replay != candidate.replay {
        TemporalModalityCompatibilityReason::ReplayMismatch
    } else if required.replacement != candidate.replacement {
        TemporalModalityCompatibilityReason::ReplacementMismatch
    } else {
        return TemporalModalityCompatibility {
            outcome: CompatibilityOutcome::Compatible,
            reason: TemporalModalityCompatibilityReason::Exact,
        };
    };
    incompatible(reason)
}

/// Explicit set of temporal surfaces one pure node may lawfully preserve.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiftedSurfaces {
    pub value: bool,
    pub closing_flow: bool,
    pub open_flow: bool,
    pub current: bool,
}

impl LiftedSurfaces {
    /// No surface is admitted. This is invalid for a published lift contract.
    pub const NONE: Self = Self {
        value: false,
        closing_flow: false,
        open_flow: false,
        current: false,
    };

    /// Every common temporal surface is admitted.
    pub const ALL: Self = Self {
        value: true,
        closing_flow: true,
        open_flow: true,
        current: true,
    };

    #[must_use]
    pub const fn admits(self, surface: TemporalSurface) -> bool {
        match surface {
            TemporalSurface::Value => self.value,
            TemporalSurface::ClosingFlow => self.closing_flow,
            TemporalSurface::OpenFlow => self.open_flow,
            TemporalSurface::Current => self.current,
        }
    }

    #[must_use]
    const fn any(self) -> bool {
        self.value || self.closing_flow || self.open_flow || self.current
    }
}

/// Explicit proof-bearing declaration that one pure semantic node lawfully
/// maps admitted modalities itemwise while preserving their temporal fields.
///
/// The proof references are semantic inputs. Their presence never grants
/// effects or authority and is not inferred from implementation code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModalityLiftContract<'a> {
    pub node_contract: DescriptorRef<'a>,
    pub receiving_port: Id<'a>,
    pub outgoing_port: Id<'a>,
    pub input_type: TypeContractRef<'a>,
    pub output_type: TypeContractRef<'a>,
    pub admitted: LiftedSurfaces,
    pub purity_proof: DescriptorRef<'a>,
    pub law_proof: DescriptorRef<'a>,
}

impl ModalityLiftContract<'_> {
    /// Validates complete semantic proof references and named endpoints.
    pub fn validate(self) -> Result<(), ModalityLiftContractError> {
        if self.node_contract.kind != Id("conduit/node-contract")
            || self.node_contract.schema_version != 0
        {
            return Err(ModalityLiftContractError::InvalidNodeContract);
        }
        if Id::new(self.receiving_port.as_str()).is_err()
            || Id::new(self.outgoing_port.as_str()).is_err()
        {
            return Err(ModalityLiftContractError::InvalidPort);
        }
        if self.input_type.validate().is_err() || self.output_type.validate().is_err() {
            return Err(ModalityLiftContractError::InvalidType);
        }
        if !valid_namespaced_descriptor(self.purity_proof) {
            return Err(ModalityLiftContractError::InvalidPurityProof);
        }
        if !valid_namespaced_descriptor(self.law_proof) {
            return Err(ModalityLiftContractError::InvalidLawProof);
        }
        if !self.admitted.any() {
            return Err(ModalityLiftContractError::NoAdmittedSurface);
        }
        Ok(())
    }

    /// Computes the exact identity of the node, endpoints, types, admitted
    /// surfaces, and proof identities.
    pub fn semantic_hash(&self) -> Result<SemanticHash, ModalityLiftIdentityError<Infallible>> {
        self.validate()
            .map_err(ModalityLiftIdentityError::InvalidContract)?;
        let mut sink = ModalityHashSink::new();
        self.write_canonical(&mut sink)?;
        Ok(sink.finish())
    }

    /// Streams the exact current lift descriptor without allocation.
    pub fn write_canonical<S: CanonicalSink>(
        &self,
        sink: &mut S,
    ) -> Result<(), ModalityLiftIdentityError<S::Error>> {
        self.validate()
            .map_err(ModalityLiftIdentityError::InvalidContract)?;
        let node_fields = descriptor_fields(&self.node_contract);
        let input_fields = type_fields(&self.input_type);
        let output_fields = type_fields(&self.output_type);
        let purity_fields = descriptor_fields(&self.purity_proof);
        let law_fields = descriptor_fields(&self.law_proof);
        let admitted_fields = [
            semantic("value", CanonicalValue::Boolean(self.admitted.value)),
            semantic(
                "closing_flow",
                CanonicalValue::Boolean(self.admitted.closing_flow),
            ),
            semantic(
                "open_flow",
                CanonicalValue::Boolean(self.admitted.open_flow),
            ),
            semantic("current", CanonicalValue::Boolean(self.admitted.current)),
        ];
        let fields = [
            semantic("node_contract", CanonicalValue::Map(&node_fields)),
            semantic(
                "receiving_port",
                CanonicalValue::Identifier(self.receiving_port),
            ),
            semantic(
                "outgoing_port",
                CanonicalValue::Identifier(self.outgoing_port),
            ),
            semantic("input_type", CanonicalValue::Map(&input_fields)),
            semantic("output_type", CanonicalValue::Map(&output_fields)),
            semantic("admitted", CanonicalValue::Map(&admitted_fields)),
            semantic("purity_proof", CanonicalValue::Map(&purity_fields)),
            semantic("law_proof", CanonicalValue::Map(&law_fields)),
        ];
        CanonicalDescriptor {
            kind: Id("conduit/modality-lift-contract"),
            schema_version: 0,
            body: CanonicalValue::Map(&fields),
        }
        .write_canonical(sink)
        .map_err(ModalityLiftIdentityError::Canonical)
    }
}

/// Invalid explicit modality-lift declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModalityLiftContractError {
    InvalidNodeContract,
    InvalidPort,
    InvalidType,
    InvalidPurityProof,
    InvalidLawProof,
    NoAdmittedSurface,
}

impl ModalityLiftContractError {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidNodeContract => "modality-lift-invalid-node-contract",
            Self::InvalidPort => "modality-lift-invalid-port",
            Self::InvalidType => "modality-lift-invalid-type",
            Self::InvalidPurityProof => "modality-lift-invalid-purity-proof",
            Self::InvalidLawProof => "modality-lift-invalid-law-proof",
            Self::NoAdmittedSurface => "modality-lift-no-admitted-surface",
        }
    }
}

/// Canonical identity construction failure for an explicit lift contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModalityLiftIdentityError<E> {
    InvalidContract(ModalityLiftContractError),
    Canonical(CanonicalError<E>),
}

/// Failure to apply an exact, already-declared modality lift.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModalityLiftError {
    InvalidContract(ModalityLiftContractError),
    InvalidInput(TemporalModalityError),
    InputTypeMismatch,
    SurfaceNotAdmitted(TemporalSurface),
}

/// Applies one explicit lawful lift, changing only the item type.
///
/// This function does not execute the node, pair flows, allocate state, or
/// infer purity. The caller must supply the exact declared lift contract.
pub fn lift_temporal_modality<'a>(
    contract: ModalityLiftContract<'a>,
    input: TemporalModalityContract<'a>,
) -> Result<TemporalModalityContract<'a>, ModalityLiftError> {
    contract
        .validate()
        .map_err(ModalityLiftError::InvalidContract)?;
    let surface = input.surface().map_err(ModalityLiftError::InvalidInput)?;
    if input.item_type != contract.input_type {
        return Err(ModalityLiftError::InputTypeMismatch);
    }
    if !contract.admitted.admits(surface) {
        return Err(ModalityLiftError::SurfaceNotAdmitted(surface));
    }
    Ok(TemporalModalityContract {
        item_type: contract.output_type,
        ..input
    })
}

fn valid_namespaced_descriptor(descriptor: DescriptorRef<'_>) -> bool {
    Id::new(descriptor.kind.as_str()).is_ok() && descriptor.kind.as_str().contains('/')
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

fn type_fields<'a>(reference: &'a TypeContractRef<'a>) -> [MapField<'a>; 3] {
    [
        semantic(
            "contract_id",
            CanonicalValue::Identifier(reference.contract_id),
        ),
        semantic(
            "schema_version",
            CanonicalValue::Integer(i128::from(reference.schema_version)),
        ),
        semantic(
            "semantic_hash",
            CanonicalValue::Bytes(reference.semantic_hash.as_bytes()),
        ),
    ]
}

const fn incompatible(
    reason: TemporalModalityCompatibilityReason,
) -> TemporalModalityCompatibility {
    TemporalModalityCompatibility {
        outcome: CompatibilityOutcome::Incompatible,
        reason,
    }
}

fn semantic<'a>(name: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    }
}

struct ModalityHashSink(sha2::Sha256);

impl ModalityHashSink {
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

impl CanonicalSink for ModalityHashSink {
    type Error = Infallible;

    fn write(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        use sha2::Digest as _;

        self.0.update(bytes);
        Ok(())
    }
}
