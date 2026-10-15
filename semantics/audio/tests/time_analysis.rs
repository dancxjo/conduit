use conduit_audio::*;
fn frame(values: &[f32], clock: u64, start: u64) -> Vec<u8> {
    let payload: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
    PcmFrameHeader::new(
        PcmSampleRepresentation::Float32LittleEndian,
        8000,
        PcmChannelLayout::Mono,
        values.len() as u16,
        clock,
        start,
        false,
    )
    .unwrap()
    .encode_frame(&payload)
    .unwrap()
}
#[test]
fn silence_impulse_and_quarter_cycle_sine_have_independent_known_energy() {
    for (values, squares, peak, rms) in [
        ([0., 0., 0., 0.], 0., 0., 0.),
        ([1., 0., 0., 0.], 1., 1., 0.5),
        ([0., 1., 0., -1.], 2., 1., core::f64::consts::FRAC_1_SQRT_2),
    ] {
        let encoded = frame(&values, 7, 123);
        let view = PreparedPcmTimeAnalysis::prepare(&encoded, 7, 8000, 0, 4).unwrap();
        assert_eq!(view.waveform().collect::<Vec<_>>(), values.map(f64::from));
        assert_eq!(
            view.interval(),
            PcmAnalysisInterval {
                clock_id: 7,
                start_frame: 123,
                frames: 4,
                sample_rate_hz: 8000
            }
        );
        assert_eq!(view.statistics().square_sum, squares);
        assert_eq!(view.statistics().mean_square, squares / 4.);
        assert_eq!(view.statistics().absolute_peak, peak);
        assert!((view.statistics().rms - rms).abs() < 1e-15);
        let (header, payload) = PcmFrameHeader::decode_frame(&encoded).unwrap();
        assert_eq!(
            view.source_digest(),
            header.semantic_digest(payload).unwrap()
        );
    }
}
#[test]
fn window_retains_original_clock_and_full_source_identity() {
    let encoded = frame(&[0., 1., -1., 0.], 9, 100);
    let selected = PreparedPcmTimeAnalysis::prepare(&encoded, 9, 8000, 1, 2).unwrap();
    assert_eq!(selected.interval().start_frame, 101);
    assert_eq!(selected.waveform().collect::<Vec<_>>(), [1., -1.]);
    assert_eq!(selected.statistics().square_sum, 2.);
    assert_eq!(selected.statistics().samples_at_or_above_full_scale, 2);
    assert_eq!(
        selected.source_digest(),
        PreparedPcmTimeAnalysis::prepare(&encoded, 9, 8000, 0, 4)
            .unwrap()
            .source_digest()
    );
}
#[test]
fn wrong_timebase_nonfinite_short_and_overflow_refuse() {
    use PcmTimeAnalysisRefusal as R;
    let good = frame(&[0., 1., 0., -1.], 7, 100);
    for (clock, rate, offset, count, reason) in [
        (8, 8000, 0, 4, R::WrongTimebase),
        (7, 16000, 0, 4, R::WrongTimebase),
        (7, 8000, 0, 1, R::InsufficientWindow),
        (7, 8000, 2, 4, R::WindowOutsideFrame),
    ] {
        assert_eq!(
            PreparedPcmTimeAnalysis::prepare(&good, clock, rate, offset, count).err(),
            Some(reason)
        );
    }
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        // Outside selected window still refuses malformed input evidence.
        let encoded = frame(&[0., 0., bad], 7, 0);
        assert_eq!(
            PreparedPcmTimeAnalysis::prepare(&encoded, 7, 8000, 0, 2).err(),
            Some(R::NonfiniteSample)
        );
    }
    let overflow = frame(&[0., 0.], 7, u64::MAX);
    assert_eq!(
        PreparedPcmTimeAnalysis::prepare(&overflow, 7, 8000, 0, 2).err(),
        Some(R::FrameIndexOverflow)
    );
}

#[test]
fn integer_profiles_have_exact_full_scale_normalization() {
    for (representation, payload, expected) in [
        (
            PcmSampleRepresentation::Signed16LittleEndian,
            [-32768_i16, 16384]
                .into_iter()
                .flat_map(i16::to_le_bytes)
                .collect::<Vec<_>>(),
            [-1., 0.5],
        ),
        (
            PcmSampleRepresentation::Signed24LittleEndian,
            vec![0, 0, 128, 0, 0, 64],
            [-1., 0.5],
        ),
    ] {
        let encoded = PcmFrameHeader::new(
            representation,
            48000,
            PcmChannelLayout::Mono,
            2,
            1,
            0,
            false,
        )
        .unwrap()
        .encode_frame(&payload)
        .unwrap();
        let observation = PreparedPcmTimeAnalysis::prepare(&encoded, 1, 48000, 0, 2).unwrap();
        assert_eq!(observation.waveform().collect::<Vec<_>>(), expected);
        assert_eq!(observation.statistics().square_sum, 1.25);
        assert_eq!(observation.statistics().mean_square, 0.625);
    }
}

