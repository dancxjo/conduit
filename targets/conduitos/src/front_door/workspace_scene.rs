//! Quiet foreground surface for the born Body, projected from its current journey.
use super::Error;
use crate::{
    display::PixelTarget,
    product_journey::{JourneyProjection, JourneyStatus},
};
use alloc::format;
use conduit_presentation::{
    GraphicsCommand, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle, LayoutRect,
};

pub(super) fn scene(
    journey: &JourneyProjection,
    refusal: Option<&str>,
    display: &impl PixelTarget,
) -> Result<GraphicsScene, Error> {
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
    text(
        &mut scene,
        screen,
        24,
        "CONDUIT",
        GraphicsPaintRole::Foreground,
    )?;
    text(
        &mut scene,
        screen,
        64,
        journey.friendly_name.as_deref().unwrap_or("My Body"),
        GraphicsPaintRole::Accent,
    )?;
    text(
        &mut scene,
        screen,
        104,
        "Keyboard canvas",
        GraphicsPaintRole::Foreground,
    )?;
    let status = match journey.status {
        JourneyStatus::BornLulled => "Body born. Preparing to wake its Form...",
        JourneyStatus::Awake => "Body awake. Planning its Form...",
        JourneyStatus::Planned => "Form ready. Starting its Play...",
        JourneyStatus::Playing => "Listening. Type to send a key through your Form.",
        JourneyStatus::ResultVisible => "Play completed. Your result is below.",
        JourneyStatus::Stopped => "Play stopped.",
        JourneyStatus::Lulled => "Body lulled. Its Form remains included.",
        _ => "Preparing your Body...",
    };
    text(
        &mut scene,
        screen,
        144,
        status,
        GraphicsPaintRole::Foreground,
    )?;
    if let Some(result) = &journey.result {
        text(&mut scene, screen, 208, result, GraphicsPaintRole::Accent)?;
    }
    if let Some(refusal) = refusal {
        text(
            &mut scene,
            screen,
            288,
            "Wake could not finish. Details:",
            GraphicsPaintRole::Status,
        )?;
        text(&mut scene, screen, 320, refusal, GraphicsPaintRole::Status)?;
    }
    let footer_y = i16::try_from(screen.height.saturating_sub(48)).map_err(|_| Error::Scene)?;
    text(
        &mut scene,
        screen,
        footer_y,
        &format!(
            "{}  ·  F2 details  ·  F9 Tour  ·  F8 Stop",
            journey.status.as_str()
        ),
        GraphicsPaintRole::Foreground,
    )?;
    Ok(scene)
}

fn text(
    scene: &mut GraphicsScene,
    screen: LayoutRect,
    y: i16,
    value: &str,
    role: GraphicsPaintRole,
) -> Result<(), Error> {
    let bounds = LayoutRect {
        x: 32,
        y,
        width: screen.width.saturating_sub(64),
        height: 48,
    };
    scene
        .push(GraphicsCommand::text(bounds, screen, role, value).map_err(|_| Error::Scene)?)
        .map_err(|_| Error::Scene)
}
