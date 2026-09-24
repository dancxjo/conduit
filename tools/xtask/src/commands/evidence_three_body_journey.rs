//! Cross-Body semantic Journey verification for three independent biographies.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const CONTRACT_SCHEMA: &str = "conduit.evidence/semantic-journey-contract@2";
const TRACK_SCHEMA: &str = "conduit.evidence/body-journey-track@2";
const INDEX_SCHEMA: &str = "conduit.evidence/three-body-journey-index@3";
const MAXIMUM_DOCUMENT_BYTES: usize = 1024 * 1024;
const MAXIMUM_MEDIA_BYTES: u64 = 64 * 1024 * 1024;
const MAXIMUM_STEPS: usize = 32;
const MAXIMUM_HOSTS_PER_BODY: usize = 8;
const MAXIMUM_EVIDENCE_PER_STEP: usize = 8;
const REQUIRED_TRACKS: usize = 3;

#[path = "evidence_three_body_journey_artifacts.rs"]
mod artifacts;
use artifacts::verify_artifacts;

#[path = "evidence_three_body_journey_contract.rs"]
mod contract;
#[path = "evidence_three_body_journey_page.rs"]
mod page;
#[path = "evidence_three_body_journey_support.rs"]
mod support;
use support::{
    read_bounded_json, valid_commit, valid_identity, valid_narrative, valid_sha256,
    validate_relative_path,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
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

impl JourneyMilestone {
    const fn required_assertion(self) -> &'static str {
        match self {
            Self::BodyAbsent => "body-absent",
            Self::BootstrapStarted => "bootstrap-started",
            Self::BodyBorn => "body-born",
            Self::BodyWoken => "body-awake",
            Self::FormUsed => "standing-form-used",
            Self::BodyInspected => "body-inspected",
            Self::WorkloadRevised => "workload-revised",
            Self::HostAdded => "host-added",
            Self::FaultObserved => "fault-observed",
            Self::BodyRepaired => "body-repaired",
            Self::BodyContinued => "body-long-running",
            Self::BodyLulled => "body-lulled",
            Self::BodyFulfilled => "body-fulfilled",
        }
    }
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

pub(super) fn write_contract(commit: String, output: PathBuf) -> Result<(), String> {
    contract::write(commit, output)
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ProvenanceField {
    Body,
    Host,
    Boot,
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
    boot_id: Option<String>,
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

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ThreeBodyJourneyIndex {
    schema: String,
    disposition: String,
    journey_id: String,
    git_commit: String,
    semantic_steps: Vec<ContractStep>,
    tracks: Vec<BodyTrack>,
    recorded_generative: Option<BodyTrack>,
}

pub(super) fn run(
    expected_git_commit: String,
    contract_path: PathBuf,
    track_paths: Vec<PathBuf>,
    recorded_path: Option<PathBuf>,
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
    let recorded_generative = recorded_path
        .as_deref()
        .map(|source| artifacts::read_recording(source, &tracks, &track_paths))
        .transpose()?;
    artifacts::require_documentary(&tracks, recorded_generative.as_ref())?;
    let index = ThreeBodyJourneyIndex {
        schema: INDEX_SCHEMA.into(),
        disposition: "complete".into(),
        journey_id: contract.journey_id,
        git_commit: contract.git_commit,
        semantic_steps: contract.steps,
        tracks,
        recorded_generative,
    };
    publish(&index, &output)?;
    println!("THREE-BODY JOURNEY INDEX COMPLETE: {}", output.display());
    Ok(())
}

pub(super) fn stage(
    publication_root: PathBuf,
    site_root: PathBuf,
    expected_git_commit: String,
) -> Result<(), String> {
    if !valid_commit(&expected_git_commit) {
        return Err("three-Body Journey staging requires an exact commit".into());
    }
    let index_path = publication_root.join("index.json");
    let index: ThreeBodyJourneyIndex = read_bounded_json(&index_path)?;
    if index.schema != INDEX_SCHEMA
        || index.disposition != "complete"
        || index.git_commit != expected_git_commit
    {
        return Err("three-Body Journey index is malformed or stale".into());
    }
    let contract = contract::canonical(&expected_git_commit);
    artifacts::require_documentary(&index.tracks, index.recorded_generative.as_ref())?;
    validate(&contract, &index.tracks, &expected_git_commit)?;
    if index.journey_id != contract.journey_id || index.semantic_steps.len() != contract.steps.len()
    {
        return Err("three-Body Journey index diverges from the canonical contract".into());
    }
    for track in &index.tracks {
        let track_path = publication_root.join(&track.track_id).join("track.json");
        let retained: BodyTrack = read_bounded_json(&track_path)?;
        if retained.track_id != track.track_id || retained.body_id != track.body_id {
            return Err(format!(
                "retained track '{}' diverges from its index",
                track.track_id
            ));
        }
        verify_artifacts(&retained, &track_path)?;
    }
    if let Some(recording) = &index.recorded_generative {
        let sources: Vec<_> = index
            .tracks
            .iter()
            .map(|track| publication_root.join(&track.track_id).join("track.json"))
            .collect();
        let retained = artifacts::read_recording(
            &publication_root.join("live-conformance/track.json"),
            &index.tracks,
            &sources,
        )?;
        if serde_json::to_value(&retained).map_err(|e| e.to_string())?
            != serde_json::to_value(recording).map_err(|e| e.to_string())?
        {
            return Err("retained live recording diverges from its index".into());
        }
    }
    if !publication_root.join("index.html").is_file() {
        return Err("three-Body Journey publication lacks index.html".into());
    }
    let journeys = site_root.join("journeys");
    let gallery_index = journeys.join("index.html");
    let gallery_json = journeys.join("gallery.json");
    if !gallery_index.is_file() || !gallery_json.is_file() {
        return Err("Pages root lacks a built journeys gallery".into());
    }
    let gallery: serde_json::Value = read_bounded_json(&gallery_json)?;
    if gallery
        .get("current_commit")
        .and_then(serde_json::Value::as_str)
        != Some(expected_git_commit.as_str())
    {
        return Err("journeys gallery belongs to a different commit".into());
    }
    let current = journeys.join("current/three-bodies");
    let historic = journeys
        .join("commits")
        .join(&expected_git_commit)
        .join("three-bodies");
    if current.exists() || historic.exists() {
        return Err("three-Body Journey staging refuses overwrite".into());
    }
    copy_publication(&publication_root, &current)?;
    copy_publication(&publication_root, &historic)?;
    let mut html = std::fs::read_to_string(&gallery_index)
        .map_err(|error| format!("read journeys gallery entrance: {error}"))?;
    let start_marker = "<!-- conduit-three-body-flagship@2 -->";
    let end_marker = "<!-- conduit-three-body-flagship:end -->";
    let insertion = "<!-- conduit-three-body-flagship@2 --><section class=\"flagship admitted\" aria-labelledby=\"flagship-title\"><div><p class=\"eyebrow\">Accepted three-Body Journey</p><h2 id=\"flagship-title\">The same meaning. Three radically different lives.</h2><p class=\"lede\">Three independently born Bodies traverse one shared semantic contract, each retaining its own machinery, identity, biography, and evidence.</p><p><a class=\"primary\" href=\"current/three-bodies/\">Enter the Journey</a></p></div><div class=\"body-lanes\"><article><b>A</b><h3>ConduitOS</h3><p>Native, freestanding, graphical</p></article><article><b>B</b><h3>Browser</h3><p>DOM, WASM, interactive</p></article><article><b>C</b><h3>Distributed</h3><p>Multi-Host, Line, generative</p></article></div><ol class=\"semantic-spine\"><li>Birth</li><li>Wake</li><li>Use</li><li>Inspect</li><li>Change</li><li>Replan</li><li>Add Host</li><li>Fault</li><li>Repair</li><li>Continue</li><li>Lull</li><li>Fulfill</li></ol><p class=\"boundary\"><strong>What this establishes:</strong> semantic portability with independent realization identities. It does not claim equal pixels, prose, timing, placement, or Body identity.</p></section><!-- conduit-three-body-flagship:end -->";
    let insertion = insertion.replace(
        "<ol class=\"semantic-spine\"><li>Birth</li><li>Wake</li><li>Use</li><li>Inspect</li><li>Change</li><li>Replan</li><li>Add Host</li><li>Fault</li><li>Repair</li><li>Continue</li><li>Lull</li><li>Fulfill</li></ol>",
        "<ol class=\"semantic-spine\"><li>Before</li><li>Bootstrap</li><li>Birth</li><li>Wake</li><li>Use</li><li>Inspect</li><li>Change / replan</li><li>Add Host</li><li>Fault</li><li>Repair</li><li>Continue</li><li>Lull</li><li>Fulfill</li></ol>",
    );
    let start = html
        .find(start_marker)
        .ok_or("journeys gallery entrance lacks its flagship marker")?;
    let end = html[start..]
        .find(end_marker)
        .map(|offset| start + offset + end_marker.len())
        .ok_or("journeys gallery entrance lacks its flagship end marker")?;
    html.replace_range(start..end, &insertion);
    let history_marker = format!("<li><code>{expected_git_commit}</code>");
    let history_position = html
        .find(&history_marker)
        .and_then(|start| html[start..].find("</li>").map(|offset| start + offset))
        .ok_or("journeys gallery entrance lacks exact-commit history")?;
    html.insert_str(
        history_position,
        &format!(" · <a href=\"commits/{expected_git_commit}/three-bodies/\">Three Bodies</a>"),
    );
    std::fs::write(&gallery_index, html)
        .map_err(|error| format!("write journeys gallery entrance: {error}"))?;
    println!("STAGED three-Body Journey for {expected_git_commit}");
    Ok(())
}

fn copy_publication(source: &Path, destination: &Path) -> Result<(), String> {
    std::fs::create_dir_all(destination)
        .map_err(|error| format!("create three-Body Journey destination: {error}"))?;
    for entry in std::fs::read_dir(source)
        .map_err(|error| format!("read three-Body Journey publication: {error}"))?
    {
        let entry = entry.map_err(|error| format!("read publication entry: {error}"))?;
        let kind = entry
            .file_type()
            .map_err(|error| format!("inspect publication entry: {error}"))?;
        let target = destination.join(entry.file_name());
        if kind.is_symlink() {
            return Err("three-Body Journey publication refuses symlinks".into());
        } else if kind.is_dir() {
            copy_publication(&entry.path(), &target)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), target)
                .map_err(|error| format!("copy three-Body Journey artifact: {error}"))?;
        } else {
            return Err("three-Body Journey publication refuses special files".into());
        }
    }
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
    validate_contract(contract, expected_git_commit)?;
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
            "at least one body must retain multi-host, Line, and distributed Plan truth".into(),
        );
    }
    Ok(())
}

