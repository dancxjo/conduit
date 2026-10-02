use conduit_core::{
    semantic_digest, BoundedResourceRef, KindId, Quantity, QuantityUnit, ResourceClassId,
    ResourceExtent, ResourceLifetime, ResourceSemanticIdentity, ResourceVersionIdentity,
};
use conduit_data::*;
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence, NativeRustBinding};

fn example_pages<const N: usize>(
    identities: [[u8; 32]; N],
) -> BoundedSequence<DatasetExamplePage, 128> {
    DatasetSplitMembership::pages(
        identities.map(|identity| DatasetExampleIdentity::new(identity).unwrap()),
    )
    .unwrap()
}

fn maximum_example_pages() -> BoundedSequence<DatasetExamplePage, 128> {
    DatasetSplitMembership::pages((0_u32..4096).map(|index| {
        let mut identity = [0_u8; 32];
        identity[..4].copy_from_slice(&(index + 1).to_le_bytes());
        DatasetExampleIdentity::new(identity).unwrap()
    }))
    .unwrap()
}

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

#[test]
fn scientific_provenance_round_trips_through_native_payloads() {
    let measured = ObservationProvenance::measured(
        "scientific/instrument-capture@1".into(),
        resource(1, "data/observation-block@1", 128),
    )
    .unwrap();
    let structured = measured.clone().into_structured().unwrap();
    assert_eq!(
        ObservationProvenance::from_structured(structured).unwrap(),
        measured
    );

    let derived = ObservationProvenance::derived(
        "scientific/windowed-linear-resample@1".into(),
        BoundedSequence::try_from_iter([
            ScientificObservationIdentity::new([2; 32]).unwrap(),
            ScientificObservationIdentity::new([3; 32]).unwrap(),
        ])
        .unwrap(),
        "calibration/head-correction-1".into(),
    )
    .unwrap();
    let structured = derived.clone().into_structured().unwrap();
    assert_eq!(
        ObservationProvenance::from_structured(structured).unwrap(),
        derived
    );
}

fn f32_tensor(values: &[f32], dimensions: Vec<u64>, roles: Vec<TensorAxisRole>) -> TensorValue {
    let bytes = values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>();
    TensorValue {
        element: TensorElement::F32,
        dimensions: BoundedSequence::try_from_iter(dimensions).unwrap(),
        axes: BoundedSequence::try_from_iter(roles.into_iter().map(|role| TensorAxis {
            role,
            identity: None,
            unit: Some(QuantityUnit::Millimeter),
        }))
        .unwrap(),
        content_digest: tensor_content_digest(&bytes),
        backing: TensorBacking::Inline(BoundedBytes::new(&bytes).unwrap()),
    }
}

fn signal(clock: &str, channels: u64, value: f32) -> SampledSignal {
    SampledSignal {
        clock_identity: clock.into(),
        start: SignalStart::at_sample(0),
        cadence: SignalCadence::regular(Quantity::new(1, QuantityUnit::Second), 100).unwrap(),
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
        value: ObservationValue::sampled_signal(signal(clock, channels, identity as f32)).unwrap(),
        provenance: ObservationProvenance::measured(
            "scientific/instrument-capture@1".into(),
            resource(identity, "data/observation-block@1", 128),
        )
        .unwrap(),
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
        observations: BoundedSequence::try_from_iter([audio, ema]).unwrap(),
        missing_data: BoundedSequence::try_from_iter([MissingDataMask {
            observation_identity: [2; 32],
            mask: TensorValue {
                element: TensorElement::U8,
                dimensions: BoundedSequence::try_from_iter([2, 2]).unwrap(),
                axes: BoundedSequence::try_from_iter([
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
                ])
                .unwrap(),
                content_digest: tensor_content_digest(&mask_bytes),
                backing: TensorBacking::Inline(BoundedBytes::new(&mask_bytes).unwrap()),
            },
        }])
        .unwrap(),
    }
}

