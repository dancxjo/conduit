//! Canonical host-neutral execution of the accepted presentation nucleus.

use alloc::{
    format,
    string::{String, ToString},
};
use conduit_core::{ConfigurationValue, PlannedGear};
use conduit_presentation::{
    GraphicsCommand, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle, LayoutAlignment,
    LayoutAxis, LayoutFrame, LayoutRect, PresentationComposition, PresentationIconKey,
};

pub fn execute_layout_source(placement: &PlannedGear) -> Result<LayoutFrame, String> {
    if placement.kind_id.as_str() != super::LAYOUT_VIEWPORT_KIND {
        return Err("layout source is not layout/viewport".into());
    }
    LayoutFrame::viewport(
        u16_config(placement, super::WIDTH_KEY)?,
        u16_config(placement, super::HEIGHT_KEY)?,
        u8::try_from(u16_config(placement, super::CHILDREN_KEY)?)
            .map_err(|_| "layout child count is out of range".to_string())?,
        u16_config(placement, super::CHILD_WIDTH_KEY)?,
        u16_config(placement, super::CHILD_HEIGHT_KEY)?,
    )
    .map_err(|error| format!("layout viewport refused: {error:?}"))
}

pub fn execute_layout_transform(
    placement: &PlannedGear,
    frame: LayoutFrame,
) -> Result<LayoutFrame, String> {
    match placement.kind_id.as_str() {
        super::LAYOUT_INSET_KIND => frame.inset(u16_config(placement, super::INSET_KEY)?),
        super::LAYOUT_ROW_KIND => frame.distribute(
            LayoutAxis::Horizontal,
            u16_config(placement, super::GAP_KEY)?,
        ),
        super::LAYOUT_COLUMN_KIND => {
            frame.distribute(LayoutAxis::Vertical, u16_config(placement, super::GAP_KEY)?)
        }
        super::LAYOUT_STACK_KIND => Ok(frame.stack()),
        super::LAYOUT_ALIGN_KIND => frame.align(
            alignment(placement, super::HORIZONTAL_KEY)?,
            alignment(placement, super::VERTICAL_KEY)?,
        ),
        _ => return Err("unsupported layout transform".into()),
    }
    .map_err(|error| format!("layout transform refused: {error:?}"))
}

pub fn execute_presentation_source(
    placement: &PlannedGear,
) -> Result<PresentationComposition, String> {
    if placement.kind_id.as_str() != super::PRESENTATION_ICON_KIND {
        return Err("presentation source is not presentation/icon".into());
    }
    PresentationComposition::icon(
        text_config(placement, super::ICON_KEY)?,
        text_config(placement, super::ACCESSIBILITY_NAME_KEY)?,
    )
    .map_err(|error| format!("presentation icon refused: {error:?}"))
}

pub fn execute_presentation_transform(
    placement: &PlannedGear,
    value: PresentationComposition,
) -> Result<PresentationComposition, String> {
    match placement.kind_id.as_str() {
        super::PRESENTATION_FRAME_KIND => value.frame(
            text_config(placement, super::ROLE_KEY)?,
            text_config(placement, super::ACCESSIBILITY_NAME_KEY)?,
        ),
        super::PRESENTATION_BADGE_KIND => value.badge(
            text_config(placement, super::STATE_KEY)?,
            text_config(placement, super::ACCESSIBILITY_NAME_KEY)?,
        ),
        _ => return Err("unsupported presentation composition transform".into()),
    }
    .map_err(|error| format!("presentation composition refused: {error:?}"))
}

