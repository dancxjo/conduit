//! Bounded zero-Body Patchbay state for an ordinary ConduitOS boot.

use alloc::{format, string::String, vec};
use conduit_body::SeedId;
use conduit_core::{BootId, CheckedFormId, HostId, KeyEvent, OfferGeneration, SourceDocumentId};
use conduit_presentation::{
    Presentation, PresentationBasis, PresentationProperty, PresentationPropertyValue,
    PresentationRelationship, PresentationRelationshipKind, PresentationRole, PresentationSubject,
    PresentationText,
};

use crate::display::DisplayError;
use crate::product_journey::{JourneyProjection, JourneyStatus};

#[cfg(any(test, feature = "native-compositor"))]
mod presenter;
mod scene;
#[cfg(any(test, feature = "native-compositor"))]
pub use presenter::{FrontDoorPresenter, PresenterError};

const ENTER: u8 = 40;
const ESCAPE: u8 = 41;
const TAB: u8 = 43;
const F2: u8 = 59;
const RIGHT: u8 = 79;
const LEFT: u8 = 80;
const DOWN: u8 = 81;
const UP: u8 = 82;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Selection {
    Seed,
    Details,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    World,
    SeedOpened,
    DetailsOpened,
}

pub struct FrontDoor {
    host_id: HostId,
    boot_id: BootId,
    offer_generation: OfferGeneration,
    profile_id: String,
    build_id: String,
    image_id: String,
    source_document_id: SourceDocumentId,
    checked_form_id: CheckedFormId,
    seed_id: SeedId,
    selection: Selection,
    status: Status,
    revision: u64,
    offer_count: u64,
    details_page: u8,
    journey: Option<JourneyProjection>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    StaleInput,
    Presentation,
    Display(DisplayError),
    Scene,
}

impl Error {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StaleInput => "front-door-input-stale",
            Self::Presentation => "front-door-presentation-refused",
            Self::Display(error) => error.as_str(),
            Self::Scene => "front-door-scene-refused",
        }
    }
}

