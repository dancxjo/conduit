//! Listening fixtures for resolved cycle quantities and output-relative amplitude.
use conduit_speech::{control::prepare_voice_control, semantic::*, *};
pub(super) fn write(output: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut storage = [VoiceEvent::boundary(VoiceBoundary::word); MAXIMUM_EVENTS];
    let text = pronounce("Hello, world!", &mut storage).map_err(|error| format!("{error:?}"))?;
    for (name, hertz, denominator) in [
        ("control-profile-half", None, 2),
        ("control-120hz", Some(120), 1),
        ("control-200hz", Some(200), 1),
        ("control-120hz-half", Some(120), 2),
    ] {
        let cycle = hertz.map(|rate| SpeechFundamentalCycle::new(rate, 1).unwrap());
        let amplitude = SpeechRelativeIntensity::new(denominator, 1).unwrap();
        let prepared = prepare_voice_control(cycle.as_ref(), &amplitude)
            .map_err(|error| format!("{error:?}"))?;
        let controls = vec![prepared.compact(); text.events().len()];
        let renderer = Renderer::prepare_controlled(text.events(), &controls)
            .map_err(|error| format!("{error:?}"))?;
        super::write_rendered(output, name, renderer)?;
        println!("{output}/{name}.wav: cycle {hertz:?} Hz; relative amplitude 1/{denominator}; period_q8={}, amplitude_q15={}", prepared.compact().period_q8, prepared.compact().amplitude_q15);
    }
    Ok(())
}