fn frames() -> (CoordinateFrame, CoordinateFrame) {
    (
        CoordinateFrame {
            identity: "frame/ema-head".into(),
            axes: BoundedSequence::try_from_iter([
                CoordinateAxisName::new("anterior-posterior".into()).unwrap(),
                CoordinateAxisName::new("inferior-superior".into()).unwrap(),
            ])
            .unwrap(),
            unit: QuantityUnit::Millimeter,
        },
        CoordinateFrame {
            identity: "frame/head-normalized".into(),
            axes: BoundedSequence::try_from_iter([
                CoordinateAxisName::new("x".into()).unwrap(),
                CoordinateAxisName::new("y".into()).unwrap(),
            ])
            .unwrap(),
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
        calibration_sources: BoundedSequence::try_from_iter([ScientificObservationIdentity::new(
            [9; 32],
        )
        .unwrap()])
        .unwrap(),
        method_profile: "science/rigid-head-correction@1".into(),
    }
}

#[test]
fn paired_audio_and_ema_keep_source_clocks_then_derive_a_separate_aligned_view() {
    let set = observation_set();
    set.validate().unwrap();
    let structured = set.clone().into_structured().unwrap();
    assert_eq!(ObservationSet::from_structured(structured).unwrap(), set);
    assert_ne!(set.semantic_digest().unwrap(), [0; 32]);
    let relation = ClockRelation::new(
        "clock-relation/ema-to-audio@1".into(),
        ClockRelationQuality::estimated(Quantity::new(1, QuantityUnit::Millisecond)).unwrap(),
        0,
        "clock/ema".into(),
        1,
        0,
        "clock/audio".into(),
        480,
    )
    .unwrap();
    assert_eq!(
        relation.semantic_digest().unwrap(),
        [
            61, 45, 81, 23, 186, 250, 231, 2, 242, 73, 243, 53, 36, 236, 170, 64, 119, 214, 197,
            76, 189, 108, 35, 184, 84, 140, 153, 64, 156, 93, 182, 66,
        ]
    );
    let zero_error_relation = ClockRelation::new(
        relation.identity().clone(),
        ClockRelationQuality::estimated(Quantity::new(0, QuantityUnit::Millisecond)).unwrap(),
        *relation.source_anchor(),
        relation.source_clock().clone(),
        *relation.source_ticks(),
        *relation.target_anchor(),
        relation.target_clock().clone(),
        *relation.target_ticks(),
    )
    .unwrap();
    assert_eq!(
        zero_error_relation.validate(),
        Err(ScientificAlignmentRefusal::InvalidRelation)
    );
    let (source_frame, target_frame) = frames();
    let calibration = calibration();
    calibration.validate(&source_frame, &target_frame).unwrap();
    for frame in [source_frame.clone(), target_frame.clone()] {
        let structured = frame.clone().into_structured().unwrap();
        assert_eq!(CoordinateFrame::from_structured(structured).unwrap(), frame);
    }
    let structured = calibration.clone().into_structured().unwrap();
    assert_eq!(
        CalibrationTransform::from_structured(structured).unwrap(),
        calibration
    );
    let aligned = AlignedTrainingView::derive(AlignmentDerivation {
        set: &set,
        source_observation_identity: [2; 32],
        relation: &relation,
        calibration: Some((&calibration, &source_frame, &target_frame)),
        target_clock: "clock/audio",
        derived_identity: [7; 32],
        derived_value: ObservationValue::sampled_signal(signal("clock/audio", 2, 7.0)).unwrap(),
        resampling_profile: "science/windowed-linear-resample@1",
    })
    .unwrap();
    let structured = aligned.clone().into_structured().unwrap();
    assert_eq!(
        AlignedTrainingView::from_structured(structured).unwrap(),
        aligned
    );
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
        ObservationProvenance::Derived(_)
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
    set.missing_data[0].mask.dimensions = BoundedSequence::try_from_iter([4]).unwrap();
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
        shards: BoundedSequence::try_from_iter([resource(13, "data/corpus-shard@1", 4096)])
            .unwrap(),
        split_identities: BoundedSequence::try_from_iter(["train".into(), "test".into()]).unwrap(),
    };
    dataset.validate().unwrap();
    let structured = dataset.clone().into_structured().unwrap();
    assert_eq!(
        DatasetDescriptor::from_structured(structured).unwrap(),
        dataset
    );
    assert_ne!(dataset.semantic_digest().unwrap(), [0; 32]);
    assert_eq!(
        dataset.require_resources(&[[12; 32]]),
        Err(ScientificCorpusRefusal::MissingResource)
    );
    dataset.require_resources(&[[12; 32], [13; 32]]).unwrap();
    let train = DatasetSplitMembership {
        dataset_identity: dataset.identity,
        split_identity: "train".into(),
        examples: example_pages([[21; 32], [22; 32]]),
    };
    let mut test = DatasetSplitMembership {
        dataset_identity: dataset.identity,
        split_identity: "test".into(),
        examples: example_pages([[23; 32]]),
    };
    dataset.validate_membership(&train).unwrap();
    dataset.validate_membership(&test).unwrap();
    assert_ne!(
        train.semantic_digest().unwrap(),
        test.semantic_digest().unwrap()
    );
    prove_splits_disjoint(&train, &test).unwrap();
    test.examples = example_pages([[23; 32], [22; 32]]);
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

#[test]
fn dataset_split_membership_preserves_the_full_4096_example_bound() {
    let membership = DatasetSplitMembership {
        dataset_identity: [11; 32],
        split_identity: "maximum".into(),
        examples: maximum_example_pages(),
    };

    assert_eq!(membership.identities().count(), 4096);
    membership.validate().unwrap();

    let structured = membership.clone().into_structured().unwrap();
    assert_eq!(
        DatasetSplitMembership::from_structured(structured).unwrap(),
        membership
    );
}
