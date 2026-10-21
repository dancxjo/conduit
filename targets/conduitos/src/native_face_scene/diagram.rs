//! A bounded visual projection of the Patchbay subjects already in this Face.
//! Gear/Port/Cord identities and endpoints come from the Face, never a fixture
//! graph or a renderer-owned connection table. Full labels remain in Details.
use super::*;
use conduit_face::{
    FacePropertyValue, FaceRelationshipKind, FaceRole, FaceSubject, GraphicsPath, GraphicsPoint,
};

#[path = "diagram_geometry.rs"]
mod geometry;
use geometry::{DiagramGeometry, GAP, PORT_ROW, visible_ports};
#[path = "diagram_hits.rs"]
mod hit_regions;
use hit_regions::DiagramHits;
const MAX_VISIBLE_LINKS: usize = 12;

pub(super) fn pages(face: &Face, width: u16, height: u16) -> usize {
    let gears = face
        .subjects
        .iter()
        .filter(|subject| subject.role == FaceRole::Gear)
        .count();
    let geometry = DiagramGeometry::new(
        LayoutRect {
            x: 0,
            y: 0,
            width,
            height,
        },
        gears,
    );
    gears.div_ceil(geometry.capacity()).max(1)
}

pub(super) fn exists(face: &Face) -> bool {
    face.subjects
        .iter()
        .any(|subject| subject.role == FaceRole::Gear)
}

pub(super) fn link_notice(face: &Face, screen: LayoutRect, page: usize) -> String {
    let gear_count = face
        .subjects
        .iter()
        .filter(|s| s.role == FaceRole::Gear)
        .count();
    let geometry = DiagramGeometry::new(screen, gear_count);
    let height = geometry.height;
    let ordering = super::connected_layout::order(face);
    let gears = visible_gears(
        face,
        &ordering,
        page * geometry.capacity(),
        geometry.capacity(),
    );
    let cords = face
        .subjects
        .iter()
        .filter(|s| s.role == FaceRole::Cord)
        .collect::<Vec<_>>();
    let eligible = cords
        .iter()
        .filter(|cord| endpoints(face, &gears, cord, height).is_some())
        .count();
    let shown = eligible.min(link_capacity(face, &gears, height));
    if screen.width < 640 {
        alloc::format!(
            "{} · {shown}/{} cords shown · {} omitted · F2/F3",
            ordering.label(),
            cords.len(),
            cords.len() - shown
        )
    } else {
        alloc::format!(
            "{} · Tab subjects · {shown}/{} cords shown · {} omitted · F2 all facts · F3 return",
            ordering.label(),
            cords.len(),
            cords.len() - shown
        )
    }
}

/// Cards, their Port rows and overflow notices are mandatory on this page.
/// Admit Cord paths from the commands left after those rows and shared chrome;
/// reporting and pointer selection use the exact same visible prefix.
fn link_capacity(face: &Face, gears: &[&FaceSubject], height: u16) -> usize {
    let cards: usize = gears
        .iter()
        .map(|gear| {
            let count = ports(face, gear).count();
            let shown = visible_ports(height, count).min(count);
            2 + usize::from(!gear_labels(&gear.name).1.is_empty())
                + shown
                + usize::from(count > shown && height >= 90)
        })
        .sum();
    conduit_face::MAX_GRAPHICS_COMMANDS
        .saturating_sub(super::chrome::COMMANDS + cards)
        .min(MAX_VISIBLE_LINKS)
}

fn endpoints(
    face: &Face,
    gears: &[&FaceSubject],
    cord: &FaceSubject,
    height: u16,
) -> Option<((usize, usize), (usize, usize))> {
    let source = property_identity(face, &cord.identity, "source-port")?;
    let sink = property_identity(face, &cord.identity, "sink-port")?;
    let from = port_location(face, gears, &cord.identity, source, "outgoing", height)?;
    let to = port_location(face, gears, &cord.identity, sink, "receiving", height)?;
    (from.0 != to.0).then_some((from, to))
}

