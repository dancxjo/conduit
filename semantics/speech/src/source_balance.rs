//! Energy admission for the compact profile's distinct excitation sources.
use crate::generated::*;

#[test]
fn turbulent_source_energy_does_not_overpower_the_voiced_source() {
    let noise_energy: i64 = (0..4096)
        .map(|noise| {
            let sample = speech_excitation(ExcitationInput {
                phase: 0,
                period: 67,
                noise,
                voiced: 0,
                frication: 1,
            })
            .unwrap();
            assert!(sample.abs() <= 512);
            i64::from(sample).pow(2)
        })
        .sum();
    for period in 58..=69 {
        let voice_energy: i64 = (0..period)
            .map(|phase| {
                let sample = speech_excitation(ExcitationInput {
                    phase,
                    period,
                    noise: 0,
                    voiced: 1,
                    frication: 0,
                })
                .unwrap();
                i64::from(sample).pow(2)
            })
            .sum();
        assert!(voice_energy > 0);
        // Compare mean-square energy without floating point or rounding a RMS.
        assert!(noise_energy * i64::from(period) < voice_energy * 4096);
    }
}
