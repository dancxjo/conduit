//! Opt-in native/Node-browser/linear semantic conformance, not platform enactment.
use super::*;
use alloc::borrow::ToOwned;
use conduit_face::{FacePropertyValue, FaceRelationshipKind, FaceRole, ProjectionItem};
use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Stdio},
};

fn specimen() -> Face {
    serde_json::from_str(include_str!(
        "../../../../proof/conformance/face-waist/domain-derived/patchbay-face.json"
    ))
    .unwrap()
}
fn reseal(face: Face) -> Face {
    let basis = face.basis.clone();
    face.with_basis(basis).unwrap()
}
fn text(value: &FacePropertyValue) -> &str {
    match value {
        FacePropertyValue::Text(value) | FacePropertyValue::Identity(value) => value,
        _ => panic!("fixture needs text identity"),
    }
}
fn browser(face: &Face, navigation: &patchbay_application::PatchbayNavigationProjection) -> Value {
    let mut child = Command::new("node")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../proof/browser/patchbay-cross-mask-oracle.mjs"
        ))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("opt-in cross-Mask conformance requires Node");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            &serde_json::to_vec(&json!({
                "face": face, "navigation": navigation, "interaction": {"selected_subject": null}
            }))
            .unwrap(),
        )
        .unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice(&result.stdout).unwrap()
}

