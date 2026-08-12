//! Exact semantic profile of the deterministic reference synthesizer.

/// Derives compatibility from the immutable DSP profile sealed into a Plan.
/// This describes synthesis semantics; it neither selects nor opens playback.
pub fn compatibility_profile(
    profile: conduit_synth::ReferenceSynthProfile,
) -> Result<conduit_std_catalog::SoundCompatibilityProfile, conduit_synth::SynthProfileError> {
    let profile = profile.validate()?;
    Ok(conduit_std_catalog::SoundCompatibilityProfile {
        profile_id: conduit_synth::REFERENCE_SYNTH_PROFILE_ID.into(),
        seam: conduit_std_catalog::SoundSeam::Synthesis,
        minimum_pitch_millihertz: conduit_core::MINIMUM_PITCH_MILLIHERTZ,
        maximum_pitch_millihertz: conduit_core::MAXIMUM_PITCH_MILLIHERTZ,
        maximum_polyphony: u16::from(profile.maximum_voices),
        maximum_events_per_second: conduit_synth::REFERENCE_SAMPLE_RATE_HZ,
        preserves_velocity: true,
        preserves_sustain: true,
        preserves_pitch_bend: true,
        maximum_pitch_bend_range_microcents: conduit_core::MAXIMUM_PITCH_BEND_RANGE_MICROCENTS,
        preserves_modulation: true,
        accepts_microtonal_pitch: true,
        supports_subtractive_filter: true,
        pcm: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_is_derived_from_validated_plan_sealed_synth_facts() {
        let required = conduit_synth::ReferenceSynthProfile::musician_reference();
        let profile = compatibility_profile(required).unwrap();
        assert_eq!(profile.seam, conduit_std_catalog::SoundSeam::Synthesis);
        assert_eq!(
            profile.maximum_polyphony,
            u16::from(required.maximum_voices)
        );
        assert!(profile.preserves_velocity);
        assert!(profile.preserves_sustain);
        assert!(profile.preserves_pitch_bend);
        assert!(profile.preserves_modulation);
        assert!(profile.accepts_microtonal_pitch);
        assert!(profile.supports_subtractive_filter);
        assert!(profile.pcm.is_none());
    }

    #[test]
    fn invalid_profile_cannot_become_a_conformance_claim() {
        let mut invalid = conduit_synth::ReferenceSynthProfile::musician_reference();
        invalid.maximum_voices = 0;
        assert_eq!(
            compatibility_profile(invalid),
            Err(conduit_synth::SynthProfileError::VoiceLimit)
        );
    }
}
