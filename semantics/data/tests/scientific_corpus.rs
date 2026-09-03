use conduit_core::{
    semantic_digest, BoundedResourceRef, KindId, Quantity, QuantityUnit, ResourceClassId,
    ResourceExtent, ResourceLifetime, ResourceSemanticIdentity, ResourceVersionIdentity,
};
use conduit_data::*;

fn resource(identity: u8, profile: &str, bytes: u64) -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([identity; 32]),
        content_profile: KindId::from(profile),
        access_class: ResourceClassId::from("scientific-corpus/read@1"),
        extent: ResourceExtent { bytes, items: None },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([identity + 32; 32]),
            expires_at: None,
        },
    }
}

fn f32_tensor(values: &[f32], dimensions: Vec<u64>, roles: Vec<TensorAxisRole>) -> TensorValue {
    let bytes = values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>();
    TensorValue {
        element: TensorElement::F32,
        dimensions,
        axes: roles
            .into_iter()
            .map(|role| TensorAxis {
                role,
                identity: None,
                unit: Some(QuantityUnit::Millimeter),
            })
            .collect(),
        content_digest: tensor_content_digest(&bytes),
        backing: TensorBacking::Inline(bytes),
    }
}

fn signal(clock: &str, channels: u64, value: f32) -> SampledSignal {
    SampledSignal {
        clock_identity: clock.into(),
        start: SignalStart::SampleIndex(0),
        cadence: SignalCadence::Regular {
            samples: 100,
            per: Quantity::new(1, QuantityUnit::Second),
        },
        sample_count: 2,
        continuity: SignalContinuity::Continuous,
        samples: f32_tensor(
            &vec![value; (2 * channels) as usize],
            vec![2, channels],
            vec![TensorAxisRole::Time, TensorAxisRole::Channel],
        ),
    }
}

fn measured(
    identity: u8,
    kind: &str,
    clock: &str,
    frame: Option<&str>,
    channels: u64,
) -> ScientificObservation {
    ScientificObservation {
        identity: [identity; 32],
        semantic_kind: kind.into(),
        clock_identity: Some(clock.into()),
        coordinate_frame: frame.map(Into::into),
        value: ObservationValue::SampledSignal(Box::new(signal(clock, channels, identity as f32))),
        provenance: ObservationProvenance::Measured {
            source: resource(identity, "data/observation-block@1", 128),
            measurement_profile: "scientific/instrument-capture@1".into(),
        },
    }
}

fn observation_set() -> ObservationSet {
    let audio = measured(1, "audio/pcm-signal@1", "clock/audio", None, 1);
    let ema = measured(
        2,
        "science/articulatory-trajectory@1",
        "clock/ema",
        Some("frame/ema-head"),
        2,
    );
    let mask_bytes = vec![0_u8, 1, 2, 3];
    ObservationSet {
        identity: semantic_digest("test/example@1", b"paired-example"),
        session_identity: "session/synthetic-1".into(),
        subject_identity: Some("subject/pseudonymous-1".into()),
        observations: vec![audio, ema],
        missing_data: vec![MissingDataMask {
            observation_identity: [2; 32],
            mask: TensorValue {
                element: TensorElement::U8,
                dimensions: vec![2, 2],
                axes: vec![
                    TensorAxis {
                        role: TensorAxisRole::Time,
                        identity: None,
                        unit: None,
                    },
                    TensorAxis {
                        role: TensorAxisRole::Channel,
                        identity: None,
                        unit: None,
                    },
                ],
                content_digest: tensor_content_digest(&mask_bytes),
                backing: TensorBacking::Inline(mask_bytes),
            },
        }],
    }
}

fn frames() -> (CoordinateFrame, CoordinateFrame) {
    (
        CoordinateFrame {
            identity: "frame/ema-head".into(),
            axes: vec!["anterior-posterior".into(), "inferior-superior".into()],
            unit: QuantityUnit::Millimeter,
        },
        CoordinateFrame {
            identity: "frame/head-normalized".into(),
            axes: vec!["x".into(), "y".into()],
            unit: QuantityUnit::Millimeter,
        },
    )
}

fn calibration() -> CalibrationTransform {
    CalibrationTransform {
        identity: "calibration/head-correction-1".into(),
        source_frame: "frame/ema-head".into(),
        target_frame: "frame/head-normalized".into(),
        linear: f32_tensor(
            &[1.0, 0.0, 0.0, 1.0],
            vec![2, 2],
            vec![
                TensorAxisRole::SpatialCoordinate,
                TensorAxisRole::SpatialCoordinate,
            ],
        ),
        translation: f32_tensor(
            &[0.1, -0.1],
            vec![2],
            vec![TensorAxisRole::SpatialCoordinate],
        ),
        calibration_sources: vec![[9; 32]],
        method_profile: "science/rigid-head-correction@1".into(),
    }
}

