//! One canonical general World/details/connectivity/inspection layout.
use super::{BirthAction, Detail, Facts};
use crate::{control::View, data::Refusal};
use alloc::{format, string::String};
use conduit_face::{
    GraphicsCommand, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle, GraphicsTextRole,
    LayoutRect,
};
pub fn scene(facts: &Facts, reading: View) -> Result<(bool, GraphicsScene), Refusal> {
    facts.validate()?;
    if facts.scope.width == 0 || facts.scope.height == 0 {
        return Err(Refusal::Bounds);
    }
    let screen = LayoutRect {
        x: 0,
        y: 0,
        width: facts.scope.width,
        height: facts.scope.height,
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
            .map_err(|_| Refusal::Layout)?,
        )
        .map_err(|_| Refusal::Layout)?;
    text(&mut scene, 18, 18, "CONDUIT / PATCHBAY / WORLD")?;
    if let Some(line) = &facts.connectivity
        && !reading.exact_details_open
    {
        text(&mut scene, 18, 42, "CURRENT BODY")?;
        exact_text(&mut scene, 18, 66, line.body.as_str())?;
        text(&mut scene, 18, 132, line.status.label())?;
        exact_text(&mut scene, 18, 156, &line.line)?;
        if let Some(value) = &line.value {
            exact_text(&mut scene, 18, 184, value)?;
        }
        return Ok((false, scene));
    }
    if facts.lifecycle[1].is_some() && !reading.exact_details_open {
        return Ok((true, GraphicsScene::empty()));
    }
    if reading.exact_details_open {
        let (kind, value) = current_detail(facts, reading);
        let label = kind.label();
        text(&mut scene, 18, 42, "EXACT HOST DETAILS")?;
        text(&mut scene, 18, 76, label)?;
        exact_text(&mut scene, 18, 100, &value)?;
        text(&mut scene, 18, 160, "F2 NEXT DETAIL    ESC WORLD")?;
    } else if reading.plot_open {
        text(&mut scene, 18, 42, "THIS HOST    BODY: NONE")?;
        text(&mut scene, 18, 76, "PLOT OPEN / INSPECTION ONLY")?;
        exact_text(&mut scene, 18, 100, &facts.plot)?;
        text(
            &mut scene,
            18,
            150,
            "PROVENANCE: CHECKED DATA EMBEDDED IN THIS IMAGE",
        )?;
        if let Some(action) = &facts.birth {
            text(&mut scene, 18, 176, &local_action_label(action))?;
        }
    } else {
        text(&mut scene, 18, 42, "THIS HOST    BODY: NONE")?;
        text(&mut scene, 18, 70, "BODIES NEARBY    NONE OBSERVED")?;
        text(&mut scene, 18, 96, "PLOTS")?;
        text(
            &mut scene,
            26,
            118,
            if reading.selected_plot {
                "> CONDUITOS ENTRANCE PLOT"
            } else {
                "  CONDUITOS ENTRANCE PLOT"
            },
        )?;
        text(
            &mut scene,
            26,
            140,
            if !reading.selected_plot {
                "> DETAILS"
            } else {
                "  DETAILS"
            },
        )?;
        text(
            &mut scene,
            18,
            176,
            "ARROWS SELECT  ENTER OPEN  F2 DETAILS  F9 TOUR",
        )?;
    }
    Ok((false, scene))
}

pub fn current_detail(facts: &Facts, reading: View) -> (Detail, String) {
    match reading.details_page {
        0 => (Detail::Profile, facts.profile.clone()),
        1 => (Detail::Build, facts.build.clone()),
        2 => (Detail::Image, facts.image.clone()),
        3 => (Detail::Host, facts.scope.control.host.as_str().into()),
        4 => (Detail::Boot, facts.scope.control.boot.as_str().into()),
        5 => (
            Detail::Offers,
            format!(
                "COUNT {} / GENERATION {}",
                facts.offer_count, facts.offer_generation
            ),
        ),
        6 => (Detail::Plot, facts.plot.clone()),
        7 => (Detail::Source, facts.source.clone()),
        8 => (Detail::Checked, facts.checked.clone()),
        page => {
            let index = usize::from(page.saturating_sub(9)).min(6);
            let kind = [
                Detail::Expanded,
                Detail::Body,
                Detail::Wake,
                Detail::Plan,
                Detail::Play,
                Detail::Input,
                Detail::Result,
            ][index];
            (
                kind,
                facts.lifecycle[index]
                    .clone()
                    .unwrap_or_else(|| "NONE".into()),
            )
        }
    }
}
fn local_action_label(action: &BirthAction) -> String {
    let availability = action
        .unavailable
        .as_ref()
        .map_or_else(String::new, |explanation| {
            format!(" / UNAVAILABLE: {explanation}")
        });
    format!(
        "{} {}{availability}",
        action.binding,
        action.name.to_ascii_uppercase()
    )
}
fn text(scene: &mut GraphicsScene, x: i16, y: i16, value: &str) -> Result<(), Refusal> {
    styled_text(
        scene,
        x,
        y,
        value,
        GraphicsPaintRole::Foreground,
        GraphicsTextRole::Body,
    )
}

fn styled_text(
    scene: &mut GraphicsScene,
    x: i16,
    y: i16,
    value: &str,
    role: GraphicsPaintRole,
    typography: GraphicsTextRole,
) -> Result<(), Refusal> {
    let bounds = LayoutRect {
        x,
        y,
        width: 610,
        height: 18,
    };
    scene
        .push(
            GraphicsCommand::text(bounds, bounds, role, value)
                .and_then(|command| command.with_text_role(typography))
                .map_err(|_| Refusal::Layout)?,
        )
        .map_err(|_| Refusal::Layout)
}

fn exact_text(scene: &mut GraphicsScene, x: i16, y: i16, value: &str) -> Result<(), Refusal> {
    let mut split = value.len().min(conduit_face::MAX_GRAPHICS_TEXT_BYTES);
    while !value.is_char_boundary(split) {
        split -= 1;
    }
    styled_text(
        scene,
        x,
        y,
        &value[..split],
        GraphicsPaintRole::Accent,
        GraphicsTextRole::Code,
    )?;
    if split < value.len() {
        styled_text(
            scene,
            x,
            y + 22,
            &value[split..],
            GraphicsPaintRole::Accent,
            GraphicsTextRole::Code,
        )?;
    }
    Ok(())
}
