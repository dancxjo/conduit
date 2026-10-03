//! Internal artifact writer; xtask will own the supported proof entrance.
use conduit_speech::{
    EnglishPhoneme as P, EnglishPosition as W, EnglishStress as S, RealizationInput, Renderer,
    VoiceBoundary, VoiceEvent, SAMPLE_RATE_HZ,
};
use std::{fs, io::Write};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args().nth(1).ok_or("output directory required")?;
    fs::create_dir_all(&output)?;
    for (name, phones) in [
        (
            "hello-world",
            vec![P::h, P::eh, P::l, P::ow, P::w, P::er, P::l, P::d],
        ),
        (
            "vowel-inventory",
            vec![
                P::iy,
                P::ih,
                P::eh,
                P::ae,
                P::aa,
                P::ao,
                P::uh,
                P::uw,
                P::ah,
                P::ax,
                P::er,
            ],
        ),
    ] {
        let mut events: Vec<_> = phones
            .into_iter()
            .map(|phoneme| {
                VoiceEvent::segment(RealizationInput {
                    phoneme,
                    stress: S::primary,
                    position: W::medial,
                })
            })
            .collect();
        events.push(VoiceEvent::boundary(VoiceBoundary::turn));
        let mut renderer = Renderer::prepare(&events).map_err(|error| format!("{error:?}"))?;
        let bytes = u32::try_from(renderer.total_frames() * 2)?;
        let mut file = fs::File::create(format!("{output}/{name}.wav"))?;
        file.write_all(b"RIFF")?;
        file.write_all(&(bytes + 36).to_le_bytes())?;
        file.write_all(b"WAVEfmt ")?;
        file.write_all(&16u32.to_le_bytes())?;
        file.write_all(&1u16.to_le_bytes())?;
        file.write_all(&1u16.to_le_bytes())?;
        file.write_all(&SAMPLE_RATE_HZ.to_le_bytes())?;
        file.write_all(&(SAMPLE_RATE_HZ * 2).to_le_bytes())?;
        file.write_all(&2u16.to_le_bytes())?;
        file.write_all(&16u16.to_le_bytes())?;
        file.write_all(b"data")?;
        file.write_all(&bytes.to_le_bytes())?;
        let mut block = [0_i16; 128];
        while !renderer.is_complete() {
            let count = renderer
                .render(&mut block)
                .map_err(|error| format!("{error:?}"))?;
            for sample in &block[..count] {
                file.write_all(&sample.to_le_bytes())?;
            }
        }
        println!("{output}/{name}.wav");
    }
    Ok(())
}
