//! Bounded deterministic aural projection of one exact Face revision.
//!
//! This module chooses utterance order and wording from existing Face records.
//! It does not synthesize audio, create a Show, interpret an open domain
//! identity, or grant an action authority. Those remain later Mask stages.

use alloc::{format, string::String, vec::Vec};
use sha2::{Digest, Sha256};

use conduit_core::{IntervalEndpoint, TemporalRelation, ValueConstraint};

use crate::{
    FaceUtteranceClauseKind, FaceUtteranceProvenance, Presentation, PresentationActionAvailability,
    PresentationCompositionKind, PresentationDisclosureLevel, PresentationError,
    PresentationRelationshipKind, PresentationRole, PresentationTemporalRole,
};

pub const MAX_FACE_UTTERANCE_CLAUSES: usize = 10_256;
pub const MAX_FACE_UTTERANCE_CLAUSE_BYTES: usize = 4_096;
pub const MAX_FACE_UTTERANCE_PLAN_BYTES: usize = 2 * 1024 * 1024;

/// One bounded sentence or exact Face wording item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceUtteranceClause {
    pub kind: FaceUtteranceClauseKind,
    pub text: String,
    pub provenance: FaceUtteranceProvenance,
}

/// A deterministic, digest-bound plan for speaking one Face revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceUtterancePlan {
    pub source_face_identity: String,
    pub source_face_revision: u64,
    pub clauses: Vec<FaceUtteranceClause>,
    pub encoded_bytes: usize,
    pub digest: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FaceUtterancePlanError {
    InvalidFace(PresentationError),
    TooManyClauses,
    ClauseTooLong,
    TooManyBytes,
}

impl core::fmt::Display for FaceUtterancePlanError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "cannot project Face utterance plan: {self:?}")
    }
}

