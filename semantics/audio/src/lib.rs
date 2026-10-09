#![no_std]

//! Portable bounded audio and musical value contracts.
//!
//! Host callbacks, devices, MIDI/OPL protocols, DSP implementations,
//! scheduling, Plans, Plays, and Signs remain with their owning layers.

extern crate alloc;

#[allow(
    clippy::clone_on_copy,
    clippy::manual_range_contains,
    clippy::too_many_arguments,
    dead_code
)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));

    impl Copy for NoteOccurrenceId {}

    impl core::hash::Hash for NoteOccurrenceId {
        fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
            self.0.hash(state);
        }
    }
}

pub use generated::{
    AudioAmplitudePowerEligible, AudioAmplitudePowerRelationship, AudioAmplitudePowerRequest,
    AudioCycleDuration, AudioFrequencyHz, AudioPowerRatio, AudioRelativeAmplitude,
    AudioRenderDemand, AudioResonator, AudioToneTerminal, AudioToneTerminalForm, BeatReference,
    CancellationDisposition, Gate, GateForm, IncompatibilityReason, InstrumentAnalogEvent,
    InstrumentButtonEvent, InstrumentControl, InstrumentMapping, InstrumentPitchMillihertz,
    ModulationDestination, ModulationDestinationForm, MusicalControl, MusicalControlEvent,
    MusicalControlModulation, MusicalControlPitchBend, MusicalControlSustain, MusicalNoteEvent,
    MusicalPitch, NoteOccurrenceId, PcmChannelLayout, PcmChannelLayoutForm, PcmClipProfile,
    PcmCompatibilityProfile, PcmFrameHeader, PcmSampleRepresentation, PcmSampleRepresentationForm,
    PressureDisposition, RhythmRecoveryState, SoundSeam, SoundStreamState, SoundTerminalBehavior,
    TimingClassification, TimingFeedback, ToneIntent,
};

mod audio_info;
mod audio_render_demand;
mod pcm_clip;
mod sampled_signal_mapping;
mod sound_info;
mod tone_terminal;

pub use audio_info::*;
pub use audio_render_demand::*;
pub use pcm_clip::*;
pub use sampled_signal_mapping::*;
pub use sound_info::*;
pub use tone_terminal::*;

mod acoustic_quantities;
pub use acoustic_quantities::*;

mod source_programs {
    include!(concat!(env!("OUT_DIR"), "/acoustic_programs.rs"));
}
mod trajectory;
pub use generated::{
    AudioExactTimeOffset, AudioOriginIdentity, AudioQuantityTrajectory, AudioTimelineIdentity,
    AudioTrajectoryAnchor, AudioTrajectoryDomain, AudioTrajectoryEndpoints,
    AudioTrajectoryInterpolation, AudioTrajectoryOutside, AudioTrajectoryProvenance,
    AudioTrajectoryProvenanceKind, AudioTrajectoryQuantity, AudioTrajectoryQuery,
    AudioTrajectorySegment,
};
pub use trajectory::*;

mod source_execution;
pub use source_execution::{AudioSourceExecution, AudioSourceExecutionRefusal};
mod rate_projection;
pub use generated::{
    AudioCumulativeFrameBasis, AudioCumulativeFrameCursor, AudioCumulativeFrameRequest,
    AudioCumulativeFrameResult, AudioFrameGridFidelity, AudioFrameQuantization,
    AudioIntegerFrameTarget, AudioSampleProjectionChain, AudioSampleProjectionQuantity,
    AudioSampleProjectionRequest, AudioSampleProjectionResult, AudioSampleRateBasis,
    AudioTimeFraction,
};
pub use rate_projection::*;
mod rate_projection_report;
pub use rate_projection_report::*;

mod decibel_projection;
pub use decibel_projection::*;
pub use generated::{
    AudioAmplitudePowerReferences, AudioDecibelBasis, AudioDecibelConvention, AudioDecibelFraction,
    AudioDecibelLevel, AudioDecibelReference, AudioDecibelReferenceRole, AudioDecibelValue,
    AudioLevelRatio, AudioReferencedAmplitudePowerRequest, AudioReferencedLevelRatio,
};
mod decibel_projection_report;
pub use decibel_projection_report::*;
