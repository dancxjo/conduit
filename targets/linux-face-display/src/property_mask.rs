//! A property-focused Look of the same immutable Face. Selects readable Info
//! properties; all decoding, quantity precision, and units use Face formatting.
//! No sensor protocol, Body identity, freshness decision, or new Face is made.
use conduit_face::{
    Face, FacePropertyValue, FaceRole, GraphicsCommand, GraphicsPaintRole, GraphicsScene,
    GraphicsShapeStyle, GraphicsTextRole, LayoutRect, display_typed_value,
};
use conduitos::display::typography::TextLayout;
struct Row {
    label: String,
    value: String,
    quantity: bool,
}
fn rows(face: &Face) -> Option<Vec<Row>> {
    let mut rows = Vec::new();
    for subject in &face.subjects {
        if subject.role != FaceRole::Info {
            continue;
        }
        for property in face
            .properties
            .iter()
            .filter(|p| p.subject == subject.identity)
        {
            let (value, quantity) = match &property.value {
                FacePropertyValue::TypedValue { contract, bytes } => {
                    (display_typed_value(contract, bytes), true)
                }
                FacePropertyValue::Text(value) => (value.clone(), false),
                FacePropertyValue::Flag(value) => (value.to_string(), false),
                _ => continue,
            };
            let label = format!("{} / {}", subject.name, property.name.replace('-', " "));
            // Larger documents retain the ordinary scene's complete bounded
            // wrapping/pagination rather than truncating a semantic value.
            if label.len() > 192 || value.len() > 192 {
                return None;
            }
            rows.push(Row {
                label,
                value,
                quantity,
            });
        }
    }
    // Present finite typed values first, retaining authored order within each
    // class. Plain status rows remain directly alongside those quantities.
    rows.sort_by_key(|row| !row.quantity);
    (!rows.is_empty()).then_some(rows)
}
fn text(
    scene: &mut GraphicsScene,
    screen: LayoutRect,
    bounds: LayoutRect,
    value: &str,
    role: GraphicsTextRole,
    paint: GraphicsPaintRole,
) -> Option<()> {
    if TextLayout::new_scaled(value, role.into(), bounds.width, 100)
        .ok()?
        .finish_height()
        > u32::from(bounds.height)
    {
        return None;
    }
    scene
        .push(
            GraphicsCommand::text(bounds, screen, paint, value)
                .ok()?
                .with_text_role(role)
                .ok()?,
        )
        .ok()?;
    Some(())
}
/// Selects one finite property page. `None` retains the complete ordinary Face
/// scene as the refusal/fallback, never partial or fabricated sensor wording.
pub fn frame(
    face: &Face,
    width: u16,
    height: u16,
    tick: usize,
) -> Option<(GraphicsScene, usize, usize)> {
    let rows = rows(face)?;
    let count = usize::from(height.checked_sub(144)? / 84);
    if count == 0 {
        return None;
    }
    let pages = rows.len().div_ceil(count);
    let page = tick % pages;
    let screen = LayoutRect {
        x: 0,
        y: 0,
        width,
        height,
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
            .ok()?,
        )
        .ok()?;
    text(
        &mut scene,
        screen,
        LayoutRect {
            x: 24,
            y: 16,
            width: width.checked_sub(48)?,
            height: 40,
        },
        "Conduit",
        GraphicsTextRole::Title,
        GraphicsPaintRole::Accent,
    )?;
    for (i, row) in rows.iter().skip(page * count).take(count).enumerate() {
        let y = 72 + i as i16 * 84;
        text(
            &mut scene,
            screen,
            LayoutRect {
                x: 32,
                y,
                width: width.checked_sub(64)?,
                height: 24,
            },
            &row.label,
            GraphicsTextRole::Label,
            GraphicsPaintRole::Muted,
        )?;
        text(
            &mut scene,
            screen,
            LayoutRect {
                x: 32,
                y: y + 28,
                width: width.checked_sub(64)?,
                height: 48,
            },
            &row.value,
            if row.quantity {
                GraphicsTextRole::Title
            } else {
                GraphicsTextRole::Body
            },
            GraphicsPaintRole::Foreground,
        )?;
    }
    let footer = format!(
        "Face revision {} | {}/{} | Read only",
        face.revision,
        page + 1,
        pages
    );
    text(
        &mut scene,
        screen,
        LayoutRect {
            x: 24,
            y: height as i16 - 40,
            width: width.checked_sub(48)?,
            height: 32,
        },
        &footer,
        GraphicsTextRole::Label,
        GraphicsPaintRole::Muted,
    )?;
    Some((scene, page, pages))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn property_look_keeps_face_and_explicit_unavailable_status() {
        let mut face = crate::view::unavailable("Awaiting source");
        face.subjects[0].role = FaceRole::Info;
        face.properties.push(conduit_face::FaceProperty {
            subject: face.subjects[0].identity.clone(),
            name: "sensor-status".into(),
            value: FacePropertyValue::Text("unavailable".into()),
        });
        face.properties.push(conduit_face::FaceProperty {
            subject: face.subjects[0].identity.clone(),
            name: "fresh".into(),
            value: FacePropertyValue::Flag(false),
        });
        // Reconstruct only this test fixture's content identity after editing it.
        face = Face::new(
            7,
            face.basis,
            face.subjects,
            face.relationships,
            face.properties,
            face.text,
        )
        .unwrap();
        let original = face.clone();
        let (scene, _, _) = frame(&face, 1024, 600, 0).unwrap();
        assert_eq!(face, original);
        assert!(
            scene
                .commands()
                .iter()
                .any(|c| c.payload() == "unavailable")
        );
        assert!(scene.commands().iter().any(|c| c.payload() == "false"));
    }
}
