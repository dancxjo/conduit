//! Add bounded Body Parts to one portable front-door Face.

use conduit_body::{Body, Wake};
use conduit_face::{
    Face, FaceError, FaceTemporalFact, FaceTemporalRole, TemporalInstant, TemporalReference,
    TemporalRelationError, TemporalScale,
};
use patchbay_application::PartsView;

use crate::portable_projection::{ContentBuilder, PortableProjectionError};
use crate::PatchbayFace;

impl PatchbayFace {
    pub fn to_portable_front_door(
        &self,
        body: &Body,
        wake: &Wake,
        parts: &PartsView,
    ) -> Result<Face, PortableProjectionError> {
        if parts.body_id != body.body_id {
            return Err(PortableProjectionError::LifecycleMismatch);
        }
        let face = self.to_portable(body, wake)?;
        append_parts_to_face(self.revision, face, body, parts, None)
    }

    pub fn to_portable_front_door_with_temporal_reference(
        &self,
        body: &Body,
        wake: &Wake,
        parts: &PartsView,
        reference: TemporalReference,
    ) -> Result<Face, PortableProjectionError> {
        if parts.body_id != body.body_id {
            return Err(PortableProjectionError::LifecycleMismatch);
        }
        let face = self.to_portable(body, wake)?;
        append_parts_to_face(self.revision, face, body, parts, Some(reference))
    }

    pub fn to_portable_lulled_front_door(
        &self,
        body: &Body,
        parts: &PartsView,
    ) -> Result<Face, PortableProjectionError> {
        if parts.body_id != body.body_id {
            return Err(PortableProjectionError::LifecycleMismatch);
        }
        let face = self.to_portable_with_wake(body, None)?;
        append_parts_to_face(self.revision, face, body, parts, None)
    }

    pub fn to_portable_lulled_front_door_with_temporal_reference(
        &self,
        body: &Body,
        parts: &PartsView,
        reference: TemporalReference,
    ) -> Result<Face, PortableProjectionError> {
        if parts.body_id != body.body_id {
            return Err(PortableProjectionError::LifecycleMismatch);
        }
        let face = self.to_portable_with_wake(body, None)?;
        append_parts_to_face(self.revision, face, body, parts, Some(reference))
    }
}

fn append_parts_to_face(
    revision: u64,
    mut face: Face,
    body: &Body,
    parts: &PartsView,
    reference: Option<TemporalReference>,
) -> Result<Face, PortableProjectionError> {
    let mut content = ContentBuilder::from_parts(
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
    );
    crate::portable_world_projection::append_body_parts(body, parts, &mut content);
    crate::portable_correlations::append_planned_correlations(&mut content);
    face.basis.sign_ids.extend(
        parts
            .parts
            .iter()
            .flat_map(|row| row.details.evidence_signs.iter().cloned()),
    );
    face.basis.sign_ids.extend(
        parts
            .wants_to_join
            .iter()
            .flat_map(|row| row.evidence_signs.iter().cloned()),
    );
    let mut temporal_references = face.temporal_references;
    let mut temporal_facts = face.temporal_facts;
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
                    (Some(sign_id), Some(clock), Some(ticks)) => FaceTemporalFact::new(
                        format!("part/{}", row.details.part_id.as_str()),
                        FaceTemporalRole::Observation,
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
                    _ => Err(PortableProjectionError::InvalidFace(
                        FaceError::InvalidTemporalInstant,
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
    Face::new_with_semantics_and_temporal(
        revision,
        face.basis,
        content.subjects,
        content.relationships,
        content.properties,
        content.text,
        face.actions,
        face.disclosures,
        temporal_references,
        temporal_facts,
    )
    .map_err(PortableProjectionError::InvalidFace)
}

fn temporal_relation_error(error: TemporalRelationError) -> PortableProjectionError {
    PortableProjectionError::InvalidFace(match error {
        TemporalRelationError::InvalidInstant => FaceError::InvalidTemporalInstant,
        TemporalRelationError::Incomparable => FaceError::IncomparableTemporalInstants,
        TemporalRelationError::IntervalOverflow => FaceError::TemporalIntervalOverflow,
    })
}
