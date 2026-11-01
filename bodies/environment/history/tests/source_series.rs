use conduit_core::{Quantity, Unit};
use conduit_environment_history::source_series::*;
use conduit_environment_measurement::{source_observation::*, EnvironmentalReading, ReadingField};
use conduit_face::{Face, FaceBasis, FaceContributionBasis, FacePropertyValue as V};
fn value(time: u64, counter: u64) -> Vec<u8> {
    let c = ObservationContract::prepare().unwrap();
    let mut codec = PreparedObservationCodec::prepare(&c).unwrap();
    codec
        .encode(SourceObservation {
            observed_at_host_boot_ms: time,
            outcome: SourceOutcome::Available(SourceSample {
                source_counter: counter,
                reported_source_ms: Some(999999),
                reading: EnvironmentalReading::new(
                    Quantity::new(20, Unit::Celsius),
                    ReadingField::Available(Quantity::new(50, Unit::Percent)),
                    ReadingField::Available(Quantity::new(101325, Unit::Pascal)),
                )
                .unwrap(),
            }),
        })
        .unwrap()
        .to_vec()
}
fn basis() -> FaceContributionBasis {
    FaceContributionBasis {
        checked_plot_id: "checked/series-test".into(),
        plan_id: "plan/series-test".into(),
        active_play_id: "play/series-test".into(),
        required_interaction_context: None,
    }
}
fn seal(f: conduit_face::FaceFragment) -> Face {
    Face::new(
        1,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some("source/series-test".into()),
            checked_plot_id: Some(basis().checked_plot_id),
            expanded_plot_id: Some("expanded/series-test".into()),
            plan_id: Some(basis().plan_id),
            active_play_id: Some(basis().active_play_id),
            sign_ids: vec![],
        },
        f.subjects,
        f.relationships,
        f.properties,
        f.text,
    )
    .unwrap()
}
fn collection<'a>(series: &'a [SourceSeries<'a>]) -> SourceSeriesCollection<'a> {
    SourceSeriesCollection {
        subject: "observations",
        collector_host: "host/collector",
        collector_boot: "boot/collector",
        observed_through_boot_ms: 5000,
        series,
    }
}
fn series<'a>(records: &'a [SourceSeriesRecord<'a>]) -> SourceSeries<'a> {
    SourceSeries {
        subject: "series/blue",
        source: "equipment/blue",
        name: "Blue",
        samples_outside_view: 3,
        stale_after_ms: 1000,
        records,
    }
}
#[test]
fn four_streams_share_exact_collector_clock_and_preserve_canonical_pressure() {
    let bytes = value(4500, 42);
    let records = [SourceSeriesRecord {
        sequence: 4,
        observation: &bytes,
    }];
    let series: Vec<_> = [
        ("Blue", "series/blue", "equipment/blue"),
        ("Green", "series/green", "equipment/green"),
        ("White", "series/white", "equipment/white"),
        ("Yellow", "series/yellow", "equipment/yellow"),
    ]
    .into_iter()
    .map(|(name, subject, source)| SourceSeries {
        name,
        subject,
        source,
        ..series(&records)
    })
    .collect();
    let face = seal(collection(&series).contribution(basis()).unwrap());
    let original = face.clone();
    let graph = SourceSeriesGraph::prepare(&face, "observations").unwrap();
    assert_eq!(graph.series().len(), 4);
    assert_eq!(graph.collector_host(), "host/collector");
    assert_eq!(graph.collector_boot(), "boot/collector");
    for s in graph.series() {
        assert_eq!(s.points[0].observation.observed_at_host_boot_ms, 4500);
        assert_eq!(
            s.state_at(5000).unwrap(),
            SourceSeriesState::Available { age_ms: 500 }
        );
        let SourceOutcome::Available(sample) = s.points[0].observation.outcome else {
            panic!()
        };
        assert_eq!(sample.reported_source_ms, Some(999999));
        assert_eq!(
            sample.reading.atmospheric_pressure(),
            ReadingField::Available(Quantity::new(101325, Unit::Pascal))
        );
    }
    assert!(face
        .properties
        .iter()
        .filter_map(|p| match &p.value {
            V::TypedValue { bytes, .. } => Some(bytes),
            _ => None,
        })
        .all(|b| b == &bytes));
    assert_eq!(face, original);
}
#[test]
fn cutoff_staleness_failures_and_missing_sequences_are_not_zero_values() {
    let c = ObservationContract::prepare().unwrap();
    let mut codec = PreparedObservationCodec::prepare(&c).unwrap();
    let bytes = value(4500, 42);
    let failure = codec
        .encode(SourceObservation {
            observed_at_host_boot_ms: 4800,
            outcome: SourceOutcome::Unavailable(SourceFailure::CaptureTimeout),
        })
        .unwrap()
        .to_vec();
    let records = [
        SourceSeriesRecord {
            sequence: 4,
            observation: &bytes,
        },
        SourceSeriesRecord {
            sequence: 7,
            observation: &failure,
        },
    ];
    let face = seal(
        collection(&[series(&records)])
            .contribution(basis())
            .unwrap(),
    );
    let g = SourceSeriesGraph::prepare(&face, "observations").unwrap();
    let s = &g.series()[0];
    assert_eq!((s.points[0].sequence, s.points[1].sequence), (4, 7));
    assert_eq!(
        s.state_at(5000).unwrap(),
        SourceSeriesState::Unavailable(SourceFailure::CaptureTimeout)
    );
    let records = [SourceSeriesRecord {
        sequence: 4,
        observation: &bytes,
    }];
    let face = seal(
        collection(&[series(&records)])
            .contribution(basis())
            .unwrap(),
    );
    let g = SourceSeriesGraph::prepare(&face, "observations").unwrap();
    let s = &g.series()[0];
    assert_eq!(
        s.state_at(5500).unwrap(),
        SourceSeriesState::Stale { age_ms: 1000 }
    );
    assert_eq!(s.state_at(4499), Err(SourceSeriesRefusal::Clock));
}
#[test]
fn extent_identity_order_future_time_and_foreign_values_refuse() {
    let bytes = value(4500, 42);
    let records = [SourceSeriesRecord {
        sequence: 4,
        observation: &bytes,
    }];
    let two = [series(&records), series(&records)];
    assert_eq!(
        collection(&two).validate(),
        Err(SourceSeriesRefusal::Identity)
    );
    let mut c = collection(&[]);
    c.collector_boot = "";
    assert_eq!(c.validate(), Err(SourceSeriesRefusal::Identity));
    let one = [series(&records)];
    let mut c = collection(&one);
    c.observed_through_boot_ms = 4000;
    assert_eq!(c.validate(), Err(SourceSeriesRefusal::Clock));
    let duplicate = [
        SourceSeriesRecord {
            sequence: 4,
            observation: &bytes,
        },
        SourceSeriesRecord {
            sequence: 4,
            observation: &bytes,
        },
    ];
    assert_eq!(
        collection(&[series(&duplicate)]).validate(),
        Err(SourceSeriesRefusal::Sequence)
    );
    let oversized: Vec<_> = (1..=17)
        .map(|sequence| SourceSeriesRecord {
            sequence,
            observation: &bytes,
        })
        .collect();
    assert_eq!(
        collection(&[series(&oversized)]).validate(),
        Err(SourceSeriesRefusal::Extent)
    );
    let bad = [SourceSeriesRecord {
        sequence: 1,
        observation: &[0; 21],
    }];
    assert_eq!(
        collection(&[series(&bad)]).validate(),
        Err(SourceSeriesRefusal::Observation)
    );
}
#[test]
fn face_reader_checks_clock_canonical_type_and_sequence_after_reordering() {
    let bytes = value(4500, 42);
    let records = [SourceSeriesRecord {
        sequence: 4,
        observation: &bytes,
    }];
    let mut fragment = collection(&[series(&records)])
        .contribution(basis())
        .unwrap();
    fragment.relationships.reverse();
    fragment.properties.reverse();
    assert!(SourceSeriesGraph::prepare(&seal(fragment.clone()), "observations").is_ok());
    fragment
        .properties
        .iter_mut()
        .find(|p| p.name == "clock")
        .unwrap()
        .value = V::Text("producer-boot-ms".into());
    assert_eq!(
        SourceSeriesGraph::prepare(&seal(fragment), "observations"),
        Err(SourceSeriesRefusal::Clock)
    );
}

fn palette(s: &PreparedSourceSeries) -> conduit_environment_history::SourceSeriesStyle {
    use conduit_face::GraphicsPaintRole as P;
    conduit_environment_history::SourceSeriesStyle {
        paint: match s.name.as_str() {
            "Blue" => P::Accent,
            "Green" => P::Success,
            "White" => P::Foreground,
            "Yellow" => P::Warning,
            _ => P::Muted,
        },
        dashed: s.name == "White",
    }
}
#[test]
fn shared_quantity_axes_match_label_and_line_roles_without_mutating_face() {
    use conduit_face::{GraphicsCommandKind as K, GraphicsPaintRole as P};
    let a = value(1000, 1);
    let b = value(4500, 2);
    let records = [
        SourceSeriesRecord {
            sequence: 1,
            observation: &a,
        },
        SourceSeriesRecord {
            sequence: 2,
            observation: &b,
        },
    ];
    let series: Vec<_> = [
        ("Blue", "series/blue", "equipment/blue"),
        ("Green", "series/green", "equipment/green"),
        ("White", "series/white", "equipment/white"),
        ("Yellow", "series/yellow", "equipment/yellow"),
    ]
    .into_iter()
    .map(|(name, subject, source)| SourceSeries {
        name,
        subject,
        source,
        ..series(&records)
    })
    .collect();
    let face = seal(collection(&series).contribution(basis()).unwrap());
    let original = face.clone();
    let graph = SourceSeriesGraph::prepare(&face, "observations").unwrap();
    for width in [320, 500, 1024] {
        let scene = graph
            .native_scene_at_size(Unit::Fahrenheit, width, 440, None, palette)
            .unwrap();
        assert!(scene.commands().len() <= 48);
        for (name, paint) in [
            ("Blue", P::Accent),
            ("Green", P::Success),
            ("White", P::Foreground),
            ("Yellow", P::Warning),
        ] {
            assert!(scene
                .commands()
                .iter()
                .any(|c| c.payload() == name && c.paint == paint));
            let lines: Vec<_> = scene
                .commands()
                .iter()
                .filter(|c| c.kind == K::Polyline && c.paint == paint)
                .collect();
            assert_eq!(lines.len(), 2); // both quantities, same clock axis
            assert_eq!(lines[0].polyline_geometry().unwrap().points()[0].x(), &72);
        }
        assert!(scene
            .commands()
            .iter()
            .any(|c| c.payload() == "Temperature (°F)"));
        assert!(scene
            .commands()
            .iter()
            .any(|c| c.payload().contains("Collector observed")));
    }
    assert_eq!(face, original);
}
#[test]
fn missing_sequences_never_connect_and_fragmentation_refuses_without_bound_inflation() {
    use conduit_face::{GraphicsCommandKind as K, GraphicsError};
    let values: Vec<_> = (1..=16).map(|i| value(i * 100, i)).collect();
    let records: Vec<_> = values
        .iter()
        .enumerate()
        .map(|(i, b)| SourceSeriesRecord {
            sequence: (i * 2 + 1) as u64,
            observation: b,
        })
        .collect();
    let streams: Vec<_> = [
        ("Blue", "series/blue", "equipment/blue"),
        ("Green", "series/green", "equipment/green"),
        ("White", "series/white", "equipment/white"),
        ("Yellow", "series/yellow", "equipment/yellow"),
    ]
    .into_iter()
    .map(|(name, subject, source)| SourceSeries {
        name,
        subject,
        source,
        ..series(&records)
    })
    .collect();
    let face = seal(collection(&streams).contribution(basis()).unwrap());
    let graph = SourceSeriesGraph::prepare(&face, "observations").unwrap();
    assert_eq!(
        graph.native_scene_at_size(Unit::Fahrenheit, 500, 440, None, palette),
        Err(GraphicsError::TooManyCommands)
    );
    let face = seal(
        collection(&[series(&records[..2])])
            .contribution(basis())
            .unwrap(),
    );
    let graph = SourceSeriesGraph::prepare(&face, "observations").unwrap();
    let scene = graph
        .native_scene_at_size(Unit::Fahrenheit, 500, 440, None, palette)
        .unwrap();
    assert!(scene.commands().iter().all(|c| c.kind != K::Polyline));
}

#[test]
fn optional_placement_moves_without_changing_source_session_or_observations() {
    let bytes = value(4500, 42);
    let records = [SourceSeriesRecord {
        sequence: 4,
        observation: &bytes,
    }];
    let streams = [series(&records)];
    let c = collection(&streams);
    let plain = seal(c.contribution(basis()).unwrap());
    assert_eq!(
        SourceSeriesGraph::prepare(&plain, "observations")
            .unwrap()
            .series()[0]
            .placement,
        None
    );
    let mut previous_geometry = None;
    for location in ["north room", "south room"] {
        let face = seal(
            c.contribution_with_placements(
                basis(),
                &[SourcePlacement {
                    source: "equipment/blue",
                    placement: location,
                }],
            )
            .unwrap(),
        );
        let graph = SourceSeriesGraph::prepare(&face, "observations").unwrap();
        let stream = &graph.series()[0];
        assert_eq!(stream.placement.as_deref(), Some(location));
        assert_eq!(stream.subject, "series/blue");
        assert_eq!(stream.source, "equipment/blue");
        assert_eq!(stream.points[0].sequence, 4);
        assert_eq!(stream.points[0].observation.observed_at_host_boot_ms, 4500);
        let scene = graph
            .native_scene_at_size(Unit::Fahrenheit, 500, 440, None, palette)
            .unwrap()
            .encode();
        if let Some(previous) = &previous_geometry {
            assert_eq!(&scene, previous);
        }
        previous_geometry = Some(scene);
        assert!(face
            .properties
            .iter()
            .any(|p| matches!(&p.value,V::TypedValue {bytes:actual,..} if actual==&bytes)));
    }
}
#[test]
fn placement_requires_exact_source_and_bounded_nonempty_utf8_text() {
    let bytes = value(4500, 42);
    let records = [SourceSeriesRecord {
        sequence: 4,
        observation: &bytes,
    }];
    let streams = [series(&records)];
    let c = collection(&streams);
    for text in [
        "".to_string(),
        " ".to_string(),
        "a\nb".to_string(),
        "é".repeat(49),
    ] {
        assert_eq!(
            c.contribution_with_placements(
                basis(),
                &[SourcePlacement {
                    source: "equipment/blue",
                    placement: &text
                }]
            ),
            Err(SourceSeriesRefusal::Placement)
        );
    }
    assert!(c
        .contribution_with_placements(
            basis(),
            &[SourcePlacement {
                source: "equipment/blue",
                placement: &"é".repeat(48)
            }]
        )
        .is_ok());
    assert_eq!(
        c.contribution_with_placements(
            basis(),
            &[SourcePlacement {
                source: "equipment/foreign",
                placement: "north room"
            }]
        ),
        Err(SourceSeriesRefusal::Identity)
    );
    let same = SourcePlacement {
        source: "equipment/blue",
        placement: "north room",
    };
    assert_eq!(
        c.contribution_with_placements(
            basis(),
            &[
                same,
                SourcePlacement {
                    source: "equipment/blue",
                    placement: "south room"
                }
            ]
        ),
        Err(SourceSeriesRefusal::Identity)
    );
    let mut fragment = c.contribution(basis()).unwrap();
    fragment.properties.push(conduit_face::FaceProperty {
        subject: "series/blue".into(),
        name: "placement".into(),
        value: V::Count(1),
    });
    assert_eq!(
        SourceSeriesGraph::prepare(&seal(fragment), "observations"),
        Err(SourceSeriesRefusal::Placement)
    );
}

#[test]
fn history_collection_is_primary_without_promoting_nested_provenance() {
    let bytes = value(4500, 42);
    let records = [SourceSeriesRecord { sequence: 4, observation: &bytes }];
    let rows = [series(&records)];
    let fragment = collection(&rows).contribution(basis()).unwrap();
    assert_eq!(fragment.disclosures, vec![conduit_face::FaceDisclosure {
        subject: "observations".into(), level: conduit_face::FaceDisclosureLevel::Primary,
    }]);
}
