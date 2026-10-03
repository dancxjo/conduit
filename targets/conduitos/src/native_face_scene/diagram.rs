//! A bounded visual projection of the Patchbay subjects already in this Face.
//! Gear/Port/Cord identities and endpoints come from the Face, never a fixture
//! graph or a renderer-owned connection table. Full labels remain in Details.
use super::*;
use conduit_presentation::{
    GraphicsPath, GraphicsPoint, PresentationPropertyValue, PresentationRelationshipKind,
    PresentationRole, PresentationSubject,
};

const GAP: u16 = 16;
const PORT_ROW: u16 = 27;
const MAX_VISIBLE_LINKS: usize = 12;

fn columns(width: u16) -> usize {
    if width >= 520 { 2 } else { 1 }
}

pub(super) fn pages(face: &Presentation, width: u16) -> usize {
    let gears = face
        .subjects
        .iter()
        .filter(|subject| subject.role == PresentationRole::Gear)
        .count();
    gears.div_ceil(columns(width)).max(1)
}

pub(super) fn exists(face: &Presentation) -> bool {
    face.subjects
        .iter()
        .any(|subject| subject.role == PresentationRole::Gear)
}

pub(super) fn append(
    scene: &mut GraphicsScene,
    face: &Presentation,
    screen: LayoutRect,
    page: usize,
) -> Result<(), FaceSceneError> {
    let columns = columns(screen.width);
    let width = (screen.width - SPACE_XL * 2 - GAP * (columns as u16 - 1)) / columns as u16;
    let height = screen.height - layout::HEADER - layout::FOOTER - 16;
    let gears = face
        .subjects
        .iter()
        .filter(|subject| subject.role == PresentationRole::Gear)
        .skip(page * columns)
        .take(columns)
        .collect::<Vec<_>>();
    let card = |column: usize| LayoutRect {
        x: (SPACE_XL + column as u16 * (width + GAP)) as i16,
        y: (layout::HEADER + 8) as i16,
        width,
        height,
    };
    // Paths go underneath their endpoint cards and use only endpoints whose
    // exact Port subject and directional contract are visible on this page.
    let mut visible_links = 0;
    for cord in face
        .subjects
        .iter()
        .filter(|subject| subject.role == PresentationRole::Cord)
    {
        let (Some(source), Some(sink)) = (
            property_identity(face, &cord.identity, "source-port"),
            property_identity(face, &cord.identity, "sink-port"),
        ) else {
            continue;
        };
        let (Some((source_column, source_row)), Some((sink_column, sink_row))) = (
            port_location(face, &gears, &cord.identity, source, "outgoing", height),
            port_location(face, &gears, &cord.identity, sink, "receiving", height),
        ) else {
            continue;
        };
        if source_column == sink_column {
            continue;
        }
        if visible_links == MAX_VISIBLE_LINKS {
            break;
        }
        let from = card(source_column);
        let to = card(sink_column);
        let start_x = from.x + from.width as i16 - 1;
        let end_x = to.x;
        let start_y = from.y + 59 + source_row as i16 * PORT_ROW as i16;
        let end_y = to.y + 59 + sink_row as i16 * PORT_ROW as i16;
        let middle = start_x + (end_x - start_x) / 2;
        let mut points = Vec::with_capacity(4);
        points.push(GraphicsPoint::new(start_x, start_y).map_err(|_| FaceSceneError::SceneBound)?);
        if start_y != end_y {
            points
                .push(GraphicsPoint::new(middle, start_y).map_err(|_| FaceSceneError::SceneBound)?);
            points.push(GraphicsPoint::new(middle, end_y).map_err(|_| FaceSceneError::SceneBound)?);
        }
        points.push(GraphicsPoint::new(end_x, end_y).map_err(|_| FaceSceneError::SceneBound)?);
        let path = GraphicsPath::new(&points).map_err(|_| FaceSceneError::SceneBound)?;
        push(
            scene,
            GraphicsCommand::path(path, screen, GraphicsPaintRole::Accent),
        )?;
        visible_links += 1;
    }
    for (column, gear) in gears.into_iter().enumerate() {
        let bounds = card(column);
        push(
            scene,
            GraphicsCommand::rect(
                bounds,
                screen,
                GraphicsPaintRole::Accent,
                GraphicsShapeStyle::RoundedStroke,
            ),
        )?;
        push(
            scene,
            GraphicsCommand::text(
                LayoutRect {
                    x: bounds.x + 10,
                    y: bounds.y + 9,
                    width: bounds.width - 20,
                    height: 32,
                },
                screen,
                GraphicsPaintRole::Foreground,
                &preview(&gear.name, 24),
            )
            .and_then(|command| command.with_text_role(GraphicsTextRole::Heading)),
        )?;
        let ports = ports(face, gear).collect::<Vec<_>>();
        let shown = visible_ports(height).min(ports.len());
        for (row, port) in ports.iter().take(shown).enumerate() {
            let direction = property_text(face, &port.identity, "direction");
            let paint = if direction == Some("outgoing") {
                GraphicsPaintRole::Accent
            } else {
                GraphicsPaintRole::Muted
            };
            let y = bounds.y + 49 + row as i16 * PORT_ROW as i16;
            push(
                scene,
                GraphicsCommand::text(
                    LayoutRect {
                        x: bounds.x + 12,
                        y,
                        width: bounds.width - 24,
                        height: 23,
                    },
                    screen,
                    paint,
                    &preview(&port.name, 30),
                )
                .and_then(|command| command.with_text_role(GraphicsTextRole::Label)),
            )?;
        }
        if ports.len() > shown && height >= 90 {
            let more = alloc::format!("+{} ports · F2 for all", ports.len() - shown);
            push(
                scene,
                GraphicsCommand::text(
                    LayoutRect {
                        x: bounds.x + 12,
                        y: bounds.y + bounds.height as i16 - 26,
                        width: bounds.width - 24,
                        height: 20,
                    },
                    screen,
                    GraphicsPaintRole::Muted,
                    &more,
                )
                .and_then(|command| command.with_text_role(GraphicsTextRole::Muted)),
            )?;
        }
    }
    Ok(())
}