/// Project all currently selected wording and encounter structure without a
/// model or domain-specific script.
pub fn plan_face_utterances(
    face: &Presentation,
) -> Result<FaceUtterancePlan, FaceUtterancePlanError> {
    face.validate()
        .map_err(FaceUtterancePlanError::InvalidFace)?;
    let subject_order = ordered_subjects(face);
    let mut builder = UtteranceBuilder::new(face);

    for identity in &subject_order {
        let subject = face
            .subjects
            .iter()
            .find(|subject| subject.identity == *identity)
            .expect("ordered identity came from this Face");
        builder.push(FaceUtteranceClause {
            kind: FaceUtteranceClauseKind::Subject,
            text: format!("{}: {}.", role_token(&subject.role), subject.name),
            provenance: FaceUtteranceProvenance::subject(subject.identity.clone())
                .expect("validated subject identity remains bounded"),
        })?;
    }

    let mut relationships = face.relationships.iter().enumerate().collect::<Vec<_>>();
    relationships.sort_by_key(|(index, relationship)| {
        (
            subject_rank(&subject_order, &relationship.source),
            subject_rank(&subject_order, &relationship.target),
            relationship_kind_token(&relationship.kind),
            *index,
        )
    });
    for (index, relationship) in relationships {
        builder.push(FaceUtteranceClause {
            kind: FaceUtteranceClauseKind::Relationship,
            text: relationship_clause(face, relationship),
            provenance: FaceUtteranceProvenance::relationship(index as u32).unwrap(),
        })?;
    }

    let mut properties = face.properties.iter().enumerate().collect::<Vec<_>>();
    properties.sort_by_key(|(index, property)| {
        (
            subject_rank(&subject_order, &property.subject),
            property.name.as_str(),
            *index,
        )
    });
    for (index, property) in properties {
        builder.push(FaceUtteranceClause {
            kind: FaceUtteranceClauseKind::Property,
            text: property_clause(face, property),
            provenance: FaceUtteranceProvenance::property(index as u32).unwrap(),
        })?;
    }

    let mut composition = face.composition.iter().collect::<Vec<_>>();
    composition.sort_by_key(|relation| {
        (
            subject_rank(&subject_order, &relation.source),
            subject_rank(&subject_order, &relation.target),
            relation.identity.as_str(),
        )
    });
    for relation in composition {
        builder.push(FaceUtteranceClause {
            kind: FaceUtteranceClauseKind::Composition,
            text: composition_clause(face, relation),
            provenance: FaceUtteranceProvenance::composition(relation.identity.clone())
                .expect("validated composition identity remains bounded"),
        })?;
    }

    let mut wording = face.text.iter().enumerate().collect::<Vec<_>>();
    wording.sort_by_key(|(index, text)| (subject_rank(&subject_order, &text.subject), *index));
    for (index, text) in wording {
        builder.push(FaceUtteranceClause {
            kind: FaceUtteranceClauseKind::Text,
            text: text.text.clone(),
            provenance: FaceUtteranceProvenance::text(index as u32).unwrap(),
        })?;
    }

    for (index, disclosure) in face.disclosures.iter().enumerate() {
        builder.push(FaceUtteranceClause {
            kind: FaceUtteranceClauseKind::Disclosure,
            text: format!(
                "{}: {}.",
                disclosure_token(disclosure.level),
                subject_name(face, &disclosure.subject)
            ),
            provenance: FaceUtteranceProvenance::disclosure(index as u32).unwrap(),
        })?;
    }

    for (index, reference) in face.temporal_references.iter().enumerate() {
        builder.push(FaceUtteranceClause {
            kind: FaceUtteranceClauseKind::TemporalReference,
            text: format!(
                "Time reference {} is tick {} on {} at {:?} scale, resolution {} ticks, uncertainty {} ticks.",
                reference.identity,
                reference.instant.ticks,
                reference.instant.clock_basis,
                reference.instant.scale,
                reference.instant.resolution_ticks,
                reference.instant.uncertainty_ticks,
            ),
            provenance: FaceUtteranceProvenance::temporal_reference(index as u32).unwrap(),
        })?;
    }

    for (index, fact) in face.temporal_facts.iter().enumerate() {
        builder.push(FaceUtteranceClause {
            kind: FaceUtteranceClauseKind::TemporalFact,
            text: format!(
                "{} {} at tick {} on {}. {} relative to {}{}.",
                subject_name(face, &fact.subject),
                temporal_role_token(fact.role),
                fact.source.ticks,
                fact.source.clock_basis,
                temporal_relation_token(fact.relation),
                fact.reference,
                fact.sign_id
                    .as_ref()
                    .map_or_else(String::new, |sign| format!("; Sign {}", sign.as_str())),
            ),
            provenance: FaceUtteranceProvenance::temporal_fact(index as u32).unwrap(),
        })?;
    }

    let mut actions = face.actions.iter().collect::<Vec<_>>();
    actions.sort_by_key(|action| {
        (
            subject_rank(&subject_order, &action.target),
            action.identity.as_str(),
        )
    });
    for action in actions {
        builder.push(FaceUtteranceClause {
            kind: FaceUtteranceClauseKind::Action,
            text: action_clause(face, action),
            provenance: FaceUtteranceProvenance::action(action.identity.clone())
                .expect("validated action identity remains bounded"),
        })?;
        for argument in &action.arguments {
            builder.push(FaceUtteranceClause {
                kind: FaceUtteranceClauseKind::ActionArgument,
                text: action_argument_clause(action, argument),
                provenance: FaceUtteranceProvenance::action_argument(
                    action.identity.clone(),
                    argument.name.clone(),
                )
                .expect("validated action argument identities remain bounded"),
            })?;
        }
    }

    promote_context_clauses(face, &mut builder.clauses);
    Ok(builder.finish())
}

/// Preserve every exact clause and provenance while placing the encounter's
/// stated Context before general structure and inspection detail.
fn promote_context_clauses(face: &Presentation, clauses: &mut Vec<FaceUtteranceClause>) {
    let is_context = |subject: &str| {
        face.disclosures.iter().any(|disclosure| {
            disclosure.subject == subject
                && disclosure.level == PresentationDisclosureLevel::Context
        })
    };
    let mut context_subjects = Vec::new();
    let mut context_wording = Vec::new();
    let mut remaining = Vec::new();
    for clause in core::mem::take(clauses) {
        match &clause.provenance {
            FaceUtteranceProvenance::Subject(source) if is_context(source.identity()) => {
                context_subjects.push(clause);
            }
            FaceUtteranceProvenance::Text(source)
                if face
                    .text
                    .get(*source.index() as usize)
                    .is_some_and(|wording| is_context(&wording.subject)) =>
            {
                context_wording.push(clause);
            }
            _ => remaining.push(clause),
        }
    }
    clauses.extend(context_subjects);
    clauses.extend(context_wording);
    clauses.extend(remaining);
}

