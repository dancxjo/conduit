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
            "diphthong-inventory",
            vec![P::ey, P::ow, P::ay, P::aw, P::oy],
        ),
        (
            "stop-release",
            vec![
                P::b,
                P::aa,
                P::p,
                P::aa,
                P::d,
                P::aa,
                P::t,
                P::aa,
                P::g,
                P::aa,
                P::k,
                P::aa,
            ],
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
        write_wav(&output, name, &events)?;
        println!("{output}/{name}.wav");
    }
    for (name, text) in [
        ("text-hello-world", "Hello, world!"),
        (
            "text-native-speech",
            "This is a native speech synthesizer. Please listen.",
        ),
        ("text-spelling-rules", "The quick fox can chat and sing."),
    ] {
        let mut storage =
            [VoiceEvent::boundary(VoiceBoundary::word); conduit_speech::MAXIMUM_EVENTS];
        let prepared =
            conduit_speech::pronounce(text, &mut storage).map_err(|error| format!("{error:?}"))?;
        write_wav(&output, name, prepared.events())?;
        println!("{output}/{name}.wav: {text}");
    }
    Ok(())
}

fn write_wav(
    output: &str,
    name: &str,
    events: &[VoiceEvent],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut renderer = Renderer::prepare(events).map_err(|error| format!("{error:?}"))?;
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
    Ok(())
}
