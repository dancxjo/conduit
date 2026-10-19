//! Verify source-owned reversible Boolean meaning; no application identifiers.
use super::*;
use conduit_face::FacePropertyValue;

/// Returns the authoritative value only for an unambiguous explicit association
/// whose one-argument contract accepts both replacements. Availability is kept
/// separate: disabled choices still have a truthful current selected value.
pub(super) fn selected(face: &Face, action_index: usize) -> Option<bool> {
    let action = face.actions.get(action_index)?;
    if action.arguments.len() != 1 {
        return None;
    }
    let argument = &action.arguments[0];
    if argument.contract.value_kind.as_str() != "value/bool"
        || argument.contract.maximum_bytes != 1
        || argument.contract.validate(&[0]).is_err()
        || argument.contract.validate(&[1]).is_err()
    {
        return None;
    }
    let mut values = face
        .properties
        .iter()
        .filter(|property| property.subject == action.target && property.name == "selected");
    let FacePropertyValue::Flag(value) = &values.next()?.value else {
        return None;
    };
    if values.next().is_some() {
        return None;
    }
    let mut bindings = face.properties.iter().filter(|property| {
        property.subject == action.target && property.name == "boolean-change-action"
    });
    let FacePropertyValue::Identity(identity) = &bindings.next()?.value else {
        return None;
    };
    if identity != &action.identity || bindings.next().is_some() {
        return None;
    }
    Some(*value)
}

pub(super) fn applies(face: &Face, control: FaceControl) -> bool {
    control.argument.is_none_or(|index| index == 0) && selected(face, control.action).is_some()
}

pub(super) const INSET: u16 = 26;

/// The square and mark are distinct from the action-row/focus outlines.
pub(super) fn append(
    scene: &mut GraphicsScene,
    row: &layout::Row,
    screen: LayoutRect,
) -> Result<(), FaceSceneError> {
    let Some(checked) = row.checkbox else {
        return Ok(());
    };
    let bounds = LayoutRect {
        x: row.bounds.x + row.padding as i16,
        y: row.bounds.y + row.padding as i16,
        width: 18,
        height: 18,
    };
    push(
        scene,
        GraphicsCommand::rect(bounds, screen, row.paint, GraphicsShapeStyle::Stroke),
    )?;
    if checked {
        push(
            scene,
            GraphicsCommand::rect(
                LayoutRect {
                    x: bounds.x + 4,
                    y: bounds.y + 4,
                    width: 10,
                    height: 10,
                },
                screen,
                row.paint,
                GraphicsShapeStyle::Fill,
            ),
        )?;
    }
    Ok(())
}