fn ordered_subjects(face: &Presentation) -> Vec<String> {
    let mut remaining = face
        .subjects
        .iter()
        .map(|subject| subject.identity.clone())
        .collect::<Vec<_>>();
    remaining.sort();
    let mut ordered = Vec::with_capacity(remaining.len());
    while !remaining.is_empty() {
        let next = remaining
            .iter()
            .position(|candidate| {
                !remaining.iter().any(|predecessor| {
                    predecessor != candidate && precedes(face, predecessor, candidate)
                })
            })
            // A cyclic semantic order is still rendered deterministically. The
            // Face remains truthful; the Mask does not invent a way to break it.
            .unwrap_or(0);
        ordered.push(remaining.remove(next));
    }
    ordered
}

fn precedes(face: &Presentation, before: &str, after: &str) -> bool {
    face.relationships.iter().any(|relationship| {
        relationship.source == before
            && relationship.target == after
            && (matches!(
                relationship.kind,
                PresentationRelationshipKind::Contains
                    | PresentationRelationshipKind::Connects
                    | PresentationRelationshipKind::Describes
                    | PresentationRelationshipKind::Realizes
                    | PresentationRelationshipKind::Observes
            ) || matches!(
                &relationship.kind,
                PresentationRelationshipKind::Semantic(kind)
                    if kind.as_str().ends_with("/precedes")
            ))
    }) || face.composition.iter().any(|relation| match relation.kind {
        PresentationCompositionKind::Group | PresentationCompositionKind::RevealAfter => {
            relation.target == before && relation.source == after
        }
        _ => relation.source == before && relation.target == after,
    })
}

fn subject_rank(order: &[String], identity: &str) -> usize {
    order
        .iter()
        .position(|candidate| candidate == identity)
        .expect("validated Face reference")
}

fn role_token(role: &PresentationRole) -> String {
    match role {
        PresentationRole::Semantic(identity) => format!("Role {}", identity.as_str()),
        PresentationRole::Document => "Document".into(),
        PresentationRole::Body => "Body".into(),
        PresentationRole::Part => "Part".into(),
        PresentationRole::Candidate => "Candidate".into(),
        PresentationRole::Plot => "Plot".into(),
        PresentationRole::Gear => "Gear".into(),
        PresentationRole::Port => "Port".into(),
        PresentationRole::Cord => "Cord".into(),
        PresentationRole::Plan => "Plan".into(),
        PresentationRole::Play => "Play".into(),
        PresentationRole::Host => "Host".into(),
        PresentationRole::Capability => "Capability".into(),
        PresentationRole::Line => "Line".into(),
        PresentationRole::Manifestation => "Manifestation".into(),
        PresentationRole::Route => "Route".into(),
        PresentationRole::Diagnostic => "Diagnostic".into(),
        PresentationRole::Sign => "Sign".into(),
        PresentationRole::Info => "Info".into(),
        PresentationRole::Region => "Region".into(),
        PresentationRole::Collection => "Collection".into(),
        PresentationRole::Item => "Item".into(),
        PresentationRole::TextEntry => "Text entry".into(),
        PresentationRole::Status => "Status".into(),
        PresentationRole::Action => "Action".into(),
    }
}

fn disclosure_token(level: PresentationDisclosureLevel) -> &'static str {
    match level {
        PresentationDisclosureLevel::Primary => "Primary content",
        PresentationDisclosureLevel::CurrentAction => "Current action",
        PresentationDisclosureLevel::Context => "Context",
        PresentationDisclosureLevel::SelectedDetail => "Selected detail",
        PresentationDisclosureLevel::ExactProvenance => "Exact provenance",
    }
}

