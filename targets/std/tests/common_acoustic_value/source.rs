//! Exact shared authored Source units; no runtime graph is built here.
pub fn composed_source() -> String {
    let units = [
        include_str!("../../../../semantics/audio/types.conduit"),
        include_str!("../../../../semantics/audio/trajectory_types.conduit"),
        include_str!("../../../../semantics/audio/rate_types.conduit"),
        include_str!("../../../../semantics/audio/decibel_types.conduit"),
        include_str!("../../../../semantics/audio/exact_control_types.conduit"),
        include_str!("../../../../semantics/audio/time_analysis_types.conduit"),
        include_str!("../../../../semantics/audio/spectral_analysis_types.conduit"),
        include_str!("../../../../semantics/audio/dsp/periodic-types.conduit"),
        conduit_audio::PERIODIC_FLOW_SOURCE,
        conduit_audio::PERIODIC_OSCILLATOR_SOURCE,
        conduit_audio::PERIODIC_VALIDATION_SOURCE,
        include_str!("../../../../semantics/audio/dsp/resonator-recurrence.conduit"),
        include_str!("../../../../semantics/audio/dsp/resonator-types.conduit"),
        include_str!("../../../../semantics/audio/dsp/resonator-projection.conduit"),
        include_str!("../../../../semantics/audio/dsp/resonator.conduit"),
        include_str!("../fixtures/acoustic-value-frame.conduit"),
    ];
    let mut imports = Vec::new();
    let mut body = Vec::new();
    for unit in units {
        for line in unit.lines() {
            if line.starts_with("with ") {
                imports.push(line);
            } else {
                body.push(line);
            }
        }
    }
    imports.extend(body);
    imports.join("\n") + "\n"
}