#[test]
fn malformed_discontinuous_and_stereo_frames_are_distinct_refusals() {
    use PcmTimeAnalysisRefusal as R;
    assert!(matches!(
        PreparedPcmTimeAnalysis::prepare(&[], 1, 8000, 0, 2).err(),
        Some(R::Frame(_))
    ));
    for (layout, discontinuity, reason) in [
        (
            PcmChannelLayout::StereoLeftRight,
            false,
            R::UnsupportedLayout,
        ),
        (PcmChannelLayout::Mono, true, R::Discontinuous),
    ] {
        let header = PcmFrameHeader::new(
            PcmSampleRepresentation::Signed16LittleEndian,
            8000,
            layout,
            2,
            1,
            0,
            discontinuity,
        )
        .unwrap();
        let encoded = header
            .encode_frame(&vec![0; header.payload_bytes() as usize])
            .unwrap();
        assert_eq!(
            PreparedPcmTimeAnalysis::prepare(&encoded, 1, 8000, 0, 2).err(),
            Some(reason)
        );
    }
}

#[test]
fn maximum_window_and_maximum_finite_float_stay_bounded() {
    let encoded = frame(&[f32::MAX; 2048], 1, 0);
    let observation = PreparedPcmTimeAnalysis::prepare(&encoded, 1, 8000, 0, 2048).unwrap();
    assert_eq!(observation.waveform().len(), 2048);
    assert!(observation.statistics().square_sum.is_finite());
    assert!(observation.statistics().rms.is_finite());
    assert_eq!(
        observation.statistics().samples_at_or_above_full_scale,
        2048
    );
}

#[test]
fn checked_native_observation_retains_all_pages_and_exact_custody() {
    use conduit_plot::rust_binding::NativeRustBinding;
    for count in [2, 1024, 1025, 2048] {
        let values: Vec<_> = (0..count)
            .map(|i| if i % 2 == 0 { 0.5 } else { -0.5 })
            .collect();
        let encoded = frame(&values, 42, 900);
        let analysis =
            PreparedPcmTimeAnalysis::prepare(&encoded, 42, 8000, 0, count as u16).unwrap();
        let observation = analysis.native_observation().unwrap();
        assert_eq!(*observation.frames(), count as u16);
        assert_eq!(*observation.clock_id(), 42);
        assert_eq!(*observation.start_frame(), 900);
        assert_eq!(observation.mean_square().value(), 0.25);
        assert_eq!(observation.rms().value(), 0.5);
        assert_eq!(observation.source_digest().get(), &analysis.source_digest());
        let waveform: Vec<_> = observation
            .waveform()
            .get()
            .as_slice()
            .iter()
            .flat_map(|page| page.get().as_slice().iter().map(|v| v.value()))
            .collect();
        assert_eq!(
            waveform,
            values.into_iter().map(f64::from).collect::<Vec<_>>()
        );
        let bytes = observation.clone().encode().unwrap();
        assert!(bytes.len() <= conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES);
        assert_eq!(
            AudioTimeAnalysisObservation::decode(&bytes).unwrap(),
            observation
        );
    }
}

#[test]
fn checked_native_statistics_refuse_nonfinite_negative_and_wrong_window_count() {
    use conduit_core::IeeeF64;
    let encoded = frame(&[0., 1.], 1, 0);
    let observed = PreparedPcmTimeAnalysis::prepare(&encoded, 1, 8000, 0, 2)
        .unwrap()
        .native_observation()
        .unwrap();
    for (bad, frames) in [(-1.0, 2), (f64::NAN, 2), (f64::INFINITY, 2), (0.5, 3)] {
        assert!(AudioTimeAnalysisObservation::new(
            *observed.absolute_peak(),
            *observed.clock_id(),
            frames,
            IeeeF64::from_value(bad),
            *observed.reference(),
            *observed.rms(),
            *observed.sample_rate_hz(),
            *observed.samples_at_or_above_full_scale(),
            observed.source_digest().clone(),
            *observed.square_sum(),
            *observed.start_frame(),
            observed.waveform().clone(),
            *observed.window(),
        )
        .is_err());
    }
}
