use conduit_audio::*;
use conduit_core::Quantity;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
thread_local! { static ALLOCATIONS: Cell<(bool,usize)> = const { Cell::new((false,0)) }; }
struct Counted;
fn count() {
    let _ = ALLOCATIONS.try_with(|c| {
        let (on, n) = c.get();
        if on {
            c.set((true, n + 1));
        }
    });
}
unsafe impl GlobalAlloc for Counted {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc(l) }
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc_zeroed(l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        count();
        unsafe { System.realloc(p, l, n) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}
#[global_allocator]
static ALLOCATOR: Counted = Counted;
fn frame(samples: &[f32], rate: u32, start: u64) -> Vec<u8> {
    PcmFrameHeader::new(
        PcmSampleRepresentation::Float32LittleEndian,
        rate,
        PcmChannelLayout::Mono,
        samples.len() as u16,
        55,
        start,
        false,
    )
    .unwrap()
    .encode_frame(
        &samples
            .iter()
            .flat_map(|x| x.to_le_bytes())
            .collect::<Vec<_>>(),
    )
    .unwrap()
}
fn direct(samples: &[f32], window: PcmSpectralWindow, k: usize) -> (f64, f64) {
    let n = samples.len();
    let mut re = 0.0;
    let mut im = 0.0;
    let mut sum = 0.0;
    let mut squares = 0.0;
    for (j, x) in samples.iter().enumerate() {
        let w = match window {
            PcmSpectralWindow::Rectangular => 1.0,
            PcmSpectralWindow::PeriodicHann => {
                0.5 * (1.0 - (std::f64::consts::TAU * j as f64 / n as f64).cos())
            }
        };
        let angle = std::f64::consts::TAU * (j * k) as f64 / n as f64;
        re += f64::from(*x) * w * angle.cos();
        im -= f64::from(*x) * w * angle.sin();
        sum += w;
        squares += w * w;
    }
    let sided = if k == 0 || k == n / 2 { 1.0 } else { 2.0 };
    let power = re * re + im * im;
    (
        sided * power.sqrt() / sum,
        sided * power / (n as f64 * squares),
    )
}
#[test]
fn sine_noise_and_impulse_match_independent_direct_dft_for_both_windows() {
    let mut seed = 17u32;
    let noise: Vec<f32> = (0..64)
        .map(|_| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            ((seed >> 16) as f32 / 65536.0) - 0.5
        })
        .collect();
    let sine: Vec<f32> = (0..64)
        .map(|i| (0.5 * (std::f64::consts::TAU * 5.0 * i as f64 / 64.0).sin()) as f32)
        .collect();
    let mut impulse = vec![0.0; 64];
    impulse[9] = 0.25;
    let mut workspace = PcmSpectralWorkspace::default();
    for samples in [sine, noise, impulse] {
        for window in [
            PcmSpectralWindow::Rectangular,
            PcmSpectralWindow::PeriodicHann,
        ] {
            let encoded = frame(&samples, 48000, 123);
            let output = workspace
                .analyze(&encoded, 55, 48000, 0, 64, window)
                .unwrap();
            assert_eq!(output.bins().len(), 33);
            assert_eq!(output.interval().start_frame, 123);
            assert_eq!(
                output.source_digest(),
                PreparedPcmTimeAnalysis::prepare(&encoded, 55, 48000, 0, 64)
                    .unwrap()
                    .source_digest()
            );
            for bin in output.bins() {
                let (amplitude, power) = direct(&samples, window, usize::from(bin.index));
                assert!((bin.peak_amplitude - amplitude).abs() < 2e-13);
                assert!((bin.mean_square_contribution - power).abs() < 2e-14);
            }
        }
    }
}
#[test]
fn frequency_coordinates_are_exact_quantities_including_fractional_hertz() {
    let bytes = frame(&[0.0; 1024], 44100, 900);
    let mut workspace = PcmSpectralWorkspace::default();
    let out = workspace
        .analyze(&bytes, 55, 44100, 0, 1024, PcmSpectralWindow::Rectangular)
        .unwrap();
    assert_eq!(
        out.bins().nth(1).unwrap().frequency,
        Quantity::parse_plot_literal("43.06640625Hz").unwrap()
    );
    assert_eq!(
        out.bins().last().unwrap().frequency,
        Quantity::parse_plot_literal("22050Hz").unwrap()
    );
    assert_eq!(
        out.bins().next().unwrap().frequency,
        Quantity::parse_plot_literal("0Hz").unwrap()
    );
}
#[test]
fn known_dc_nyquist_impulse_and_sine_have_truthful_one_sided_scaling() {
    let mut workspace = PcmSpectralWorkspace::default();
    for (samples, peak_bin, expected_amplitude, expected_power) in [
        (vec![0.25; 64], 0, 0.25, 0.0625),
        (
            (0..64)
                .map(|i| if i % 2 == 0 { 0.25 } else { -0.25 })
                .collect(),
            32,
            0.25,
            0.0625,
        ),
        (
            (0..64)
                .map(|i| (0.5 * (std::f64::consts::TAU * 4.0 * i as f64 / 64.0).sin()) as f32)
                .collect(),
            4,
            0.5,
            0.125,
        ),
    ] {
        let bytes = frame(&samples, 48000, 0);
        let out = workspace
            .analyze(&bytes, 55, 48000, 0, 64, PcmSpectralWindow::Rectangular)
            .unwrap();
        let peak = out
            .bins()
            .max_by(|a, b| a.peak_amplitude.total_cmp(&b.peak_amplitude))
            .unwrap();
        assert_eq!(peak.index, peak_bin);
        assert!((peak.peak_amplitude - expected_amplitude).abs() < 2e-8);
        assert!(
            (out.bins().map(|b| b.mean_square_contribution).sum::<f64>() - expected_power).abs()
                < 2e-8
        );
    }
    let mut impulse = vec![0.0; 64];
    impulse[0] = 1.0;
    let bytes = frame(&impulse, 48000, 0);
    let out = workspace
        .analyze(&bytes, 55, 48000, 0, 64, PcmSpectralWindow::Rectangular)
        .unwrap();
    for bin in out.bins() {
        let sided = if bin.index == 0 || bin.index == 32 {
            1.0
        } else {
            2.0
        };
        assert_eq!(bin.peak_amplitude, sided / 64.0);
        assert_eq!(bin.mean_square_contribution, sided / (64.0 * 64.0));
    }
}
#[test]
fn window_timebase_nonfinite_and_interval_refusals_are_distinct() {
    let bytes = frame(&[0.0; 64], 48000, 0);
    let mut workspace = PcmSpectralWorkspace::default();
    for n in [0, 8, 17, 2048] {
        assert!(matches!(
            workspace.analyze(&bytes, 55, 48000, 0, n, PcmSpectralWindow::Rectangular),
            Err(PcmSpectralRefusal::UnsupportedWindow)
        ));
    }
    assert!(matches!(
        workspace.analyze(&bytes, 56, 48000, 0, 64, PcmSpectralWindow::Rectangular),
        Err(PcmSpectralRefusal::Input(
            PcmTimeAnalysisRefusal::WrongTimebase
        ))
    ));
    assert!(matches!(
        workspace.analyze(&bytes, 55, 44100, 0, 64, PcmSpectralWindow::Rectangular),
        Err(PcmSpectralRefusal::Input(
            PcmTimeAnalysisRefusal::WrongTimebase
        ))
    ));
    assert!(matches!(
        workspace.analyze(&bytes, 55, 48000, 1, 64, PcmSpectralWindow::Rectangular),
        Err(PcmSpectralRefusal::Input(
            PcmTimeAnalysisRefusal::WindowOutsideFrame
        ))
    ));
    let mut samples = [0.0; 64];
    samples[63] = f32::INFINITY;
    let bytes = frame(&samples, 48000, 0);
    assert!(matches!(
        workspace.analyze(&bytes, 55, 48000, 0, 16, PcmSpectralWindow::Rectangular),
        Err(PcmSpectralRefusal::Input(
            PcmTimeAnalysisRefusal::NonfiniteSample
        ))
    ));
    let bytes = frame(&[0.0; 64], 48000, u64::MAX - 32);
    assert!(matches!(
        workspace.analyze(&bytes, 55, 48000, 0, 64, PcmSpectralWindow::Rectangular),
        Err(PcmSpectralRefusal::Input(
            PcmTimeAnalysisRefusal::FrameIndexOverflow
        ))
    ));
}
#[test]
fn scratch_reuse_is_reset_equivalent_and_no_analysis_or_bin_iteration_allocates() {
    let samples: Vec<f32> = (0..1024)
        .map(|i| if i % 3 == 0 { f32::MAX } else { -f32::MAX })
        .collect();
    let bytes = frame(&samples, 192000, 0);
    let mut workspace = PcmSpectralWorkspace::default();
    for n in [1024, 16, 64, 256, 1024] {
        ALLOCATIONS.with(|c| c.set((true, 0)));
        let output = workspace
            .analyze(&bytes, 55, 192000, 0, n, PcmSpectralWindow::PeriodicHann)
            .unwrap();
        let total = output
            .bins()
            .map(|b| {
                assert!(b.peak_amplitude.is_finite() && b.mean_square_contribution.is_finite());
                b.mean_square_contribution
            })
            .sum::<f64>();
        let allocations = ALLOCATIONS.with(|c| {
            let (_, n) = c.get();
            c.set((false, 0));
            n
        });
        assert_eq!(allocations, 0);
        assert!(total.is_finite());
        let mut expected = PcmSpectralWorkspace::default();
        let fresh = expected
            .analyze(&bytes, 55, 192000, 0, n, PcmSpectralWindow::PeriodicHann)
            .unwrap();
        let actual = workspace
            .analyze(&bytes, 55, 192000, 0, n, PcmSpectralWindow::PeriodicHann)
            .unwrap();
        assert_eq!(
            fresh.bins().collect::<Vec<_>>(),
            actual.bins().collect::<Vec<_>>()
        );
    }
}

