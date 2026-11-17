//! Generic rhetorical reading proof; no application names or owner effects.
use crate::native_face_scene::{NativeFaceScene, tests};
use alloc::{string::String, vec};
use conduit_core::{
    BoundedResourceRef, CheckedValueContract, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity, kind_id,
};
use conduit_face::*;
use conduit_plot::rust_binding::NativeRustBinding;

fn composed_face() -> Face {
    let mut face = tests::face(8);
    face.subjects[0].role = FaceRole::Body;
    for (identity, role, name) in [
        ("group", FaceRole::Document, "Encounter"),
        ("illustration", FaceRole::Document, "Illustration"),
        (
            "photo",
            FaceRole::Semantic(kind_id("media/image")),
            "Landscape",
        ),
        ("readings", FaceRole::Document, "Current readings"),
        (
            "clock",
            FaceRole::Semantic(kind_id("time/civil-observation")),
            "Date and time",
        ),
        (
            "weather",
            FaceRole::Semantic(kind_id("weather/conditions")),
            "Conditions",
        ),
        ("provenance", FaceRole::Plot, "Face contribution provenance"),
    ] {
        face.subjects.push(FaceSubject {
            identity: identity.into(),
            role,
            name: name.into(),
        });
    }
    for (source, target) in [
        ("z-first", "group"),
        ("z-first", "provenance"),
        ("group", "illustration"),
        ("illustration", "photo"),
        ("group", "readings"),
        ("readings", "clock"),
        ("readings", "weather"),
    ] {
        face.relationships.push(FaceRelationship {
            source: source.into(),
            target: target.into(),
            kind: FaceRelationshipKind::Contains,
        });
    }
    for (subject, text) in [
        (
            "z-first",
            "The current Body is awake with its admitted workload.",
        ),
        ("clock", "2026-11-17 · 00:30:00 · America/Los_Angeles"),
        ("weather", "Seattle: 8 °C, cloudy. Weather is current."),
        (
            "photo",
            "A detailed landscape caption and its complete attribution.",
        ),
        (
            "provenance",
            "Original source, exact Plan and current Play remain inspectable.",
        ),
    ] {
        face.text.push(FaceText {
            subject: subject.into(),
            text: text.into(),
        });
    }
    face.actions.push(FaceAction {
        identity: "library".into(),
        intent: "test/open-library".into(),
        target: "z-first".into(),
        name: "Open library".into(),
        arguments: vec![],
        disclosure: FaceDisclosureLevel::CurrentAction,
        availability: FaceActionAvailability::Available,
    });
    let resource = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([1; 32]),
        content_profile: kind_id("media/image-rgba8@1"),
        access_class: ResourceClassId::from("conduit.resource/image-content@1"),
        extent: ResourceExtent {
            bytes: 16,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([2; 32]),
            expires_at: None,
        },
    };
    let image = ImageResource::new(
        ImageColorProfile::named("srgb".into()).unwrap(),
        resource,
        ImagePixelExtent::new(2, 2).unwrap(),
        ImageFormat::rgba8(),
        "fixture/photo".into(),
    )
    .unwrap()
    .into_structured()
    .unwrap();
    face.properties.push(FaceProperty {
        subject: "photo".into(),
        name: "image".into(),
        value: FacePropertyValue::TypedValue {
            contract: CheckedValueContract::new(
                image.value_type().profile().unwrap().value_kind().clone(),
                MAX_FACE_VALUE_BYTES,
                vec![],
            )
            .unwrap(),
            bytes: image.canonical_bytes().unwrap(),
        },
    });
    Face::new_with_semantics(
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
    .with_composition(vec![
        FaceCompositionRelation {
            identity: "composition/associated-readings".into(),
            source: "readings".into(),
            target: "group".into(),
            kind: FaceCompositionKind::Associate,
        },
        FaceCompositionRelation {
            identity: "composition/subordinate-illustration".into(),
            source: "illustration".into(),
            target: "group".into(),
            kind: FaceCompositionKind::Subordinate,
        },
    ])
    .unwrap()
}

#[test]
fn associated_content_leads_context_without_losing_details_controls_or_background() {
    let face = composed_face();
    for (width, height) in [(1024, 600), (640, 480)] {
        let mut scene = NativeFaceScene::prepare(face.clone(), width, height).unwrap();
        let first_page = scene
            .primary
            .iter()
            .filter(|row| row.page == 0)
            .map(|row| row.text.as_str())
            .collect::<alloc::vec::Vec<_>>()
            .join(" ");
        assert!(
            first_page.contains("00:30:00"),
            "{width}×{height}: {first_page}"
        );
        assert!(
            first_page.contains("Seattle: 8 °C"),
            "{width}×{height}: {first_page}"
        );
        assert!(
            !scene
                .primary
                .iter()
                .any(|row| row.text.contains("detailed landscape caption"))
        );
        assert!(
            scene
                .details
                .iter()
                .any(|row| row.text.contains("detailed landscape caption"))
        );
        assert!(
            scene
                .details
                .iter()
                .any(|row| row.text.contains("Original source"))
        );
        for action in 0..face.actions.len() {
            assert!(
                scene
                    .primary
                    .iter()
                    .any(|row| row.control.is_some_and(|control| control.action == action))
            );
            assert!(
                scene
                    .details
                    .iter()
                    .any(|row| row.control.is_some_and(|control| control.action == action))
            );
        }
        let frame = scene.frame().unwrap();
        assert!(frame.scene.commands().len() <= MAX_GRAPHICS_COMMANDS);
        assert!(
            frame
                .scene
                .commands()
                .iter()
                .any(|command| command.image_key()
                    == Some(GraphicsImageKey::new([1; 32], [2; 32]).unwrap()))
        );
        let focused = scene.focus_next(true).unwrap();
        scene.show_details(true);
        assert_eq!(scene.focused(), Some(focused));
        scene.show_details(false);
        assert_eq!(scene.focused(), Some(focused));
        assert_eq!(scene.face(), &face);
    }
}

#[test]
fn uncontained_peer_association_does_not_invent_a_reading_hierarchy() {
    let mut face = composed_face();
    face.composition.clear();
    face = face
        .with_composition(vec![FaceCompositionRelation {
            identity: "composition/peer-association".into(),
            source: "clock".into(),
            target: "photo".into(),
            kind: FaceCompositionKind::Associate,
        }])
        .unwrap();
    let (primary, _) = super::super::prepare(&face, true).unwrap();
    let before: alloc::vec::Vec<String> = primary.iter().map(|item| item.text.clone()).collect();
    let mut items = primary;
    super::arrange_primary(&face, &mut items);
    assert_eq!(
        items
            .iter()
            .map(|item| item.text.clone())
            .collect::<alloc::vec::Vec<_>>(),
        before
    );
    assert!(
        items
            .iter()
            .any(|item| item.text.contains("detailed landscape caption"))
    );
}