impl FrontDoor {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        profile_id: impl Into<String>,
        build_id: impl Into<String>,
        image_id: impl Into<String>,
        source_document_id: SourceDocumentId,
        checked_form_id: CheckedFormId,
        offer_count: u64,
    ) -> Self {
        let seed_id = SeedId::bind(&source_document_id, &checked_form_id);
        Self {
            host_id,
            boot_id,
            offer_generation,
            profile_id: profile_id.into(),
            build_id: build_id.into(),
            image_id: image_id.into(),
            source_document_id,
            checked_form_id,
            seed_id,
            selection: Selection::Seed,
            status: Status::World,
            revision: 1,
            offer_count,
            details_page: 0,
            journey: None,
        }
    }

    pub const fn status(&self) -> Status {
        self.status
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub fn seed_id(&self) -> &SeedId {
        &self.seed_id
    }

    pub fn observe_journey(&mut self, projection: JourneyProjection) -> Result<(), Error> {
        if projection.seed_id != self.seed_id {
            return Err(Error::Presentation);
        }
        self.status = if projection.status == JourneyStatus::SeedOpened {
            Status::SeedOpened
        } else {
            Status::World
        };
        self.journey = Some(projection);
        self.advance()
    }

    pub fn accept(&mut self, event: KeyEvent, revision: u64) -> Result<bool, Error> {
        if revision != self.revision {
            return Err(Error::StaleInput);
        }
        if event.transition() != conduit_core::KeyTransition::Pressed {
            return Ok(false);
        }
        match event.usage() {
            TAB | RIGHT | LEFT | DOWN | UP => {
                self.selection = match self.selection {
                    Selection::Seed => Selection::Details,
                    Selection::Details => Selection::Seed,
                };
                self.status = Status::World;
                self.advance()?;
                Ok(true)
            }
            F2 => {
                self.selection = Selection::Details;
                if self.status == Status::DetailsOpened {
                    self.details_page = (self.details_page + 1) % 16;
                }
                self.status = Status::DetailsOpened;
                self.advance()?;
                Ok(true)
            }
            ENTER => {
                self.status = match self.selection {
                    Selection::Seed => Status::SeedOpened,
                    Selection::Details => Status::DetailsOpened,
                };
                self.advance()?;
                Ok(true)
            }
            ESCAPE => {
                self.status = Status::World;
                self.advance()?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn advance(&mut self) -> Result<(), Error> {
        self.revision = self.revision.checked_add(1).ok_or(Error::Presentation)?;
        Ok(())
    }

    pub fn presentation(&self) -> Result<Presentation, Error> {
        let host = format!("host/{}/{}", self.host_id.as_str(), self.boot_id.as_str());
        let seed = format!("seed/{}", self.seed_id.as_str());
        let mut subjects = vec![
            PresentationSubject {
                identity: host.clone(),
                role: PresentationRole::Host,
                label: "This Host".into(),
                accessibility_name: "This Host; current Body none".into(),
            },
            PresentationSubject {
                identity: seed.clone(),
                role: PresentationRole::Seed,
                label: "ConduitOS entrance Seed".into(),
                accessibility_name: "Openable checked IMAGE Seed; opening is inert".into(),
            },
        ];
        let mut relationships = vec![PresentationRelationship {
            source: host.clone(),
            target: seed.clone(),
            kind: PresentationRelationshipKind::Observes,
        }];
        let mut properties = vec![
            property(
                &host,
                "current-body",
                PresentationPropertyValue::Text("none".into()),
            ),
            property(&host, "host-id", identity(self.host_id.as_str())),
            property(&host, "boot-id", identity(self.boot_id.as_str())),
            property(
                &host,
                "offer-generation",
                PresentationPropertyValue::Count(self.offer_generation.0),
            ),
            property(
                &host,
                "offer-count",
                PresentationPropertyValue::Count(self.offer_count),
            ),
            property(&seed, "seed-id", identity(self.seed_id.as_str())),
            property(
                &seed,
                "source-document-id",
                identity(self.source_document_id.as_str()),
            ),
            property(
                &seed,
                "checked-form-id",
                identity(self.checked_form_id.as_str()),
            ),
            property(
                &seed,
                "opened",
                PresentationPropertyValue::Flag(self.status == Status::SeedOpened),
            ),
        ];
        if self.status == Status::DetailsOpened {
            properties.extend([
                property(&host, "profile-id", identity(&self.profile_id)),
                property(&host, "build-id", identity(&self.build_id)),
                property(&host, "image-id", identity(&self.image_id)),
            ]);
        }
        if let Some(journey) = &self.journey {
            if let Some(body_id) = &journey.body_id {
                let body = format!("body/{}", body_id.as_str());
                subjects.push(PresentationSubject {
                    identity: body.clone(),
                    role: PresentationRole::Body,
                    label: "Current Body".into(),
                    accessibility_name: format!("Current Body; {:?}", journey.status),
                });
                relationships.push(PresentationRelationship {
                    source: host.clone(),
                    target: body.clone(),
                    kind: PresentationRelationshipKind::Contains,
                });
                properties.push(property(&body, "body-id", identity(body_id.as_str())));
                if let Some(born_sign_id) = &journey.born_sign_id {
                    properties.push(property(
                        &body,
                        "born-sign-id",
                        identity(born_sign_id.as_str()),
                    ));
                }
                if let Some(part_id) = &journey.part_id {
                    properties.push(property(&body, "part-id", identity(part_id.as_str())));
                }
            }
            properties.push(property(
                &seed,
                "expanded-form-id",
                identity(journey.expanded_form_id.as_str()),
            ));
            if let Some(wake_id) = &journey.wake_id {
                properties.push(property(&host, "wake-id", identity(wake_id.as_str())));
            }
            if let Some(plan_id) = &journey.plan_id {
                properties.push(property(&host, "plan-id", identity(plan_id.as_str())));
            }
            if let Some(play_id) = &journey.active_play_id {
                properties.push(property(
                    &host,
                    "active-play-id",
                    identity(play_id.as_str()),
                ));
            }
            if let Some(result) = &journey.result {
                properties.push(property(
                    &host,
                    "semantic-result",
                    PresentationPropertyValue::Text(result.clone()),
                ));
            }
        }
        let basis = self.journey.as_ref().map_or_else(
            || PresentationBasis {
                seed_id: None,
                body_id: None,
                wake_id: None,
                source_document_id: None,
                checked_form_id: None,
                expanded_form_id: None,
                plan_id: None,
                active_play_id: None,
                sign_ids: vec![],
            },
            |journey| {
                if journey.body_id.is_some() && journey.wake_id.is_some() {
                    PresentationBasis {
                        seed_id: Some(journey.seed_id.clone()),
                        body_id: journey.body_id.clone(),
                        wake_id: journey.wake_id.clone(),
                        source_document_id: Some(self.source_document_id.clone()),
                        checked_form_id: Some(self.checked_form_id.clone()),
                        expanded_form_id: Some(journey.expanded_form_id.clone()),
                        plan_id: journey.plan_id.clone(),
                        active_play_id: journey.active_play_id.clone(),
                        sign_ids: journey
                            .input_sign_id
                            .iter()
                            .chain(journey.result_sign_id.iter())
                            .cloned()
                            .collect(),
                    }
                } else {
                    PresentationBasis {
                        seed_id: None,
                        body_id: None,
                        wake_id: None,
                        source_document_id: None,
                        checked_form_id: None,
                        expanded_form_id: None,
                        plan_id: None,
                        active_play_id: None,
                        sign_ids: vec![],
                    }
                }
            },
        );
        let host_text = self.journey.as_ref().map_or_else(
            || "BODY NONE; entering Patchbay creates no Body, Wake, Plan, or Play".into(),
            |journey| {
                if journey.body_id.is_none() {
                    "BODY NONE; OPEN is inspection only and has no effects".into()
                } else {
                    format!("CANONICAL BODY/WAKE/PLAN/PLAY STATE: {:?}", journey.status)
                }
            },
        );
        Presentation::new(
            self.revision,
            basis,
            subjects,
            relationships,
            properties,
            vec![
                PresentationText {
                    subject: host,
                    text: host_text,
                },
                PresentationText {
                    subject: seed,
                    text: "IMAGE-embedded checked Seed; OPEN permits inspection only".into(),
                },
            ],
        )
        .map_err(|_| Error::Presentation)
    }
}

fn identity(value: &str) -> PresentationPropertyValue {
    PresentationPropertyValue::Identity(value.into())
}

fn property(subject: &str, name: &str, value: PresentationPropertyValue) -> PresentationProperty {
    PresentationProperty {
        subject: subject.into(),
        name: name.into(),
        value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::PixelTarget;

    fn door() -> FrontDoor {
        FrontDoor::new(
            HostId::from("host"),
            BootId::from("boot"),
            OfferGeneration(3),
            "profile:one",
            "build:one",
            "image:one",
            SourceDocumentId::from("source"),
            CheckedFormId::from("checked"),
            7,
        )
    }

    fn key(usage: u8) -> KeyEvent {
        KeyEvent::new(
            usage,
            conduit_core::KeyTransition::Pressed,
            conduit_core::KeyModifiers::from_bits(0),
        )
        .unwrap()
    }

    struct Sink;

    impl PixelTarget for Sink {
        fn format(&self) -> crate::display::DisplayFormat {
            crate::display::DisplayFormat {
                width: 640,
                height: 480,
                pitch: 2_560,
                bits_per_pixel: 32,
                red_shift: 16,
                green_shift: 8,
                blue_shift: 0,
            }
        }

        fn write_pixel(&mut self, _: u32, _: u32, _: u32) -> Result<(), DisplayError> {
            Ok(())
        }
    }

    #[test]
    fn entrance_is_a_zero_body_portable_presentation_and_finite_scene() {
        let door = door();
        let presentation = door.presentation().unwrap();
        assert!(presentation.basis.body_id.is_none());
        assert!(presentation.basis.plan_id.is_none());
        assert_eq!(presentation.subjects[1].role, PresentationRole::Seed);
        let scene = door.scene(&Sink).unwrap();
        let receipt = crate::display::render_scene(&mut Sink, &scene).unwrap();
        assert_eq!(receipt.commands, 8);
        assert!(receipt.pixels_written > 0);
    }

    #[test]
    fn open_is_inert_and_details_are_progressive() {
        let mut door = door();
        assert!(door.accept(key(ENTER), 1).unwrap());
        assert_eq!(door.status(), Status::SeedOpened);
        assert_eq!(door.revision(), 2);
        assert!(door.presentation().unwrap().basis.body_id.is_none());
        assert!(door.accept(key(F2), 2).unwrap());
        let details = door.presentation().unwrap();
        assert_eq!(door.status(), Status::DetailsOpened);
        assert!(
            details
                .properties
                .iter()
                .any(|property| property.name == "profile-id")
        );
        assert!(details.basis.body_id.is_none());
    }

    #[test]
    fn stale_input_and_capacity_are_explicit_without_body_transition() {
        let mut door = door();
        assert_eq!(door.accept(key(ENTER), 0), Err(Error::StaleInput));
        door.revision = u64::MAX;
        assert_eq!(door.accept(key(ENTER), u64::MAX), Err(Error::Presentation));
        assert!(door.presentation().unwrap().basis.body_id.is_none());
    }

    #[test]
    fn releases_and_unrelated_keys_do_not_act() {
        let mut door = door();
        let release = KeyEvent::new(
            ENTER,
            conduit_core::KeyTransition::Released,
            conduit_core::KeyModifiers::from_bits(0),
        )
        .unwrap();
        assert!(!door.accept(release, 1).unwrap());
        assert!(!door.accept(key(4), 1).unwrap());
    }
}
