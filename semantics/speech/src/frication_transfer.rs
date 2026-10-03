//! Independent path and spectral laws for the compact frication profile.
use crate::{generated::*, prosody_parity::context, Renderer};

#[test]
fn diffuse_frication_is_direct_noise_and_keeps_its_source_class() {
    for phone in [EnglishPhone::f, EnglishPhone::th] {
        let target = speech_voice_target(phone).unwrap();
        assert_eq!((target.gain1, target.gain2, target.gain3), (0, 0, 0));
        assert!(target.bypass_gain > 0);
        assert_eq!((target.voiced, target.frication, target.closure), (0, 1, 0));
        for phase in [0, 1, 20, 30, 50, 66] {
            for noise in [0, 1, 4095, 65535] {
                let mut input =
                    speech_pitch(context(EnglishStress::unspecified, target, 128)).unwrap();
                input.state.phase = phase;
                input.state.noise = noise;
                input.state.voicing = i32::MIN;
                let excited = speech_frame_excite(input).unwrap();
                let actual = speech_frame(input).unwrap();
                assert_eq!(
                    actual.sample,
                    excited.upper_excitation * target.bypass_gain / 256 * 16
                );
                assert_eq!(actual.state.voicing, 0);
            }
        }
    }
    for phone in [EnglishPhone::v, EnglishPhone::dh] {
        let target = speech_voice_target(phone).unwrap();
        assert!(target.gain1 > 0 && target.bypass_gain > 0);
        assert_eq!((target.gain2, target.gain3), (0, 0));
        assert_eq!((target.voiced, target.frication, target.closure), (1, 1, 0));
    }
    for (phone, _) in PHONES {
        let target = speech_voice_target(*phone).unwrap();
        if !matches!(
            phone,
            EnglishPhone::f | EnglishPhone::v | EnglishPhone::th | EnglishPhone::dh
        ) {
            assert_eq!(target.bypass_gain, 0);
        }
    }
}

#[test]
fn unused_bypass_does_not_refuse_but_selected_overflow_does() {
    let target = speech_voice_target(EnglishPhone::eh).unwrap();
    let input = speech_pitch(context(EnglishStress::unspecified, target, 128)).unwrap();
    let mut working = speech_frame_excite(input).unwrap();
    working.upper_excitation = i32::MAX;
    assert_eq!(speech_frame_result(working).unwrap().sample, 0);
    working.input.target = speech_voice_target(EnglishPhone::f).unwrap();
    assert_eq!(speech_frame_result(working), None);
}

fn isolated(phoneme: EnglishPhoneme) -> std::vec::Vec<i16> {
    let events = [VoiceEvent::segment(RealizationInput {
        phoneme,
        stress: EnglishStress::primary,
        position: EnglishPosition::medial,
    })];
    let mut renderer = Renderer::prepare(&events).unwrap();
    let mut samples = std::vec::Vec::new();
    while !renderer.is_complete() {
        let mut block = [0; 128];
        let count = renderer.render(&mut block).unwrap();
        samples.extend_from_slice(&block[..count]);
    }
    samples
}

fn spectrum(samples: &[i16]) -> (f64, f64) {
    // One interior Hann window; the DFT is test-only and independent of DSP.
    let window: std::vec::Vec<_> = samples[64..576]
        .iter()
        .enumerate()
        .map(|(n, value)| {
            f64::from(*value) * (0.5 - 0.5 * (2.0 * core::f64::consts::PI * n as f64 / 511.0).cos())
        })
        .collect();
    let mut total = 0.0;
    let mut low = 0.0;
    let mut moment = 0.0;
    for bin in 0..=256 {
        let (mut real, mut imaginary) = (0.0, 0.0);
        for (n, value) in window.iter().enumerate() {
            let phase = 2.0 * core::f64::consts::PI * (bin * n) as f64 / 512.0;
            real += value * phase.cos();
            imaginary -= value * phase.sin();
        }
        let power = real * real + imaginary * imaginary;
        let frequency = bin as f64 * 8000.0 / 512.0;
        total += power;
        moment += frequency * power;
        if frequency < 1500.0 {
            low += power;
        }
    }
    assert!(total > 0.0);
    (low / total, moment / total)
}

#[test]
fn sibilants_emphasize_upper_spectrum_and_retain_distinct_place_cues() {
    let s = isolated(EnglishPhoneme::s);
    let sh = isolated(EnglishPhoneme::sh);
    assert_eq!(s.len(), 640);
    assert_eq!(sh.len(), 640);
    let (s_low, s_centroid) = spectrum(&s);
    let (sh_low, sh_centroid) = spectrum(&sh);
    assert!(s_low < 0.2 && sh_low < 0.2);
    assert!(s_centroid > sh_centroid + 500.0);
    assert!(s
        .iter()
        .chain(sh.iter())
        .all(|value| i32::from(*value).abs() < 32767));
}