pub fn execute_graphics_transform(
    placement: &PlannedGear,
    composition: Option<PresentationComposition>,
    scene: Option<GraphicsScene>,
) -> Result<GraphicsScene, String> {
    let bounds = configured_rect(placement, false)?;
    let clip = configured_rect(placement, true)?;
    let paint = match text_config(placement, super::PAINT_KEY)? {
        "background" => GraphicsPaintRole::Background,
        "foreground" => GraphicsPaintRole::Foreground,
        "accent" => GraphicsPaintRole::Accent,
        "status" => GraphicsPaintRole::Status,
        _ => return Err("unsupported graphics paint role".into()),
    };
    let mut output = scene.unwrap_or_else(GraphicsScene::empty);
    let command = match placement.kind_id.as_str() {
        super::GRAPHICS_RECT_KIND => {
            if !scene_is_absent_and_composition_present(scene, composition) {
                return Err(
                    "graphics rectangle requires a resolved presentation obligation".into(),
                );
            }
            GraphicsCommand::rect(
                bounds,
                clip,
                paint,
                match text_config(placement, super::STYLE_KEY)? {
                    "fill" => GraphicsShapeStyle::Fill,
                    "stroke" => GraphicsShapeStyle::Stroke,
                    _ => return Err("unsupported graphics shape style".into()),
                },
            )
        }
        super::GRAPHICS_TEXT_KIND if composition.is_none() && scene.is_some() => {
            GraphicsCommand::text(
                bounds,
                clip,
                paint,
                text_config(placement, super::GRAPHICS_TEXT_KEY)?,
            )
        }
        super::GRAPHICS_ICON_KIND if composition.is_none() && scene.is_some() => {
            GraphicsCommand::icon(
                bounds,
                clip,
                paint,
                PresentationIconKey::from_token(text_config(placement, super::GRAPHICS_ICON_KEY)?)
                    .ok_or_else(|| "unknown canonical graphics icon".to_string())?,
            )
        }
        _ => return Err("unsupported graphics input or Kind".into()),
    }
    .map_err(|error| format!("graphics command refused: {error:?}"))?;
    output
        .push(command)
        .map_err(|error| format!("graphics scene refused: {error:?}"))?;
    Ok(output)
}

fn scene_is_absent_and_composition_present(
    scene: Option<GraphicsScene>,
    composition: Option<PresentationComposition>,
) -> bool {
    scene.is_none()
        && composition
            .map(|value| !value.items().is_empty())
            .unwrap_or(false)
}

fn configured_rect(placement: &PlannedGear, clip: bool) -> Result<LayoutRect, String> {
    let keys = if clip {
        [
            super::CLIP_X_KEY,
            super::CLIP_Y_KEY,
            super::CLIP_WIDTH_KEY,
            super::CLIP_HEIGHT_KEY,
        ]
    } else {
        [
            super::GRAPHICS_X_KEY,
            super::GRAPHICS_Y_KEY,
            super::GRAPHICS_WIDTH_KEY,
            super::GRAPHICS_HEIGHT_KEY,
        ]
    };
    Ok(LayoutRect {
        x: i16::try_from(u64_config(placement, keys[0])?)
            .map_err(|_| "graphics x coordinate overflows".to_string())?,
        y: i16::try_from(u64_config(placement, keys[1])?)
            .map_err(|_| "graphics y coordinate overflows".to_string())?,
        width: u16::try_from(u64_config(placement, keys[2])?)
            .map_err(|_| "graphics width overflows".to_string())?,
        height: u16::try_from(u64_config(placement, keys[3])?)
            .map_err(|_| "graphics height overflows".to_string())?,
    })
}

fn u64_config(placement: &PlannedGear, key: &str) -> Result<u64, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (found, ConfigurationValue::U64(value)) if found == key => Some(*value),
            _ => None,
        })
        .ok_or_else(|| format!("presentation configuration '{key}' is missing"))
}

fn u16_config(placement: &PlannedGear, key: &str) -> Result<u16, String> {
    u16::try_from(u64_config(placement, key)?)
        .map_err(|_| format!("layout configuration '{key}' is out of range"))
}

fn text_config<'a>(placement: &'a PlannedGear, key: &str) -> Result<&'a str, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (found, ConfigurationValue::Text(value)) if found == key => Some(value.as_str()),
            _ => None,
        })
        .ok_or_else(|| format!("presentation configuration '{key}' is missing"))
}

fn alignment(placement: &PlannedGear, key: &str) -> Result<LayoutAlignment, String> {
    match text_config(placement, key)? {
        "start" => Ok(LayoutAlignment::Start),
        "center" => Ok(LayoutAlignment::Center),
        "end" => Ok(LayoutAlignment::End),
        _ => Err(format!("layout alignment '{key}' is invalid")),
    }
}
