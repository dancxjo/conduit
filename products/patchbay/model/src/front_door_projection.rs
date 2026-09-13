//! Build one immutable portable projection from the living front-door session.

use crate::{
    LocalFrontDoor, LocalFrontDoorProjection, PartsView, PatchbayGraph, PatchbayPresentation,
};
use conduit_body::WakeLifecycle;
use conduit_presentation::{
    Presentation, PresentationBasis, PresentationDisclosure, PresentationDisclosureLevel,
    PresentationPropertyValue, PresentationRole,
};

impl LocalFrontDoor {
    pub fn project(&self) -> Result<LocalFrontDoorProjection, String> {
        let parts = PartsView::project(
            &self.body,
            &self.membership,
            &self.candidates,
            &self.here,
            self.plan.as_ref().map(|document| &document.exact),
            self.active_play.as_ref(),
            self.wake.is_some(),
        )
        .map_err(|error| format!("{error:?}"))?;
        let snapshot = self.topology.snapshot(
            self.model.startup_snapshot(),
            &self.candidates,
            &self.membership,
        );
        let topology = conduit_observatory::build_report(&snapshot)?;
        let Some(editor) = self.editor.as_ref() else {
            return idle_body_projection(self, parts);
        };
        let form_name = self
            .form_name
            .as_deref()
            .ok_or("idle Body Patchbay projection is not yet available")?;
        let graph = PatchbayGraph::from_expanded(
            &editor
                .expand_form(form_name)
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        let projection = PatchbayPresentation::new(
            self.revision,
            editor.view(),
            self.plan.clone(),
            self.play.clone(),
            Some(topology),
            Vec::new(),
        )
        .map_err(|error| error.to_string())?
        .with_graph(graph)
        .map_err(|error| error.to_string())?;
        let presentation = match &self.wake {
            Some(wake) => projection
                .to_portable_front_door(&self.body, wake, &parts)
                .map_err(|error| error.to_string())?,
            None => projection
                .to_portable_lulled_front_door(&self.body, &parts)
                .map_err(|error| error.to_string())?,
        };
        let navigation = crate::PatchbayNavigationProjection::for_embodied(&presentation)?;
        Ok(LocalFrontDoorProjection {
            presentation,
            navigation,
            parts,
        })
    }
}

fn idle_body_projection(
    session: &LocalFrontDoor,
    parts: PartsView,
) -> Result<LocalFrontDoorProjection, String> {
    if !session.body.workset.is_empty() || session.wake.is_some() {
        return Err("only a born-lulled empty workset may use the idle projection".into());
    }
    let mut content = crate::portable_projection::ContentBuilder::new();
    let body_subject = content.subject_with_identity(
        format!("body/{}", session.body.body_id.as_str()),
        PresentationRole::Body,
        session
            .birth_evidence
            .as_ref()
            .map_or("Current Body", |evidence| evidence.friendly_name.as_str()),
        "Born Body with no installed Forms",
    );
    content.property(
        &body_subject,
        "workset-form-count",
        PresentationPropertyValue::Count(0),
    );
    content.line(
        &body_subject,
        "Born and lulled; no Form is installed, planned, or playing.",
    );
    crate::portable_world_projection::append_body_parts(&session.body, &parts, &mut content);
    let mut sign_ids = session.body.sign_ids.clone();
    sign_ids.extend(
        parts
            .parts
            .iter()
            .flat_map(|part| part.details.evidence_signs.iter().cloned()),
    );
    sign_ids.sort();
    sign_ids.dedup();
    let presentation = Presentation::new_with_semantics(
        session.revision,
        PresentationBasis {
            body_id: Some(session.body.body_id.clone()),
            wake_id: None,
            source_document_id: None,
            checked_form_id: None,
            expanded_form_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids,
        },
        content.subjects,
        content.relationships,
        content.properties,
        content.text,
        crate::portable_projection::lifecycle_actions(WakeLifecycle::Lulled, &body_subject),
        vec![PresentationDisclosure {
            subject: body_subject,
            level: PresentationDisclosureLevel::Primary,
        }],
    )
    .map_err(|error| error.to_string())?;
    let navigation = crate::PatchbayNavigationProjection::for_embodied(&presentation)?;
    Ok(LocalFrontDoorProjection {
        presentation,
        navigation,
        parts,
    })
}
