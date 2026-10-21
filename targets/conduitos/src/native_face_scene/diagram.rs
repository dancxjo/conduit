//! A bounded visual projection of the Patchbay subjects already in this Face.
//! Gear/Port/Cord identities and endpoints come from the Face, never a fixture
//! graph or a renderer-owned connection table. Full labels remain in Details.
use super::*;
use conduit_face::{
    FacePropertyValue, FaceRelationshipKind, FaceRole, FaceSubject, GraphicsPath, GraphicsPoint,
};

#[path = "diagram_geometry.rs"]
mod geometry;
#[path = "diagram/selection.rs"]
mod selection;
use geometry::{DiagramGeometry, GAP, GearHits, PORT_ROW, visible_ports};
pub(super) use selection::hit_test;
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
    let shown = eligible.min(MAX_VISIBLE_LINKS);
    if screen.width < 640 {
        alloc::format!(
            "{} · {shown}/{} cords shown · {} omitted · F2/F3",
            ordering.label(),
            cords.len(),
            cords.len() - shown
        )
    } else {
        alloc::format!(
            "{} · {shown}/{} cords · {} omitted · Alt+arrows inspect · F2/F3",
            ordering.label(),
            cords.len(),
            cords.len() - shown
        )
    }
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
) -> Result<GearHits<'a>, FaceSceneError> {
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
    let hits = core::array::from_fn(|index| {
        gears
            .get(index)
            .map(|gear| (card(index), gear.identity.as_str()))
    });
    // Paths go underneath their endpoint cards and use only endpoints whose
    // exact Port subject and directional contract are visible on this page.
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
        if visible_links == MAX_VISIBLE_LINKS {
            break;
        }
        let path =
            selection::cord_path(card(source_index), card(sink_index), source_row, sink_row)?;
        push(
            scene,
            GraphicsCommand::path(path, screen, GraphicsPaintRole::Accent),
        )?;
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
mod tests {
    use super::*;

    #[test]
    fn notice_counts_capped_links_and_missing_endpoints_without_changing_face() {
        let mut face: Face = serde_json::from_str(include_str!(
            "../../../../proof/conformance/face-waist/domain-derived/patchbay-face.json"
        ))
        .unwrap();
        let cord = face
            .subjects
            .iter()
            .find(|s| s.role == FaceRole::Cord)
            .unwrap()
            .clone();
        let properties = face
            .properties
            .iter()
            .filter(|p| p.subject == cord.identity)
            .cloned()
            .collect::<Vec<_>>();
        let relationships = face
            .relationships
            .iter()
            .filter(|r| r.source == cord.identity)
            .cloned()
            .collect::<Vec<_>>();
        for index in 0..13 {
            let identity = alloc::format!("cord/overflow/{index}");
            let mut subject = cord.clone();
            subject.identity = identity.clone();
            face.subjects.push(subject);
            for property in &properties {
                let mut property = property.clone();
                property.subject = identity.clone();
                face.properties.push(property);
            }
            for relationship in &relationships {
                let mut relationship = relationship.clone();
                relationship.source = identity.clone();
                face.relationships.push(relationship);
            }
        }
        let screen = LayoutRect {
            x: 0,
            y: 0,
            width: 640,
            height: 720,
        };
        let retained = face.clone();
        assert!(
            link_notice(&face, screen, 0)
                .ends_with("12/15 cords · 3 omitted · Alt+arrows inspect · F2/F3")
        );
        assert_eq!(face, retained);
        let checked = Face::new_with_semantics(
            face.revision,
            face.basis.clone(),
            face.subjects.clone(),
            face.relationships.clone(),
            face.properties.clone(),
            face.text.clone(),
            face.actions.clone(),
            face.disclosures.clone(),
        )
        .unwrap();
        let mut scene = NativeFaceScene::prepare(checked.clone(), 640, 720).unwrap();
        scene.show_diagram(true);
        let frame = scene.frame().unwrap();
        assert_eq!(
            frame
                .scene
                .commands()
                .iter()
                .filter(|command| command.kind == conduit_face::GraphicsCommandKind::OrthogonalPath)
                .count(),
            12
        );
        assert!(frame.scene.commands().iter().any(|command| {
            command
                .payload()
                .ends_with("12/15 cords · 3 omitted · Alt+arrows inspect · F2/F3")
        }));
        assert!(
            frame.hits.is_empty(),
            "drawing cords must not invent action mappings"
        );
        assert_eq!(scene.face(), &checked);
        scene.show_details(true);
        let details: String = scene.details.iter().map(|row| row.text.as_str()).collect();
        for subject in checked
            .subjects
            .iter()
            .filter(|subject| subject.role == FaceRole::Cord)
        {
            assert!(details.contains(&subject.identity));
        }
        // A one-column page cannot draw a link to a Gear on another page.
        assert!(
            link_notice(
                &face,
                LayoutRect {
                    width: 320,
                    height: 240,
                    ..screen
                },
                0
            )
            .ends_with("0/15 cords shown · 15 omitted · F2/F3")
        );
        // Missing semantic endpoint facts never become geometry-invented links.
        face.properties.retain(|p| p.name != "sink-port");
        assert!(
            link_notice(&face, screen, 0)
                .ends_with("0/15 cords · 15 omitted · Alt+arrows inspect · F2/F3")
        );
    }
}
