//! Independent source-role laws for the continuous voiced-fricative profile.
use crate::{generated::*, prosody_parity::context};

const PAIRS: [(EnglishPhone, EnglishPhone); 4] = [
    (EnglishPhone::f, EnglishPhone::v),
    (EnglishPhone::th, EnglishPhone::dh),
    (EnglishPhone::s, EnglishPhone::z),
    (EnglishPhone::sh, EnglishPhone::zh),
];

#[test]
fn voiced_fricatives_have_a_stable_low_bar_and_retain_their_noise_bands() {
    for (unvoiced, voiced) in PAIRS {
        let noise = speech_voice_target(unvoiced).unwrap();
        let bar = speech_voice_target(voiced).unwrap();
        assert_eq!((bar.voiced, bar.frication, bar.closure), (1, 1, 0));
        assert_eq!(bar.frames, noise.frames);
        assert_eq!(
            (bar.f2, bar.b2, bar.c2, bar.gain2),
            (noise.f2, noise.b2, noise.c2, noise.gain2)
        );
        assert_eq!(
            (bar.f3, bar.b3, bar.c3, bar.gain3),
            (noise.f3, noise.b3, noise.c3, noise.gain3)
        );
        // Decode the authored Q14 pole independently rather than compare a
        // duplicated coefficient table. Rounding allows a small frequency gap.
        let radius = (-f64::from(bar.c1) / 16384.0).sqrt();
        let frequency = (f64::from(bar.b1) / (32768.0 * radius)).acos() * 8000.0
            / (2.0 * core::f64::consts::PI);
        let bandwidth = -radius.ln() * 8000.0 / core::f64::consts::PI;
        assert!(radius > 0.0 && radius < 1.0);
        assert!((frequency - f64::from(bar.f1)).abs() < 1.0);
        assert!((bandwidth - 140.0).abs() < 0.1);
        assert_eq!(bar.f1, 250);
        assert!(bar.gain1 > 0);
    }
}

#[test]
fn low_voicing_is_noise_independent_and_upper_noise_is_phase_independent() {
    for (_, phone) in PAIRS {
        let target = speech_voice_target(phone).unwrap();
        let mut first_values = std::collections::BTreeSet::new();
        let mut upper_values = std::collections::BTreeSet::new();
        for phase in [0, 1, 20, 30, 50, 66] {
            let mut first_for_phase = None;
            for seed in [0, 1, 4095, 65535] {
                let mut value =
                    speech_pitch(context(EnglishStress::unspecified, target, 128)).unwrap();
                value.state.phase = phase;
                value.state.noise = seed;
                // Unused pure-voicing differencer history cannot influence or
                // refuse this selected pair of source paths.
                value.state.voicing = i32::MIN;
                let excited = speech_frame_excite(value).unwrap();
                let source = ExcitationInput {
                    phase,
                    period: value.period,
                    noise: excited.noise,
                    voiced: 0,
                    frication: 1,
                };
                let noise = speech_excitation(source).unwrap();
                assert_eq!(excited.upper_excitation, noise);
                assert!(noise.abs() <= 2048);
                let first = speech_frame_first(excited).unwrap().first;
                assert_eq!(*first_for_phase.get_or_insert(first), first);
                first_values.insert(first);
                if seed == 1 {
                    upper_values.insert(excited.upper_excitation);
                }
                assert_eq!(speech_frame(value).unwrap().state.voicing, excited.voicing);
            }
        }
        assert!(
            first_values.len() > 1,
            "voicing must remain audible in the low path"
        );
        assert_eq!(
            upper_values.len(),
            1,
            "upper drive must contain only turbulence"
        );
    }
}
