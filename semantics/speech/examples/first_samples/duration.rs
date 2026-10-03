//! Retained listening fixture: explicit exact duration changes, unchanged phones.
use conduit_speech::{
    duration::prepare_duration_render, pronounce, semantic::SpeechExactDuration, Renderer,
    VoiceBoundary, VoiceEvent, MAXIMUM_EVENTS, SAMPLE_RATE_HZ,
};

pub(super) fn write(output: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut events = [VoiceEvent::boundary(VoiceBoundary::word); MAXIMUM_EVENTS];
    let text = pronounce("Hello, world!", &mut events).map_err(|error| format!("{error:?}"))?;
    // This listening fixture derives exact source durations from the current
    // profile and authors three relative tempi. The production adapter does not
    // choose a tempo or invent a source duration.
    let baseline = text
        .events()
        .iter()
        .map(|event| {
            Renderer::prepare(core::slice::from_ref(event))
                .map(|renderer| renderer.total_frames())
                .map_err(|error| format!("{error:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    for (name, numerator, denominator) in [
        ("duration-default", 1, 1),
        ("duration-faster", 2, 3),
        ("duration-slower", 3, 2),
    ] {
        let durations = baseline
            .iter()
            .map(|frames| {
                SpeechExactDuration::new(
                    u64::from(SAMPLE_RATE_HZ) * denominator,
                    frames * numerator,
                )
                .map_err(|error| format!("{error:?}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut counts = [0; MAXIMUM_EVENTS];
        let prepared = prepare_duration_render(text.events(), &durations, &mut counts)
            .map_err(|error| format!("{error:?}"))?;
        super::write_rendered(output, name, prepared.renderer())?;
        println!(
            "{output}/{name}.wav: Hello, world!; exact duration factor {numerator}/{denominator}"
        );
    }
    Ok(())
}
