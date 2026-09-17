//! Cross-Body semantic Journey verification for three independent biographies.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const CONTRACT_SCHEMA: &str = "conduit.evidence/semantic-journey-contract@2";
const TRACK_SCHEMA: &str = "conduit.evidence/body-journey-track@2";
const INDEX_SCHEMA: &str = "conduit.evidence/three-body-journey-index@2";
const MAXIMUM_DOCUMENT_BYTES: usize = 1024 * 1024;
const MAXIMUM_STEPS: usize = 32;
const MAXIMUM_HOSTS_PER_BODY: usize = 8;
const MAXIMUM_EVIDENCE_PER_STEP: usize = 8;
const REQUIRED_TRACKS: usize = 3;

#[path = "evidence_three_body_journey_page.rs"]
mod page;
#[path = "evidence_three_body_journey_support.rs"]
mod support;
use support::{
    read_bounded_json, valid_commit, valid_identity, valid_narrative, valid_sha256,
    validate_relative_path,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct JourneyContract {
    schema: String,
    journey_id: String,
    git_commit: String,
    steps: Vec<ContractStep>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ContractStep {
    step_id: String,
    #[serde(default)]
    milestone: Option<JourneyMilestone>,
    title: String,
    what_happened: String,
    what_conduit_established: String,
    concepts: Vec<String>,
    required_assertion: String,
    required_assertion_rung: EvidenceRung,
    allowed_dispositions: Vec<String>,
    required_evidence_classes: Vec<String>,
    required_provenance: Vec<ProvenanceField>,
    non_claims: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum JourneyMilestone {
    BodyAbsent,
    BootstrapStarted,
    BodyBorn,
    BodyWoken,
    FormUsed,
    BodyInspected,
    WorkloadRevised,
    HostAdded,
    FaultObserved,
    BodyRepaired,
    BodyContinued,
    BodyLulled,
    BodyFulfilled,
}

const REQUIRED_MILESTONES: [JourneyMilestone; 13] = [
    JourneyMilestone::BodyAbsent,
    JourneyMilestone::BootstrapStarted,
    JourneyMilestone::BodyBorn,
    JourneyMilestone::BodyWoken,
    JourneyMilestone::FormUsed,
    JourneyMilestone::BodyInspected,
    JourneyMilestone::WorkloadRevised,
    JourneyMilestone::HostAdded,
    JourneyMilestone::FaultObserved,
    JourneyMilestone::BodyRepaired,
    JourneyMilestone::BodyContinued,
    JourneyMilestone::BodyLulled,
    JourneyMilestone::BodyFulfilled,
];

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ProvenanceField {
    Body,
    Host,
    Plan,
    Play,
    Presentation,
    Manifestation,
    Line,
    Sign,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
enum EvidenceRung {
    SourceResource,
    StreamDisposition,
    DeterministicObservation,
    ModelDerivedInterpretation,
    CurrentExperience,
    PresenterPolicy,
    PurposeState,
    FulfillmentReadiness,
    BodyBiography,
    RuntimeReceipt,
    SemanticPresentation,
    GeneratedManifestation,
    AudioManifestation,
    LifecycleAction,
    FulfilledTransition,
    HumanAssessment,
}

impl EvidenceRung {
    const fn label(self) -> &'static str {
        match self {
            Self::SourceResource => "source-resource",
            Self::StreamDisposition => "stream-disposition",
            Self::DeterministicObservation => "deterministic-observation",
            Self::ModelDerivedInterpretation => "model-derived-interpretation",
            Self::CurrentExperience => "current-experience",
            Self::PresenterPolicy => "presenter-policy",
            Self::PurposeState => "purpose-state",
            Self::FulfillmentReadiness => "fulfillment-readiness",
            Self::BodyBiography => "body-biography",
            Self::RuntimeReceipt => "runtime-receipt",
            Self::SemanticPresentation => "semantic-presentation",
            Self::GeneratedManifestation => "generated-manifestation",
            Self::AudioManifestation => "audio-manifestation",
            Self::LifecycleAction => "lifecycle-action",
            Self::FulfilledTransition => "fulfilled-transition",
            Self::HumanAssessment => "human-assessment",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct BodyTrack {
    schema: String,
    journey_id: String,
    git_commit: String,
    track_id: String,
    embodiment: String,
    body_id: String,
    presenter_id: String,
    hosts: Vec<HostIdentity>,
    line_ids: Vec<String>,
    distributed_plan_ids: Vec<String>,
    steps: Vec<TrackStep>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct HostIdentity {
    host_id: String,
    boot_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TrackStep {
    step_id: String,
    assertion: String,
    disposition: String,
    provenance: StepProvenance,
    evidence: Vec<StepEvidence>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct StepProvenance {
    body_id: Option<String>,
    host_id: Option<String>,
    plan_id: Option<String>,
    play_id: Option<String>,
    presentation_id: Option<String>,
    manifestation_id: Option<String>,
    line_id: Option<String>,
    sign_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct StepEvidence {
    artifact_id: String,
    evidence_class: String,
    assertion_rung: EvidenceRung,
    documentary_description: String,
    path: PathBuf,
    sha256: String,
}

#[derive(Debug, Serialize)]
struct ThreeBodyJourneyIndex {
    schema: &'static str,
    disposition: &'static str,
    journey_id: String,
    git_commit: String,
    semantic_steps: Vec<ContractStep>,
    tracks: Vec<BodyTrack>,
}

pub(super) fn run(
    expected_git_commit: String,
    contract_path: PathBuf,
    track_paths: Vec<PathBuf>,
    output: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let contract: JourneyContract = read_bounded_json(&contract_path)?;
    let tracks = track_paths
        .iter()
        .map(|path| read_bounded_json(path))
        .collect::<Result<Vec<BodyTrack>, String>>()?;
    validate(&contract, &tracks, &expected_git_commit)?;
    for (track, source) in tracks.iter().zip(&track_paths) {
        verify_artifacts(track, source)?;
    }
    let index = ThreeBodyJourneyIndex {
        schema: INDEX_SCHEMA,
        disposition: "complete",
        journey_id: contract.journey_id,
        git_commit: contract.git_commit,
        semantic_steps: contract.steps,
        tracks,
    };
    publish(&index, &output)?;
    println!("THREE-BODY JOURNEY INDEX COMPLETE: {}", output.display());
    Ok(())
}

fn publish(index: &ThreeBodyJourneyIndex, output: &Path) -> Result<(), String> {
    let parent = output
        .parent()
        .ok_or("three-Body Journey output has no parent")?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("create three-Body Journey output: {error}"))?;
    let bytes = serde_json::to_vec_pretty(&index)
        .map_err(|error| format!("encode three-Body Journey index: {error}"))?;
    let page_path = output.with_extension("html");
    if output.exists() || page_path.exists() {
        return Err("three-Body Journey publication refuses overwrite".into());
    }
    let page = page::render(index);
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .and_then(|mut file| std::io::Write::write_all(&mut file, &bytes))
        .map_err(|error| format!("create three-Body Journey index: {error}"))?;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&page_path)
        .and_then(|mut file| std::io::Write::write_all(&mut file, page.as_bytes()))
        .map_err(|error| format!("create three-Body Journey page: {error}"))?;
    Ok(())
}

fn validate(
    contract: &JourneyContract,
    tracks: &[BodyTrack],
    expected_git_commit: &str,
) -> Result<(), String> {
    if !valid_commit(expected_git_commit) || contract.git_commit != expected_git_commit {
        return Err("semantic Journey does not match the expected exact commit".into());
    }
    if contract.schema != CONTRACT_SCHEMA
        || !valid_identity(&contract.journey_id)
        || !valid_commit(&contract.git_commit)
        || contract.steps.is_empty()
        || contract.steps.len() > MAXIMUM_STEPS
    {
        return Err("invalid bounded semantic Journey contract".into());
    }
    let mut step_ids = BTreeSet::new();
    let mut next_milestone = 0;
    for step in &contract.steps {
        if !step_ids.insert(step.step_id.as_str())
            || !valid_identity(&step.step_id)
            || !valid_narrative(&step.title)
            || !valid_narrative(&step.what_happened)
            || !valid_narrative(&step.what_conduit_established)
            || step.concepts.is_empty()
            || step.concepts.iter().any(|value| !valid_identity(value))
            || !valid_identity(&step.required_assertion)
            || step.allowed_dispositions.is_empty()
            || step.required_evidence_classes.is_empty()
            || step.non_claims.is_empty()
            || step
                .allowed_dispositions
                .iter()
                .any(|value| !valid_identity(value))
            || step
                .required_evidence_classes
                .iter()
                .any(|value| !valid_identity(value))
            || step.non_claims.iter().any(|value| !valid_identity(value))
            || step
                .required_provenance
                .iter()
                .collect::<BTreeSet<_>>()
                .len()
                != step.required_provenance.len()
        {
            return Err(format!("invalid semantic Journey step '{}'", step.step_id));
        }
        if let Some(milestone) = step.milestone {
            if REQUIRED_MILESTONES.get(next_milestone) != Some(&milestone) {
                return Err(format!(
                    "semantic Journey milestone {:?} is duplicated or out of order",
                    milestone
                ));
            }
            next_milestone += 1;
        }
    }
    if next_milestone != REQUIRED_MILESTONES.len() {
        return Err("semantic Journey omits required lifecycle milestones".into());
    }
    if tracks.len() != REQUIRED_TRACKS {
        return Err(format!(
            "exactly {REQUIRED_TRACKS} Body tracks are required"
        ));
    }
    let mut track_ids = BTreeSet::new();
    let mut body_ids = BTreeSet::new();
    let mut embodiments = BTreeSet::new();
    let mut presenter_ids = BTreeSet::new();
    let mut global_host_ids = BTreeSet::new();
    let mut global_boot_ids = BTreeSet::new();
    let mut plans = BTreeMap::new();
    let mut plays = BTreeMap::new();
    let mut presentations = BTreeMap::new();
    let mut manifestations = BTreeMap::new();
    let mut has_distributed_body = false;
    for track in tracks {
        if track.schema != TRACK_SCHEMA
            || track.journey_id != contract.journey_id
            || track.git_commit != contract.git_commit
            || !track_ids.insert(track.track_id.as_str())
            || !body_ids.insert(track.body_id.as_str())
            || !embodiments.insert(track.embodiment.as_str())
            || !presenter_ids.insert(track.presenter_id.as_str())
            || !valid_identity(&track.track_id)
            || !valid_identity(&track.embodiment)
            || !valid_identity(&track.body_id)
            || !valid_identity(&track.presenter_id)
            || track.hosts.is_empty()
            || track.hosts.len() > MAXIMUM_HOSTS_PER_BODY
            || track.steps.len() != contract.steps.len()
        {
            return Err(format!("invalid Body track '{}'", track.track_id));
        }
        let mut host_ids = BTreeSet::new();
        for host in &track.hosts {
            if !host_ids.insert(host.host_id.as_str())
                || !global_host_ids.insert(host.host_id.as_str())
                || !global_boot_ids.insert(host.boot_id.as_str())
                || !valid_identity(&host.host_id)
                || !valid_identity(&host.boot_id)
            {
                return Err(format!("invalid Host set for '{}'", track.track_id));
            }
        }
        if track.line_ids.iter().any(|line| !valid_identity(line))
            || track.line_ids.iter().collect::<BTreeSet<_>>().len() != track.line_ids.len()
            || track
                .distributed_plan_ids
                .iter()
                .any(|plan| !valid_identity(plan))
            || track
                .distributed_plan_ids
                .iter()
                .collect::<BTreeSet<_>>()
                .len()
                != track.distributed_plan_ids.len()
        {
            return Err(format!(
                "invalid distributed truth for '{}'",
                track.track_id
            ));
        }
        if track.hosts.len() > 1
            && !track.line_ids.is_empty()
            && !track.distributed_plan_ids.is_empty()
            && track.steps.iter().any(|step| {
                step.provenance
                    .line_id
                    .as_ref()
                    .is_some_and(|line| track.line_ids.contains(line))
                    && step
                        .provenance
                        .plan_id
                        .as_ref()
                        .is_some_and(|plan| track.distributed_plan_ids.contains(plan))
            })
        {
            has_distributed_body = true;
        }
        for (required, observed) in contract.steps.iter().zip(&track.steps) {
            validate_step(track, required, observed)?;
            claim_identity(
                &mut plans,
                observed.provenance.plan_id.as_deref(),
                &track.body_id,
            )?;
            claim_identity(
                &mut plays,
                observed.provenance.play_id.as_deref(),
                &track.body_id,
            )?;
            claim_identity(
                &mut presentations,
                observed.provenance.presentation_id.as_deref(),
                &track.body_id,
            )?;
            claim_identity(
                &mut manifestations,
                observed.provenance.manifestation_id.as_deref(),
                &track.body_id,
            )?;
        }
    }
    if !has_distributed_body {
        return Err(
            "at least one Body must retain multi-Host, Line, and distributed Plan truth".into(),
        );
    }
    Ok(())
}

fn validate_step(
    track: &BodyTrack,
    required: &ContractStep,
    observed: &TrackStep,
) -> Result<(), String> {
    if observed.step_id != required.step_id
        || observed.assertion != required.required_assertion
        || !required
            .allowed_dispositions
            .contains(&observed.disposition)
        || observed.evidence.is_empty()
        || observed.evidence.len() > MAXIMUM_EVIDENCE_PER_STEP
    {
        return Err(format!(
            "{} mismatches step {}",
            track.track_id, required.step_id
        ));
    }
    let classes = observed
        .evidence
        .iter()
        .map(|evidence| evidence.evidence_class.as_str())
        .collect::<BTreeSet<_>>();
    if required
        .required_evidence_classes
        .iter()
        .any(|class| !classes.contains(class.as_str()))
    {
        return Err(format!(
            "{} lacks evidence for {}",
            track.track_id, required.step_id
        ));
    }
    if !observed
        .evidence
        .iter()
        .any(|evidence| evidence.assertion_rung == required.required_assertion_rung)
    {
        return Err(format!(
            "{} lacks authoritative {:?} evidence at {}",
            track.track_id, required.required_assertion_rung, required.step_id
        ));
    }
    for field in &required.required_provenance {
        let value = match field {
            ProvenanceField::Body => observed.provenance.body_id.as_deref(),
            ProvenanceField::Host => observed.provenance.host_id.as_deref(),
            ProvenanceField::Plan => observed.provenance.plan_id.as_deref(),
            ProvenanceField::Play => observed.provenance.play_id.as_deref(),
            ProvenanceField::Presentation => observed.provenance.presentation_id.as_deref(),
            ProvenanceField::Manifestation => observed.provenance.manifestation_id.as_deref(),
            ProvenanceField::Line => observed.provenance.line_id.as_deref(),
            ProvenanceField::Sign => observed.provenance.sign_id.as_deref(),
        };
        if value.is_none_or(|value| !valid_identity(value)) {
            return Err(format!(
                "{} lacks {:?} at {}",
                track.track_id, field, required.step_id
            ));
        }
    }
    if observed
        .provenance
        .body_id
        .as_deref()
        .is_some_and(|body| body != track.body_id)
    {
        return Err(format!("{} step cites another Body", track.track_id));
    }
    if observed.provenance.host_id.as_deref().is_some_and(|host| {
        !track
            .hosts
            .iter()
            .any(|candidate| candidate.host_id == host)
    }) {
        return Err(format!("{} step cites another Host", track.track_id));
    }
    if observed
        .provenance
        .line_id
        .as_deref()
        .is_some_and(|line| !track.line_ids.iter().any(|candidate| candidate == line))
    {
        return Err(format!("{} step cites an unretained Line", track.track_id));
    }
    Ok(())
}

fn claim_identity(
    owners: &mut BTreeMap<String, String>,
    identity: Option<&str>,
    body_id: &str,
) -> Result<(), String> {
    let Some(identity) = identity else {
        return Ok(());
    };
    if owners
        .insert(identity.to_owned(), body_id.to_owned())
        .is_some_and(|owner| owner != body_id)
    {
        return Err("Body tracks collapsed exact runtime or presentation identity".into());
    }
    Ok(())
}

fn verify_artifacts(track: &BodyTrack, source: &Path) -> Result<(), String> {
    let root = source
        .parent()
        .ok_or("Body track manifest has no parent")?
        .canonicalize()
        .map_err(|error| format!("resolve Body track root: {error}"))?;
    let mut artifact_ids = BTreeSet::new();
    let mut artifact_paths = BTreeSet::new();
    for evidence in track.steps.iter().flat_map(|step| &step.evidence) {
        validate_relative_path(&evidence.path)?;
        if !artifact_ids.insert(evidence.artifact_id.as_str())
            || !artifact_paths.insert(&evidence.path)
            || !valid_identity(&evidence.artifact_id)
            || !valid_identity(&evidence.evidence_class)
            || !valid_narrative(&evidence.documentary_description)
            || !valid_sha256(&evidence.sha256)
        {
            return Err(format!(
                "{} has duplicate or invalid artifact evidence",
                track.track_id
            ));
        }
        let candidate = root.join(&evidence.path);
        let metadata = std::fs::symlink_metadata(&candidate)
            .map_err(|error| format!("inspect {}: {error}", evidence.path.display()))?;
        if !metadata.file_type().is_file() {
            return Err(format!("{} artifact is not a regular file", track.track_id));
        }
        let resolved = candidate
            .canonicalize()
            .map_err(|error| format!("resolve {}: {error}", evidence.path.display()))?;
        if !resolved.starts_with(&root) {
            return Err(format!(
                "{} artifact escaped its track root",
                track.track_id
            ));
        }
        let bytes = std::fs::read(&resolved)
            .map_err(|error| format!("read {}: {error}", evidence.path.display()))?;
        if bytes.is_empty() || bytes.len() > MAXIMUM_DOCUMENT_BYTES {
            return Err(format!(
                "{} artifact violates its byte bound",
                track.track_id
            ));
        }
        if format!("sha256:{:x}", Sha256::digest(&bytes)) != evidence.sha256 {
            return Err(format!("{} artifact digest changed", evidence.artifact_id));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "evidence_three_body_journey_tests.rs"]
mod tests;
