//! Bounded no-Body entrance for an ordinary ConduitOS boot.

use conduit_body::{
    AuthenticatedHostObservation, Body, BodyMembership, BodyMembershipRevision, MembershipProofId,
    PartId,
};
use conduit_core::{BootId, CheckedFormId, HostId, OfferGeneration, SignId, SourceDocumentId};
use conduit_presentation::{
    GraphicsCommand, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle, LayoutRect,
};

use crate::{
    arch::HidKeyTransition,
    display::{DisplayError, DisplayReceipt, PixelTarget, render_scene},
};

const ENTER: u8 = 40;
const TAB: u8 = 43;
const RIGHT: u8 = 79;
const LEFT: u8 = 80;
const DOWN: u8 = 81;
const UP: u8 = 82;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Selection {
    Birth,
    Join,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    AwaitingChoice,
    JoinUnavailable,
    BodyBorn,
}

pub struct BornBody {
    pub body: Body,
    pub membership: BodyMembership,
}

pub struct FrontDoor {
    host_id: HostId,
    boot_id: BootId,
    offer_generation: OfferGeneration,
    source_document_id: SourceDocumentId,
    checked_form_id: CheckedFormId,
    selection: Selection,
    status: Status,
    born: Option<BornBody>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Lifecycle,
    Display(DisplayError),
    Scene,
}

impl Error {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lifecycle => "front-door-lifecycle-refused",
            Self::Display(error) => error.as_str(),
            Self::Scene => "front-door-presentation-refused",
        }
    }
}

impl FrontDoor {
    pub fn new(
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        source_document_id: SourceDocumentId,
        checked_form_id: CheckedFormId,
    ) -> Self {
        Self {
            host_id,
            boot_id,
            offer_generation,
            source_document_id,
            checked_form_id,
            selection: Selection::Birth,
            status: Status::AwaitingChoice,
            born: None,
        }
    }

    pub const fn selection(&self) -> Selection {
        self.selection
    }
    pub const fn status(&self) -> Status {
        self.status
    }
    pub fn born(&self) -> Option<&BornBody> {
        self.born.as_ref()
    }

