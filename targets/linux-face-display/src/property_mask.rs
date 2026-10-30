//! A property-focused Mask of the same immutable Face. Selects readable Info
//! properties; all decoding, quantity precision, and units use Face formatting.
//! No sensor protocol, Body identity, freshness decision, or new Face is made.
use conduit_face::{
    Face, FacePropertyValue, FaceRole, GraphicsCommand, GraphicsPaintRole, GraphicsScene,
    GraphicsShapeStyle, GraphicsTextRole, LayoutRect, display_typed_value,
};
use conduitos::display::typography::{TextLayout, Typography};
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
    group: String,
    group_label: String,
    group_order: usize,
    label: String,
    value: String,
    quantity: bool,
}
fn rows(face: &Face, preferred: Option<conduit_core::Unit>) -> Option<Vec<Row>> {
    let mut rows = Vec::new();
    for (group_order, subject) in face.subjects.iter().enumerate() {
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
            let label = property.name.replace('-', " ");
            // Larger documents retain the ordinary scene's complete bounded
            // wrapping/pagination rather than truncating a semantic value.
            if label.len() > 192 || value.len() > 192 {
                return None;
            }
            rows.push(Row {
                group: subject.identity.clone(),
                group_label: subject.name.clone(),
                group_order,
                label,
                value,
                quantity,
            });
        }
    }
    // Present finite typed values first, retaining authored order within each
    // class. Plain status rows remain directly alongside those quantities.
    rows.sort_by_key(|row| (row.group_order, !row.quantity));
    (!rows.is_empty()).then_some(rows)
}
/// Only the Body identified by this Face's basis supplies its heading.
fn body_heading(face: &Face) -> Option<&str> {
    let identity = format!("body/{}", face.basis.body_id.as_ref()?.as_str());
    let mut subjects = face
        .subjects
        .iter()
        .filter(|subject| subject.role == FaceRole::Body && subject.identity == identity);
    let subject = subjects.next()?;
    (subjects.next().is_none() && !subject.name.trim().is_empty()).then_some(subject.name.as_str())
}

/// Native primary pages render subject headings, authored text and actions;
/// technical properties live only in Details (not this read-only page cycle).
/// Keep native primary pages whenever they add content beyond the Info headings
/// already named by the property Look. This is not a full Face/Details projection.
pub fn has_no_additional_native_primary_content(face: &Face) -> bool {
    !face.subjects.is_empty()
        && face.subjects.iter().all(|subject| {
            (subject.role == FaceRole::Body
                && face
                    .basis
                    .body_id
                    .as_ref()
                    .is_some_and(|id| subject.identity == format!("body/{}", id.as_str()))
                && body_heading(face) == Some(subject.name.as_str()))
                || subject.role == FaceRole::Info
                    && face.properties.iter().any(|p| {
                        p.subject == subject.identity
                            && matches!(
                                p.value,
                                FacePropertyValue::TypedValue { .. }
                                    | FacePropertyValue::Text(_)
                                    | FacePropertyValue::Flag(_)
                            )
                    })
        })
        && face.text.is_empty()
        && face.relationships.is_empty()
        && face.composition.is_empty()
        && face.actions.is_empty()
        && face.disclosures.is_empty()
        && face.temporal_references.is_empty()
        && face.temporal_facts.is_empty()
}