fn temporal_role_token(role: PresentationTemporalRole) -> &'static str {
    match role {
        PresentationTemporalRole::Event => "happened",
        PresentationTemporalRole::Observation => "was observed",
        PresentationTemporalRole::Ingestion => "was received",
    }
}

fn temporal_relation_token(relation: TemporalRelation) -> String {
    match relation {
        TemporalRelation::Past {
            minimum_ticks,
            maximum_ticks,
        } => {
            format!("Between {minimum_ticks} and {maximum_ticks} ticks before")
        }
        TemporalRelation::Present => "At the same instant as".into(),
        TemporalRelation::Future {
            minimum_ticks,
            maximum_ticks,
        } => {
            format!("Between {minimum_ticks} and {maximum_ticks} ticks after")
        }
        TemporalRelation::Indeterminate => "Overlaps in time with".into(),
    }
}

fn relationship_kind_token(kind: &PresentationRelationshipKind) -> String {
    match kind {
        PresentationRelationshipKind::Semantic(identity) => identity.as_str().into(),
        other => format!("{other:?}"),
    }
}

fn subject_name<'a>(face: &'a Presentation, identity: &str) -> &'a str {
    face.subjects
        .iter()
        .find(|subject| subject.identity == identity)
        .map(|subject| subject.name.as_str())
        .expect("validated Face subject")
}

fn relationship_clause(
    face: &Presentation,
    relationship: &crate::PresentationRelationship,
) -> String {
    let source = subject_name(face, &relationship.source);
    let target = subject_name(face, &relationship.target);
    match &relationship.kind {
        PresentationRelationshipKind::Contains => format!("{source} contains {target}."),
        PresentationRelationshipKind::Connects => format!("{source} connects to {target}."),
        PresentationRelationshipKind::Describes => format!("{source} describes {target}."),
        PresentationRelationshipKind::Realizes => format!("{source} realizes {target}."),
        PresentationRelationshipKind::Observes => format!("{source} observes {target}."),
        PresentationRelationshipKind::Semantic(identity) => format!(
            "{source} has relationship {} to {target}.",
            identity.as_str()
        ),
    }
}

fn property_clause(face: &Presentation, property: &crate::PresentationProperty) -> String {
    let subject = subject_name(face, &property.subject);
    let value = match &property.value {
        crate::PresentationPropertyValue::Identity(value) => format!("identity {value}"),
        crate::PresentationPropertyValue::BaseImplementationId(value) => {
            format!("base implementation {}", value.as_str())
        }
        crate::PresentationPropertyValue::Text(value) => format!("text {value:?}"),
        crate::PresentationPropertyValue::Count(value) => format!("count {value}"),
        crate::PresentationPropertyValue::Signed(value) => format!("signed value {value}"),
        crate::PresentationPropertyValue::Flag(value) => format!("flag {value}"),
        crate::PresentationPropertyValue::TypedValue { contract, bytes } => {
            crate::spoken_typed_value(contract, bytes)
        }
        crate::PresentationPropertyValue::ValueContract(contract) => format!(
            "a value contract of kind {} permitting at most {} bytes: {}",
            contract.value_kind.as_str(),
            contract.maximum_bytes,
            contract_constraints_clause(contract)
        ),
        crate::PresentationPropertyValue::Content(encoded) => {
            let content = conduit_core::BoundedResourceRef::validate_encoded(encoded)
                .expect("validated Face content remains canonical");
            match content.extent.items {
                Some(items) => {
                    let item_word = if items == 1 { "item" } else { "items" };
                    format!(
                        "bounded content of profile {}, access class {}, {} bytes, and {items} {item_word}",
                        content.content_profile, content.access_class, content.extent.bytes
                    )
                }
                None => format!(
                    "bounded content of profile {}, access class {}, and {} bytes",
                    content.content_profile, content.access_class, content.extent.bytes
                ),
            }
        }
    };
    format!("{subject} has property {}: {value}.", property.name)
}