fn visible_ports(height: u16) -> usize {
    usize::from(height.saturating_sub(56) / PORT_ROW).min(4)
}

fn ports<'a>(
    face: &'a Presentation,
    gear: &PresentationSubject,
) -> impl Iterator<Item = &'a PresentationSubject> {
    face.subjects.iter().filter(move |subject| {
        subject.role == PresentationRole::Port
            && face.relationships.iter().any(|relation| {
                relation.kind == PresentationRelationshipKind::Contains
                    && relation.source == gear.identity
                    && relation.target == subject.identity
            })
    })
}

fn port_location(
    face: &Presentation,
    gears: &[&PresentationSubject],
    cord: &str,
    semantic_id: &str,
    direction: &str,
    height: u16,
) -> Option<(usize, usize)> {
    let mut found = None;
    for (column, gear) in gears.iter().enumerate() {
        for (row, port) in ports(face, gear).take(visible_ports(height)).enumerate() {
            if property_identity(face, &port.identity, "semantic-id") == Some(semantic_id)
                && property_text(face, &port.identity, "direction") == Some(direction)
                && face.relationships.iter().any(|relation| {
                    relation.kind == PresentationRelationshipKind::Connects
                        && relation.source == cord
                        && relation.target == port.identity
                })
            {
                if found.is_some() {
                    return None;
                }
                found = Some((column, row));
            }
        }
    }
    found
}

fn property_identity<'a>(face: &'a Presentation, subject: &str, name: &str) -> Option<&'a str> {
    face.properties.iter().find_map(|property| {
        if property.subject == subject && property.name == name {
            match &property.value {
                PresentationPropertyValue::Identity(value) => Some(value.as_str()),
                _ => None,
            }
        } else {
            None
        }
    })
}

fn property_text<'a>(face: &'a Presentation, subject: &str, name: &str) -> Option<&'a str> {
    face.properties.iter().find_map(|property| {
        if property.subject == subject && property.name == name {
            match &property.value {
                PresentationPropertyValue::Text(value) => Some(value.as_str()),
                _ => None,
            }
        } else {
            None
        }
    })
}

fn preview(value: &str, limit: usize) -> alloc::string::String {
    if value.chars().count() <= limit {
        return value.into();
    }
    let end = value
        .char_indices()
        .nth(limit - 3)
        .map_or(value.len(), |(index, _)| index);
    alloc::format!("{}...", &value[..end])
}
