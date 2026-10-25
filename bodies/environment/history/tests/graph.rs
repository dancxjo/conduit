use conduit_core::{
    kind_id, CheckedValueContract, Quantity, Unit, QUANTITY_ENCODED_LEN, RATIO_INFO_ID,
    TEMPERATURE_INFO_ID,
};
use conduit_environment_history::{GraphRefusal, HistoryGraph};
use conduit_face::{
    Face, FaceBasis, FaceProperty, FacePropertyValue as V, FaceRelationship, FaceRelationshipKind,
    FaceRole, FaceSubject,
};
fn face(times: &[u64]) -> Face {
    let subject = "body/history";
    let mut subjects = vec![FaceSubject {
        identity: subject.into(),
        role: FaceRole::Collection,
        name: "History".into(),
    }];
    let mut props = vec![];
    let mut put = |s: &str, n: &str, v| {
        props.push(FaceProperty {
            subject: s.into(),
            name: n.into(),
            value: v,
        })
    };
    for (n, v) in [
        ("schema", V::Text("environment/history@1".into())),
        ("clock", V::Text("producer-boot-ms".into())),
        ("producer-boot", V::Identity("boot/test".into())),
        ("producer-host", V::Identity("host/test".into())),
        ("producer-plan", V::Identity("plan/test".into())),
        ("producer-play", V::Identity("play/test".into())),
        ("recovered-chunks", V::Count(u64::from(!times.is_empty()))),
        ("samples-outside-view", V::Count(0)),
        ("visible-samples", V::Count(times.len() as u64)),
        ("recovered-samples", V::Count(times.len() as u64)),
        ("preceding-history-known", V::Flag(false)),
    ] {
        put(subject, n, v);
    }
    let mut relationships = vec![];
    for (i, &time) in times.iter().enumerate() {
        let s = format!("body/sample/{i}");
        subjects.push(FaceSubject {
            identity: s.clone(),
            role: FaceRole::Item,
            name: "Sample".into(),
        });
        relationships.push(FaceRelationship {
            source: subject.into(),
            target: s.clone(),
            kind: FaceRelationshipKind::Contains,
        });
        put(&s, "sequence", V::Count(i as u64 + 1));
        put(&s, "sampled-at-boot-ms", V::Count(time));
        put(&s, "sensor-status", V::Text("available".into()));
        put(
            &s,
            "temperature",
            V::TypedValue {
                contract: CheckedValueContract::new(
                    kind_id(TEMPERATURE_INFO_ID),
                    QUANTITY_ENCODED_LEN as u32,
                    vec![],
                )
                .unwrap(),
                bytes: Quantity::new(i as i64, Unit::Celsius).encode().to_vec(),
            },
        );
        put(
            &s,
            "relative-humidity",
            V::TypedValue {
                contract: CheckedValueContract::new(
                    kind_id(RATIO_INFO_ID),
                    QUANTITY_ENCODED_LEN as u32,
                    vec![],
                )
                .unwrap(),
                bytes: Quantity::new(50, Unit::Percent).encode().to_vec(),
            },
        );
    }
    Face::new_with_semantics(
        9,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        subjects,
        relationships,
        props,
        vec![],
        vec![],
        vec![],
    )
    .unwrap()
}
#[test]
fn same_face_both_masks_use_actual_boot_time_and_bound_samples() {
    let input = face(&[100, 200, 1100]);
    let graph = HistoryGraph::prepare(&input, "body/history").unwrap();
    assert_eq!(
        graph
            .samples()
            .iter()
            .map(|s| graph.x(s))
            .collect::<Vec<_>>(),
        vec![32, 88, 592]
    );
    assert_eq!(graph.content_id(), input.identity.as_str());
    assert_eq!(graph.revision(), 9);
    let native = graph.native_show().unwrap();
    assert_eq!(native.content_id, graph.content_id());
    assert_eq!(native.revision, graph.revision());
    assert!(graph.browser_html().contains("cx=\"88\""));
    assert_eq!(graph.native_scene().unwrap().commands().len(), 11);
    let face = face(&(0..64).collect::<Vec<_>>());
    let graph = HistoryGraph::prepare(&face, "body/history").unwrap();
    assert_eq!(graph.samples().len(), 16);
    assert_eq!(graph.omitted(), 48);
    assert!(graph.native_scene().unwrap().commands().len() <= 48);
}
#[test]
fn failure_is_absence_and_gap_is_explicit_never_a_zero_or_line() {
    let mut face = face(&[100, 200, 900]);
    face.properties.retain(|p| {
        !(p.subject == "body/sample/1"
            && (p.name == "temperature" || p.name == "relative-humidity"))
    });
    for p in &mut face.properties {
        if p.subject == "body/sample/1" && p.name == "sensor-status" {
            p.value = V::Text("unavailable".into());
        }
        if p.subject == "body/sample/2" && p.name == "sequence" {
            p.value = V::Count(5);
        }
    }
    for (s, n, v) in [
        (
            "body/sample/1",
            "sensor-failure",
            V::Text("checksum".into()),
        ),
        ("body/sample/2", "missing-sequence-first", V::Count(3)),
        ("body/sample/2", "missing-sequence-last", V::Count(4)),
    ] {
        face.properties.push(FaceProperty {
            subject: s.into(),
            name: n.into(),
            value: v,
        });
    }
    face.reseal_prepared().unwrap();
    let graph = HistoryGraph::prepare(&face, "body/history").unwrap();
    assert_eq!(graph.samples()[1].temperature_milli_c(), None);
    assert_eq!(graph.samples()[2].gap_before(), Some((3, 4)));
    let html = graph.browser_html();
    assert!(!html.contains("<line"));
    assert!(html.contains("missing sequences 3 through 4"));
    assert!(html.contains("sensor unavailable"));
}
#[test]
fn duplicate_malformed_stale_and_foreign_selection_are_refused() {
    let mut f = face(&[0]);
    assert!(matches!(
        HistoryGraph::prepare(&f, "other"),
        Err(GraphRefusal::Missing)
    ));
    f.properties.push(f.properties[0].clone());
    f.reseal_prepared().unwrap();
    assert!(matches!(
        HistoryGraph::prepare(&f, "body/history"),
        Err(GraphRefusal::Duplicate)
    ));
    f.properties.pop();
    f.properties[1].value = V::Count(0);
    f.reseal_prepared().unwrap();
    assert!(matches!(
        HistoryGraph::prepare(&f, "body/history"),
        Err(GraphRefusal::InvalidProperty)
    ));
    f.revision += 1;
    assert!(matches!(
        HistoryGraph::prepare(&f, "body/history"),
        Err(GraphRefusal::InvalidFace)
    ));
    let f = face(&[]);
    let graph = HistoryGraph::prepare(&f, "body/history").unwrap();
    assert!(graph.samples().is_empty());
    assert!(graph.native_scene().is_ok());
}

