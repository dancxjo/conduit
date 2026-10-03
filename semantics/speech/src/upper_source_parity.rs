//! Pure-voicing differencing and explicit continuous-frication source roles.
use crate::{generated::*, prosody_parity::context};

#[test]
fn upper_voicing_difference_tracks_exact_source_and_keeps_noise_unchanged() {
    for (phone, _) in PHONES {
        let target = speech_voice_target(*phone).unwrap();
        for old in [-1600, 0, 1600] {
            for phase in [0, 1, 30, 50, 66] {
                let mut value =
                    speech_pitch(context(EnglishStress::unspecified, target, 0)).unwrap();
                value.state.phase = phase;
                value.state.voicing = old;
                let out = speech_frame_excite(value).unwrap();
                let source = ExcitationInput {
                    phase,
                    period: value.period,
                    noise: speech_noise(value.state.noise).unwrap(),
                    voiced: target.voiced,
                    frication: target.frication,
                };
                let voicing = speech_excitation(ExcitationInput {
                    frication: 0,
                    ..source
                })
                .unwrap();
                let mixed = speech_excitation(source).unwrap();
                assert_eq!(out.voicing, voicing);
                assert_eq!(out.excitation, mixed);
                assert_eq!(
                    out.upper_excitation,
                    if target.voiced == 1 && target.frication == 0 {
                        voicing - old
                    } else if matches!(
                        phone,
                        EnglishPhone::v | EnglishPhone::dh | EnglishPhone::z | EnglishPhone::zh
                    ) {
                        speech_excitation(ExcitationInput {
                            voiced: 0,
                            ..source
                        })
                        .unwrap()
                    } else {
                        mixed
                    }
                );
                let state = speech_frame(value).unwrap().state;
                assert_eq!(state.voicing, voicing);
                assert_eq!(speech_boundary_frame(state).unwrap().state, state);
            }
        }
    }
}

#[test]
fn steady_voicing_is_rejected_and_unused_history_cannot_refuse_noise() {
    let (programs, result_index) = crate::prosody_parity::graph("speech_frame");
    let assert_portable = |value| {
        assert_eq!(
            crate::prosody_parity::evaluate(
                &programs,
                result_index,
                &crate::frame_parity::input(&programs[0].1.input_type, value)
            ),
            speech_frame(value)
                .map(|out| crate::frame_parity::result(&programs[result_index].1.output_type, out)),
        );
    };
    let target = speech_voice_target(EnglishPhone::iy).unwrap();
    let mut value = speech_pitch(context(EnglishStress::unspecified, target, 0)).unwrap();
    let current = speech_frame_excite(value).unwrap().voicing;
    value.state.voicing = current;
    assert_eq!(speech_frame_excite(value).unwrap().upper_excitation, 0);
    // This history is deliberately outside the admitted source envelope. The
    // selected voiced branch must refuse subtraction overflow; noise must not
    // evaluate that unused subtraction or reinterpret the history as noise.
    value.state.voicing = i32::MIN;
    assert!(speech_frame_excite(value).is_none());
    assert_portable(value);
    value.target = speech_voice_target(EnglishPhone::s).unwrap();
    let out = speech_frame_excite(value).unwrap();
    assert_eq!(out.upper_excitation, out.excitation);
    assert_eq!(out.voicing, 0);
    assert_portable(value);
}