#[test]
#[ignore = "opt-in CPU cross-Mask conformance requires Node; not real-browser proof"]
fn native_browser_and_linear_reconstruct_one_exact_graph_with_explicit_gaps() {
    let original = specimen();
    let mut long = original.clone();
    long.subjects
        .iter_mut()
        .find(|subject| subject.role == FaceRole::Gear)
        .unwrap()
        .name = "Élan-源-long-label-".repeat(20);
    let mut missing = original.clone();
    let cord = missing
        .subjects
        .iter()
        .find(|subject| subject.role == FaceRole::Cord)
        .unwrap()
        .identity
        .clone();
    let sink = missing
        .properties
        .iter_mut()
        .find(|property| property.subject == cord && property.name == "sink-port")
        .unwrap();
    let old_sink = text(&sink.value).to_owned();
    sink.value = FacePropertyValue::Identity("missing/endpoint".into());
    let sink_subject = missing
        .properties
        .iter()
        .find(|property| property.name == "semantic-id" && text(&property.value) == old_sink)
        .unwrap()
        .subject
        .clone();
    missing.relationships.retain(|relation| {
        !(relation.kind == FaceRelationshipKind::Connects
            && relation.source == cord
            && relation.target == sink_subject)
    });
    for (name, face) in [
        ("canonical", original),
        ("long-label", reseal(long)),
        ("missing-endpoint", reseal(missing)),
    ] {
        let mut native = NativeFaceScene::prepare(face.clone(), 1280, 800).unwrap();
        let focus = &face
            .subjects
            .iter()
            .find(|subject| subject.role == FaceRole::Cord)
            .unwrap()
            .identity;
        native
            .inspect_subject(&face.identity, face.revision, focus)
            .unwrap();
        let bundle = native.graph_projection().unwrap();
        let projected = bundle
            .projection
            .project(&face, &bundle.navigation, &bundle.cursor)
            .unwrap();
        let actual = browser(&face, bundle);
        let selected_subjects: Vec<_> = face
            .subjects
            .iter()
            .filter(|subject| {
                projected.items.iter().any(|membership| {
                    membership.item == ProjectionItem::Subject(subject.identity.clone())
                })
            })
            .collect();
        let selected_relationships: Vec<_> = face
            .relationships
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                projected.items.iter().any(|membership| {
                    membership.item == ProjectionItem::Relationship(*index as u16)
                })
            })
            .map(|(_, value)| value)
            .collect();
        let selected_properties: Vec<_> =
            face.properties
                .iter()
                .enumerate()
                .filter(|(index, _)| {
                    projected.items.iter().any(|membership| {
                        membership.item == ProjectionItem::Property(*index as u16)
                    })
                })
                .map(|(_, value)| value)
                .collect();
        assert_eq!(actual["subjects"], json!(selected_subjects), "{name}");
        assert_eq!(
            actual["relationships"],
            json!(selected_relationships),
            "{name}"
        );
        assert_eq!(actual["properties"], json!(selected_properties), "{name}");
        assert_eq!(
            actual["observation"],
            serde_json::to_value(
                conduit_face::observe_navigation(
                    &face,
                    &bundle.navigation,
                    &bundle.projection,
                    &bundle.cursor
                )
                .unwrap()
            )
            .unwrap(),
            "{name}"
        );
        let linear = conduit_face::render_linear_navigation(
            &face,
            &bundle.navigation,
            &bundle.projection,
            &bundle.cursor,
        )
        .unwrap();
        for subject in &selected_subjects {
            assert!(
                linear.lines.contains(&format!(
                    "SUBJECT role={:?} id={:?} name={:?}",
                    subject.role, subject.identity, subject.name
                )),
                "{name}"
            );
        }
        for relation in &selected_relationships {
            assert!(
                linear.lines.contains(&format!(
                    "RELATIONSHIP kind={:?} source={:?} target={:?}",
                    relation.kind, relation.source, relation.target
                )),
                "{name}"
            );
        }
        for property in &selected_properties {
            if let FacePropertyValue::Text(value) | FacePropertyValue::Identity(value) =
                &property.value
            {
                let kind = if matches!(property.value, FacePropertyValue::Text(_)) {
                    "text"
                } else {
                    "identity"
                };
                assert!(
                    linear.lines.contains(&format!(
                        "PROPERTY subject={:?} name={:?} value={kind}:{value:?}",
                        property.subject, property.name
                    )),
                    "linear property: {name}"
                );
            }
        }
        let cords: Vec<_> = selected_subjects
            .iter()
            .filter(|subject| subject.role == FaceRole::Cord)
            .collect();
        assert_eq!(actual["cord_count"], json!(cords.len()));
        for node in actual["nodes"].as_array().unwrap() {
            let subject = face
                .subjects
                .iter()
                .find(|subject| node["id"] == subject.identity)
                .unwrap();
            assert_eq!(node["role"], json!(subject.role));
            assert_eq!(node["label"], json!(subject.name));
            for port in node["ports"].as_array().unwrap() {
                let subject = face
                    .subjects
                    .iter()
                    .find(|subject| port["id"] == subject.identity)
                    .unwrap();
                assert_eq!(subject.role, FaceRole::Port);
                assert_eq!(port["label"], json!(subject.name));
                let direction = face
                    .properties
                    .iter()
                    .find(|property| {
                        property.subject == subject.identity && property.name == "direction"
                    })
                    .unwrap();
                assert_eq!(port["direction"], json!(text(&direction.value)));
            }
        }
        let mut expected_edges: Vec<_> = cords.iter().filter_map(|cord| {
            let endpoint = |name| {
                let property = face.properties.iter().find(|property| property.subject == cord.identity && property.name == name)?;
                let port = face.properties.iter().find(|candidate| candidate.name == "semantic-id" && text(&candidate.value) == text(&property.value))?;
                let owner = face.relationships.iter().find(|relation| relation.kind == FaceRelationshipKind::Contains && relation.target == port.subject && actual["nodes"].as_array().unwrap().iter().any(|node| node["id"] == relation.source))?;
                Some((&owner.source, &port.subject))
            };
            let (source, source_port) = endpoint("source-port")?;
            let (target, sink_port) = endpoint("sink-port")?;
            Some(json!({"id": cord.identity, "source": source, "target": target, "source_port": source_port, "sink_port": sink_port}))
        }).collect();
        let mut actual_edges = actual["edges"].as_array().unwrap().clone();
        expected_edges.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
        actual_edges.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
        assert_eq!(
            actual_edges, expected_edges,
            "ordered endpoint geometry: {name}"
        );
        for edge in actual["edges"].as_array().unwrap() {
            let id = edge["id"].as_str().unwrap();
            for (property, field) in [("source-port", "source_port"), ("sink-port", "sink_port")] {
                let endpoint = text(
                    &face
                        .properties
                        .iter()
                        .find(|value| value.subject == id && value.name == property)
                        .unwrap()
                        .value,
                );
                let port = face
                    .properties
                    .iter()
                    .find(|value| value.name == "semantic-id" && text(&value.value) == endpoint)
                    .unwrap();
                assert_eq!(edge[field], json!(port.subject), "{name}");
            }
        }
        if name == "missing-endpoint" {
            assert!(
                !actual["edges"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|edge| edge["id"] == cord)
            );
            assert!(
                actual["properties"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|value| value["value"]["Identity"] == "missing/endpoint")
            );
            assert!(
                linear
                    .lines
                    .iter()
                    .any(|line| line.contains("missing/endpoint"))
            );
        }
        let mut stale = bundle.cursor.clone();
        stale.revision += 1;
        assert!(
            conduit_face::render_linear_navigation(
                &face,
                &bundle.navigation,
                &bundle.projection,
                &stale
            )
            .is_err()
        );
        assert!(matches!(
            native.inspect_subject(&face.identity, face.revision + 1, focus),
            Err(FaceSceneError::StaleFace)
        ));
        assert_eq!(native.face(), &face);
        assert_eq!(actual["stale_rejected"], true);
        assert_eq!(actual["unknown_rejected"], true);
    }
}
