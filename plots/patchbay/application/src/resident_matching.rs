//! Complete contribution comparison using the same emitter in finite chunks.
use super::{graph_face, PatchbayApplicationPort};
use alloc::{format, vec::Vec};
use conduit_body::BodyId;
use conduit_face::{
    Face, FaceContributionBasis, FaceFragment, FaceFragmentError, FaceRelationshipKind,
};

impl PatchbayApplicationPort {
    /// Compare authoritative projection without retaining a second whole graph.
    /// The caller still validates the complete Face and acknowledged Show.
    pub fn matches_face_contribution(
        &self,
        face: &Face,
        basis: FaceContributionBasis,
        body: &BodyId,
        actions_admitted: bool,
    ) -> bool {
        self.check_contribution(Some(face), basis, body, actions_admitted)
            .unwrap_or(false)
    }
    pub(super) fn validate_projection_bounds(
        &self,
        basis: FaceContributionBasis,
        body: &BodyId,
    ) -> Result<(), super::PatchbayFaceProjectionError> {
        self.check_contribution(None, basis, body, true).map(|_| ())
    }
    fn check_contribution(
        &self,
        face: Option<&Face>,
        basis: FaceContributionBasis,
        body: &BodyId,
        actions_admitted: bool,
    ) -> Result<bool, super::PatchbayFaceProjectionError> {
        let base = self.face_fragment_with_graph(basis, body, actions_admitted, false)?;
        if face.is_some_and(|face| !contains(face, &base)) {
            return Ok(false);
        }
        let Some(index) = self.selected_plot else {
            return Ok(true);
        };
        let mut counts = [
            base.relationships.len(),
            base.properties.len(),
            base.text.len(),
            base.disclosures.len(),
        ];
        let mut bounds_error = None;
        let mut checked_subjects = 0;
        let body = format!("body/{}", body.as_str());
        // Source subjects already belong to the observation. Seeding these
        // preserves the shared emitter's existing subject/disclosure decision.
        let mut subjects = base.subjects.clone();
        let mut relationships = Vec::new();
        let mut properties = Vec::new();
        let mut texts = Vec::new();
        let mut disclosures = Vec::new();
        let matches = graph_face::append_checkpointed(
            &self.plots[index].graph,
            &body,
            &mut graph_face::Content {
                subjects: &mut subjects,
                relationships: &mut relationships,
                properties: &mut properties,
                texts: &mut texts,
                disclosures: &mut disclosures,
            },
            |content| {
                let planned = |identity: &str| {
                    self.plots.iter().any(|plot| {
                        plot.plan_id.is_some()
                            && identity.strip_prefix("plot/")
                                == Some(plot.graph.checked_plot_id.as_str())
                    })
                };
                let before_filter = content.subjects.len();
                content
                    .subjects
                    .retain(|subject| !planned(&subject.identity));
                // append_checkpointed appends subjects and never changes prior
                // entries: after its initial plot checkpoint, named Gear,
                // Composition, Port and Cord emitters only append gear/, port/
                // or cord/ identities (or reuse an unchanged existing entry).
                // Consequently planned plot/ filtering can remove entries only
                // at the first checkpoint. Fail closed if that contract changes.
                if checked_subjects != 0 && before_filter != content.subjects.len() {
                    bounds_error = Some(FaceFragmentError::InvalidIdentity);
                    return false;
                }
                content
                    .disclosures
                    .retain(|disclosure| !planned(&disclosure.subject));
                content.relationships.retain(|relation| {
                    !(relation.source == body
                        && relation.kind == FaceRelationshipKind::Contains
                        && self.plots.iter().any(|plot| {
                            relation.target.strip_prefix("plot/")
                                == Some(plot.graph.checked_plot_id.as_str())
                        }))
                });
                for (index, additional) in [
                    content.relationships.len(),
                    content.properties.len(),
                    content.texts.len(),
                    content.disclosures.len(),
                ]
                .into_iter()
                .enumerate()
                {
                    counts[index] += additional;
                }
                bounds_error = [
                    (
                        content.subjects.len(),
                        conduit_face::MAX_FACE_SUBJECTS,
                        FaceFragmentError::TooManySubjects,
                    ),
                    (
                        counts[0],
                        conduit_face::MAX_FACE_RELATIONSHIPS,
                        FaceFragmentError::TooManyRelationships,
                    ),
                    (
                        counts[1],
                        conduit_face::MAX_FACE_PROPERTIES,
                        FaceFragmentError::TooManyProperties,
                    ),
                    (
                        counts[2],
                        conduit_face::MAX_FACE_TEXT_ITEMS,
                        FaceFragmentError::TooManyTextItems,
                    ),
                    (
                        counts[3],
                        conduit_face::MAX_FACE_DISCLOSURES,
                        FaceFragmentError::TooManyDisclosures,
                    ),
                ]
                .into_iter()
                .find_map(|(count, maximum, error)| (count > maximum).then_some(error));
                if bounds_error.is_some() {
                    return false;
                }
                if let Err(error) = validate_subject_tail(content.subjects, checked_subjects) {
                    bounds_error = Some(error);
                    return false;
                }
                let new_subjects = &content.subjects[checked_subjects..];
                let matches = face.is_none_or(|face| {
                    new_subjects.iter().all(|item| face.subjects.contains(item))
                        && content
                            .relationships
                            .iter()
                            .all(|item| face.relationships.contains(item))
                        && content
                            .properties
                            .iter()
                            .all(|item| face.properties.contains(item))
                        && content.texts.iter().all(|item| face.text.contains(item))
                        && content
                            .disclosures
                            .iter()
                            .all(|item| face.disclosures.contains(item))
                        && face
                            .actions
                            .iter()
                            .filter(|action| {
                                new_subjects.iter().any(|s| s.identity == action.target)
                            })
                            .all(|action| base.actions.contains(action))
                });
                checked_subjects = content.subjects.len();
                // Endpoint visibility is emitter state: keep the finite subject
                // identities so later Cords emit the exact Connects relations.
                content.relationships.clear();
                content.properties.clear();
                content.texts.clear();
                content.disclosures.clear();
                matches
            },
        );
        if let Some(error) = bounds_error {
            Err(super::PatchbayFaceProjectionError::InvalidFragment(error))
        } else {
            Ok(matches)
        }
    }
}
/// The caller owns an append-only prefix already checked by prior checkpoints.
/// Compare every new identity with the complete prefix and the other new entries.
fn validate_subject_tail(
    subjects: &[conduit_face::FaceSubject],
    checked: usize,
) -> Result<(), FaceFragmentError> {
    let tail = subjects
        .get(checked..)
        .ok_or(FaceFragmentError::InvalidIdentity)?;
    if tail.iter().any(|subject| {
        subject.identity.is_empty() || subject.identity.len() > conduit_face::MAX_FACE_ID_BYTES
    }) {
        return Err(FaceFragmentError::InvalidIdentity);
    }
    for (offset, subject) in tail.iter().enumerate() {
        if subjects[..checked + offset]
            .iter()
            .any(|previous| previous.identity == subject.identity)
        {
            return Err(FaceFragmentError::DuplicateSubject);
        }
    }
    Ok(())
}
fn contains(face: &Face, fragment: &FaceFragment) -> bool {
    macro_rules! included {
        ($field:ident) => {
            fragment
                .$field
                .iter()
                .all(|item| face.$field.contains(item))
        };
    }
    included!(subjects)
        && included!(relationships)
        && included!(composition)
        && included!(properties)
        && included!(text)
        && included!(actions)
        && included!(disclosures)
        && included!(temporal_references)
        && included!(temporal_facts)
        && face
            .actions
            .iter()
            .filter(|action| {
                fragment
                    .subjects
                    .iter()
                    .any(|s| s.identity == action.target)
            })
            .all(|action| fragment.actions.contains(action))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{ActivePlayId, CheckedPlotId, PlanId};
    #[test]
    fn later_checkpoint_cannot_reuse_an_earlier_identity_with_changed_facts() {
        use conduit_face::{FaceRole, FaceSubject};
        let mut subjects = alloc::vec![FaceSubject {
            identity: "gear/first".into(),
            role: FaceRole::Gear,
            name: "First".into(),
        }];
        validate_subject_tail(&subjects, 0).unwrap();
        subjects.push(FaceSubject {
            identity: "port/second".into(),
            role: FaceRole::Port,
            name: "Second".into(),
        });
        validate_subject_tail(&subjects, 1).unwrap();
        // Collision crosses a checkpoint and differs in non-identity facts.
        subjects.push(FaceSubject {
            identity: "gear/first".into(),
            role: FaceRole::Port,
            name: "Forged".into(),
        });
        assert_eq!(
            validate_subject_tail(&subjects, 2),
            Err(FaceFragmentError::DuplicateSubject)
        );
        subjects[2].identity.clear();
        assert_eq!(
            validate_subject_tail(&subjects, 2),
            Err(FaceFragmentError::InvalidIdentity)
        );
        assert_eq!(
            validate_subject_tail(&subjects[..1], 2),
            Err(FaceFragmentError::InvalidIdentity)
        );
    }
    #[test]
    fn chunked_bounds_preserve_full_fragment_capacity_refusal() {
        let plans = conduit_patchbay_workbench_conformance::patchbay_mask_plans().unwrap();
        let mut port = PatchbayApplicationPort::open(
            &plans.direct_expanded,
            plans.direct.plan_id,
            PlanId::from("body-plan/bounds"),
        )
        .unwrap();
        let body: BodyId = serde_json::from_str("\"body/bounds\"").unwrap();
        let basis = FaceContributionBasis {
            checked_plot_id: CheckedPlotId::from("checked/bounds"),
            plan_id: PlanId::from("plan/bounds"),
            active_play_id: ActivePlayId::from("play/bounds"),
            required_interaction_context: None,
        };
        // Exercise aggregate capacity with repeated graph facts. This deliberately
        // malformed test graph is not admitted into any product or runtime.
        let original = port.plots[0].graph.clone();
        let mut cord = original.cords[0].clone();
        cord.source_port = "absent/source".into();
        cord.sink_port = "absent/sink".into();
        port.plots[0].graph.cords = (0..conduit_face::MAX_FACE_PROPERTIES / 6 + 1)
            .map(|index| {
                let mut cord = cord.clone();
                cord.identity = format!("bounds/cord/{index}");
                cord
            })
            .collect();
        let full = port.face_fragment(basis.clone(), &body, true).unwrap_err();
        assert_eq!(
            full,
            super::super::PatchbayFaceProjectionError::InvalidFragment(
                FaceFragmentError::TooManyProperties
            )
        );
        assert_eq!(
            port.validate_projection_bounds(basis.clone(), &body),
            Err(full)
        );
        // Duplicate Source declarations are invalid even when every count fits.
        // Bypass preparation only to compare both validators' refusal contract.
        port.plots[0].graph = original.clone();
        port.plots[0].plan_id = None;
        port.plots.push(super::super::PatchbayActivePlot {
            graph: original,
            title: "Duplicate Source".into(),
            checked_plot_id: port.plots[0].checked_plot_id.clone(),
            plan_id: None,
            state: super::super::PatchbayActivePlotState::CheckedSource,
            focused: false,
        });
        let full = port.face_fragment(basis.clone(), &body, true).unwrap_err();
        assert_eq!(
            full,
            super::super::PatchbayFaceProjectionError::InvalidFragment(
                FaceFragmentError::DuplicateSubject
            )
        );
        assert_eq!(port.validate_projection_bounds(basis, &body), Err(full));
    }
}