fn validate_contract(contract: &JourneyContract, expected_git_commit: &str) -> Result<(), String> {
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
            if step.required_assertion != milestone.required_assertion() {
                return Err(format!(
                    "semantic Journey milestone {:?} has a noncanonical assertion",
                    milestone
                ));
            }
            next_milestone += 1;
        }
    }
    if next_milestone != REQUIRED_MILESTONES.len() {
        return Err("semantic Journey omits required lifecycle milestones".into());
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
            ProvenanceField::Boot => observed.provenance.boot_id.as_deref(),
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
        return Err(format!("{} step cites another body", track.track_id));
    }
    if observed.provenance.host_id.as_deref().is_some_and(|host| {
        !track
            .hosts
            .iter()
            .any(|candidate| candidate.host_id == host)
    }) {
        return Err(format!("{} step cites another host", track.track_id));
    }
    if let Some(boot) = observed.provenance.boot_id.as_deref() {
        let matching_host = observed.provenance.host_id.as_deref().is_some_and(|host| {
            track
                .hosts
                .iter()
                .any(|candidate| candidate.host_id == host && candidate.boot_id == boot)
        });
        if !matching_host {
            return Err(format!(
                "{} step cites a Boot outside its exact host pair",
                track.track_id
            ));
        }
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

#[cfg(test)]
#[path = "evidence_three_body_journey_tests.rs"]
mod tests;