pub(super) fn append<'a>(
    scene: &mut GraphicsScene,
    face: &'a Face,
    screen: LayoutRect,
    page: usize,
) -> Result<DiagramHits<'a>, FaceSceneError> {
    let gear_count = face
        .subjects
        .iter()
        .filter(|subject| subject.role == FaceRole::Gear)
        .count();
    let geometry = DiagramGeometry::new(screen, gear_count);
    let height = geometry.height;
    let ordering = super::connected_layout::order(face);
    let gears = visible_gears(
        face,
        &ordering,
        page * geometry.capacity(),
        geometry.capacity(),
    );
    let card = |index| geometry.card(index);
    let mut hits = DiagramHits::new(core::array::from_fn(|index| {
        gears
            .get(index)
            .map(|gear| (card(index), gear.identity.as_str()))
    }));
    // Paths go underneath their endpoint cards and use only endpoints whose
    // exact Port subject and directional contract are visible on this page.
    let maximum_links = link_capacity(face, &gears, height);
    let mut visible_links = 0;
    for cord in face
        .subjects
        .iter()
        .filter(|subject| subject.role == FaceRole::Cord)
    {
        let Some(((source_index, source_row), (sink_index, sink_row))) =
            endpoints(face, &gears, cord, height)
        else {
            continue;
        };
        if visible_links == maximum_links {
            break;
        }
        let from = card(source_index);
        let to = card(sink_index);
        let (start_x, end_x) = if from.x <= to.x {
            (from.x + from.width as i16 - 1, to.x)
        } else {
            (from.x, to.x + to.width as i16 - 1)
        };
        let start_y = from.y + 80 + source_row as i16 * PORT_ROW as i16;
        let end_y = to.y + 80 + sink_row as i16 * PORT_ROW as i16;
        let middle = if from.x == to.x {
            from.x + from.width as i16 + GAP as i16 / 2
        } else {
            start_x + (end_x - start_x) / 2
        };
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
        hits.cords[visible_links] = Some((path, cord.identity.as_str()));
        visible_links += 1;
    }
    for (index, gear) in gears.into_iter().enumerate() {
        let bounds = card(index);
        let (title, origin) = gear_labels(&gear.name);
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
                &crate::display::typography::single_line_preview(
                    &alloc::format!("{} · {}", index + 1, readable(title)),
                    crate::display::typography::TextRole::Heading,
                    bounds.width - 20,
                )
                .map_err(|_| FaceSceneError::SceneBound)?,
            )
            .and_then(|command| command.with_text_role(GraphicsTextRole::Heading)),
        )?;
        if !origin.is_empty() {
            push(
                scene,
                GraphicsCommand::text(
                    LayoutRect {
                        x: bounds.x + 12,
                        y: bounds.y + 37,
                        width: bounds.width - 24,
                        height: 22,
                    },
                    screen,
                    GraphicsPaintRole::Muted,
                    &preview(&readable(origin), 40),
                )
                .and_then(|command| command.with_text_role(GraphicsTextRole::Muted)),
            )?;
        }
        let ports = ports(face, gear).collect::<Vec<_>>();
        let shown = visible_ports(height, ports.len()).min(ports.len());
        for (row, port) in ports.iter().take(shown).enumerate() {
            let direction = property_text(face, &port.identity, "direction");
            let paint = if direction == Some("outgoing") {
                GraphicsPaintRole::Accent
            } else {
                GraphicsPaintRole::Muted
            };
            let y = bounds.y + 70 + row as i16 * PORT_ROW as i16;
            hits.ports[index * 4 + row] = Some((
                LayoutRect {
                    x: bounds.x + 12,
                    y,
                    width: bounds.width - 24,
                    height: 23,
                },
                port.identity.as_str(),
            ));
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
                    &preview(&super::ports::label(face, port), 30),
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
    Ok(hits)
}

fn ports<'a>(face: &'a Face, gear: &FaceSubject) -> impl Iterator<Item = &'a FaceSubject> {
    face.subjects.iter().filter(move |subject| {
        subject.role == FaceRole::Port
            && face.relationships.iter().any(|relation| {
                relation.kind == FaceRelationshipKind::Contains
                    && relation.source == gear.identity
                    && relation.target == subject.identity
            })
    })
}

fn port_location(
    face: &Face,
    gears: &[&FaceSubject],
    cord: &str,
    semantic_id: &str,
    direction: &str,
    height: u16,
) -> Option<(usize, usize)> {
    let resolved = super::ports::resolve(face, cord, semantic_id, direction)?;
    let owner = super::connected_layout::unique_owner(face, &resolved.identity)?;
    let mut found = None;
    for (column, gear) in gears.iter().enumerate() {
        if gear.identity != face.subjects[owner].identity {
            continue;
        }
        for (row, port) in ports(face, gear)
            .take(visible_ports(height, ports(face, gear).count()))
            .enumerate()
        {
            if port.identity == resolved.identity {
                if found.is_some() {
                    return None;
                }
                found = Some((column, row));
            }
        }
    }
    found
}

fn property_identity<'a>(face: &'a Face, subject: &str, name: &str) -> Option<&'a str> {
    face.properties.iter().find_map(|property| {
        if property.subject == subject && property.name == name {
            match &property.value {
                FacePropertyValue::Identity(value) => Some(value.as_str()),
                _ => None,
            }
        } else {
            None
        }
    })
}

fn property_text<'a>(face: &'a Face, subject: &str, name: &str) -> Option<&'a str> {
    face.properties.iter().find_map(|property| {
        if property.subject == subject && property.name == name {
            match &property.value {
                FacePropertyValue::Text(value) => Some(value.as_str()),
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

fn gear_labels(value: &str) -> (&str, &str) {
    let semantic = value.split_once(" Gear").map_or(value, |(name, _)| name);
    semantic
        .rsplit_once('/')
        .map_or((value, ""), |(origin, local)| (local, origin))
}

fn readable(value: &str) -> alloc::string::String {
    let plain = value.replace(['-', '_'], " ").replace('/', " · ");
    let mut chars = plain.chars();
    chars.next().map_or(plain.clone(), |first| {
        alloc::format!("{}{}", first.to_uppercase(), chars.as_str())
    })
}

fn visible_gears<'a>(
    face: &'a Face,
    ordering: &super::connected_layout::Order,
    skip: usize,
    capacity: usize,
) -> Vec<&'a FaceSubject> {
    if ordering.fallback {
        face.subjects
            .iter()
            .filter(|s| s.role == FaceRole::Gear)
            .skip(skip)
            .take(capacity)
            .collect()
    } else {
        ordering.subjects[..ordering.count]
            .iter()
            .skip(skip)
            .take(capacity)
            .map(|&index| &face.subjects[index])
            .collect()
    }
}

#[cfg(test)]
#[path = "diagram/tests.rs"]
mod tests;