    pub fn accept(&mut self, transition: HidKeyTransition) -> Result<bool, Error> {
        if !transition.pressed() || self.born.is_some() {
            return Ok(false);
        }
        match transition.usage() {
            TAB | RIGHT | LEFT | DOWN | UP => {
                self.selection = match self.selection {
                    Selection::Birth => Selection::Join,
                    Selection::Join => Selection::Birth,
                };
                self.status = Status::AwaitingChoice;
                Ok(true)
            }
            ENTER if self.selection == Selection::Join => {
                self.status = Status::JoinUnavailable;
                Ok(true)
            }
            ENTER => {
                self.birth()?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn birth(&mut self) -> Result<(), Error> {
        let body = Body::born(
            self.source_document_id.clone(),
            self.checked_form_id.clone(),
            1,
            SignId::from("conduitos/front-door/body-born/1"),
        )
        .map_err(|_| Error::Lifecycle)?;
        let part_id =
            PartId::bind(&body.body_id, self.host_id.as_str(), 1).map_err(|_| Error::Lifecycle)?;
        let proof_id = MembershipProofId::bind("explicit-conduitos-local-birth")
            .map_err(|_| Error::Lifecycle)?;
        let mut membership =
            BodyMembership::new(body.body_id.clone()).map_err(|_| Error::Lifecycle)?;
        membership
            .admit(
                &body.body_id,
                BodyMembershipRevision(0),
                part_id.clone(),
                proof_id.clone(),
                SignId::from("conduitos/front-door/part-admitted/1"),
            )
            .map_err(|_| Error::Lifecycle)?;
        membership
            .observe_present(
                &body.body_id,
                BodyMembershipRevision(1),
                &part_id,
                AuthenticatedHostObservation {
                    host_id: self.host_id.clone(),
                    boot_id: self.boot_id.clone(),
                    offer_generation: self.offer_generation,
                    proof_id,
                    sequence: 1,
                },
                SignId::from("conduitos/front-door/host-attached/1"),
            )
            .map_err(|_| Error::Lifecycle)?;
        body.validate().map_err(|_| Error::Lifecycle)?;
        membership.validate().map_err(|_| Error::Lifecycle)?;
        self.born = Some(BornBody { body, membership });
        self.status = Status::BodyBorn;
        Ok(())
    }

    pub fn render(&self, display: &mut impl PixelTarget) -> Result<DisplayReceipt, Error> {
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
        text(&mut scene, 18, 18, "CONDUIT / PATCHBAY")?;
        text(&mut scene, 18, 42, "THIS HOST DOES NOT BELONG TO A BODY")?;
        text(
            &mut scene,
            26,
            82,
            if self.selection == Selection::Birth {
                "> CREATE A NEW BODY"
            } else {
                "  CREATE A NEW BODY"
            },
        )?;
        text(
            &mut scene,
            26,
            104,
            if self.selection == Selection::Join {
                "> JOIN AN EXISTING BODY"
            } else {
                "  JOIN AN EXISTING BODY"
            },
        )?;
        text(&mut scene, 18, 146, "ARROWS/TAB SELECT  ENTER CONFIRM")?;
        text(
            &mut scene,
            18,
            174,
            match self.status {
                Status::AwaitingChoice => "READY",
                Status::JoinUnavailable => "JOIN UNAVAILABLE: NO ADMITTED BODY CANDIDATE",
                Status::BodyBorn => "BODY BORN; THIS HOST IS ATTACHED",
            },
        )?;
        render_scene(display, &scene).map_err(Error::Display)
    }
}

fn text(scene: &mut GraphicsScene, x: i16, y: i16, value: &str) -> Result<(), Error> {
    let bounds = LayoutRect {
        x,
        y,
        width: 290,
        height: 12,
    };
    scene
        .push(
            GraphicsCommand::text(bounds, bounds, GraphicsPaintRole::Foreground, value)
                .map_err(|_| Error::Scene)?,
        )
        .map_err(|_| Error::Scene)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn door() -> FrontDoor {
        FrontDoor::new(
            HostId::from("host"),
            BootId::from("boot"),
            OfferGeneration(3),
            SourceDocumentId::from("source"),
            CheckedFormId::from("checked"),
        )
    }
    fn key(usage: u8) -> HidKeyTransition {
        HidKeyTransition::new(usage, true, 0)
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
    fn entrance_renders_as_a_finite_scene() {
        let receipt = door().render(&mut Sink).unwrap();
        assert_eq!(receipt.commands, 7);
        assert!(receipt.pixels_written > 0);
    }

    #[test]
    fn birth_attaches_the_exact_current_host_and_boot() {
        let mut door = door();
        assert!(door.accept(key(ENTER)).unwrap());
        let born = door.born().unwrap();
        assert_eq!(door.status(), Status::BodyBorn);
        let observation = born.membership.parts[0].current.as_ref().unwrap();
        assert_eq!(observation.host_id.as_str(), "host");
        assert_eq!(observation.boot_id.as_str(), "boot");
        assert_eq!(observation.offer_generation, OfferGeneration(3));
    }

    #[test]
    fn unavailable_join_refuses_without_birthing() {
        let mut door = door();
        assert!(door.accept(key(TAB)).unwrap());
        assert!(door.accept(key(ENTER)).unwrap());
        assert_eq!(door.status(), Status::JoinUnavailable);
        assert!(door.born().is_none());
    }

    #[test]
    fn releases_and_unrelated_keys_do_not_act() {
        let mut door = door();
        assert!(!door.accept(HidKeyTransition::new(ENTER, false, 0)).unwrap());
        assert!(!door.accept(key(4)).unwrap());
    }
}
