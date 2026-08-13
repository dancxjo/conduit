//! Bounded zero-Body Patchbay state for an ordinary ConduitOS boot.

use alloc::{format, string::String, vec};
use conduit_body::SeedId;
use conduit_core::{BootId, CheckedFormId, HostId, KeyEvent, OfferGeneration, SourceDocumentId};
use conduit_presentation::{
    GraphicsCommand, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle, LayoutRect,
    Presentation, PresentationBasis, PresentationProperty, PresentationPropertyValue,
    PresentationRelationship, PresentationRelationshipKind, PresentationRole, PresentationSubject,
    PresentationText,
};

use crate::display::{DisplayError, PixelTarget};

#[cfg(any(test, feature = "native-compositor"))]
mod presenter;
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
                    self.details_page = (self.details_page + 1) % 6;
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
        let subjects = vec![
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
        let relationships = vec![PresentationRelationship {
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
        Presentation::new(
            self.revision,
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
            },
            subjects,
            relationships,
            properties,
            vec![
                PresentationText {
                    subject: host,
                    text: "BODY NONE; entering Patchbay creates no Body, Wake, Plan, or Play"
                        .into(),
                },
                PresentationText {
                    subject: seed,
                    text: "IMAGE-embedded checked Seed; OPEN permits inspection only".into(),
                },
            ],
        )
        .map_err(|_| Error::Presentation)
    }

    pub fn scene(&self, display: &impl PixelTarget) -> Result<GraphicsScene, Error> {
        self.presentation()?
            .validate()
            .map_err(|_| Error::Presentation)?;
        let format = display.format().validate().map_err(Error::Display)?;
        let screen = LayoutRect {
            x: 0,
            y: 0,
            width: u16::try_from(format.width).map_err(|_| Error::Scene)?,
            height: u16::try_from(format.height).map_err(|_| Error::Scene)?,
        };
        let mut scene = GraphicsScene::empty();
        scene
            .push(
                GraphicsCommand::rect(
                    screen,
                    screen,
                    GraphicsPaintRole::Background,
                    GraphicsShapeStyle::Fill,
                )
                .map_err(|_| Error::Scene)?,
            )
            .map_err(|_| Error::Scene)?;
        text(&mut scene, 18, 18, "CONDUIT / PATCHBAY / WORLD")?;
        match self.status {
            Status::World => {
                text(&mut scene, 18, 42, "THIS HOST    BODY: NONE")?;
                text(&mut scene, 18, 70, "BODIES NEARBY    NONE OBSERVED")?;
                text(&mut scene, 18, 96, "SEEDS")?;
                text(
                    &mut scene,
                    26,
                    118,
                    if self.selection == Selection::Seed {
                        "> CONDUITOS ENTRANCE SEED"
                    } else {
                        "  CONDUITOS ENTRANCE SEED"
                    },
                )?;
                text(
                    &mut scene,
                    26,
                    140,
                    if self.selection == Selection::Details {
                        "> DETAILS"
                    } else {
                        "  DETAILS"
                    },
                )?;
                text(&mut scene, 18, 176, "ARROWS SELECT  ENTER OPEN  F2 DETAILS")?;
            }
            Status::SeedOpened => {
                text(&mut scene, 18, 42, "THIS HOST    BODY: NONE")?;
                text(&mut scene, 18, 76, "SEED OPEN / INSPECTION ONLY")?;
                exact_text(&mut scene, 18, 100, self.seed_id.as_str())?;
                text(
                    &mut scene,
                    18,
                    150,
                    "PROVENANCE: CHECKED DATA EMBEDDED IN THIS IMAGE",
                )?;
                text(
                    &mut scene,
                    18,
                    176,
                    "NO BODY / WAKE / PLAN / PLAY / EFFECT CREATED",
                )?;
            }
            Status::DetailsOpened => {
                let (label, value) = self.detail();
                text(&mut scene, 18, 42, "EXACT HOST DETAILS")?;
                text(&mut scene, 18, 76, label)?;
                exact_text(&mut scene, 18, 100, &value)?;
                text(&mut scene, 18, 160, "F2 NEXT DETAIL    ESC WORLD")?;
            }
        }
        Ok(scene)
    }

    fn detail(&self) -> (&'static str, String) {
        match self.details_page {
            0 => ("PROFILE ID", self.profile_id.clone()),
            1 => ("BUILD ID", self.build_id.clone()),
            2 => ("IMAGE BINDING", self.image_id.clone()),
            3 => ("HOST ID", self.host_id.as_str().into()),
            4 => ("BOOT ID", self.boot_id.as_str().into()),
            _ => (
                "CURRENT OFFERS",
                format!(
                    "COUNT {} / GENERATION {}",
                    self.offer_count, self.offer_generation.0
                ),
            ),
        }
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

fn text(scene: &mut GraphicsScene, x: i16, y: i16, value: &str) -> Result<(), Error> {
    let bounds = LayoutRect {
        x,
        y,
        width: 610,
        height: 12,
    };
    scene
        .push(
            GraphicsCommand::text(bounds, bounds, GraphicsPaintRole::Foreground, value)
                .map_err(|_| Error::Scene)?,
        )
        .map_err(|_| Error::Scene)
}

fn exact_text(scene: &mut GraphicsScene, x: i16, y: i16, value: &str) -> Result<(), Error> {
    let split = value
        .len()
        .min(conduit_presentation::MAX_GRAPHICS_TEXT_BYTES);
    text(scene, x, y, &value[..split])?;
    if split < value.len() {
        text(scene, x, y + 22, &value[split..])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
