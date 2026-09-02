use conduit_core::{Quantity, QuantityUnit};
use conduit_data::*;

fn signal(clock: &str, start: u64, count: u64, channels: u64) -> SampledSignal {
    let payload = vec![0_u8; usize::try_from(count * channels * 4).unwrap()];
    SampledSignal {
        clock_identity: clock.into(),
        start: SignalStart::SampleIndex(start),
        cadence: SignalCadence::Regular {
            samples: 100,
            per: Quantity::new(1, QuantityUnit::Second),
        },
        sample_count: count,
        continuity: SignalContinuity::Continuous,
        samples: TensorValue {
            element: TensorElement::F32,
            dimensions: vec![count, channels],
            axes: vec![
                TensorAxis {
                    role: TensorAxisRole::Time,
                    identity: Some("observation".into()),
                    unit: None,
                },
                TensorAxis {
                    role: TensorAxisRole::Feature,
                    identity: Some("channel".into()),
                    unit: Some(QuantityUnit::One),
                },
            ],
            content_digest: tensor_content_digest(&payload),
            backing: TensorBacking::Inline(payload),
        },
    }
}

#[test]
fn independently_clocked_audio_f0_and_articulation_do_not_invent_segments() {
    let audio = signal("clock/audio", 0, 8, 1);
    let f0 = signal("clock/f0", 0, 8, 1);
    let articulation = signal("clock/ema", 0, 8, 2);
    for value in [&audio, &f0, &articulation] {
        value.validate().unwrap();
        assert!(!format!("{:?}", value.summary().unwrap()).contains("phone"));
    }
    assert_ne!(audio.semantic_digest(), f0.semantic_digest());
    assert_eq!(articulation.samples.dimensions, [8, 2]);
}

#[test]
fn windows_preserve_source_clock_and_identity() {
    let value = signal("clock/ema", 10, 8, 2);
    let window = value.window(3, 4).unwrap();
    assert_eq!(window.start, SignalStart::SampleIndex(13));
    assert_eq!(window.source_signal, value.semantic_digest().unwrap());
    assert_eq!(
        value.window(7, 2),
        Err(SampledSignalRefusal::WindowOutOfBounds)
    );
}

#[test]
fn concatenation_requires_exact_contiguity_and_compatible_descriptors() {
    let first = signal("clock/ema", 0, 4, 2);
    let second = signal("clock/ema", 4, 4, 2);
    let joined = concatenate(&[first.clone(), second.clone()]).unwrap();
    assert_eq!(joined.sample_count, 8);
    assert_eq!(joined.sample_shape, [2]);
    assert_eq!(joined.source_parts.len(), 2);
    let mut gap = second.clone();
    gap.start = SignalStart::SampleIndex(5);
    assert_eq!(
        concatenate(&[first.clone(), gap]),
        Err(SampledSignalRefusal::NoncontiguousSignals)
    );
    let mut discontinuous = second;
    discontinuous.continuity = SignalContinuity::Discontinuous {
        gap_identity: "capture-gap".into(),
    };
    assert_eq!(
        concatenate(&[first, discontinuous]),
        Err(SampledSignalRefusal::IncompatibleSignals)
    );
}

#[test]
fn malformed_clock_cadence_count_and_shape_refuse() {
    let mut value = signal("clock/ema", 0, 4, 2);
    value.clock_identity.clear();
    assert_eq!(value.validate(), Err(SampledSignalRefusal::InvalidClock));
    value = signal("clock/ema", 0, 4, 2);
    value.sample_count = 0;
    assert_eq!(value.validate(), Err(SampledSignalRefusal::EmptySignal));
    value = signal("clock/ema", 0, 4, 2);
    value.samples.dimensions[0] = 3;
    assert_eq!(value.validate(), Err(SampledSignalRefusal::TensorInvalid));
    value = signal("clock/ema", 0, 4, 2);
    value.samples.axes[0].role = TensorAxisRole::Channel;
    assert_eq!(
        value.validate(),
        Err(SampledSignalRefusal::MissingSampleAxis)
    );
}