#[test]
fn checked_source_axis_projects_exact_frequency_in_reference_and_prepared_evaluators() {
    use conduit_core::{ConfigurationValue, Scalar};
    use conduit_plot::rust_binding::NativeRustBinding;
    use conduit_plot::{
        check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
        PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog,
        StartupCatalog,
    };
    let source = include_str!("../spectral_analysis_types.conduit");
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(
        &checked,
        "audio/spectrum-bin-frequency",
        &ProfileCatalog::new(),
    )
    .unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("pure checked bin-frequency program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let profile = PcmSpectralProfile::new(
        PcmSpectralGeometry::new(64, 64, 64).unwrap(),
        55,
        48000,
        PcmSpectralWindow::Rectangular,
    )
    .unwrap();
    let observation = profile
        .prepare_observation(
            &mut PcmSpectralWorkspace::default(),
            &frame(&[0.0; 64], 48000, 0),
            0,
        )
        .unwrap();
    let request = AudioSpectrumBinRequest::new(Scalar::from_raw_microunits(2_000_000), observation)
        .unwrap()
        .encode()
        .unwrap();
    let expected = Quantity::parse_plot_literal("1500Hz").unwrap().encode();
    assert_eq!(program.evaluate(&request).unwrap(), expected);
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    assert_eq!(prepared.evaluate(&request).unwrap(), expected);
    ALLOCATIONS.with(|c| c.set((true, 0)));
    for _ in 0..16 {
        assert_eq!(prepared.evaluate(&request).unwrap(), expected);
    }
    let allocations = ALLOCATIONS.with(|c| {
        let (_, n) = c.get();
        c.set((false, 0));
        n
    });
    assert_eq!(allocations, 0);
}

