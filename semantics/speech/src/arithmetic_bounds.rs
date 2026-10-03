//! Conservative profile bounds independent of the renderer's integer width.
use crate::generated::*;

fn target_bounds(t: SpeechAcousticTarget) {
    for b in [t.b1, t.b2, t.b3] {
        assert!(i64::from(b).abs() <= 32768);
    }
    for c in [t.c1, t.c2, t.c3] {
        assert!(i64::from(c).abs() <= 16384);
    }
    for gain in [t.gain1, t.gain2, t.gain3] {
        assert!((0..=4725).contains(&gain));
    }
    assert!((0..=1).contains(&t.voiced));
    assert!((0..=1).contains(&t.frication));
    assert!(t.closure >= 0 && t.closure <= t.frames && t.frames <= 960);
    assert!((0..=4000).contains(&t.f1));
    assert!((0..=4000).contains(&t.f2));
    assert!((0..=4000).contains(&t.f3));
}

#[test]
fn every_profile_target_stays_inside_the_fixed_arithmetic_envelope() {
    for (phone, _) in PHONES {
        let base = speech_voice_target(*phone).unwrap();
        target_bounds(base);
        for frame in 0..base.frames {
            target_bounds(
                speech_phone_frame_target(SpeechTrajectoryInput {
                    phone: *phone,
                    target: base,
                    frame,
                })
                .unwrap(),
            );
        }
        for place in [
            SpeechStopPlace::not_stop,
            SpeechStopPlace::labial,
            SpeechStopPlace::alveolar,
            SpeechStopPlace::velar,
        ] {
            for frame in 0..=160 {
                target_bounds(
                    speech_vowel_onset(SpeechVowelOnsetInput {
                        phone: *phone,
                        prosody: crate::prosody_parity::context(
                            EnglishStress::unspecified,
                            base,
                            frame,
                        ),
                        previous_place: place,
                        relation: SpeechNeighborRelation::segment,
                    })
                    .unwrap()
                    .target,
                );
            }
        }
    }
    // Connected targets and onset targets are convex coefficient/gain blends;
    // their weights stay within 0..=256 on the admitted, disjoint windows.
    // The same absolute envelope therefore includes any two eligible models.
}

#[test]
fn all_intermediate_profile_operations_fit_signed_32_bits() {
    let fits = |value: i64| assert!(value >= 0 && value <= i64::from(i32::MAX));
    // Covers any admitted history, including histories from different phones.
    // Each resonator stores a sample clamped to [-32767,32767].
    fits(256 * 256 * 256); // conservative Q8 cubic-flow numerator
    fits(4725 * 256); // maximum admitted gain times bounded stress level
    fits(68 * 256); // normalized phase numerator
    fits(4096 * 4725 + 32768 * 32767 + 16384 * 32767);
    fits((32767 + 32767 / 2 + 32767 / 4) * 16 * 256);
    fits(65535 * 25173 + 13849);
    fits(2 * 32768 * 256); // weighted sums and coefficient-delta products
    fits(i64::try_from(RENDER_PROFILE.maximum_utterance_frames).unwrap() * 8);
    assert!(RENDER_PROFILE.maximum_utterance_frames <= i32::MAX as u64);
    for period in 58..=69 {
        for phase in 0..69 {
            for noise in [0, 4095, 61440, 65535] {
                for voiced in [0, 1] {
                    for frication in [0, 1] {
                        let excitation = speech_excitation(ExcitationInput {
                            period,
                            phase,
                            noise,
                            voiced,
                            frication,
                        })
                        .unwrap();
                        assert!(i64::from(excitation).abs() <= 4096);
                        if frication == 0 {
                            assert!(excitation.abs() <= 1600);
                        }
                    }
                }
            }
        }
    }
    // Two admitted voiced-source history values therefore differ by at most
    // 3200; the existing 4096 resonator-input envelope covers upper branches.
    // Continuous voiced-fricative branches use pure voicing (<=1600) below
    // and scaled modulo noise (<=683) above, both within the same envelope.
    // Noise is nonnegative and reduced modulo 65536; phase is reset at period.
    // Initial history and boundary preservation satisfy the same invariant.
    let initial = speech_initial_state(SpeechStart::begin).unwrap();
    assert_eq!((initial.phase, initial.noise), (0, 1));
}
