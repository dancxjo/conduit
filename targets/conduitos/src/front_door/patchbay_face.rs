//! Compose the actual resident owner's fragment with native lifecycle meaning.
use super::*;
impl FrontDoor {
    pub(super) fn append_patchbay_fragment(&self, face: Face) -> Result<Face, Error> {
        let Some(fragment) = &self.patchbay_fragment else {
            return Ok(face);
        };
        if !patchbay_application::patchbay_face_contribution_matches(&face, &fragment.basis) {
            return Err(Error::Face);
        }
        let mut subjects = face.subjects;
        let mut relationships = face.relationships;
        let workspace = self.workspace.as_ref().ok_or(Error::Face)?;
        let body = format!("body/{}", workspace.body_id.as_str());
        // Membership declarations belong to the Body, not the application actor.
        for plot in &workspace.plots {
            let identity = format!("plot/{}", plot.plot.checked_plot_id.as_str());
            if !subjects.iter().any(|subject| subject.identity == identity) {
                subjects.push(FaceSubject {
                    identity: identity.clone(),
                    role: FaceRole::Plot,
                    name: plot.title.into(),
                });
                relationships.push(FaceRelationship {
                    source: body.clone(),
                    target: identity,
                    kind: FaceRelationshipKind::Contains,
                });
            }
        }
        subjects.extend(fragment.subjects.clone());
        relationships.extend(fragment.relationships.clone());
        let mut properties = face.properties;
        properties.extend(fragment.properties.clone());
        let mut text = face.text;
        text.extend(fragment.text.clone());
        let mut actions = face.actions;
        actions.extend(fragment.actions.clone());
        let mut disclosures = face.disclosures;
        disclosures.extend(fragment.disclosures.clone());
        let mut references = face.temporal_references;
        references.extend(fragment.temporal_references.clone());
        let mut facts = face.temporal_facts;
        facts.extend(fragment.temporal_facts.clone());
        let mut composition = face.composition;
        composition.extend(fragment.composition.clone());
        Face::new_with_semantics_and_temporal(
            face.revision,
            face.basis,
            subjects,
            relationships,
            properties,
            text,
            actions,
            disclosures,
            references,
            facts,
        )
        .and_then(|face| face.with_composition(composition))
        .map_err(|_| Error::Face)
    }
}