#[test]
fn relation_order_does_not_change_chronology_and_false_failure_is_refused() {
    let mut f = face(&[10, 20, 100]);
    f.relationships.reverse();
    f.reseal_prepared().unwrap();
    let graph = HistoryGraph::prepare(&f, "body/history").unwrap();
    assert_eq!(
        graph
            .samples()
            .iter()
            .map(|s| s.sequence())
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    f.properties.push(FaceProperty {
        subject: "body/sample/0".into(),
        name: "sensor-failure".into(),
        value: V::Text("checksum".into()),
    });
    f.reseal_prepared().unwrap();
    assert!(matches!(
        HistoryGraph::prepare(&f, "body/history"),
        Err(GraphRefusal::InvalidProperty)
    ));
}
#[test]
fn quantity_contract_and_sub_milli_precision_are_not_silently_reinterpreted() {
    let mut f = face(&[10]);
    let quantity = f
        .properties
        .iter_mut()
        .find(|p| p.name == "temperature")
        .unwrap();
    if let V::TypedValue { contract, .. } = &mut quantity.value {
        contract.value_kind = kind_id(conduit_core::QUANTITY_INFO_ID);
    }
    f.reseal_prepared().unwrap();
    assert!(matches!(
        HistoryGraph::prepare(&f, "body/history"),
        Err(GraphRefusal::InvalidProperty)
    ));
    let mut f = face(&[10]);
    let quantity = f
        .properties
        .iter_mut()
        .find(|p| p.name == "temperature")
        .unwrap();
    if let V::TypedValue { bytes, .. } = &mut quantity.value {
        *bytes = Quantity::from_decimal(12345, -4, Unit::Celsius)
            .unwrap()
            .encode()
            .to_vec();
    }
    f.reseal_prepared().unwrap();
    assert!(matches!(
        HistoryGraph::prepare(&f, "body/history"),
        Err(GraphRefusal::InvalidProperty)
    ));
}

#[test]
fn extreme_coordinate_retains_its_axis_and_partial_record_is_refused() {
    let mut input = face(&[0]);
    let value = input
        .properties
        .iter_mut()
        .find(|p| p.name == "temperature")
        .unwrap();
    if let V::TypedValue { bytes, .. } = &mut value.value {
        *bytes = Quantity::from_decimal(i128::from(i64::MAX), -3, Unit::Celsius)
            .unwrap()
            .encode()
            .to_vec();
    }
    input.reseal_prepared().unwrap();
    let graph = HistoryGraph::prepare(&input, "body/history").unwrap();
    assert_eq!(graph.temperature_range(), (i64::MAX - 1000, i64::MAX));
    assert_eq!(graph.y(i64::MAX, false), 40);
    assert!(graph.native_scene().is_ok());
    input.properties.retain(|p| p.name != "relative-humidity");
    input.reseal_prepared().unwrap();
    assert!(matches!(
        HistoryGraph::prepare(&input, "body/history"),
        Err(GraphRefusal::InvalidProperty)
    ));
}

#[test]
fn quantity_looks_change_both_graphical_labels_without_changing_the_face_or_dot_coordinates() {
    use conduit_environment_history::HistoryTemperaturePresentation;
    use conduit_face::{LookInfo, LookQuantityFacetOption, LookTemperatureUnit as T};
    let face = face(&[1000, 2000, 9000]);
    let original = face.clone();
    let graph = HistoryGraph::prepare(&face, "body/history").unwrap();
    let dots = graph
        .samples()
        .iter()
        .map(|sample| {
            (
                graph.x(sample),
                sample.temperature_milli_c().map(|v| graph.y(v, false)),
            )
        })
        .collect::<Vec<_>>();
    let base = conduit_face::editorial_look().unwrap();
    for (unit, label, first, top) in [
        (T::Celsius, "°C", "0.000", "2.000"),
        (T::Kelvin, "K", "273.150", "275.150"),
        (T::Fahrenheit, "°F", "32.000", "35.600"),
    ] {
        let look = LookInfo::new(
            base.generative().clone(),
            base.identity().into(),
            LookQuantityFacetOption::some(unit).unwrap(),
            base.spoken().clone(),
            3,
            base.visual().clone(),
        )
        .unwrap();
        let shown = HistoryTemperaturePresentation::prepare(&graph, &look).unwrap();
        assert_eq!(shown.unit_label(), label);
        assert_eq!(shown.samples[0].as_ref().unwrap().number(), first);
        assert_eq!(shown.top.number(), top);
        assert_eq!(shown.bottom.number(), first);
        for (sample, displayed) in graph.samples().iter().zip(&shown.samples) {
            assert_eq!(
                displayed.as_ref().map(|value| value.source()),
                sample.temperature_source()
            );
        }
        let html = graph.browser_html_with_look(&look).unwrap();
        assert!(html.contains(&format!("Temperature ({label})")));
        assert!(html.contains(&format!("Temperature {label}</th>")));
        assert!(html.contains(&format!("<td>{first}</td>")));
        let scene = graph.native_scene_with_look(&look).unwrap();
        assert!(scene.commands().iter().any(|command| command
            .payload()
            .contains(&format!("Temperature {label}: top {top}; bottom {first}"))));
        assert_eq!(
            graph
                .samples()
                .iter()
                .map(|sample| (
                    graph.x(sample),
                    sample.temperature_milli_c().map(|v| graph.y(v, false))
                ))
                .collect::<Vec<_>>(),
            dots
        );
        assert_eq!(face, original);
    }
}
