use alloc::{format, string::String};

use conduit_presentation::{
    GraphicsCommand, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle, LayoutRect,
};

use crate::{
    display::PixelTarget,
    product_journey::{JourneyProjection, JourneyStatus},
};

use super::{Error, FrontDoor, Selection, Status};

impl FrontDoor {
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
        if let Some(journey) = &self.journey
            && !matches!(
                journey.status,
                JourneyStatus::World | JourneyStatus::SeedOpened
            )
            && self.status != Status::DetailsOpened
        {
            text(
                &mut scene,
                18,
                42,
                &format!("PRODUCT JOURNEY    {:?}", journey.status),
            )?;
            if let Some(body_id) = &journey.body_id {
                exact_text(&mut scene, 18, 66, body_id.as_str())?;
            }
            let (action, result) = match journey.status {
                JourneyStatus::BornLulled => ("F4 WAKE", "BODY BORN / LULLED"),
                JourneyStatus::Awake => ("F5 PLAN", "WAKE AWAITS EXACT PLAN"),
                JourneyStatus::Planned => ("F6 PLAY", "PLAN READY FOR INSPECTION"),
                JourneyStatus::Playing => ("TYPE A KEY", "PRODUCTION KERNEL AWAITS INPUT"),
                JourneyStatus::ResultVisible => (
                    "F7 LULL",
                    journey.result.as_deref().unwrap_or("RESULT VISIBLE"),
                ),
                JourneyStatus::Stopped => ("F7 LULL", "PLAY STOPPED / NO LATE VALUE"),
                JourneyStatus::Lulled => ("F2 DETAILS", "BODY RETAINED / WAKE ENDED"),
                JourneyStatus::World | JourneyStatus::SeedOpened => unreachable!(),
            };
            text(&mut scene, 18, 132, result)?;
            text(&mut scene, 18, 160, action)?;
            text(&mut scene, 18, 184, "F8 STOP    F2 EXACT DETAILS")?;
            return Ok(scene);
        }
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
                text(&mut scene, 18, 176, "NO EFFECT CREATED    F3 BE BORN")?;
            }
            Status::DetailsOpened => {
                let (label, value) = self.current_detail();
                text(&mut scene, 18, 42, "EXACT HOST DETAILS")?;
                text(&mut scene, 18, 76, label)?;
                exact_text(&mut scene, 18, 100, &value)?;
                text(&mut scene, 18, 160, "F2 NEXT DETAIL    ESC WORLD")?;
            }
        }
        Ok(scene)
    }

    pub fn current_detail(&self) -> (&'static str, String) {
        match self.details_page {
            0 => ("PROFILE ID", self.profile_id.clone()),
            1 => ("BUILD ID", self.build_id.clone()),
            2 => ("IMAGE BINDING", self.image_id.clone()),
            3 => ("HOST ID", self.host_id.as_str().into()),
            4 => ("BOOT ID", self.boot_id.as_str().into()),
            5 => (
                "CURRENT OFFERS",
                format!(
                    "COUNT {} / GENERATION {}",
                    self.offer_count, self.offer_generation.0
                ),
            ),
            6 => ("SEED ID", self.seed_id.as_str().into()),
            7 => (
                "SOURCE DOCUMENT ID",
                self.source_document_id.as_str().into(),
            ),
            8 => ("CHECKED FORM ID", self.checked_form_id.as_str().into()),
            9 => (
                "EXPANDED FORM ID",
                self.journey
                    .as_ref()
                    .map(|value| value.expanded_form_id.as_str().into())
                    .unwrap_or_else(|| "NONE".into()),
            ),
            10 => lifecycle_detail(
                &self.journey,
                |value| value.body_id.as_ref().map(|id| id.as_str()),
                "BODY ID",
            ),
            11 => lifecycle_detail(
                &self.journey,
                |value| value.wake_id.as_ref().map(|id| id.as_str()),
                "WAKE ID",
            ),
            12 => lifecycle_detail(
                &self.journey,
                |value| value.plan_id.as_ref().map(|id| id.as_str()),
                "PLAN ID",
            ),
            13 => lifecycle_detail(
                &self.journey,
                |value| value.active_play_id.as_ref().map(|id| id.as_str()),
                "ACTIVE PLAY ID",
            ),
            14 => lifecycle_detail(
                &self.journey,
                |value| value.input_sign_id.as_ref().map(|id| id.as_str()),
                "INPUT SIGN ID",
            ),
            _ => lifecycle_detail(
                &self.journey,
                |value| value.result_sign_id.as_ref().map(|id| id.as_str()),
                "RESULT SIGN ID",
            ),
        }
    }
}

fn lifecycle_detail<'a>(
    journey: &'a Option<JourneyProjection>,
    select: impl FnOnce(&'a JourneyProjection) -> Option<&'a str>,
    label: &'static str,
) -> (&'static str, String) {
    (
        label,
        journey
            .as_ref()
            .and_then(select)
            .map(Into::into)
            .unwrap_or_else(|| "NONE".into()),
    )
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
