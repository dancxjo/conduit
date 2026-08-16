//! Add bounded Body Parts to one portable front-door Presentation.

use conduit_body::{Body, Wake};
use conduit_presentation::{
    Presentation, PresentationError, PresentationTemporalFact, PresentationTemporalRole,
    TemporalInstant, TemporalReference, TemporalRelationError, TemporalScale,
};

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
        append_parts_to_presentation(self.revision, presentation, body, parts, None)
    }

    pub fn to_portable_front_door_with_temporal_reference(
        &self,
        body: &Body,
        wake: &Wake,
        parts: &PartsView,
        reference: TemporalReference,
    ) -> Result<Presentation, PortableProjectionError> {
        if parts.body_id != body.body_id {
            return Err(PortableProjectionError::LifecycleMismatch);
        }
        let presentation = self.to_portable(body, wake)?;
        append_parts_to_presentation(self.revision, presentation, body, parts, Some(reference))
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
        append_parts_to_presentation(self.revision, presentation, body, parts, None)
    }

    pub fn to_portable_lulled_front_door_with_temporal_reference(
        &self,
        body: &Body,
        parts: &PartsView,
        reference: TemporalReference,
    ) -> Result<Presentation, PortableProjectionError> {
        if parts.body_id != body.body_id {
            return Err(PortableProjectionError::LifecycleMismatch);
        }
        let presentation = self.to_portable_with_wake(body, None)?;
        append_parts_to_presentation(self.revision, presentation, body, parts, Some(reference))
    }
}

fn append_parts_to_presentation(
    revision: u64,
    mut presentation: Presentation,
    body: &Body,
    parts: &PartsView,
    reference: Option<TemporalReference>,
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
    let mut temporal_references = presentation.temporal_references;
    let mut temporal_facts = presentation.temporal_facts;
    if let Some(reference) = reference {
        let facts = parts
            .parts
            .iter()
            .map(|row| {
                match (
                    row.details.presence_sign_id.as_ref(),
                    row.details.presence_clock.as_ref(),
                    row.details.presence_observed_at_millis,
                ) {
                    (None, None, None) => Ok(None),
                    (Some(sign_id), Some(clock), Some(ticks)) => PresentationTemporalFact::new(
                        format!("part/{}", row.details.part_id.as_str()),
                        PresentationTemporalRole::Observation,
                        Some(sign_id.clone()),
                        TemporalInstant {
                            ticks,
                            scale: match clock.scale {
                                conduit_body::HostPresenceClockScale::Milliseconds => {
                                    TemporalScale::Milliseconds
                                }
                            },
                            clock_basis: clock.basis_id.clone(),
                            resolution_ticks: clock.resolution_ticks,
                            uncertainty_ticks: clock.uncertainty_ticks,
                        },
                        &reference,
                    )
                    .map(Some)
                    .map_err(temporal_relation_error),
                    _ => Err(PortableProjectionError::InvalidPresentation(
                        PresentationError::InvalidTemporalInstant,
                    )),
                }
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        if !facts.is_empty() {
            temporal_references.push(reference);
            temporal_facts.extend(facts);
        }
    }
    Presentation::new_with_semantics_and_temporal(
        revision,
        presentation.basis,
        content.subjects,
        content.relationships,
        content.properties,
        content.text,
        presentation.actions,
        presentation.disclosures,
        temporal_references,
        temporal_facts,
    )
    .map_err(PortableProjectionError::InvalidPresentation)
}

fn temporal_relation_error(error: TemporalRelationError) -> PortableProjectionError {
    PortableProjectionError::InvalidPresentation(match error {
        TemporalRelationError::InvalidInstant => PresentationError::InvalidTemporalInstant,
        TemporalRelationError::Incomparable => PresentationError::IncomparableTemporalInstants,
        TemporalRelationError::IntervalOverflow => PresentationError::TemporalIntervalOverflow,
    })
}
