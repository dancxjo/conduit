//! Fresh selected-IMAGE Owner export → checked browser Mask component fixture.
//! Rust update/acknowledgement exercises the checked component Host Calls only.
//! This is not Chromium, WASM, live membership, physical Show or recorder ACK proof.
use super::*;
use conduit_face::{FacePropertyValue, FaceRole, PreparedQuantitySeries, QUANTITY_SERIES_ROLE};

#[test]
#[ignore = "requires fresh genuine environment Owner export; run supported Owner producer first"]
fn original_recording_owner_face_preserves_complete_reading_and_shared_series() {
    let input = std::env::var_os("CONDUIT_ENVIRONMENT_HISTORY_OWNER_FACE").expect(
        "CONDUIT_ENVIRONMENT_HISTORY_OWNER_FACE is required; run the actual Owner producer",
    );
    let output = std::env::var_os("CONDUIT_ENVIRONMENT_HISTORY_OWNER_VIEW");
    if let Some(output) = &output {
        assert_ne!(
            &input, output,
            "view output must not overwrite the Owner export"
        );
    }
    assert!(
        std::fs::metadata(&input).unwrap().len() <= MASK_BYTES as u64,
        "Owner export exceeds the existing Mask input bound"
    );
    let bytes = std::fs::read(&input).unwrap();
    assert!(
        bytes.len() <= MASK_BYTES as usize,
        "Owner export grew beyond the Mask input bound"
    );
    let original: Face = serde_json::from_slice(&bytes).expect("fresh complete Owner Face JSON");
    original.validate().unwrap();
    let provenance = original
        .properties
        .iter()
        .find(|property| {
            property.subject.starts_with("contribution/inspection/")
                && property.name == "active-play-id"
                && matches!(&property.value, FacePropertyValue::Identity(id) if !id.is_empty())
        })
        .expect("Owner Face must retain actual contribution Play provenance");
    for name in ["plan-id", "checked-plot-id"] {
        assert!(
            original.properties.iter().any(|property| {
                property.subject == provenance.subject
                    && property.name == name
                    && matches!(&property.value, FacePropertyValue::Identity(id) if !id.is_empty())
            }),
            "missing {name} on the same contribution provenance subject"
        );
    }
    let body_id = original.basis.body_id.clone().expect("actual Owner Body");
    let series = original.subjects.iter().filter(|subject| {
        matches!(&subject.role, FaceRole::Semantic(role) if role.as_str() == QUANTITY_SERIES_ROLE)
    }).map(|subject| {
        let prepared = PreparedQuantitySeries::prepare(&original, &subject.identity, 64).unwrap();
        (subject.identity.clone(), prepared)
    }).collect::<Vec<_>>();
    assert_eq!(
        series.len(),
        2,
        "fresh export must retain both original temperature and humidity channels"
    );
    for (identity, prepared) in &series {
        assert!(matches!(identity.as_str(), "channel/t" | "channel/h"));
        assert_eq!(
            prepared.producer(),
            [
                "host/reported",
                "boot/reported",
                "plan/reported",
                "play/reported"
            ]
        );
        assert_eq!(prepared.samples().len(), 2);
        assert_eq!(
            (
                prepared.samples()[0].sequence,
                prepared.samples()[0].boot_ms
            ),
            (1, 7)
        );
        assert!(prepared.samples()[0].value.is_some());
        let unavailable = prepared.samples()[1];
        assert_eq!((unavailable.sequence, unavailable.boot_ms), (4, 101));
        assert_eq!(unavailable.gap_before, Some((2, 3)));
        assert_eq!(unavailable.value, None);
        assert_eq!(unavailable.unavailable_reason, Some("checksum"));
    }

    let mut mask = OwnerBrowserMask::prepare_component_fixture(
        HostBasis {
            visual_constraints: None,
            body_id,
            host_id: "host/browser-environment-owner-component-fixture".into(),
            boot_id: "boot/browser-environment-owner-component-fixture".into(),
        },
        original.clone(),
        1,
        false,
    )
    .unwrap();
    assert_eq!(
        mask.face, original,
        "preparation must not reseal or replace the Owner Face"
    );
    let prepared = mask.view();
    assert_eq!(prepared.show_state, "prepared");
    mask.acknowledge(Acknowledgement {
        show_id: prepared.show_id,
        face_id: prepared.face_id,
        face_revision: prepared.face_revision,
    })
    .unwrap();
    let view = mask.view();
    assert_eq!(view.show_state, "available");
    assert_eq!(view.face_id, original.identity.as_str());
    assert_eq!(view.face_revision, original.revision.to_string());
    assert_eq!(view.subjects.len(), original.subjects.len());
    assert_eq!(view.relationships.len(), original.relationships.len());
    assert_eq!(view.actions.len(), original.actions.len());
    for (reading, relation) in view.relationships.iter().zip(&original.relationships) {
        assert_eq!(reading.source, relation.source);
        assert_eq!(reading.target, relation.target);
        assert_eq!(reading.kind, format!("{:?}", relation.kind));
    }
    for (subject, source) in view.subjects.iter().zip(&original.subjects) {
        assert_eq!(subject.identity, source.identity);
        assert_eq!(subject.name, source.name);
        assert_eq!(
            subject.text,
            original
                .text
                .iter()
                .filter(|text| text.subject == source.identity)
                .map(|text| text.text.clone())
                .collect::<Vec<_>>(),
            "complete original reading must remain exact"
        );
        let properties = original
            .properties
            .iter()
            .filter(|property| property.subject == source.identity)
            .collect::<Vec<_>>();
        assert_eq!(
            subject.properties.len(),
            properties.len(),
            "no original fact may be stripped for plot capacity"
        );
        for (reading, property) in subject.properties.iter().zip(&properties) {
            assert!(reading.starts_with(&format!("{}: ", property.name)));
            match &property.value {
                FacePropertyValue::Identity(value) | FacePropertyValue::Text(value) => {
                    assert_eq!(reading, &format!("{}: {value}", property.name));
                }
                FacePropertyValue::Count(value) => {
                    assert_eq!(reading, &format!("{}: {value}", property.name));
                }
                FacePropertyValue::Flag(value) => {
                    assert_eq!(reading, &format!("{}: {value}", property.name))
                }
                FacePropertyValue::Signed(value) => {
                    assert_eq!(reading, &format!("{}: {value}", property.name))
                }
                FacePropertyValue::TypedValue { contract, bytes } => {
                    assert_eq!(
                        reading,
                        &format!(
                            "{}: {}",
                            property.name,
                            conduit_face::display_typed_value(contract, bytes)
                        )
                    );
                    let value = subject
                        .values
                        .iter()
                        .find(|value| value.name == property.name)
                        .unwrap();
                    assert_eq!(value.kind, contract.value_kind.as_str());
                    assert_eq!(
                        value.text,
                        conduit_face::display_typed_value(contract, bytes)
                    );
                }
                _ => {}
            }
        }
    }
    let json = serde_json::to_value(&view).unwrap();
    for (identity, prepared) in &series {
        let channel = json["subjects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|subject| subject["identity"] == *identity)
            .unwrap();
        let plot = &channel["quantity_series"]["plot"];
        assert!(!plot.is_null(), "shared plot absent: optional plot capacity/refusal; original Face bytes={}, encoded view bytes={}, output capacity={OUTPUT_CAPACITY}, channel={channel}", bytes.len(), serde_json::to_vec(&view).unwrap().len());
        assert_eq!(
            plot,
            &serde_json::to_value(prepared.project(None, 16).unwrap()).unwrap()
        );
        assert_eq!(plot["producer"][1], "boot/reported");
        assert_eq!(plot["source_samples"], 2);
        assert_eq!(plot["unavailable_samples"], 1);
        assert_eq!(plot["gap_runs"], 1);
        assert_eq!(plot["omitted_samples"], "0");
        let points = plot["points"].as_array().unwrap();
        assert_eq!(
            points.len(),
            1,
            "only available samples become plotted points"
        );
        assert_eq!(points[0]["boot_ms"], "7");
    }
    assert_eq!(mask.face, original);
    assert_eq!(
        std::fs::read(&input).unwrap(),
        bytes,
        "Owner export remains untouched"
    );
    let encoded = serde_json::to_vec(&view).unwrap();
    assert!(
        encoded.len() <= OUTPUT_CAPACITY,
        "complete environment Owner view requires {} bytes; existing capacity is {OUTPUT_CAPACITY}",
        encoded.len()
    );
    if let Some(output) = output {
        std::fs::write(output, encoded).unwrap();
    }
}