#[test]
fn paired_audio_and_ema_keep_source_clocks_then_derive_a_separate_aligned_view() {
    let set = observation_set();
    set.validate().unwrap();
    assert_ne!(set.semantic_digest().unwrap(), [0; 32]);
    let relation = ClockRelation {
        identity: "clock-relation/ema-to-audio@1".into(),
        source_clock: "clock/ema".into(),
        target_clock: "clock/audio".into(),
        source_anchor: 0,
        target_anchor: 0,
        source_ticks: 1,
        target_ticks: 480,
        quality: ClockRelationQuality::Estimated {
            maximum_error: Quantity::new(1, QuantityUnit::Millisecond),
        },
    };
    let (source_frame, target_frame) = frames();
    let calibration = calibration();
    calibration.validate(&source_frame, &target_frame).unwrap();
    let aligned = AlignedTrainingView::derive(AlignmentDerivation {
        set: &set,
        source_observation_identity: [2; 32],
        relation: &relation,
        calibration: Some((&calibration, &source_frame, &target_frame)),
        target_clock: "clock/audio",
        derived_identity: [7; 32],
        derived_value: ObservationValue::SampledSignal(Box::new(signal("clock/audio", 2, 7.0))),
        resampling_profile: "science/windowed-linear-resample@1",
    })
    .unwrap();
    assert_eq!(
        set.observations[0].clock_identity.as_deref(),
        Some("clock/audio")
    );
    assert_eq!(
        set.observations[1].clock_identity.as_deref(),
        Some("clock/ema")
    );
    assert_eq!(aligned.target_clock, "clock/audio");
    assert_eq!(
        aligned.derived_observation.coordinate_frame.as_deref(),
        Some("frame/head-normalized")
    );
    assert!(matches!(
        aligned.derived_observation.provenance,
        ObservationProvenance::Derived { .. }
    ));
    assert_ne!(aligned.semantic_digest().unwrap(), [0; 32]);
}

#[test]
fn clock_calibration_and_missingness_mismatches_refuse() {
    let mut set = observation_set();
    set.observations[1].clock_identity = Some("clock/wrong".into());
    assert_eq!(
        set.validate(),
        Err(ScientificObservationRefusal::ClockMismatch)
    );
    set = observation_set();
    set.missing_data[0].mask.dimensions = vec![4];
    set.missing_data[0].mask.axes.truncate(1);
    assert_eq!(
        set.validate(),
        Err(ScientificObservationRefusal::MaskShapeMismatch)
    );
    let (source_frame, target_frame) = frames();
    let mut transform = calibration();
    transform.source_frame = "frame/other".into();
    assert_eq!(
        transform.validate(&source_frame, &target_frame),
        Err(ScientificAlignmentRefusal::CalibrationFrameMismatch)
    );
    transform = calibration();
    transform.linear = f32_tensor(
        &[1.0, 0.0],
        vec![2],
        vec![TensorAxisRole::SpatialCoordinate],
    );
    assert_eq!(
        transform.validate(&source_frame, &target_frame),
        Err(ScientificAlignmentRefusal::CalibrationShapeMismatch)
    );
}

#[test]
fn corpus_resources_and_stable_splits_detect_missing_content_and_leakage() {
    let dataset = DatasetDescriptor {
        identity: [11; 32],
        schema_profile: "science/paired-audio-ema@1".into(),
        citation_identity: Some("doi/10.synthetic.conduit".into()),
        license_profile: Some("license/research-example@1".into()),
        example_count: 3,
        manifest: resource(12, CORPUS_MANIFEST_PROFILE, 512),
        shards: vec![resource(13, "data/corpus-shard@1", 4096)],
        split_identities: vec!["train".into(), "test".into()],
    };
    dataset.validate().unwrap();
    assert_ne!(dataset.semantic_digest().unwrap(), [0; 32]);
    assert_eq!(
        dataset.require_resources(&[[12; 32]]),
        Err(ScientificCorpusRefusal::MissingResource)
    );
    dataset.require_resources(&[[12; 32], [13; 32]]).unwrap();
    let train = DatasetSplitMembership {
        dataset_identity: dataset.identity,
        split_identity: "train".into(),
        examples: vec![[21; 32], [22; 32]],
    };
    let mut test = DatasetSplitMembership {
        dataset_identity: dataset.identity,
        split_identity: "test".into(),
        examples: vec![[23; 32]],
    };
    dataset.validate_membership(&train).unwrap();
    dataset.validate_membership(&test).unwrap();
    assert_ne!(
        train.semantic_digest().unwrap(),
        test.semantic_digest().unwrap()
    );
    prove_splits_disjoint(&train, &test).unwrap();
    test.examples.push([22; 32]);
    assert_eq!(
        prove_splits_disjoint(&train, &test),
        Err(ScientificCorpusRefusal::SplitLeakage)
    );

    let mut malformed = dataset;
    malformed.manifest.extent.bytes = 0;
    assert_eq!(
        malformed.validate(),
        Err(ScientificCorpusRefusal::InvalidManifest)
    );
}
