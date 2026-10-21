//! Native encounter context over the shared checked Commons Source.
//! The Body remains owned by ProductJourney; geometry and highlight are local.
use super::*;
use alloc::{boxed::Box, vec::Vec};
use conduit_commons_plot::{Source, field, text};
use conduit_core::{StructuredInfoValue, StructuredInfoValueShape, kind_id};
use core::cell::RefCell;
mod scene;

pub(super) struct Commons {
    source: RefCell<Box<Source>>,
    selected: String,
    generation: u64,
    focus: usize,
}

impl Commons {
    pub(super) fn prepare() -> Result<Self, Error> {
        let source = Box::new(Source::prepare().map_err(|_| Error::Face)?);
        let selected = place_text(&source.places, "hub", "identity")?.into();
        Ok(Self {
            source: RefCell::new(source),
            selected,
            generation: 0,
            focus: 0,
        })
    }

    pub(super) fn selected(&self) -> &str {
        &self.selected
    }

    pub(super) fn creche(&self) -> bool {
        place_text(&self.source.borrow().places, "creche", "identity")
            .is_ok_and(|id| id == self.selected)
    }

    pub(super) fn workshop(&self) -> bool {
        place_text(&self.source.borrow().places, "workshop", "identity")
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

    pub(super) fn accept(
        &mut self,
        event: KeyEvent,
        view: HomeView,
        journey: &JourneyProjection,
    ) -> Result<Option<HomeInput>, Error> {
        use conduit_human::KeyTransition;
        let graph = self.source.borrow().places.clone();
        let hub = place_text(&graph, "hub", "identity")?;
        let in_hub = self.selected == hub;
        if self.workshop() && !(view == HomeView::Launcher && event.usage() == ESCAPE) {
            return Ok(None);
        }
        if event.transition() != KeyTransition::Pressed {
            return Ok(Some(HomeInput::Unchanged));
        }
        let target = match event.usage() {
            RIGHT | DOWN | LEFT | UP | TAB if in_hub => {
                self.focus = match event.usage() {
                    RIGHT | DOWN => 1,
                    LEFT | UP => 0,
                    _ => 1 - self.focus,
                };
                return Ok(Some(HomeInput::Changed));
            }
            ENTER if in_hub => place_text(
                &graph,
                if self.focus == 0 {
                    "workshop"
                } else {
                    "creche"
                },
                "identity",
            )?,
            ESCAPE if !in_hub => hub,
            _ => return Ok(Some(HomeInput::Unchanged)),
        };
        self.navigate(journey, target, !in_hub)?;
        Ok(Some(if self.creche() && journey.body_id.is_none() {
            HomeInput::OpenCreche
        } else {
            HomeInput::Changed
        }))
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
        for slot in ["hub", "workshop", "creche"] {
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
                value: FacePropertyValue::Flag(self.selected == identity),
            });
        }
        for slot in ["containment", "reachability"] {
            let StructuredInfoValueShape::Collection(relations) =
                field(graph, slot).map_err(|_| Error::Face)?.shape()
            else {
                return Err(Error::Face);
            };
            for relation in relations {
                let get = |name| {
                    text(field(relation, name).map_err(|_| Error::Face)?).map_err(|_| Error::Face)
                };
                face.relationships.push(FaceRelationship {
                    source: get("source")?.into(),
                    target: get("target")?.into(),
                    kind: FaceRelationshipKind::Semantic(kind_id(get("meaning")?)),
                });
            }
        }
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