fn text(
    scene: &mut GraphicsScene,
    screen: LayoutRect,
    bounds: LayoutRect,
    value: &str,
    role: GraphicsTextRole,
    paint: GraphicsPaintRole,
    typography: Typography,
) -> Option<()> {
    if TextLayout::new_selected(value, typography.style(role.into()), bounds.width, 100)
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
fn row_height(row: &Row, width: u16, typography: Typography) -> Option<u16> {
    if row.quantity {
        return Some(84);
    }
    let label = format!("{}: {}", row.label, row.value);
    let measured = TextLayout::new_selected(
        &label,
        typography.style(GraphicsTextRole::Body.into()),
        width.checked_sub(64)?,
        100,
    )
    .ok()?
    .finish_height();
    u16::try_from(measured).ok()?.checked_add(8)
}
fn group_height(row: &Row, width: u16, typography: Typography) -> Option<u16> {
    let measured = TextLayout::new_selected(
        &row.group_label,
        typography.style(GraphicsTextRole::Heading.into()),
        width.checked_sub(48)?,
        100,
    )
    .ok()?
    .finish_height();
    u16::try_from(measured).ok()?.checked_add(8)
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
    frame_with_look(face, width, height, tick, preferred, None)
}
pub fn frame_with_look(
    face: &Face,
    width: u16,
    height: u16,
    tick: usize,
    preferred: Option<conduit_core::Unit>,
    look: Option<&conduit_face::LookInfo>,
) -> Option<(GraphicsScene, usize, usize)> {
    let typography = crate::raster_target::selected_typography(look);
    let rows = rows(face, preferred)?;
    let heading = body_heading(face).unwrap_or("Conduit");
    let title_width = width.checked_sub(48)?;
    let title_height = u16::try_from(
        TextLayout::new_selected(
            heading,
            typography.style(GraphicsTextRole::Title.into()),
            title_width,
            100,
        )
        .ok()?
        .finish_height(),
    )
    .ok()?
    .max(40);
    let rows_start = u16::try_from(crate::view::STATUS_STRIP_HEIGHT)
        .ok()?
        .checked_add(32)?
        .checked_add(title_height)?;
    let bottom = height.checked_sub(40)?;
    let mut page_ranges = Vec::new();
    let mut start = 0;
    while start < rows.len() {
        let mut y = rows_start;
        let mut end = start;
        let mut group = None;
        while end < rows.len() {
            let row = &rows[end];
            let extra = if group == Some(row.group.as_str()) {
                row_height(row, width, typography)?
            } else {
                group_height(row, width, typography)?
                    .checked_add(row_height(row, width, typography)?)?
            };
            if y.checked_add(extra)? > bottom {
                break;
            }
            y += extra;
            group = Some(row.group.as_str());
            end += 1;
        }
        if end == start {
            return None;
        }
        page_ranges.push((start, end));
        start = end;
    }
    let pages = page_ranges.len();
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
            height: title_height,
        },
        heading,
        GraphicsTextRole::Title,
        GraphicsPaintRole::Accent,
        typography,
    )?;
    let (start, end) = page_ranges[page];
    let mut y = i16::try_from(rows_start).ok()?;
    let mut group = None;
    for row in &rows[start..end] {
        if group != Some(row.group.as_str()) {
            text(
                &mut scene,
                screen,
                LayoutRect {
                    x: 24,
                    y,
                    width: width.checked_sub(48)?,
                    height: group_height(row, width, typography)?,
                },
                &row.group_label,
                GraphicsTextRole::Heading,
                GraphicsPaintRole::Accent,
                typography,
            )?;
            y += i16::try_from(group_height(row, width, typography)?).ok()?;
            group = Some(row.group.as_str());
        }
        if !row.quantity {
            let label = format!("{}: {}", row.label, row.value);
            text(
                &mut scene,
                screen,
                LayoutRect {
                    x: 32,
                    y,
                    width: width.checked_sub(64)?,
                    height: row_height(row, width, typography)?,
                },
                &label,
                GraphicsTextRole::Body,
                GraphicsPaintRole::Foreground,
                typography,
            )?;
            y += i16::try_from(row_height(row, width, typography)?).ok()?;
            continue;
        }
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
            typography,
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
            typography,
        )?;
        y += 84;
    }
    let footer = format!("Page {} of {}", page + 1, pages);
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
        typography,
    )?;
    Some((scene, page, pages))
}
#[cfg(test)]
#[path = "property_mask_tests.rs"]
mod tests;
