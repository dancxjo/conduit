//! Known stress changes only bounded sonorant gains; uncertainty stays neutral.
use crate::{
    generated::*,
    prosody_parity::{context, evaluate, graph, input},
};

#[test]
fn every_phone_and_stress_preserves_context_and_has_exact_portable_intensity() {
    let (programs, result) = graph("speech_stress_intensity");
    assert_eq!(programs.len(), 3);
    let ty = &programs[0].1.input_type;
    for (phone, _) in PHONES {
        let target = speech_voice_target(*phone).unwrap();
        for (stress, _) in STRESSES {
            let eligible = target.voiced == 1 && target.frication == 0 && target.closure == 0;
            let level = if eligible {
                match stress {
                    EnglishStress::secondary => 224,
                    EnglishStress::unstressed => 192,
                    EnglishStress::reduced => 160,
                    _ => 256,
                }
            } else {
                256
            };
            for frame in [
                0,
                63,
                128,
                target.frames / 2,
                target.frames - 129,
                target.frames - 1,
            ] {
                let mut value = context(*stress, target, frame);
                value.attack = frame != 63;
                value.release = frame != target.frames - 1;
                value.state.phase = 57;
                value.state.noise = 65535;
                value.state.first1 = -32767;
                let level = if level == 256 {
                    256
                } else if frame < 128 {
                    256 - (256 - level) * frame / 128
                } else if target.frames - frame < 128 {
                    256 - (256 - level) * (target.frames - frame) / 128
                } else {
                    level
                };
                let mut expected = value;
                expected.target.gain1 = target.gain1 * level / 256;
                expected.target.gain2 = target.gain2 * level / 256;
                expected.target.gain3 = target.gain3 * level / 256;
                let actual = speech_stress_intensity(value).unwrap();
                assert_eq!(actual, expected, "{phone:?}/{stress:?}/{frame}");
                assert_eq!(
                    evaluate(&programs, result, &input(ty, value)),
                    Some(input(ty, expected))
                );
                for (gain, base) in [
                    (actual.target.gain1, target.gain1),
                    (actual.target.gain2, target.gain2),
                    (actual.target.gain3, target.gain3),
                ] {
                    assert!(gain >= 0 && gain <= base);
                }
            }
        }
    }
}

#[test]
fn neutral_branch_is_exact_and_selected_scaling_refuses_overflow() {
    let (programs, result) = graph("speech_stress_intensity");
    let ty = &programs[0].1.input_type;
    let mut target = speech_voice_target(EnglishPhone::iy).unwrap();
    target.gain1 = i32::MAX;
    for stress in [
        EnglishStress::primary,
        EnglishStress::unknown,
        EnglishStress::unspecified,
    ] {
        let value = context(stress, target, 0);
        assert_eq!(speech_stress_intensity(value), Some(value));
        assert_eq!(
            evaluate(&programs, result, &input(ty, value)),
            Some(input(ty, value))
        );
    }
    for stress in [
        EnglishStress::secondary,
        EnglishStress::unstressed,
        EnglishStress::reduced,
    ] {
        let value = context(stress, target, 128);
        assert_eq!(speech_stress_intensity(value), None);
        assert_eq!(evaluate(&programs, result, &input(ty, value)), None);
    }
}

#[test]
fn local_intensity_is_neutral_at_joins_and_monotone_on_each_edge() {
    let target = speech_voice_target(EnglishPhone::iy).unwrap();
    for (stress, _) in STRESSES {
        let level = match stress {
            EnglishStress::secondary => 224,
            EnglishStress::unstressed => 192,
            EnglishStress::reduced => 160,
            _ => 256,
        };
        let mut previous = target.gain1;
        for frame in 0..target.frames {
            let out = speech_stress_intensity(context(*stress, target, frame)).unwrap();
            let gain = out.target.gain1;
            assert!((target.gain1 * level / 256..=target.gain1).contains(&gain));
            if frame == 0 || frame == target.frames - 1 {
                assert_eq!(gain, target.gain1);
            }
            if frame <= 128 {
                assert!(gain <= previous);
            }
            if frame >= target.frames - 128 {
                assert!(gain >= previous);
            }
            if (128..=target.frames - 128).contains(&frame) {
                assert_eq!(gain, target.gain1 * level / 256);
            }
            previous = gain;
        }
    }
}
