use super::*;
use crate::native_face_scene::NativeFaceScene;
use alloc::borrow::ToOwned;
use conduit_face::{FaceProperty, FaceRelationship, GraphicsCommandKind};

fn fan_face() -> (Face, String, String, String) {
    let mut face: Face = serde_json::from_str(include_str!(
        "../../../../../proof/conformance/face-waist/domain-derived/patchbay-face.json"
    ))
    .unwrap();
    let cords: alloc::vec::Vec<_> = face
        .subjects
        .iter()
        .filter(|s| s.role == FaceRole::Cord)
        .collect();
    let source = property(&face, &cords[0].identity, "source-port", true)
        .unwrap()
        .to_owned();
    let sink = property(&face, &cords[1].identity, "sink-port", true)
        .unwrap()
        .to_owned();
    let source_subject = endpoint(&face, cords[0], "source-port", "outgoing")
        .unwrap()
        .identity
        .clone();
    let sink_subject = endpoint(&face, cords[1], "sink-port", "receiving")
        .unwrap()
        .identity
        .clone();
    let id = "cord/fan-direct".to_owned();
    face.subjects.push(FaceSubject {
        identity: id.clone(),
        role: FaceRole::Cord,
        name: "Direct text route".into(),
    });
    for (name, value) in [
        ("source-port", source),
        ("sink-port", sink),
        ("value-kind", "value/text".into()),
    ] {
        face.properties.push(FaceProperty {
            subject: id.clone(),
            name: name.into(),
            value: FacePropertyValue::Identity(value),
        });
    }
    for target in [&source_subject, &sink_subject] {
        face.relationships.push(FaceRelationship {
            source: id.clone(),
            target: target.clone(),
            kind: FaceRelationshipKind::Connects,
        });
    }
    let face = Face::new_with_semantics(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
    )
    .unwrap();
    (face, id, source_subject, sink_subject)
}

#[test]
fn typed_direction_labels_and_exact_endpoints_survive_mixed_fan_graphs() {
    let (face, cord_id, source, sink) = fan_face();
    let cord = face
        .subjects
        .iter()
        .find(|s| s.identity == cord_id)
        .unwrap();
    assert_eq!(
        cord_summary(&face, cord),
        format!("Source OUT {source} -> Sink IN {sink}")
    );
    let mut reversed = face.clone();
    reversed.relationships.reverse();
    assert_eq!(cord_summary(&reversed, cord), cord_summary(&face, cord));
    let mut scene = NativeFaceScene::prepare(face.clone(), 640, 720).unwrap();
    scene.show_diagram(true);
    let frame = scene.frame().unwrap();
    assert_eq!(
        frame
            .scene
            .commands()
            .iter()
            .filter(|c| c.payload().starts_with("IN value/text"))
            .count(),
        2
    );
    assert_eq!(
        frame
            .scene
            .commands()
            .iter()
            .filter(|c| c.payload().starts_with("OUT value/text"))
            .count(),
        2
    );
    assert_eq!(
        frame
            .scene
            .commands()
            .iter()
            .filter(|c| c.kind == GraphicsCommandKind::OrthogonalPath)
            .count(),
        3
    );
    assert!(
        scene
            .details
            .iter()
            .any(|row| row.text == cord_summary(&face, cord))
    );
    assert_eq!(scene.presentation(), &face);
}

#[test]
fn missing_relationship_cannot_be_replaced_by_a_nearby_port() {
    let (mut face, cord_id, source, _) = fan_face();
    face.relationships.retain(|r| {
        !(r.source == cord_id && r.target == source && r.kind == FaceRelationshipKind::Connects)
    });
    let face = Face::new_with_semantics(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
    )
    .unwrap();
    let cord = face
        .subjects
        .iter()
        .find(|s| s.identity == cord_id)
        .unwrap();
    assert!(cord_summary(&face, cord).starts_with("Cord endpoints not uniquely resolved"));
    let mut scene = NativeFaceScene::prepare(face.clone(), 640, 720).unwrap();
    scene.show_diagram(true);
    assert_eq!(
        scene
            .frame()
            .unwrap()
            .scene
            .commands()
            .iter()
            .filter(|c| c.kind == GraphicsCommandKind::OrthogonalPath)
            .count(),
        2
    );
    assert_eq!(scene.presentation(), &face);
}

#[test]
fn missing_port_facts_are_not_inferred_from_misleading_wording() {
    let (mut face, _, source, _) = fan_face();
    face.subjects
        .iter_mut()
        .find(|s| s.identity == source)
        .unwrap()
        .name = "OUT value/text".into();
    face.properties
        .retain(|p| !(p.subject == source && p.name == "value-kind"));
    face.properties
        .iter_mut()
        .find(|p| p.subject == source && p.name == "direction")
        .unwrap()
        .value = FacePropertyValue::Text("sideways".into());
    let face = Face::new_with_semantics(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
    )
    .unwrap();
    let mut scene = NativeFaceScene::prepare(face.clone(), 640, 720).unwrap();
    scene.show_diagram(true);
    let frame = scene.frame().unwrap();
    assert!(
        frame
            .scene
            .commands()
            .iter()
            .any(|c| c.payload().starts_with("PORT type unavailable"))
    );
    assert_eq!(
        frame
            .scene
            .commands()
            .iter()
            .filter(|c| c.kind == GraphicsCommandKind::OrthogonalPath)
            .count(),
        1
    );
    assert_eq!(scene.presentation(), &face);
}

