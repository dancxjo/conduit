//! Standard, unconfigured interactive Scene accounting. These requested-backing
//! bounds exclude allocator overhead, stack and external Mask/kernel/Show owners.
use super::*;
use conduit_face::FaceRole;
use core::mem::size_of;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StandardNativeSceneBounds {
    /// Complete retained Scene plus maximum local argument draft and notice.
    pub retained_bytes: usize,
    /// Scene construction, including semantic document and layout temporaries.
    pub preparation_peak_bytes: usize,
    /// Additional requested backing during one input or local repaint.
    pub input_transient_bytes: usize,
    pub frame_transient_bytes: usize,
}
impl StandardNativeSceneBounds {
    /// One lease can cover construction then every retained-input/frame peak.
    pub fn lifetime_peak_bytes(&self) -> Option<usize> {
        Some(
            self.preparation_peak_bytes.max(
                self.retained_bytes
                    .checked_add(self.input_transient_bytes.max(self.frame_transient_bytes))?,
            ),
        )
    }
}
fn add(a: usize, b: usize) -> Result<usize, FaceSceneError> {
    a.checked_add(b).ok_or(FaceSceneError::DocumentBound)
}
fn mul(a: usize, b: usize) -> Result<usize, FaceSceneError> {
    a.checked_mul(b).ok_or(FaceSceneError::DocumentBound)
}
// Count the existing aural/document templates from borrowed semantic fields.
// This is an upper-bound visitor, not another formatter or render model. All
// source fields contribute per occurrence; no clause owns a whole Face clone.
fn subject_name_bytes(face: &Face, identity: &str) -> Result<usize, FaceSceneError> {
    face.subjects
        .iter()
        .find(|s| s.identity == identity)
        .map(|s| s.name.len())
        .ok_or(FaceSceneError::InvalidFace)
}
fn contract_text_bytes(
    contract: &conduit_core::CheckedValueContract,
) -> Result<usize, FaceSceneError> {
    // constraints.rs: fixed wording/numbers, <=6-byte Debug escapes, <=2-byte
    // hex expansion and joined clauses. Owned constraint slots/bytes contribute
    // once here; 256 covers the kind/maximum/empty-constraint wording.
    add(
        add(
            mul(
                contract
                    .retained_backing_bytes()
                    .ok_or(FaceSceneError::DocumentBound)?,
                8,
            )?,
            mul(contract.constraints.len(), 512)?,
        )?,
        256,
    )
}
fn typed_text_bytes(
    contract: &conduit_core::CheckedValueContract,
) -> Result<usize, FaceSceneError> {
    // typed_value.rs: decimal expansion <=256 places plus coefficient, built-in
    // Unit spelling (UNIT_MAX_SOURCE_BYTES=128), or kind+byte-count fallback.
    add(contract.value_kind.as_str().len(), 512)
}
fn document_text_bytes(face: &Face) -> Result<(usize, usize), FaceSceneError> {
    use conduit_face::{FaceActionAvailability as Availability, FacePropertyValue as Value};
    let (mut primary, mut details) = (0, 0);
    for subject in &face.subjects {
        let role = match &subject.role {
            FaceRole::Semantic(id) => add(id.as_str().len(), 5)?,
            _ => 16,
        };
        details = add(details, add(add(role, subject.name.len())?, 3)?)?;
        // Heading, including the optional Item completion marker. Omissions
        // from disclosure/named fields only decrease this bound.
        primary = add(primary, add(subject.name.len(), "Incomplete · ".len())?)?;
        if matches!(&subject.role, FaceRole::Semantic(id) if id.as_str() == conduit_data::MEASUREMENT_PLOT_FACE_KIND)
        {
            // Existing four-series labels: names<=64, three u64 counts<=20,
            // fixed separators plus two axis labels and joining newlines.
            primary = add(primary, 2 * 80 + 4 * (64 + 3 * 20 + 40) + 5)?;
        }
    }
    for relation in &face.relationships {
        let semantic = match &relation.kind {
            conduit_face::FaceRelationshipKind::Semantic(id) => id.as_str().len(),
            _ => 0,
        };
        details = add(
            details,
            add(
                add(
                    subject_name_bytes(face, &relation.source)?,
                    subject_name_bytes(face, &relation.target)?,
                )?,
                add(semantic, 32)?,
            )?,
        )?;
    }
    for relation in &face.composition {
        let semantic = match &relation.kind {
            conduit_face::FaceCompositionKind::Semantic(value) => value.identity().len(),
            _ => 0,
        };
        details = add(
            details,
            add(
                add(
                    subject_name_bytes(face, &relation.source)?,
                    subject_name_bytes(face, &relation.target)?,
                )?,
                add(semantic, 64)?,
            )?,
        )?;
    }
    for property in &face.properties {
        let value = match &property.value {
            Value::Identity(value) => add(value.len(), 9)?,
            Value::BaseImplementationId(value) => add(value.as_str().len(), 20)?,
            Value::Text(value) => add(mul(value.len(), 6)?, 7)?,
            Value::Count(_) => 26,
            Value::Signed(_) => 34,
            Value::Flag(_) => 10,
            Value::TypedValue { contract, .. } => typed_text_bytes(contract)?,
            Value::ValueContract(contract) => contract_text_bytes(contract)?,
            Value::Content(bytes) => add(mul(bytes.len(), 6)?, 128)?,
        };
        details = add(
            details,
            add(
                subject_name_bytes(face, &property.subject)?,
                add(property.name.len(), add(value, 32)?)?,
            )?,
        )?;
        if let Value::TypedValue { contract, .. } = &property.value {
            // Ordinary typed Info uses two rows; other roles one joined row.
            // Current-value and chart-series suppression can only reduce it.
            primary = add(
                primary,
                add(property.name.len(), add(typed_text_bytes(contract)?, 2)?)?,
            )?;
        }
    }
    for text in &face.text {
        primary = add(primary, text.text.len())?;
        details = add(details, text.text.len())?;
    }
    for disclosure in &face.disclosures {
        details = add(
            details,
            add(subject_name_bytes(face, &disclosure.subject)?, 24)?,
        )?;
    }
    // These independent aural clauses have the existing 4096-byte admission
    // limit. Keep that limit for temporal wording, not every other clause.
    details = add(
        details,
        mul(
            add(face.temporal_references.len(), face.temporal_facts.len())?,
            conduit_face::MAX_FACE_UTTERANCE_CLAUSE_BYTES,
        )?,
    )?;
    for action in &face.actions {
        let target = subject_name_bytes(face, &action.target)?;
        let reason = match &action.availability {
            Availability::Unavailable {
                reason_code,
                explanation,
            }
            | Availability::Refused {
                reason_code,
                explanation,
            } => add(reason_code.len(), explanation.len())?,
            Availability::Available => 0,
        };
        details = add(
            details,
            add(add(action.name.len(), target)?, add(reason, 64)?)?,
        )?;
        primary = add(
            primary,
            add(action.name.len().max(target), add(reason, 32)?)?,
        )?;
        let current = face
            .properties
            .iter()
            .filter(|p| p.subject == action.target && p.name == "current-value")
            .filter_map(|p| match &p.value {
                Value::TypedValue { bytes, .. } => Some(bytes.len()),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        for argument in &action.arguments {
            details = add(
                details,
                add(
                    add(action.name.len(), argument.value_name.len())?,
                    add(
                        argument.contract.value_kind.as_str().len(),
                        add(contract_text_bytes(&argument.contract)?, 128)?,
                    )?,
                )?,
            )?;
            // Existing argument hint contains <=128 choice bytes, 7 separators,
            // or fixed Boolean/entry wording. A current field instead prefixes
            // its retained value with the actual target name and edit hint.
            primary = add(
                primary,
                add(argument.value_name.len(), 160)?.max(add(target, add(current.max(7), 32)?)?),
            )?;
        }
    }
    Ok((primary, details))
}
impl NativeFaceScene {
    /// Pure checked arithmetic, not semantic Face validation/readiness. Qualifies only `NativeFaceScene::prepare` with
    /// its borrowed default keymap and no configured Look/local preference API.
    /// Graph navigation is refused explicitly until its separate projection is
    /// qualified; an unsupported graph is never silently made input-less.
    pub fn standard_interactive_memory_bounds(
        face: &Face,
    ) -> Result<StandardNativeSceneBounds, FaceSceneError> {
        if face
            .subjects
            .iter()
            .any(|s| matches!(s.role, FaceRole::Gear | FaceRole::Port | FaceRole::Cord))
        {
            return Err(FaceSceneError::UnsupportedInput);
        }
        let face_bytes = face
            .retained_backing_bytes()
            .ok_or(FaceSceneError::DocumentBound)?;
        let mut clauses = 0;
        for count in [
            face.subjects.len(),
            face.relationships.len(),
            face.properties.len(),
            face.composition.len(),
            face.text.len(),
            face.disclosures.len(),
            face.temporal_references.len(),
            face.temporal_facts.len(),
            face.actions.len(),
        ] {
            clauses = add(clauses, count)?;
        }
        let mut arguments = 0;
        let mut maximum_arguments = 0;
        let mut maximum_value_bytes = 0;
        for action in &face.actions {
            arguments = add(arguments, action.arguments.len())?;
            maximum_arguments = maximum_arguments.max(action.arguments.len());
            let mut action_values = 0;
            for argument in &action.arguments {
                action_values = add(
                    action_values,
                    (argument.contract.maximum_bytes as usize)
                        .min(conduit_face::MAX_FACE_INTERACTION_BYTES),
                )?;
            }
            maximum_value_bytes = maximum_value_bytes
                .max(action_values.min(conduit_face::MAX_FACE_INTERACTION_BYTES));
        }
        clauses = add(clauses, arguments)?;
        if clauses > conduit_face::MAX_FACE_UTTERANCE_CLAUSES {
            return Err(FaceSceneError::DocumentBound);
        }
        let items = mul(clauses.max(1), 2)?;
        let (primary_text, details_text) = document_text_bytes(face)?;
        // Successful layout has independent document/row hard limits. Every
        // row consumes >=1 source UTF-8 byte. Count the TWO documents separately
        // instead of projecting a worst-case entire Face into every clause.
        let row_bytes = |text: usize| {
            mul(
                mul(
                    text.min(layout::MAX_DOCUMENT_BYTES)
                        .clamp(4, layout::MAX_ROWS),
                    2,
                )?,
                size_of::<layout::Row>(),
            )
        };
        let primary_rows = row_bytes(primary_text)?;
        let details_rows = row_bytes(details_text)?;
        let documents = add(
            add(primary_rows, details_rows)?,
            add(
                primary_text.min(layout::MAX_DOCUMENT_BYTES),
                details_text.min(layout::MAX_DOCUMENT_BYTES),
            )?,
        )?;
        let values_table = mul(maximum_arguments, size_of::<Option<Vec<u8>>>())?;
        // At most one active action; its successful total bytes <=8192. Each
        // byte Vec can retain geometric spare capacity/minimum allocation.
        let draft = add(mul(maximum_value_bytes, 2)?, mul(maximum_arguments, 8)?)?;
        let retained = add(
            add(add(face_bytes, documents)?, add(values_table, draft)?)?,
            96,
        )?;
        let frame = add(
            mul(layout::MAX_PAGE_ROWS, size_of::<FaceHit<'static>>())?,
            // Page marker, bounded preview candidate and copied notice/footer.
            add(mul(conduit_face::MAX_GRAPHICS_TEXT_BYTES, 4)?, 96)?,
        )?;
        let chart = if face.properties.iter().any(measurement_chart::is_series) {
            measurement_chart::transient_backing_bound().ok_or(FaceSceneError::DocumentBound)?
        } else {
            0
        };
        let frame = add(frame, chart)?;
        let editing = mul(
            add(
                maximum_value_bytes,
                conduit_human::KEYMAP_MAXIMUM_TEXT_BYTES,
            )?,
            3,
        )?;
        // Argument assembly clones admitted draft bytes; action names/types and
        // focus wishes are drawn from the existing exact Face. Correlation and
        // Show ownership belong to the caller's separate Mask envelope.
        let submitted = add(
            add(
                mul(face_bytes, 2)?,
                mul(conduit_face::MAX_FACE_INTERACTION_BYTES, 3)?,
            )?,
            add(
                mul(
                    maximum_arguments,
                    size_of::<conduit_face::FaceInteractionArgument>(),
                )?,
                size_of::<conduit_face::FaceInteraction>(),
            )?,
        )?;
        // The new occurrence also clones the acknowledged Show identifier and
        // derives fixed-size content IDs; all are within Face identifier limits.
        let submitted = add(submitted, mul(conduit_face::MAX_FACE_ID_BYTES, 8)?)?;
        let input = add(frame, add(editing, submitted)?)?;
        // Aural sorts/promotes complete clause objects while each clause owns
        // only its own text/provenance. Source IDs may repeat in provenance;
        // bound two <=256-byte IDs per clause, independently of Face size.
        let clause_objects = mul(
            clauses.max(1),
            size_of::<conduit_face::FaceUtteranceClause>(),
        )?;
        let utterance = add(
            mul(clause_objects, 6)?,
            add(
                mul(details_text, 2)?,
                mul(clauses, 2 * conduit_face::MAX_FACE_ID_BYTES)?,
            )?,
        )?;
        let document_items = mul(mul(items, 4)?, size_of::<document::Item>())?;
        let document_strings = add(mul(primary_text, 2)?, details_text)?;
        // Exactly one current formatting expression at a time in each phase,
        // including a rejected oversized clause, quoted/hex source values and
        // contract wording intermediates. No items*Face ownership cross-product.
        let formatting_scratch = add(
            mul(face_bytes, 8)?,
            conduit_face::MAX_FACE_UTTERANCE_CLAUSE_BYTES,
        )?;
        let preparation = add(
            retained,
            add(
                utterance,
                add(
                    document_items,
                    add(
                        document_strings,
                        add(
                            mul(formatting_scratch, 2)?,
                            add(primary_rows.max(details_rows), frame)?,
                        )?,
                    )?,
                )?,
            )?,
        )?;
        Ok(StandardNativeSceneBounds {
            retained_bytes: retained,
            preparation_peak_bytes: preparation,
            input_transient_bytes: input,
            frame_transient_bytes: frame,
        })
    }

    /// Actual retained requested backing, independently useful for checking a
    /// qualified envelope. Refuses GraphReading/custom Look/preference state.
    pub fn standard_interactive_retained_backing_bytes(&self) -> Option<usize> {
        if self.graph_reading.is_some()
            || self.look_corners.is_some()
            || self.local_creche_preferences
        {
            return None;
        }
        let mut bytes = self.face.retained_backing_bytes()?;
        for rows in [&self.primary, &self.details] {
            bytes = bytes.checked_add(rows.capacity().checked_mul(size_of::<layout::Row>())?)?;
            for row in rows {
                bytes = bytes.checked_add(row.text.capacity())?;
            }
        }
        bytes = bytes.checked_add(self.input.retained_backing_bytes()?)?;
        bytes.checked_add(self.local_notice.as_ref().map_or(0, String::capacity))
    }
}

#[cfg(test)]
#[path = "interactive_backing/tests.rs"]
mod tests;
