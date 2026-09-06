use conduit_semantic_catalog::{
    compatibility, IncompatibilityReason, PcmCompatibilityProfile, SoundCompatibilityProfile,
    SoundSeam,
};
use serde::Serialize;

use crate::cli::GlobalOpts;

use super::CatalogError;

mod adapters;
mod forms;
mod plans;

const SCHEMA: &str = "conduit.sound/conformance-matrix@1";
const MAXIMUM_REALIZATIONS: usize = 6;
const MAXIMUM_REQUIREMENTS: usize = 7;
const MAXIMUM_CELLS: usize = MAXIMUM_REALIZATIONS * MAXIMUM_REQUIREMENTS;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ProofClass {
    HostedLiveDevice,
    FreestandingEmulator,
    PhysicalPeteCreateHil,
    DeterministicReference,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
enum EvidenceStatus {
    Accepted,
    MissingRequiredProof,
}

#[derive(Debug, Serialize)]
struct Realization {
    name: &'static str,
    implementation_id: &'static str,
    profile: SoundCompatibilityProfile,
    current_proof_class: ProofClass,
    required_proof_class: ProofClass,
    evidence_status: EvidenceStatus,
}

#[derive(Debug, Serialize)]
struct Requirement {
    id: &'static str,
    description: &'static str,
    profile: SoundCompatibilityProfile,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case", tag = "verdict")]
enum Verdict {
    Compatible,
    Unsupported { reason: IncompatibilityReason },
}

#[derive(Debug, Serialize)]
struct Cell {
    realization: &'static str,
    requirement: &'static str,
    verdict: Verdict,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    basis: &'static str,
    proof_class_policy: &'static str,
    maximum_realizations: usize,
    maximum_requirements: usize,
    maximum_cells: usize,
    realization_count: usize,
    requirement_count: usize,
    cell_count: usize,
    realizations: Vec<Realization>,
    requirements: Vec<Requirement>,
    cells: Vec<Cell>,
    canonical_forms: Vec<forms::CanonicalForm>,
    cross_realization_plans: plans::PlanComparison,
    recursive_realization: RecursiveRealization,
    lossy_adapter: adapters::LossyAdapterProof,
}

#[derive(Debug, Serialize)]
struct RecursiveRealization {
    requirement_kind: &'static str,
    stages: [&'static str; 2],
    selection_basis: &'static str,
    proof_class: ProofClass,
    proof_command: &'static str,
}

pub(super) fn run(opts: &GlobalOpts) -> Result<(), CatalogError> {
    if opts.dry_run {
        println!(
            "derive exact sound profiles from implementation functions and emit bounded compatibility cells without opening devices"
        );
        return Ok(());
    }
    let report = build()?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(CatalogError::encoding)?
    );
    Ok(())
}

fn build() -> Result<Report, CatalogError> {
    let realizations = realizations()?;
    let requirements = requirements();
    if realizations.len() > MAXIMUM_REALIZATIONS || requirements.len() > MAXIMUM_REQUIREMENTS {
        return Err(CatalogError::new(
            "sound-matrix-bound-exceeded",
            "sound conformance inventory exceeds its reviewed bound",
        ));
    }
    let mut cells = Vec::with_capacity(MAXIMUM_CELLS);
    for realization in &realizations {
        for requirement in &requirements {
            let verdict = match compatibility(&requirement.profile, &realization.profile) {
                Ok(()) => Verdict::Compatible,
                Err(reason) => Verdict::Unsupported { reason },
            };
            cells.push(Cell {
                realization: realization.name,
                requirement: requirement.id,
                verdict,
            });
        }
    }
    let canonical_forms = forms::build()?;
    let cross_realization_plans = plans::build()?;
    Ok(Report {
        schema: SCHEMA,
        basis: "profiles-derived-from-implementation-profile-functions",
        proof_class_policy: "compatibility-does-not-promote-missing-device-evidence",
        maximum_realizations: MAXIMUM_REALIZATIONS,
        maximum_requirements: MAXIMUM_REQUIREMENTS,
        maximum_cells: MAXIMUM_CELLS,
        realization_count: realizations.len(),
        requirement_count: requirements.len(),
        cell_count: cells.len(),
        realizations,
        requirements,
        cells,
        canonical_forms,
        cross_realization_plans,
        recursive_realization: RecursiveRealization {
            requirement_kind: conduit_semantic_catalog::MUSIC_PLAY_THROUGH_SYNTH.requirement_kind,
            stages: conduit_semantic_catalog::MUSIC_PLAY_THROUGH_SYNTH.stages,
            selection_basis: "ordinary-offers-faces-resources-policy-and-constraints",
            proof_class: ProofClass::DeterministicReference,
            proof_command: "cargo test -p conduit-composite --test sound_realization",
        },
        lossy_adapter: adapters::build()?,
    })
}

fn realizations() -> Result<Vec<Realization>, CatalogError> {
    let pc = conduitos::pc_speaker_offer::compatibility_profile(
        conduitos::pc_speaker_offer::PcSpeakerRealization {
            base_id: [1; 32],
            pit_input_hz: conduitos::pc_speaker_offer::PC_SPEAKER_PIT_INPUT_HZ,
            minimum_divisor: 19,
            maximum_divisor: u16::MAX,
            maximum_error_parts_per_million: 2_500,
            event_slots: 8,
            operation_slots: 1,
        },
    )
    .map_err(|error| CatalogError::new("pc-speaker-profile-invalid", format!("{error:?}")))?;
    let midi = conduit_std_host::hosted_midi::output_compatibility_profile()
        .map_err(|error| CatalogError::new("midi-profile-invalid", error))?;
    let synth = conduit_std_host::hosted_synth::compatibility_profile(
        conduit_synth::ReferenceSynthProfile::musician_reference(),
    )
    .map_err(|error| CatalogError::new("synth-profile-invalid", format!("{error:?}")))?;
    Ok(vec![
        realization(
            "pc-speaker",
            conduitos::pc_speaker_offer::PC_SPEAKER_IMPLEMENTATION,
            pc,
            ProofClass::FreestandingEmulator,
            ProofClass::FreestandingEmulator,
            EvidenceStatus::Accepted,
        ),
        realization(
            "pete-create-oi",
            conduit_pete::SPEAKER_IMPLEMENTATION,
            conduit_pete::compatibility_profile(),
            ProofClass::DeterministicReference,
            ProofClass::PhysicalPeteCreateHil,
            EvidenceStatus::MissingRequiredProof,
        ),
        realization(
            "adlib-opl2",
            conduitos::opl2_offer::OPL2_IMPLEMENTATION,
            conduitos::opl2_offer::compatibility_profile(),
            ProofClass::FreestandingEmulator,
            ProofClass::FreestandingEmulator,
            EvidenceStatus::Accepted,
        ),
        realization(
            "midi-output",
            conduit_std_offers::MUSIC_PLAY_MIDI_IMPLEMENTATION,
            midi,
            ProofClass::DeterministicReference,
            ProofClass::HostedLiveDevice,
            EvidenceStatus::MissingRequiredProof,
        ),
        realization(
            "software-synth",
            conduit_synth::REFERENCE_SYNTH_IMPLEMENTATION_ID,
            synth,
            ProofClass::DeterministicReference,
            ProofClass::DeterministicReference,
            EvidenceStatus::Accepted,
        ),
        realization(
            "hosted-pcm",
            conduit_std_offers::AUDIO_PLAY_ALSA_HW_IMPLEMENTATION,
            conduit_std_host::hosted_audio::compatibility_profile(),
            ProofClass::HostedLiveDevice,
            ProofClass::HostedLiveDevice,
            EvidenceStatus::Accepted,
        ),
    ])
}

fn realization(
    name: &'static str,
    implementation_id: &'static str,
    profile: SoundCompatibilityProfile,
    current_proof_class: ProofClass,
    required_proof_class: ProofClass,
    evidence_status: EvidenceStatus,
) -> Realization {
    Realization {
        name,
        implementation_id,
        profile,
        current_proof_class,
        required_proof_class,
        evidence_status,
    }
}

fn requirements() -> Vec<Requirement> {
    vec![
        requirement(
            "tone",
            "bounded monophonic pitch and gate",
            SoundSeam::Tone,
            1,
        ),
        requirement(
            "simple-monophonic-notes",
            "ordered notes and rests with no expressive controls",
            SoundSeam::MusicalEvents,
            1,
        ),
        requirement(
            "simple-polyphonic-notes",
            "eight simultaneous notes with no expressive controls",
            SoundSeam::MusicalEvents,
            8,
        ),
        expressive_requirement(),
        microtonal_requirement(),
        synthesis_requirement(),
        pcm_requirement(),
    ]
}

fn requirement(
    id: &'static str,
    description: &'static str,
    seam: SoundSeam,
    maximum_polyphony: u16,
) -> Requirement {
    Requirement {
        id,
        description,
        profile: SoundCompatibilityProfile {
            profile_id: format!("conduit-conformance/{id}@1"),
            seam,
            minimum_pitch_millihertz: 440_000,
            maximum_pitch_millihertz: 660_000,
            maximum_polyphony,
            maximum_events_per_second: if seam == SoundSeam::Tone { 0 } else { 16 },
            preserves_velocity: false,
            preserves_sustain: false,
            preserves_pitch_bend: false,
            maximum_pitch_bend_range_microcents: 0,
            preserves_modulation: false,
            accepts_microtonal_pitch: false,
            supports_subtractive_filter: false,
            pcm: None,
        },
    }
}

fn expressive_requirement() -> Requirement {
    let mut value = requirement(
        "expressive-notes",
        "eight voices with velocity, sustain, bend, and modulation",
        SoundSeam::MusicalEvents,
        8,
    );
    value.profile.preserves_velocity = true;
    value.profile.preserves_sustain = true;
    value.profile.preserves_pitch_bend = true;
    value.profile.maximum_pitch_bend_range_microcents =
        conduit_midi::MIDI_PITCH_BEND_RANGE_MICROCENTS;
    value.profile.preserves_modulation = true;
    value
}

fn synthesis_requirement() -> Requirement {
    let mut value = requirement(
        "expressive-subtractive-synthesis",
        "expressive events plus the subtractive synthesis control surface",
        SoundSeam::Synthesis,
        8,
    );
    value.profile.preserves_velocity = true;
    value.profile.preserves_sustain = true;
    value.profile.preserves_pitch_bend = true;
    value.profile.maximum_pitch_bend_range_microcents =
        conduit_midi::MIDI_PITCH_BEND_RANGE_MICROCENTS;
    value.profile.preserves_modulation = true;
    value.profile.accepts_microtonal_pitch = true;
    value.profile.supports_subtractive_filter = true;
    value
}

fn microtonal_requirement() -> Requirement {
    let mut value = requirement(
        "microtonal-monophonic-notes",
        "monophonic notes requiring preservation of exact microtonal pitch",
        SoundSeam::MusicalEvents,
        1,
    );
    value.profile.accepts_microtonal_pitch = true;
    value
}

fn pcm_requirement() -> Requirement {
    let mut value = requirement(
        "pcm-s16le-48000-stereo-p256",
        "signed 16-bit 48 kHz stereo PCM in 256-frame blocks",
        SoundSeam::PcmPlayback,
        0,
    );
    value.profile.minimum_pitch_millihertz = 0;
    value.profile.maximum_pitch_millihertz = 0;
    value.profile.maximum_events_per_second = 0;
    value.profile.pcm = Some(PcmCompatibilityProfile {
        representation: conduit_audio::PcmSampleRepresentation::Signed16LittleEndian,
        sample_rate_hz: 48_000,
        layout: conduit_audio::PcmChannelLayout::StereoLeftRight,
        maximum_frames_per_block: 256,
        maximum_frame_bytes: 1_024,
    });
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verdict<'a>(report: &'a Report, realization: &str, requirement: &str) -> &'a Verdict {
        &report
            .cells
            .iter()
            .find(|cell| cell.realization == realization && cell.requirement == requirement)
            .unwrap()
            .verdict
    }

    #[test]
    fn exact_bounded_matrix_keeps_unsupported_as_a_result() {
        let report = build().unwrap();
        assert_eq!(report.realization_count, 6);
        assert_eq!(report.requirement_count, 7);
        assert_eq!(report.cell_count, 42);
        assert_eq!(
            report.recursive_realization.stages,
            [
                conduit_semantic_catalog::MUSIC_SYNTH_KIND,
                conduit_semantic_catalog::AUDIO_PLAY_KIND
            ]
        );
        assert!(matches!(
            verdict(&report, "pc-speaker", "tone"),
            Verdict::Compatible
        ));
        assert!(matches!(
            verdict(&report, "pc-speaker", "pcm-s16le-48000-stereo-p256"),
            Verdict::Unsupported {
                reason: IncompatibilityReason::WrongSemanticSeam
            }
        ));
        assert!(matches!(
            verdict(&report, "pete-create-oi", "simple-polyphonic-notes"),
            Verdict::Unsupported {
                reason: IncompatibilityReason::PolyphonyExceedsOffer
            }
        ));
        assert!(matches!(
            verdict(&report, "adlib-opl2", "expressive-notes"),
            Verdict::Unsupported {
                reason: IncompatibilityReason::VelocityUnsupported
            }
        ));
        assert!(matches!(
            verdict(&report, "midi-output", "expressive-notes"),
            Verdict::Compatible
        ));
        assert!(matches!(
            verdict(&report, "midi-output", "microtonal-monophonic-notes"),
            Verdict::Unsupported {
                reason: IncompatibilityReason::MicrotonalPitchUnsupported
            }
        ));
        assert!(matches!(
            verdict(
                &report,
                "software-synth",
                "expressive-subtractive-synthesis"
            ),
            Verdict::Compatible
        ));
        assert!(matches!(
            verdict(&report, "hosted-pcm", "pcm-s16le-48000-stereo-p256"),
            Verdict::Compatible
        ));
    }

    #[test]
    fn missing_physical_evidence_is_visible_but_does_not_change_compatibility() {
        let report = build().unwrap();
        let create = report
            .realizations
            .iter()
            .find(|realization| realization.name == "pete-create-oi")
            .unwrap();
        assert!(matches!(
            create.current_proof_class,
            ProofClass::DeterministicReference
        ));
        assert!(matches!(
            create.required_proof_class,
            ProofClass::PhysicalPeteCreateHil
        ));
        assert!(matches!(
            create.evidence_status,
            EvidenceStatus::MissingRequiredProof
        ));
    }
}
