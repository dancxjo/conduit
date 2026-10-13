use super::*;
use crate::native_face_scene::NativeFaceScene;
use alloc::{format, string::String, vec::Vec};
use conduit_presentation::{
    GraphicsCommandKind, GraphicsShapeStyle, PresentationProperty, PresentationPropertyValue,
    PresentationRelationship, PresentationSubject,
};

// Explicit renderer fixtures, separate from actual authored Patchbay proof.
fn graph(names: &[&str], edges: &[(usize, usize)]) -> Presentation {
    let mut face: Presentation = serde_json::from_str(include_str!(
        "../../../../../proof/conformance/presentation-waist/domain-derived/patchbay-face.json"
    ))
    .unwrap();
    face.subjects.clear();
    face.relationships.clear();
    face.properties.clear();
    face.text.clear();
    face.actions.clear();
    face.disclosures.clear();
    for (index, name) in names.iter().enumerate() {
        let gear = format!("gear/{index}");
        face.subjects.push(PresentationSubject {
            identity: gear.clone(),
            role: PresentationRole::Gear,
            name: (*name).into(),
        });
        for (direction, suffix) in [("receiving", "in"), ("outgoing", "out")] {
            let port = format!("port/{index}/{suffix}");
            face.subjects.push(PresentationSubject {
                identity: port.clone(),
                role: PresentationRole::Port,
                name: suffix.into(),
            });
            face.relationships.push(PresentationRelationship {
                source: gear.clone(),
                target: port.clone(),
                kind: PresentationRelationshipKind::Contains,
            });
            for (name, value) in [
                (
                    "semantic-id",
                    PresentationPropertyValue::Identity(port.clone()),
                ),
                (
                    "direction",
                    PresentationPropertyValue::Text(direction.into()),
                ),
                (
                    "value-kind",
                    PresentationPropertyValue::Identity("value/text".into()),
                ),
            ] {
                face.properties.push(PresentationProperty {
                    subject: port.clone(),
                    name: name.into(),
                    value,
                });
            }
        }
    }
    for (index, &(from, to)) in edges.iter().enumerate() {
        let cord = format!("cord/{index}");
        face.subjects.push(PresentationSubject {
            identity: cord.clone(),
            role: PresentationRole::Cord,
            name: format!("Link {index}"),
        });
        for (name, port) in [
            ("source-port", format!("port/{from}/out")),
            ("sink-port", format!("port/{to}/in")),
        ] {
            face.properties.push(PresentationProperty {
                subject: cord.clone(),
                name: name.into(),
                value: PresentationPropertyValue::Identity(port.clone()),
            });
            face.relationships.push(PresentationRelationship {
                source: cord.clone(),
                target: port,
                kind: PresentationRelationshipKind::Connects,
            });
        }
    }
    seal(face)
}

fn seal(face: Presentation) -> Presentation {
    Presentation::new_with_semantics(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
    )
    .unwrap()
}

fn names(face: &Presentation, ordering: &Order) -> Vec<String> {
    ordering.subjects[..ordering.count]
        .iter()
        .map(|&i| face.subjects[i].name.clone())
        .collect()
}

#[test]
fn directional_fan_graphs_and_disconnected_components_have_stable_order() {
    let face = graph(
        &["Sink", "Detached", "Source", "Branch"],
        &[(2, 3), (3, 0), (2, 0)],
    );
    let result = order(&face);
    assert_eq!(
        names(&face, &result),
        ["Source", "Branch", "Sink", "Detached"]
    );
    assert_eq!((result.cycle_breaks, result.unresolved), (0, 0));
    let mut reversed = face.clone();
    reversed.relationships.reverse();
    assert_eq!(names(&reversed, &order(&reversed)), names(&face, &result));
    for (width, height) in [(320, 240), (640, 720)] {
        let mut scene = NativeFaceScene::prepare(face.clone(), width, height).unwrap();
        scene.show_diagram(true);
        let first = scene.frame().unwrap().scene;
        let cards: Vec<_> = first
            .commands()
            .iter()
            .filter(|c| c.style == GraphicsShapeStyle::RoundedStroke)
            .map(|c| c.bounds)
            .collect();
        for (index, left) in cards.iter().enumerate() {
            for right in &cards[index + 1..] {
                assert!(
                    i32::from(left.x) + i32::from(left.width) <= i32::from(right.x)
                        || i32::from(right.x) + i32::from(right.width) <= i32::from(left.x)
                        || i32::from(left.y) + i32::from(left.height) <= i32::from(right.y)
                        || i32::from(right.y) + i32::from(right.height) <= i32::from(left.y)
                );
            }
        }
        assert_eq!(first, scene.frame().unwrap().scene);
        assert_eq!(scene.presentation(), &face);
    }
}