fn composition_clause(
    face: &Presentation,
    relation: &crate::PresentationCompositionRelation,
) -> String {
    let source = subject_name(face, &relation.source);
    let target = subject_name(face, &relation.target);
    match &relation.kind {
        PresentationCompositionKind::Group => format!("{source} belongs with {target}."),
        PresentationCompositionKind::Contrast => format!("Contrast {source} with {target}."),
        PresentationCompositionKind::Juxtapose => {
            format!("Consider {source} and {target} together.")
        }
        PresentationCompositionKind::Emphasize => {
            format!("Emphasize {source} over {target}.")
        }
        PresentationCompositionKind::Subordinate => format!("{source} supports {target}."),
        PresentationCompositionKind::Associate => {
            format!("Consider {source} in connection with {target}.")
        }
        PresentationCompositionKind::RevealAfter => {
            format!("Encounter {source} after {target}.")
        }
        PresentationCompositionKind::Semantic(payload) => format!(
            "{source} has presentation relationship {} to {target}.",
            payload.identity()
        ),
    }
}

fn action_clause(face: &Presentation, action: &crate::PresentationAction) -> String {
    let target = subject_name(face, &action.target);
    match &action.availability {
        PresentationActionAvailability::Available => {
            format!("Available action: {}, for {target}.", action.name)
        }
        PresentationActionAvailability::Unavailable {
            reason_code,
            explanation,
        } => format!(
            "Unavailable action: {}, for {target}. Reason {reason_code}: {explanation}",
            action.name
        ),
        PresentationActionAvailability::Refused {
            reason_code,
            explanation,
        } => format!(
            "Refused action: {}, for {target}. Reason {reason_code}: {explanation}",
            action.name
        ),
    }
}

fn action_argument_clause(
    action: &crate::PresentationAction,
    argument: &crate::FaceActionArgument,
) -> String {
    format!(
        "For action {}, the argument {} has kind {} and permits at most {} bytes: {}.",
        action.name,
        argument.value_name,
        argument.contract.value_kind.as_str(),
        argument.contract.maximum_bytes,
        contract_constraints_clause(&argument.contract)
    )
}

fn contract_constraints_clause(contract: &conduit_core::CheckedValueContract) -> String {
    let constraints = contract
        .constraints
        .iter()
        .map(constraint_clause)
        .collect::<Vec<_>>()
        .join("; ");
    if constraints.is_empty() {
        "no additional value constraint".into()
    } else {
        constraints
    }
}

