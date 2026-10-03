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
            assert!(sample.abs() <= 683);
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

#[test]
fn hello_noise_and_vowel_have_audible_bounded_levels_and_exact_pause() {
    use crate::{Renderer, VoiceBoundary, VoiceEvent};
    let segment = |phoneme| {
        VoiceEvent::segment(RealizationInput {
            phoneme,
            stress: EnglishStress::primary,
            position: EnglishPosition::medial,
        })
    };
    let events = [
        segment(EnglishPhoneme::h),
        segment(EnglishPhoneme::eh),
        VoiceEvent::boundary(VoiceBoundary::turn),
    ];
    let mut renderer = Renderer::prepare(&events).unwrap();
    let mut pcm = std::vec::Vec::new();
    while !renderer.is_complete() {
        let mut block = [0; 128];
        let count = renderer.render(&mut block).unwrap();
        pcm.extend_from_slice(&block[..count]);
    }
    let h_frames = usize::try_from(speech_voice_target(EnglishPhone::h).unwrap().frames).unwrap();
    let vowel_frames =
        usize::try_from(speech_voice_target(EnglishPhone::eh).unwrap().frames).unwrap();
    // Exclude source/model attack, release and adjacent spectral-transition windows.
    let mean_square = |samples: &[i16]| {
        samples.iter().map(|v| i64::from(*v).pow(2)).sum::<i64>() / samples.len() as i64
    };
    let noise = mean_square(&pcm[64..h_frames - 64]);
    let tone = mean_square(&pcm[h_frames + 160..h_frames + vowel_frames - 160]);
    assert!(
        tone > 500 * 500,
        "voiced PCM must retain the corrected profile level"
    );
    assert!(noise < 4 * tone, "noise must not exceed the vowel by 6 dB");
    assert!(pcm[h_frames + vowel_frames..].iter().all(|v| *v == 0));
    assert!(pcm
        .iter()
        .all(|v| i32::from(*v).abs() < i32::from(i16::MAX)));
}
