//! Internal artifact writer; xtask will own the supported proof entrance.
use conduit_speech::{
    EnglishPhoneme as P, EnglishPosition as W, EnglishStress as S, RealizationInput, Renderer,
    VoiceBoundary, VoiceEvent, SAMPLE_RATE_HZ,
};
use std::{fs, io::Write};
#[cfg(feature = "semantic-bindings")]
#[path = "first_samples/control.rs"]
mod control_samples;
#[cfg(feature = "semantic-bindings")]
#[path = "first_samples/duration.rs"]
mod duration_samples;
#[cfg(feature = "semantic-bindings")]
#[path = "first_samples/rich_phone.rs"]
mod rich_phone_samples;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args().nth(1).ok_or("output directory required")?;
    fs::create_dir_all(&output)?;
    println!("checked speech source: {}", conduit_speech::SOURCE_ID);
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
            "voiced-frication",
            vec![
                P::f,
                P::aa,
                P::v,
                P::aa,
                P::th,
                P::aa,
                P::dh,
                P::aa,
                P::s,
                P::aa,
                P::z,
                P::aa,
                P::sh,
                P::aa,
                P::zh,
                P::aa,
            ],
        ),
        (
            "nasal-place",
            vec![
                P::m,
                P::aa,
                P::n,
                P::aa,
                P::ng,
                P::aa,
                P::m,
                P::iy,
                P::n,
                P::iy,
                P::ng,
                P::iy,
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
    let mut stress_events = Vec::new();
    for stress in [
        S::primary,
        S::secondary,
        S::unstressed,
        S::reduced,
        S::unknown,
        S::unspecified,
    ] {
        stress_events.push(VoiceEvent::segment(RealizationInput {
            phoneme: P::eh,
            stress,
            position: W::isolated,
        }));
        stress_events.push(VoiceEvent::boundary(VoiceBoundary::word));
    }
    write_wav(&output, "stress-prosody", &stress_events)?;
    println!("{output}/stress-prosody.wav: primary, secondary, unstressed, reduced, unknown, unspecified");
    for (name, text) in [
        ("text-hello-world", "Hello, world!"),
        ("text-digits", "0123456789"),
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
    let mut direct_storage =
        [VoiceEvent::boundary(VoiceBoundary::word); conduit_speech::MAXIMUM_EVENTS];
    let pronounced = conduit_speech::pronounce("Hello, world!", &mut direct_storage)
        .map_err(|error| format!("{error:?}"))?;
    let direct: Vec<_> = pronounced
        .events()
        .iter()
        .map(|event| match event.realization() {
            Some(input) => conduit_speech::realize(input)
                .map(|selected| {
                    VoiceEvent::phone(conduit_speech::SpeechPhoneInput {
                        phone: selected.phone,
                        stress: input.stress,
                    })
                })
                .ok_or("realization refused"),
            None => Ok(*event),
        })
        .collect::<Result<_, _>>()?;
    write_wav(&output, "direct-phone-hello-world", &direct)?;
    println!("{output}/direct-phone-hello-world.wav: supplied phones; no source phoneme in renderer input");
    #[cfg(feature = "semantic-bindings")]
    duration_samples::write(&output)?;
    #[cfg(feature = "semantic-bindings")]
    control_samples::write(&output)?;
    #[cfg(feature = "semantic-bindings")]
    rich_phone_samples::write(&output)?;
    Ok(())
}

fn write_wav(
    output: &str,
    name: &str,
    events: &[VoiceEvent],
) -> Result<(), Box<dyn std::error::Error>> {
    let renderer = Renderer::prepare(events).map_err(|error| format!("{error:?}"))?;
    write_rendered(output, name, renderer)
}

fn write_rendered(
    output: &str,
    name: &str,
    mut renderer: Renderer<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
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
