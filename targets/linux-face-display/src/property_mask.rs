//! A property-focused Look of the same immutable Face. Selects readable Info
//! properties; all decoding, quantity precision, and units use Face formatting.
//! No sensor protocol, Body identity, freshness decision, or new Face is made.
use conduit_face::{
    Face, FacePropertyValue, FaceRole, GraphicsCommand, GraphicsPaintRole, GraphicsScene,
    GraphicsShapeStyle, GraphicsTextRole, LayoutRect, display_typed_value,
};
use conduitos::display::typography::TextLayout;
// Local Mask preference uses the existing checked Quantity conversion. It does
// not change the input Face, contract, or its canonical measurement bytes.
fn display_typed_value_in_unit(
    contract: &conduit_core::CheckedValueContract,
    bytes: &[u8],
    unit: conduit_core::Unit,
) -> Option<String> {
    contract.validate(bytes).ok()?;
    let value = conduit_core::Quantity::decode(bytes).ok()?;
    if !matches!(
        value.unit(),
        conduit_core::Unit::Celsius | conduit_core::Unit::Fahrenheit
    ) {
        return None;
    }
    let converted = value.convert_to_decimal(unit).ok()?;
    Some(display_typed_value(contract, &converted.encode()))
}

struct Row {
    label: String,
    value: String,
    quantity: bool,
}
fn rows(face: &Face, preferred: Option<conduit_core::Unit>) -> Option<Vec<Row>> {
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
                FacePropertyValue::TypedValue { contract, bytes } => (
                    preferred
                        .and_then(|unit| display_typed_value_in_unit(contract, bytes, unit))
                        .unwrap_or_else(|| display_typed_value(contract, bytes)),
                    true,
                ),
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
    preferred: Option<conduit_core::Unit>,
) -> Option<(GraphicsScene, usize, usize)> {
    let rows = rows(face, preferred)?;
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
            y: 16 + crate::view::STATUS_STRIP_HEIGHT as i16,
            width: width.checked_sub(48)?,
            height: 40,
        },
        "Conduit",
        GraphicsTextRole::Title,
        GraphicsPaintRole::Accent,
    )?;
    for (i, row) in rows.iter().skip(page * count).take(count).enumerate() {
        let y = 72 + crate::view::STATUS_STRIP_HEIGHT as i16 + i as i16 * 84;
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
    fn preferred_temperature_unit_keeps_exact_face_and_other_values() {
        use conduit_core::{
            CheckedValueContract, QUANTITY_ENCODED_LEN, QUANTITY_INFO_ID, Quantity, Unit, kind_id,
        };
        let mut face = crate::view::unavailable("Awaiting source");
        face.subjects[0].role = FaceRole::Info;
        face.properties.push(conduit_face::FaceProperty {
            subject: face.subjects[0].identity.clone(),
            name: "value".into(),
            value: FacePropertyValue::TypedValue {
                contract: CheckedValueContract::new(
                    kind_id(QUANTITY_INFO_ID),
                    QUANTITY_ENCODED_LEN as u32,
                    vec![],
                )
                .unwrap(),
                bytes: Quantity::from_decimal(225, -1, Unit::Celsius)
                    .unwrap()
                    .encode()
                    .to_vec(),
            },
        });
        face.properties.push(conduit_face::FaceProperty {
            subject: face.subjects[0].identity.clone(),
            name: "ready".into(),
            value: FacePropertyValue::Flag(true),
        });
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
        let canonical = rows(&face, None).unwrap();
        assert_eq!(canonical[0].value, "22.5 °C");
        let preferred = rows(&face, Some(Unit::Fahrenheit)).unwrap();
        assert_eq!(preferred[0].value, "72.5 °F");
        assert_eq!(preferred[1].value, "true");
        assert_eq!(face, original);
    }
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
        let (scene, _, _) = frame(&face, 1024, 600, 0, None).unwrap();
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

#[cfg(test)]
mod status_layout_tests {
    use super::*;
    #[test]
    fn all_property_pages_reserve_notice_strip_without_moving_footer_or_losing_rows() {
        let mut face = crate::view::unavailable("Awaiting source");
        face.subjects[0].role = FaceRole::Info;
        for i in 0..12 {
            face.properties.push(conduit_face::FaceProperty {
                subject: face.subjects[0].identity.clone(),
                name: format!("reading-{i}"),
                value: FacePropertyValue::Text(format!("value-{i}")),
            });
        }
        let mut values = Vec::new();
        for page in 0..3 {
            let (scene, actual, pages) = frame(&face, 1024, 600, page, None).unwrap();
            assert_eq!((actual, pages), (page, 3));
            assert!(scene.commands().len() <= 13);
            for command in scene.commands() {
                if command.payload().is_empty() {
                    continue;
                }
                assert!(command.bounds.y >= crate::view::STATUS_STRIP_HEIGHT as i16);
                if command.payload().starts_with("Face revision") {
                    assert_eq!(command.bounds.y, 560);
                    assert_eq!(command.bounds.height, 32);
                } else {
                    assert!(i32::from(command.bounds.y) + i32::from(command.bounds.height) <= 560);
                }
                if command.payload().starts_with("value-") {
                    values.push(command.payload().to_owned());
                }
            }
        }
        assert_eq!(
            values,
            (0..12).map(|i| format!("value-{i}")).collect::<Vec<_>>()
        );
    }
}
