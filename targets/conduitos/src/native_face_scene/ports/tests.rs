use super::*;
use crate::native_face_scene::NativeFaceScene;
use alloc::borrow::ToOwned;
use conduit_presentation::{GraphicsCommandKind, PresentationProperty, PresentationRelationship};

fn fan_face() -> (Presentation, String, String, String) {
    let mut face: Presentation = serde_json::from_str(include_str!(
        "../../../../../proof/conformance/presentation-waist/domain-derived/patchbay-face.json"
    ))
    .unwrap();
    let cords: alloc::vec::Vec<_> = face
        .subjects
        .iter()
        .filter(|s| s.role == PresentationRole::Cord)
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
    face.subjects.push(PresentationSubject {
        identity: id.clone(),
        role: PresentationRole::Cord,
        name: "Direct text route".into(),
    });
    for (name, value) in [
        ("source-port", source),
        ("sink-port", sink),
        ("value-kind", "value/text".into()),
    ] {
        face.properties.push(PresentationProperty {
            subject: id.clone(),
            name: name.into(),
            value: PresentationPropertyValue::Identity(value),
        });
    }
    for target in [&source_subject, &sink_subject] {
        face.relationships.push(PresentationRelationship {
            source: id.clone(),
            target: target.clone(),
            kind: PresentationRelationshipKind::Connects,
        });
    }
    let face = Presentation::new_with_semantics(
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
        !(r.source == cord_id
            && r.target == source
            && r.kind == PresentationRelationshipKind::Connects)
    });
    let face = Presentation::new_with_semantics(
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