#[test]
fn prepared_spectrum_slots_match_native_binding_without_play_allocation() {
    use conduit_plot::rust_binding::NativeRustBinding;
    for n in [16, 32, 64, 128, 256, 512, 1024] {
        let samples: Vec<f32> = (0..2 * n)
            .map(|i| if i % 3 == 0 { 0.25 } else { -0.5 })
            .collect();
        let input = frame(&samples, 48000, 9000);
        for window in [
            PcmSpectralWindow::Rectangular,
            PcmSpectralWindow::PeriodicHann,
        ] {
            let profile = PcmSpectralProfile::new(
                PcmSpectralGeometry::new(2 * n, n, n / 2).unwrap(),
                55,
                48000,
                window,
            )
            .unwrap();
            let mut encoder = PreparedPcmSpectrumObservationEncoder::prepare(profile).unwrap();
            assert!(encoder.encoded_len() <= conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES);
            for index in 0..profile.geometry().windows() {
                let expected = profile
                    .prepare_observation(&mut PcmSpectralWorkspace::default(), &input, index)
                    .unwrap()
                    .encode()
                    .unwrap();
                ALLOCATIONS.with(|c| c.set((true, 0)));
                let actual = encoder.encode_window(&input, index).unwrap();
                assert_eq!(actual, expected);
                let allocations = ALLOCATIONS.with(|c| {
                    let (_, n) = c.get();
                    c.set((false, 0));
                    n
                });
                assert_eq!(allocations, 0);
                assert!(AudioSpectrumObservation::decode(&expected).is_ok());
            }
            ALLOCATIONS.with(|c| c.set((true, 0)));
            assert!(matches!(
                encoder.encode_window(&input, 3),
                Err(PcmSpectralEncodingRefusal::Analysis(
                    PcmSpectralRefusal::WindowIndexOutsideGeometry
                ))
            ));
            let allocations = ALLOCATIONS.with(|c| {
                let (_, n) = c.get();
                c.set((false, 0));
                n
            });
            assert_eq!(allocations, 0);
        }
    }
}
