//! Discrete source laws and portable parity, independent of tract filtering.
use crate::{
    frame_parity::integers,
    generated::*,
    prosody_parity::{evaluate, graph},
};

#[test]
fn fixed_period_flow_has_zero_dc_closed_phase_and_bounded_excitation() {
    for period in 58..=69 {
        let open = period * 3 / 4;
        let flow = |phase: i32| {
            if phase < 0 {
                return 0;
            }
            let q = phase * 256 / open;
            if q >= 256 {
                0
            } else {
                q * q * (256 - q) / 256
            }
        };
        let mut sum = 0;
        let mut positive = false;
        let mut negative = false;
        for phase in 0..period {
            let expected = flow(phase) - flow(phase - 1);
            let value = speech_excitation(ExcitationInput {
                period,
                phase,
                noise: 2048,
                voiced: 1,
                frication: 0,
            })
            .unwrap();
            assert_eq!(value, expected);
            assert!(value.abs() <= 1600);
            if phase > open {
                assert_eq!(value, 0);
            }
            positive |= value > 0;
            negative |= value < 0;
            sum += value;
        }
        assert_eq!(sum, 0, "period {period}");
        assert!(positive && negative);
        // Closure itself retains the final negative flow difference; only the
        // following closed samples are zero. Removing it would manufacture DC.
        assert!(
            speech_excitation(ExcitationInput {
                period,
                phase: open,
                noise: 2048,
                voiced: 1,
                frication: 0,
            })
            .unwrap()
                < 0
        );
    }
}

#[test]
fn source_graph_matches_portable_at_open_closure_and_refusal_edges() {
    let (programs, result) = graph("speech_excitation");
    assert_eq!(programs.len(), 5);
    let ty = &programs[0].1.input_type;
    let mut cases = std::vec::Vec::new();
    for period in 58..=69 {
        let open = period * 3 / 4;
        for phase in [0, 1, open / 2, open - 1, open, open + 1, period - 1, 68] {
            for (voiced, frication) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                for noise in [0, 4095] {
                    cases.push(ExcitationInput {
                        period,
                        phase,
                        noise,
                        voiced,
                        frication,
                    });
                }
            }
        }
    }
    for (period, phase) in [(0, 0), (1, 0), (i32::MAX, 0), (67, i32::MAX)] {
        let value = ExcitationInput {
            period,
            phase,
            noise: 0,
            voiced: 1,
            frication: 0,
        };
        assert_eq!(speech_excitation(value), None);
        cases.push(value);
    }
    for value in cases {
        let input = integers(
            ty,
            &[
                ("period", value.period),
                ("phase", value.phase),
                ("noise", value.noise),
                ("voiced", value.voiced),
                ("frication", value.frication),
            ],
        )
        .canonical_bytes()
        .unwrap();
        assert_eq!(
            evaluate(&programs, result, &input),
            speech_excitation(value).map(|sample| sample.to_le_bytes().to_vec()),
            "{value:?}"
        );
    }
}
