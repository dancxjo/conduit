use conduit_audio::*;

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
fn signal(n: usize, period: f64, fundamental: f64, harmonic: f64) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let phase = std::f64::consts::TAU * i as f64 / period;
            (fundamental * phase.sin() + harmonic * (2.0 * phase).sin()) as f32
        })
        .collect()
}
#[test]
fn known_period_has_exact_rate_and_window_custody() {
    let samples = signal(1024, 40.0, 0.5, 0.0);
    let mut workspace = PcmPitchWorkspace::default();
    for rate in [8000, 16000, 48000, 192000] {
        let bytes = frame(&samples, rate, 1000);
        let result = workspace
            .observe(&bytes, 55, rate, 0, 1024, 8, 256)
            .unwrap();
        assert_eq!(result.interval.start_frame, 1000);
        assert_eq!(result.interval.frames, 1024);
        assert_eq!(result.interval.sample_rate_hz, rate);
        let PcmPitchEstimate::Periodic {
            lag_frames,
            correlation,
        } = result.estimate
        else {
            panic!("{:?}", result.estimate)
        };
        assert_eq!(lag_frames, 40);
        assert!(correlation > 0.99999);
    }
}
#[test]
fn silence_dc_noise_and_octave_competition_do_not_fabricate_definite_pitch() {
    let mut workspace = PcmPitchWorkspace::default();
    for value in [0.0, 0.75] {
        let bytes = frame(&vec![value; 1024], 16000, 0);
        assert_eq!(
            workspace
                .observe(&bytes, 55, 16000, 0, 1024, 8, 256)
                .unwrap()
                .estimate,
            PcmPitchEstimate::InsufficientEnergy
        );
    }
    let mut seed = 17u32;
    let noise: Vec<f32> = (0..1024)
        .map(|_| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 16) as f32 / 65536.0 - 0.5
        })
        .collect();
    let bytes = frame(&noise, 16000, 0);
    assert_eq!(
        workspace
            .observe(&bytes, 55, 16000, 0, 1024, 8, 256)
            .unwrap()
            .estimate,
        PcmPitchEstimate::Unknown
    );
    for fundamental in [0.05, 0.01] {
        let bytes = frame(&signal(1024, 80.0, fundamental, 0.5), 16000, 0);
        assert!(matches!(
            workspace
                .observe(&bytes, 55, 16000, 0, 1024, 8, 256)
                .unwrap()
                .estimate,
            PcmPitchEstimate::Uncertain { .. }
        ));
    }
}
#[test]
fn shorter_outside_profile_period_is_not_relabelled_as_subharmonic() {
    let bytes = frame(&signal(1024, 20.0, 0.5, 0.0), 16000, 0);
    assert_eq!(
        PcmPitchWorkspace::default()
            .observe(&bytes, 55, 16000, 0, 1024, 30, 256)
            .unwrap()
            .estimate,
        PcmPitchEstimate::Unknown
    );
}
#[test]
fn window_coordinates_are_independent_of_enclosing_block() {
    let samples = signal(1536, 40.0, 0.5, 0.0);
    let whole = frame(&samples, 16000, 100);
    let slice = frame(&samples[256..1280], 16000, 356);
    let mut workspace = PcmPitchWorkspace::default();
    let a = workspace
        .observe(&whole, 55, 16000, 256, 1024, 8, 256)
        .unwrap();
    let b = workspace
        .observe(&slice, 55, 16000, 0, 1024, 8, 256)
        .unwrap();
    assert_eq!(a.interval, b.interval);
    assert_eq!(a.estimate, b.estimate);
    // Different physical source blocks deliberately retain different custody.
    assert_ne!(a.source_digest, b.source_digest);
}
#[test]
fn malformed_geometry_timebase_and_nonfinite_input_refuse() {
    let mut workspace = PcmPitchWorkspace::default();
    let bytes = frame(&vec![0.0; 1024], 16000, 0);
    assert_eq!(
        workspace.observe(&bytes, 55, 16000, 0, 32, 8, 10),
        Err(PcmPitchRefusal::UnsupportedWindow)
    );
    assert_eq!(
        workspace.observe(&bytes, 55, 16000, 0, 1024, 8, 400),
        Err(PcmPitchRefusal::InvalidLagRange)
    );
    assert_eq!(
        workspace.observe(&bytes, 99, 16000, 0, 1024, 8, 256),
        Err(PcmPitchRefusal::Input(
            PcmTimeAnalysisRefusal::WrongTimebase
        ))
    );
    let bytes = frame(&vec![f32::NAN; 1024], 16000, 0);
    assert_eq!(
        workspace.observe(&bytes, 55, 16000, 0, 1024, 8, 256),
        Err(PcmPitchRefusal::Input(
            PcmTimeAnalysisRefusal::NonfiniteSample
        ))
    );
}

#[test]
fn repeated_observation_keeps_fixed_workspace_without_allocation() {
    let bytes = frame(&signal(1024, 40.0, 0.5, 0.0), 16000, 0);
    let mut workspace = PcmPitchWorkspace::default();
    ALLOCATIONS.with(|c| c.set((true, 0)));
    let result = workspace.observe(&bytes, 55, 16000, 0, 1024, 8, 256);
    let count = ALLOCATIONS.with(|c| {
        let (_, n) = c.get();
        c.set((false, n));
        n
    });
    assert!(result.is_ok());
    assert_eq!(count, 0);
}

#[test]
fn checked_native_observation_retains_custody_and_declared_confidence_projection() {
    let bytes = frame(&signal(1024, 40.0, 0.5, 0.0), 16000, 200);
    let observed = PcmPitchWorkspace::default()
        .observe(&bytes, 55, 16000, 0, 1024, 8, 256)
        .unwrap();
    let native = observed.prepare_native().unwrap();
    assert_eq!(*native.candidate_lag(), 40);
    assert_eq!(*native.window_start_frame(), 200);
    assert_eq!(*native.state(), AudioPitchState::Periodic);
    assert_eq!(native.source_digest().get(), &observed.source_digest);
    assert!(*native.correlation_ppm().get() >= 999999);
    let mut forged = observed;
    forged.estimate = PcmPitchEstimate::Periodic {
        lag_frames: 40,
        correlation: 0.5,
    };
    assert!(forged.prepare_native().is_err());
    forged.estimate = PcmPitchEstimate::Periodic {
        lag_frames: 4,
        correlation: 1.0,
    };
    assert!(forged.prepare_native().is_err());
}