#[test]
fn large_graphs_keep_every_identity_reachable_and_report_omitted_geometry() {
    let (mut face, _, _, _) = fan_face();
    let original = face.clone();
    let graph_ids: alloc::vec::Vec<_> = original
        .subjects
        .iter()
        .filter(|s| matches!(s.role, FaceRole::Gear | FaceRole::Port | FaceRole::Cord))
        .map(|s| s.identity.clone())
        .collect();
    for group in 1..4 {
        let prefix = format!("group{group}/");
        for subject in original
            .subjects
            .iter()
            .filter(|s| graph_ids.contains(&s.identity))
        {
            let mut subject = subject.clone();
            subject.identity = format!("{prefix}{}", subject.identity);
            face.subjects.push(subject);
        }
        for property in original
            .properties
            .iter()
            .filter(|p| graph_ids.contains(&p.subject))
        {
            if !matches!(
                property.name.as_str(),
                "semantic-id" | "direction" | "value-kind" | "source-port" | "sink-port"
            ) {
                continue;
            }
            let mut property = property.clone();
            property.subject = format!("{prefix}{}", property.subject);
            if matches!(
                property.name.as_str(),
                "semantic-id" | "source-port" | "sink-port"
            ) && let FacePropertyValue::Identity(value) = &mut property.value
            {
                *value = format!("{prefix}{value}");
            }
            face.properties.push(property);
        }
        for relation in original
            .relationships
            .iter()
            .filter(|r| graph_ids.contains(&r.source) && graph_ids.contains(&r.target))
        {
            let mut relation = relation.clone();
            relation.source = format!("{prefix}{}", relation.source);
            relation.target = format!("{prefix}{}", relation.target);
            face.relationships.push(relation);
        }
    }
    let face = Face::new_with_semantics(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
    )
    .unwrap();
    let total = face
        .subjects
        .iter()
        .filter(|s| s.role == FaceRole::Cord)
        .count();
    assert_eq!(total, 12);
    for (width, height) in [(320, 240), (640, 720)] {
        let mut scene = NativeFaceScene::prepare(face.clone(), width, height).unwrap();
        scene.show_diagram(true);
        assert!(scene.pages() > 1);
        for _ in 0..scene.pages() {
            let frame = scene.frame().unwrap();
            assert!(frame.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
            let shown = frame
                .scene
                .commands()
                .iter()
                .filter(|c| c.kind == GraphicsCommandKind::OrthogonalPath)
                .count();
            let notice = format!(
                "{shown}/{total} cords shown · {} omitted · F2 all facts · F3 return",
                total - shown
            );
            assert!(frame.scene.commands().iter().any(|c| c.payload() == notice));
            scene.turn_page(true);
        }
        scene.show_details(true);
        let mut visible = alloc::vec::Vec::new();
        for _ in 0..scene.pages() {
            let frame = scene.frame().unwrap();
            assert!(frame.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
            visible.extend(
                frame
                    .scene
                    .commands()
                    .iter()
                    .map(|c| c.payload().to_owned()),
            );
            scene.turn_page(true);
        }
        for subject in face
            .subjects
            .iter()
            .filter(|s| matches!(s.role, FaceRole::Gear | FaceRole::Port | FaceRole::Cord))
        {
            let fact = format!("{:?} identity: {}", subject.role, subject.identity);
            assert!(
                visible.iter().any(|text| text == &fact),
                "missing {} at {width}x{height}",
                subject.identity
            );
        }
        assert_eq!(scene.presentation(), &face);
    }
}

#[test]
fn off_page_ambiguous_endpoint_never_borrows_a_visible_ports_geometry() {
    let (mut face, cord_id, source, _) = fan_face();
    for index in 0..4 {
        face.subjects.push(FaceSubject {
            identity: format!("gear/disconnected/{index}"),
            role: FaceRole::Gear,
            name: format!("Disconnected {index}"),
        });
    }
    let hidden_gear = "gear/ambiguous-offpage";
    face.subjects.push(FaceSubject {
        identity: hidden_gear.into(),
        role: FaceRole::Gear,
        name: "Off-page ambiguity".into(),
    });
    let duplicate_id = "port/duplicate-source";
    let mut duplicate = face
        .subjects
        .iter()
        .find(|s| s.identity == source)
        .unwrap()
        .clone();
    duplicate.identity = duplicate_id.into();
    face.subjects.push(duplicate);
    let properties: alloc::vec::Vec<_> = face
        .properties
        .iter()
        .filter(|p| p.subject == source)
        .cloned()
        .collect();
    for mut property in properties {
        property.subject = duplicate_id.into();
        face.properties.push(property);
    }
    face.relationships.push(FaceRelationship {
        source: hidden_gear.into(),
        target: duplicate_id.into(),
        kind: FaceRelationshipKind::Contains,
    });
    face.relationships.push(FaceRelationship {
        source: cord_id.clone(),
        target: duplicate_id.into(),
        kind: FaceRelationshipKind::Connects,
    });
    let face = Face::new_with_semantics(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
    )
    .unwrap();
    let cord = face
        .subjects
        .iter()
        .find(|s| s.identity == cord_id)
        .unwrap();
    assert!(cord_summary(&face, cord).starts_with("Cord endpoints not uniquely resolved"));
    let mut scene = NativeFaceScene::prepare(face.clone(), 640, 720).unwrap();
    scene.show_diagram(true);
    assert!(scene.pages() > 1);
    assert_eq!(
        scene
            .frame()
            .unwrap()
            .scene
            .commands()
            .iter()
            .filter(|c| c.kind == GraphicsCommandKind::OrthogonalPath)
            .count(),
        2
    );
    assert_eq!(scene.presentation(), &face);
}