#[test]
fn cycles_and_self_links_preserve_every_declared_cord_and_bounded_pages() {
    let face = graph(
        &["A", "B", "C", "Detached"],
        &[(0, 1), (1, 2), (2, 0), (0, 0)],
    );
    let result = order(&face);
    assert!(result.cycle_breaks > 0);
    assert_eq!(result.count, 4);
    assert_eq!(result.unresolved, 0);
    for (width, height) in [(320, 240), (640, 720)] {
        let mut scene = NativeFaceScene::prepare(face.clone(), width, height).unwrap();
        scene.show_diagram(true);
        for _ in 0..scene.pages() {
            let frame = scene.frame().unwrap();
            assert!(frame.scene.commands().len() <= conduit_presentation::MAX_GRAPHICS_COMMANDS);
            let shown = frame
                .scene
                .commands()
                .iter()
                .filter(|c| c.kind == GraphicsCommandKind::OrthogonalPath)
                .count();
            assert!(frame.scene.commands().iter().any(|c| {
                c.payload()
                    .contains(&format!("{shown}/4 cords shown · {} omitted", 4 - shown))
            }));
            scene.turn_page(true);
        }
        scene.show_details(true);
        let mut text = String::new();
        for _ in 0..scene.pages() {
            for command in scene.frame().unwrap().scene.commands() {
                text.push_str(command.payload());
            }
            scene.turn_page(true);
        }
        for index in 0..4 {
            assert!(text.contains(&format!("cord/{index}")));
        }
        assert_eq!(scene.presentation(), &face);
    }
}

#[test]
fn missing_and_off_page_ambiguous_owners_never_invent_connectivity() {
    let mut face = graph(&["A", "B", "Other"], &[(0, 1)]);
    face.relationships.push(PresentationRelationship {
        source: "gear/2".into(),
        target: "port/0/out".into(),
        kind: PresentationRelationshipKind::Contains,
    });
    let face = seal(face);
    assert_eq!(order(&face).unresolved, 1);
    assert_eq!(unique_owner(&face, "port/0/out"), None);
    let mut scene = NativeFaceScene::prepare(face.clone(), 320, 240).unwrap();
    scene.show_diagram(true);
    for _ in 0..scene.pages() {
        assert!(
            !scene
                .frame()
                .unwrap()
                .scene
                .commands()
                .iter()
                .any(|c| c.kind == GraphicsCommandKind::OrthogonalPath)
        );
        scene.turn_page(true);
    }
    let mut missing = graph(&["A", "B"], &[(0, 1)]);
    missing
        .relationships
        .retain(|r| !(r.kind == PresentationRelationshipKind::Connects && r.target == "port/1/in"));
    let missing = seal(missing);
    assert_eq!(order(&missing).unresolved, 1);
}

#[test]
fn over_budget_layout_keeps_original_order_and_all_details() {
    let values: Vec<String> = (0..MAX_GEARS + 1).map(|i| format!("Node {i}")).collect();
    let labels: Vec<&str> = values.iter().map(String::as_str).collect();
    let face = graph(&labels, &[]);
    assert!(order(&face).fallback);
    let mut scene = NativeFaceScene::prepare(face.clone(), 640, 720).unwrap();
    scene.show_diagram(true);
    assert!(
        scene
            .frame()
            .unwrap()
            .scene
            .commands()
            .iter()
            .any(|c| c.payload().starts_with("Grid bound"))
    );
    scene.show_details(true);
    let mut text = String::new();
    for _ in 0..scene.pages() {
        let frame = scene.frame().unwrap();
        assert!(frame.scene.commands().len() <= conduit_presentation::MAX_GRAPHICS_COMMANDS);
        for command in frame.scene.commands() {
            text.push_str(command.payload());
        }
        scene.turn_page(true);
    }
    for index in 0..MAX_GEARS + 1 {
        assert!(text.contains(&format!("Gear identity: gear/{index}")));
    }
    assert_eq!(scene.presentation(), &face);
}

#[test]
fn exact_node_and_cord_limits_are_admitted_without_bitmap_overflow() {
    let values: Vec<String> = (0..MAX_GEARS).map(|i| format!("Node {i}")).collect();
    let labels: Vec<&str> = values.iter().map(String::as_str).collect();
    let edges: Vec<_> = (0..MAX_CORDS)
        .map(|i| (i % MAX_GEARS, (i + 1) % MAX_GEARS))
        .collect();
    let face = graph(&labels, &edges);
    let result = order(&face);
    assert!(!result.fallback);
    assert_eq!(result.count, MAX_GEARS);
    assert_eq!(result.unresolved, 0);
    let mut seen = [false; MAX_GEARS];
    for subject in &result.subjects[..result.count] {
        let index = face.subjects[*subject]
            .identity
            .strip_prefix("gear/")
            .unwrap()
            .parse::<usize>()
            .unwrap();
        assert!(!seen[index]);
        seen[index] = true;
    }
    let mut too_many = edges;
    too_many.push((0, 1));
    assert!(order(&graph(&labels, &too_many)).fallback);
}
