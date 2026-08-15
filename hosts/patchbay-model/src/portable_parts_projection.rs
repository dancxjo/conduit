//! Add bounded Body Parts to one portable front-door Presentation.

use conduit_body::{Body, Wake};
use conduit_presentation::Presentation;

use crate::portable_projection::{ContentBuilder, PortableProjectionError};
use crate::{PartsView, PatchbayPresentation};

impl PatchbayPresentation {
    pub fn to_portable_front_door(
        &self,
        body: &Body,
        wake: &Wake,
        parts: &PartsView,
    ) -> Result<Presentation, PortableProjectionError> {
        if parts.body_id != body.body_id {
            return Err(PortableProjectionError::LifecycleMismatch);
        }
        let presentation = self.to_portable(body, wake)?;
        append_parts_to_presentation(self.revision, presentation, body, parts)
    }

    pub fn to_portable_lulled_front_door(
        &self,
        body: &Body,
        parts: &PartsView,
    ) -> Result<Presentation, PortableProjectionError> {
        if parts.body_id != body.body_id {
            return Err(PortableProjectionError::LifecycleMismatch);
        }
        let presentation = self.to_portable_with_wake(body, None)?;
        append_parts_to_presentation(self.revision, presentation, body, parts)
    }
}

fn append_parts_to_presentation(
    revision: u64,
    mut presentation: Presentation,
    body: &Body,
    parts: &PartsView,
) -> Result<Presentation, PortableProjectionError> {
    let mut content = ContentBuilder::from_parts(
        presentation.subjects,
        presentation.relationships,
        presentation.properties,
        presentation.text,
    );
    crate::portable_world_projection::append_body_parts(body, parts, &mut content);
    presentation.basis.sign_ids.extend(
        parts
            .parts
            .iter()
            .flat_map(|row| row.details.evidence_signs.iter().cloned()),
    );
    presentation.basis.sign_ids.extend(
        parts
            .wants_to_join
            .iter()
            .flat_map(|row| row.evidence_signs.iter().cloned()),
    );
    Presentation::new_with_semantics(
        revision,
        presentation.basis,
        content.subjects,
        content.relationships,
        content.properties,
        content.text,
        presentation.actions,
        presentation.disclosures,
    )
    .map_err(PortableProjectionError::InvalidPresentation)
}
