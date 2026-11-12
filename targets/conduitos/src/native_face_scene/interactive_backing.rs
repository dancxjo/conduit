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
        // One semantic clause per item above. Primary may contain subject text,
        // properties and action/argument rows; Details contains all clauses.
        // All primary item families are subsets of this same count.
        let items = mul(clauses.max(1), 2)?;
        // A primary field may include source name/current value rather than
        // exactly one spoken clause. Eight source backings conservatively cover
        // source UTF-8 plus worst Debug escaping (six ASCII bytes per byte),
        // canonical numeric/unit wording and separators. The existing layout
        // independently refuses each document over MAX_DOCUMENT_BYTES.
        let item_text = add(
            mul(face_bytes, 8)?,
            conduit_face::MAX_FACE_UTTERANCE_CLAUSE_BYTES,
        )?;
        let document_text = mul(items, item_text)?.min(layout::MAX_DOCUMENT_BYTES);
        // Each output row consumes >=1 original UTF-8 byte, with existing
        // independent layout MAX_ROWS and MAX_DOCUMENT_BYTES hard refusals.
        let rows = document_text.min(layout::MAX_ROWS);
        // Existing growable rows use doubling; charge old+new allocation at a
        // growth boundary separately from retained capacity (two documents).
        let retained_rows = mul(mul(rows.max(4), 2)?, size_of::<layout::Row>())?;
        let documents = mul(add(retained_rows, document_text)?, 2)?;
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
        // Aural planning has bounded clause text, clause/provenance objects and
        // sort/promote tables. Document construction clones clauses and may
        // format source values before layout rejects an oversized document.
        // Each item formatting uses no more than eight complete Face backings
        // plus its bounded clause text; retain this conservative per-item term.
        let clause_objects = mul(items, size_of::<conduit_face::FaceUtteranceClause>())?;
        let utterance = add(
            mul(clause_objects, 6)?,
            add(mul(document_text, 2)?, mul(mul(items, face_bytes)?, 8)?)?,
        )?;
        let document_items = mul(mul(items, 4)?, size_of::<document::Item>())?;
        let formatting = mul(
            items,
            add(
                mul(face_bytes, 8)?,
                conduit_face::MAX_FACE_UTTERANCE_CLAUSE_BYTES,
            )?,
        )?;
        let preparation = add(
            retained,
            add(
                utterance,
                add(document_items, add(formatting, add(retained_rows, frame)?)?)?,
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
mod tests {
    use super::*;
    #[test]
    fn ordinary_editing_rows_and_maximum_draft_fit_qualified_retained_bound() {
        let face = super::super::tests::face(1);
        let bounds = NativeFaceScene::standard_interactive_memory_bounds(&face).unwrap();
        let show = super::super::tests::fixture::show(&face);
        let mut scene = NativeFaceScene::prepare(face, 640, 480).unwrap();
        assert!(
            scene.standard_interactive_retained_backing_bytes().unwrap() <= bounds.retained_bytes
        );
        // The exact Source fixture permits 64 bytes on one active argument.
        // Exercise that ordinary retained state, plus the complete local notice.

        scene
            .focus_named(&FaceFocusRequest {
                action_id: "edit".into(),
                argument_name: Some("value".into()),
            })
            .unwrap();
        let key = conduit_human::KeyEvent::new(
            4,
            conduit_human::KeyTransition::Pressed,
            conduit_human::KeyModifiers::NONE,
        )
        .unwrap();
        for sequence in 1..=64 {
            assert!(matches!(
                scene.key(key, &show, sequence).unwrap(),
                FaceSceneInput::Changed
            ));
        }
        assert!(matches!(
            scene.key(key, &show, 65),
            Err(FaceSceneError::InputBound)
        ));
        scene.set_local_notice("A bounded local notice").unwrap();
        assert!(
            scene.standard_interactive_retained_backing_bytes().unwrap() <= bounds.retained_bytes
        );
        assert!(
            scene.frame().unwrap().retained_backing_bytes().unwrap()
                <= bounds.frame_transient_bytes
        );
        assert!(bounds.preparation_peak_bytes >= bounds.retained_bytes);
        let mut graph = scene.face().clone();
        graph.subjects[0].role = FaceRole::Gear;
        // Use the same semantic constructor so the refusal is for genuine graph
        // projection demand rather than a stale content identity.
        let graph = Face::new_with_semantics(
            graph.revision,
            graph.basis,
            graph.subjects,
            graph.relationships,
            graph.properties,
            graph.text,
            graph.actions,
            graph.disclosures,
        )
        .unwrap();
        assert_eq!(
            NativeFaceScene::standard_interactive_memory_bounds(&graph),
            Err(FaceSceneError::UnsupportedInput)
        );
    }
}
