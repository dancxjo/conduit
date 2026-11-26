// Shared across 2021/2024 crates; keep their import-order rules from rewriting one another.
#[rustfmt::skip]
use conduit_core::{CheckedValueContract, Quantity, QUANTITY_ENCODED_LEN, VOLTAGE_INFO_ID};
use conduit_face::{
    Face, FaceBasis, FaceProperty, FacePropertyValue as V, FaceRelationship,
    FaceRelationshipKind as K, FaceRole, FaceSubject, QUANTITY_SERIES_ROLE,
};
pub fn fixture() -> Face {
    let mut subjects = vec![
        FaceSubject {
            identity: "source".into(),
            role: FaceRole::Collection,
            name: "Voltage record".into(),
        },
        FaceSubject {
            identity: "voltage".into(),
            role: FaceRole::Semantic(QUANTITY_SERIES_ROLE.into()),
            name: "Supply voltage".into(),
        },
    ];
    let mut properties = vec![];
    let mut put = |subject: &str, name: &str, value| {
        properties.push(FaceProperty {
            subject: subject.into(),
            name: name.into(),
            value,
        })
    };
    put("voltage", "series-source", V::Identity("source".into()));
    put(
        "voltage",
        "series-value-property",
        V::Text("voltage".into()),
    );
    put("source", "clock", V::Text("producer-boot-ms".into()));
    for (name, value) in [
        ("producer-host", "host/meter"),
        ("producer-boot", "boot/meter"),
        ("producer-plan", "plan/meter"),
        ("producer-play", "play/meter"),
    ] {
        put("source", name, V::Identity(value.into()));
    }
    put("source", "visible-samples", V::Count(3));
    put("source", "samples-outside-view", V::Count(2));
    let mut relationships = vec![];
    for (sequence, time, value) in [
        (1, 1000, Some("10V")),
        (2, 2000, None),
        (4, 11000, Some("12V")),
    ] {
        let subject = format!("sample/{sequence}");
        subjects.push(FaceSubject {
            identity: subject.clone(),
            role: FaceRole::Item,
            name: format!("Observation {sequence}"),
        });
        for parent in ["source", "voltage"] {
            relationships.push(FaceRelationship {
                source: parent.into(),
                target: subject.clone(),
                kind: K::Contains,
            });
        }
        put(&subject, "series-source", V::Identity("source".into()));
        put(&subject, "sequence", V::Count(sequence));
        put(&subject, "sampled-at-boot-ms", V::Count(time));
        put(&subject, "series-available", V::Flag(value.is_some()));
        if let Some(value) = value {
            put(
                &subject,
                "voltage",
                V::TypedValue {
                    contract: CheckedValueContract::new(
                        VOLTAGE_INFO_ID.into(),
                        QUANTITY_ENCODED_LEN as u32,
                        vec![],
                    )
                    .unwrap(),
                    bytes: Quantity::parse_plot_literal(value)
                        .unwrap()
                        .encode()
                        .to_vec(),
                },
            );
        } else {
            put(
                &subject,
                "series-unavailability",
                V::Text("meter-unavailable".into()),
            );
        }
        if sequence == 4 {
            put(&subject, "missing-sequence-first", V::Count(3));
            put(&subject, "missing-sequence-last", V::Count(3));
        }
    }
    Face::new_with_semantics(
        1,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some("source/series-fixture".into()),
            checked_plot_id: Some("checked/series-fixture".into()),
            expanded_plot_id: Some("expanded/series-fixture".into()),
            plan_id: Some("plan/view".into()),
            active_play_id: Some("play/view".into()),
            sign_ids: vec![],
        },
        subjects,
        relationships,
        properties,
        vec![],
        vec![],
        vec![],
    )
    .unwrap()
}
