//! Subframe phonation agrees with the checked graph and declared Q8 clock.
use crate::{frame_parity, generated::*, prosody_parity};

#[test]
fn resolved_cycle_frames_match_the_portable_graph_at_phase_and_domain_edges() {
    let (programs, result) = prosody_parity::graph("speech_frame");
    for phone in [
        EnglishPhone::iy,
        EnglishPhone::z,
        EnglishPhone::h,
        EnglishPhone::t,
    ] {
        let target = speech_voice_target(phone).unwrap();
        for period_q8 in [2048, 17066, 65535, 131072] {
            for phase_q8 in [0, period_q8 / 2, period_q8 - 1] {
                let value = SpeechFrameInput {
                    cycle: SpeechFrameCycleControl {
                        mode: SpeechCycleControlMode::resolved,
                        phase_q8,
                        period_q8,
                    },
                    attack: true,
                    release: true,
                    target,
                    period: 67,
                    frame: target.closure,
                    state: speech_initial_state(SpeechStart::begin).unwrap(),
                };
                let input = frame_parity::input(&programs[0].1.input_type, value);
                let actual = speech_frame(value).unwrap();
                assert_eq!(actual.phase_q8, (phase_q8 + 256) % period_q8);
                assert_eq!(actual.state.phase, actual.phase_q8 / 256);
                assert_eq!(
                    prosody_parity::evaluate(&programs, result, &input).unwrap(),
                    frame_parity::result(&programs[result].1.output_type, actual)
                );
            }
        }
    }
}

#[test]
fn fractional_cycles_retain_subframe_offset_and_bound_every_advance() {
    let target = speech_voice_target(EnglishPhone::iy).unwrap();
    let mut state = speech_initial_state(SpeechStart::begin).unwrap();
    let mut phase_q8 = 0;
    for frame in 0..24000 {
        let result = speech_frame(SpeechFrameInput {
            cycle: SpeechFrameCycleControl {
                mode: SpeechCycleControlMode::resolved,
                phase_q8,
                period_q8: 17066,
            },
            attack: false,
            release: false,
            target,
            period: 67,
            frame: frame % target.frames,
            state,
        })
        .unwrap();
        assert_eq!(result.phase_q8, ((frame + 1) * 256) % 17066);
        assert!((0..17066).contains(&result.phase_q8));
        phase_q8 = result.phase_q8;
        state = result.state;
    }
}
