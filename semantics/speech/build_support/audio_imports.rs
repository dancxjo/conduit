//! The final flag selects roots used by generated Native fields.
//! Other entries remain available for explicit authored quantity conversions.
//! Exact owner Native roots used by Speech authoring and bindings.
pub const PATHS: &[(&str, &str, bool)] = &[
    ("AudioFrequencyHz", "conduit_audio::AudioFrequencyHz", true),
    (
        "AudioCycleDuration",
        "conduit_audio::AudioCycleDuration",
        false,
    ),
    (
        "AudioTrajectoryAnchor",
        "conduit_audio::AudioTrajectoryAnchor",
        true,
    ),
    (
        "AudioFrequencyControl",
        "conduit_audio::AudioFrequencyControl",
        true,
    ),
    (
        "AudioTrajectoryQuery",
        "conduit_audio::AudioTrajectoryQuery",
        true,
    ),
    (
        "AudioExactTimeOffset",
        "conduit_audio::AudioExactTimeOffset",
        true,
    ),
    (
        "AudioScopedControlTrajectory",
        "conduit_audio::AudioScopedControlTrajectory",
        true,
    ),
    (
        "AudioControlConflictPolicy",
        "conduit_audio::AudioControlConflictPolicy",
        true,
    ),
    (
        "AudioControlGapPolicy",
        "conduit_audio::AudioControlGapPolicy",
        true,
    ),
];
