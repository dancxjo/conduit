//! Native encounter context over the shared checked Commons Source.
//! The Body remains owned by ProductJourney; geometry and highlight are local.
use super::*;
use alloc::{boxed::Box, vec::Vec};
use conduit_commons_plot::{Source, field, text};
use conduit_core::{StructuredInfoValue, StructuredInfoValueShape, kind_id};
use core::cell::RefCell;
mod heading;

pub(super) struct Commons {
    source: RefCell<Box<Source>>,
    selected: String,
    generation: u64,
}

impl Commons {
    pub(super) fn prepare() -> Result<Self, Error> {
        let source = Box::new(Source::prepare().map_err(|_| Error::Face)?);
        let selected = place_text(&source.places, "hub", "identity")?.into();
        Ok(Self {
            source: RefCell::new(source),
            selected,
            generation: 0,
        })
    }

    pub(super) fn selected(&self) -> &str {
        &self.selected
    }

    pub(super) fn creche(&self) -> bool {
        place_text(&self.source.borrow().places, "creche", "identity")
            .is_ok_and(|id| id == self.selected)
    }

    fn observe(
        &self,
        journey: &JourneyProjection,
        target: &str,
        back: bool,
    ) -> Result<StructuredInfoValue, Error> {
        let body = journey.body_id.as_ref().map_or("", |body| body.as_str());
        let life = if body.is_empty() {
            "none"
        } else if journey.status == JourneyStatus::Fulfilled {
            "fulfilled"
        } else if journey.wake_id.is_none() {
            "lulled"
        } else {
            "awake"
        };
        self.source
            .borrow_mut()
            .encounter(body, life, journey.revision, &self.selected, target, back)
            .map_err(|_| Error::Face)
    }

    /// The zero-Body Hub has one meaningful destination: the Birth encounter.
    pub(super) fn accept_birth(
        &mut self,
        event: KeyEvent,
        journey: &JourneyProjection,
    ) -> Result<HomeInput, Error> {
        if event.usage() != ENTER || event.transition() != conduit_human::KeyTransition::Pressed {
            return Ok(HomeInput::Unchanged);
        }
        let graph = self.source.borrow().places.clone();
        self.navigate(journey, place_text(&graph, "creche", "identity")?, false)?;
        Ok(HomeInput::OpenCreche)
    }

    pub(super) fn back_to_hub(&mut self, journey: &JourneyProjection) -> Result<(), Error> {
        let graph = self.source.borrow().places.clone();
        let hub = place_text(&graph, "hub", "identity")?;
        self.navigate(journey, hub, self.selected != hub)
    }

    fn navigate(
        &mut self,
        journey: &JourneyProjection,
        target: &str,
        back: bool,
    ) -> Result<(), Error> {
        let generation = self.generation.checked_add(1).ok_or(Error::Face)?;
        let value = self.observe(journey, target, back)?;
        self.selected = text(field(&value, "selected").map_err(|_| Error::Face)?)
            .map_err(|_| Error::Face)?
            .into();
        self.generation = generation;
        Ok(())
    }

    pub(super) fn project(
        &self,
        mut face: Face,
        journey: &JourneyProjection,
    ) -> Result<Face, Error> {
        let observation = self.observe(journey, &self.selected, false)?;
        let graph = field(&observation, "places").map_err(|_| Error::Face)?;
        let slot = if self.creche() { "creche" } else { "hub" };
        let identity = place_text(graph, slot, "identity")?;
        face.subjects.push(FaceSubject {
            identity: identity.into(),
            name: place_text(graph, slot, "name")?.into(),
            role: FaceRole::Semantic(kind_id(place_text(graph, slot, "role")?)),
        });
        face.disclosures.push(FaceDisclosure {
            subject: identity.into(),
            level: FaceDisclosureLevel::Primary,
        });
        face.properties.push(FaceProperty {
            subject: identity.into(),
            name: "selected".into(),
            value: FacePropertyValue::Flag(true),
        });
        let body_actions = matches!(field(&observation, "body_actions").map_err(|_| Error::Face)?.shape(), StructuredInfoValueShape::Leaf(bytes) if bytes == [1]);
        if !body_actions {
            for action in &mut face.actions {
                if action.availability.is_available() {
                    action.availability = conduit_face::FaceActionAvailability::Unavailable {
                        reason_code: "commons/context-withholds-owner-action".into(),
                        explanation:
                            "Commons does not offer this Body operation in the current context."
                                .into(),
                    };
                }
            }
        }
        let source = self.source.borrow();
        face.properties.push(FaceProperty {
            subject: self.selected.clone(),
            name: "commons/source".into(),
            value: FacePropertyValue::Identity(source.checked.source_document_id.as_str().into()),
        });
        face.text.push(FaceText {
            subject: self.selected.clone(),
            text: self.heading()?,
        });
        // Rebuild content identity after composing exact Source truth.
        let composition = face.composition;
        Face::new_with_semantics_and_temporal(
            face.revision,
            face.basis,
            face.subjects,
            face.relationships,
            face.properties,
            face.text,
            face.actions,
            face.disclosures,
            face.temporal_references,
            face.temporal_facts,
        )
        .and_then(|face| face.with_composition(composition))
        .and_then(|face| {
            face.with_interaction_context(conduit_face::FaceInteractionContext {
                identity: format!("commons/native/{}/{}", self.selected, self.generation),
                basis: Vec::new(),
            })
        })
        .map_err(|_| Error::Face)
    }
}

pub(super) fn place_text<'a>(
    graph: &'a StructuredInfoValue,
    slot: &str,
    name: &str,
) -> Result<&'a str, Error> {
    text(field(field(graph, slot).map_err(|_| Error::Face)?, name).map_err(|_| Error::Face)?)
        .map_err(|_| Error::Face)
}

#[cfg(test)]
mod tests;
