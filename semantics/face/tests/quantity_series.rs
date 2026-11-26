//! Independent voltage observations, not environmental schemas or Body custody.
use conduit_core::{CheckedValueContract, Quantity, QUANTITY_ENCODED_LEN, VOLTAGE_INFO_ID};
use conduit_face::{
    Face, FaceBasis, FaceProperty, FacePropertyValue as V, FaceRelationship,
    FaceRelationshipKind as K, FaceRole, FaceSubject, PreparedQuantitySeries as Series,
    QuantitySeriesRefusal as R, QUANTITY_SERIES_ROLE,
};
fn fixture() -> Face {
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
#[test]
fn irregular_voltage_times_gaps_units_and_unavailability_remain_exact() {
    let face = fixture();
    let before = face.clone();
    let series = Series::prepare(&face, "voltage", 3).unwrap();
    assert_eq!(
        series.producer(),
        ["host/meter", "boot/meter", "plan/meter", "play/meter"]
    );
    assert_eq!(series.omitted_samples(), 2);
    assert_eq!(
        (0..3)
            .map(|i| series.time_millionths(i).unwrap())
            .collect::<Vec<_>>(),
        [0, 100_000, 1_000_000]
    );
    assert_eq!(
        series.samples()[0].value,
        Some(Quantity::parse_plot_literal("10V").unwrap())
    );
    assert_eq!(series.samples()[1].value, None);
    assert_eq!(
        series.samples()[1].unavailable_reason,
        Some("meter-unavailable")
    );
    assert_eq!(series.samples()[2].gap_before, Some((3, 3)));
    assert_eq!(face, before);
}
#[test]
fn foreign_source_regressed_time_and_missing_gap_refuse_without_mutation() {
    for mode in 0..3 {
        let mut face = fixture();
        let property = match mode {
            0 => (
                "sample/2",
                "series-source",
                V::Identity("other-boot-source".into()),
            ),
            1 => ("sample/2", "sampled-at-boot-ms", V::Count(999)),
            _ => ("sample/4", "missing-sequence-first", V::Count(2)),
        };
        face.properties
            .iter_mut()
            .find(|p| p.subject == property.0 && p.name == property.1)
            .unwrap()
            .value = property.2;
        face.reseal_prepared().unwrap();
        let before = face.clone();
        let expected = [R::Provenance, R::Order, R::Gap][mode];
        assert!(matches!(Series::prepare(&face, "voltage", 3), Err(reason) if reason == expected));
        assert_eq!(face, before);
    }
}
#[test]
fn capacity_and_equal_times_are_explicit() {
    let mut face = fixture();
    assert!(matches!(
        Series::prepare(&face, "voltage", 2),
        Err(R::Capacity)
    ));
    for property in &mut face.properties {
        if property.name == "sampled-at-boot-ms" {
            property.value = V::Count(1000);
        }
    }
    face.reseal_prepared().unwrap();
    let series = Series::prepare(&face, "voltage", 3).unwrap();
    assert_eq!(
        (0..3)
            .map(|i| series.time_millionths(i).unwrap())
            .collect::<Vec<_>>(),
        [0, 0, 0]
    );
}

#[test]
fn inconsistent_family_availability_order_and_accounting_refuse_distinctly() {
    for (mode, expected) in [
        (0, R::Availability),
        (1, R::Quantity),
        (2, R::Order),
        (3, R::Provenance),
        (4, R::Availability),
        (5, R::Clock),
    ] {
        let mut face = fixture();
        let replacement = match mode {
            0 => ("sample/2", "series-available", V::Flag(true)),
            1 => (
                "sample/4",
                "voltage",
                V::TypedValue {
                    contract: CheckedValueContract::new(
                        conduit_core::TEMPERATURE_INFO_ID.into(),
                        QUANTITY_ENCODED_LEN as u32,
                        vec![],
                    )
                    .unwrap(),
                    bytes: Quantity::parse_plot_literal("12°C")
                        .unwrap()
                        .encode()
                        .to_vec(),
                },
            ),
            2 => ("sample/2", "sequence", V::Count(1)),
            3 => ("source", "visible-samples", V::Count(2)),
            4 => (
                "sample/1",
                "series-unavailability",
                V::Text("false-failure".into()),
            ),
            _ => ("source", "clock", V::Text("wall-clock-ms".into())),
        };
        if let Some(property) = face
            .properties
            .iter_mut()
            .find(|property| property.subject == replacement.0 && property.name == replacement.1)
        {
            property.value = replacement.2;
        } else {
            face.properties.push(FaceProperty {
                subject: replacement.0.into(),
                name: replacement.1.into(),
                value: replacement.2,
            });
        }
        face.reseal_prepared().unwrap();
        assert!(matches!(Series::prepare(&face, "voltage", 3), Err(reason) if reason == expected));
    }
}