fn constraint_clause(constraint: &ValueConstraint) -> String {
    match constraint {
        ValueConstraint::ByteLength { minimum, maximum } => {
            format!("between {minimum} and {maximum} bytes")
        }
        ValueConstraint::UnsignedRange {
            minimum,
            maximum,
            minimum_endpoint,
            maximum_endpoint,
        } => range_clause("value", minimum.as_ref(), maximum.as_ref(), *minimum_endpoint, *maximum_endpoint),
        ValueConstraint::SignedRange {
            minimum,
            maximum,
            minimum_endpoint,
            maximum_endpoint,
        } => range_clause("value", minimum.as_ref(), maximum.as_ref(), *minimum_endpoint, *maximum_endpoint),
        ValueConstraint::FixedIntegerRange {
            minimum,
            maximum,
            minimum_endpoint,
            maximum_endpoint,
        } => range_clause(
            "fixed-width integer",
            minimum.as_ref().map(|value| format!("canonical 0x{}", hex_bytes(value))).as_ref(),
            maximum.as_ref().map(|value| format!("canonical 0x{}", hex_bytes(value))).as_ref(),
            *minimum_endpoint,
            *maximum_endpoint,
        ),
        ValueConstraint::QuantityRange {
            minimum,
            maximum,
            minimum_endpoint,
            maximum_endpoint,
        } => range_clause(
            "quantity",
            minimum.as_ref().map(|value| format!("{}e{} {}", value.coefficient(), value.exponent(), value.unit().semantic_id())).as_ref(),
            maximum.as_ref().map(|value| format!("{}e{} {}", value.coefficient(), value.exponent(), value.unit().semantic_id())).as_ref(),
            *minimum_endpoint,
            *maximum_endpoint,
        ),
        ValueConstraint::FloatFinite => "a finite IEEE-754 value".into(),
        ValueConstraint::FloatRange {
            minimum,
            maximum,
            minimum_endpoint,
            maximum_endpoint,
        } => range_clause(
            "IEEE-754 value",
            minimum.as_ref().map(|value| format!("canonical 0x{}", hex_bytes(value))).as_ref(),
            maximum.as_ref().map(|value| format!("canonical 0x{}", hex_bytes(value))).as_ref(),
            *minimum_endpoint,
            *maximum_endpoint,
        ),
        ValueConstraint::CanonicalMembership { members, negated } => format!(
            "{} the canonical values {}",
            if *negated { "none of" } else { "one of" },
            members
                .iter()
                .map(|member| canonical_member(member))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ValueConstraint::TextPattern {
            pattern,
            anchored_start,
            anchored_end,
            negated,
        } => format!(
            "{}the exact checked portable text pattern as a {} with {} states, start state {}, at most {} characters, and at most {} match steps",
            if *negated { "not " } else { "" },
            pattern_profile(*anchored_start, *anchored_end),
            pattern.states.len(),
            pattern.start_state,
            pattern.maximum_input_characters,
            pattern.maximum_match_steps,
        ),
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn pattern_profile(anchored_start: bool, anchored_end: bool) -> &'static str {
    match (anchored_start, anchored_end) {
        (false, false) => "bounded search",
        (true, false) => "prefix match",
        (false, true) => "suffix match",
        (true, true) => "whole-value match",
    }
}

fn range_clause<T: core::fmt::Display>(
    noun: &str,
    minimum: Option<&T>,
    maximum: Option<&T>,
    minimum_endpoint: IntervalEndpoint,
    maximum_endpoint: IntervalEndpoint,
) -> String {
    match (minimum, maximum) {
        (Some(minimum), Some(maximum)) => format!(
            "a {noun} from {} {minimum} through {} {maximum}",
            endpoint_word(minimum_endpoint),
            endpoint_word(maximum_endpoint)
        ),
        (Some(minimum), None) => {
            format!(
                "a {noun} from {} {minimum} with no upper semantic bound",
                endpoint_word(minimum_endpoint)
            )
        }
        (None, Some(maximum)) => {
            format!(
                "a {noun} with no lower semantic bound through {} {maximum}",
                endpoint_word(maximum_endpoint)
            )
        }
        (None, None) => format!("a {noun} with no semantic bounds"),
    }
}

fn endpoint_word(endpoint: IntervalEndpoint) -> &'static str {
    match endpoint {
        IntervalEndpoint::Inclusive => "inclusive",
        IntervalEndpoint::Exclusive => "exclusive",
    }
}

fn canonical_member(member: &[u8]) -> String {
    match core::str::from_utf8(member) {
        Ok(text) if !text.chars().any(char::is_control) => format!("{text:?}"),
        _ => {
            let mut encoded = String::with_capacity(member.len() * 2 + 2);
            encoded.push_str("0x");
            for byte in member {
                use core::fmt::Write;
                write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
            }
            encoded
        }
    }
}

struct UtteranceBuilder {
    source_face_identity: String,
    source_face_revision: u64,
    clauses: Vec<FaceUtteranceClause>,
    encoded_bytes: usize,
}

impl UtteranceBuilder {
    fn new(face: &Presentation) -> Self {
        Self {
            source_face_identity: face.identity.as_str().into(),
            source_face_revision: face.revision,
            clauses: Vec::new(),
            encoded_bytes: 0,
        }
    }

    fn push(&mut self, clause: FaceUtteranceClause) -> Result<(), FaceUtterancePlanError> {
        if self.clauses.len() == MAX_FACE_UTTERANCE_CLAUSES {
            return Err(FaceUtterancePlanError::TooManyClauses);
        }
        if clause.text.len() > MAX_FACE_UTTERANCE_CLAUSE_BYTES {
            return Err(FaceUtterancePlanError::ClauseTooLong);
        }
        let next = self
            .encoded_bytes
            .checked_add(clause.text.len())
            .and_then(|bytes| bytes.checked_add(1))
            .ok_or(FaceUtterancePlanError::TooManyBytes)?;
        if next > MAX_FACE_UTTERANCE_PLAN_BYTES {
            return Err(FaceUtterancePlanError::TooManyBytes);
        }
        self.encoded_bytes = next;
        self.clauses.push(clause);
        Ok(())
    }

    fn finish(self) -> FaceUtterancePlan {
        let mut digest = Sha256::new();
        hash_bytes(&mut digest, self.source_face_identity.as_bytes());
        digest.update(self.source_face_revision.to_le_bytes());
        for clause in &self.clauses {
            digest.update([clause.kind as u8]);
            hash_bytes(&mut digest, clause.text.as_bytes());
            hash_provenance(&mut digest, &clause.provenance);
        }
        FaceUtterancePlan {
            source_face_identity: self.source_face_identity,
            source_face_revision: self.source_face_revision,
            clauses: self.clauses,
            encoded_bytes: self.encoded_bytes,
            digest: digest.finalize().into(),
        }
    }
}

fn hash_provenance(digest: &mut Sha256, provenance: &FaceUtteranceProvenance) {
    match provenance {
        FaceUtteranceProvenance::Subject(value) => {
            digest.update([0]);
            hash_bytes(digest, value.identity().as_bytes());
        }
        FaceUtteranceProvenance::Relationship(value) => {
            digest.update([1]);
            digest.update(value.index().to_le_bytes());
        }
        FaceUtteranceProvenance::Property(value) => {
            digest.update([6]);
            digest.update(value.index().to_le_bytes());
        }
        FaceUtteranceProvenance::Composition(value) => {
            digest.update([2]);
            hash_bytes(digest, value.identity().as_bytes());
        }
        FaceUtteranceProvenance::Text(value) => {
            digest.update([3]);
            digest.update(value.index().to_le_bytes());
        }
        FaceUtteranceProvenance::Action(value) => {
            digest.update([4]);
            hash_bytes(digest, value.identity().as_bytes());
        }
        FaceUtteranceProvenance::ActionArgument(value) => {
            digest.update([5]);
            hash_bytes(digest, value.action_identity().as_bytes());
            hash_bytes(digest, value.argument_name().as_bytes());
        }
        FaceUtteranceProvenance::Disclosure(value) => {
            digest.update([7]);
            digest.update(value.index().to_le_bytes());
        }
        FaceUtteranceProvenance::TemporalReference(value) => {
            digest.update([8]);
            digest.update(value.index().to_le_bytes());
        }
        FaceUtteranceProvenance::TemporalFact(value) => {
            digest.update([9]);
            digest.update(value.index().to_le_bytes());
        }
    }
}

fn hash_bytes(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

#[cfg(test)]
mod provenance_digest_tests {
    use super::*;

    #[test]
    fn provenance_retains_the_v1_manual_digest_tags_and_field_order() {
        let mut digest = Sha256::new();
        for value in [
            FaceUtteranceProvenance::subject("subject".into()).unwrap(),
            FaceUtteranceProvenance::relationship(0x0102_0304).unwrap(),
            FaceUtteranceProvenance::property(7).unwrap(),
            FaceUtteranceProvenance::composition("composition".into()).unwrap(),
            FaceUtteranceProvenance::text(9).unwrap(),
            FaceUtteranceProvenance::action("action".into()).unwrap(),
            FaceUtteranceProvenance::action_argument("action".into(), "argument".into()).unwrap(),
        ] {
            hash_provenance(&mut digest, &value);
        }
        assert_eq!(
            <[u8; 32]>::from(digest.finalize()),
            [
                0xaf, 0xbc, 0xda, 0x8e, 0x91, 0x21, 0xa8, 0x80, 0xd8, 0xc3, 0x1d, 0xa9, 0xf9, 0x80,
                0x5d, 0x52, 0x27, 0xaf, 0x98, 0x66, 0x93, 0x37, 0x92, 0x04, 0x8e, 0xb1, 0xfb, 0x06,
                0xd4, 0x81, 0x8d, 0xa9,
            ]
        );
    }
}
