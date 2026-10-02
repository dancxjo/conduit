//! Cross-Body semantic Journey verification for three independent biographies.

use crate::three_body_actions::{JourneyActionKind, REQUIRED_ACTIONS};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const CONTRACT_SCHEMA: &str = "conduit.evidence/semantic-journey-contract@4";
const TRACK_SCHEMA: &str = "conduit.evidence/body-journey-track@6";
const INDEX_SCHEMA: &str = "conduit.evidence/three-body-journey-index@7";
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
    actions: Vec<ContractAction>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ContractAction {
    action_id: String,
    action: JourneyActionKind,
    title: String,
    what_happened: String,
    what_conduit_established: String,
    concepts: Vec<String>,
    required_receipt_assertions: Vec<String>,
    non_claims: Vec<String>,
}

pub(super) fn write_contract(commit: String, output: PathBuf) -> Result<(), String> {
    contract::write(commit, output)
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
enum EvidenceRung {
    SourceResource,
    StreamDisposition,
    DeterministicObservation,
    ModelDerivedInterpretation,
    CurrentExperience,
    MaskPolicy,
    PurposeState,
    FulfillmentReadiness,
    BodyBiography,
    RuntimeReceipt,
    SemanticFace,
    GeneratedShow,
    AudioShow,
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
            Self::MaskPolicy => "mask-policy",
            Self::PurposeState => "purpose-state",
            Self::FulfillmentReadiness => "fulfillment-readiness",
            Self::BodyBiography => "body-biography",
            Self::RuntimeReceipt => "runtime-receipt",
            Self::SemanticFace => "semantic-face",
            Self::GeneratedShow => "generated-show",
            Self::AudioShow => "audio-show",
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
    mask_plot_id: String,
    construction: Vec<ConstructionTruth>,
    hosts: Vec<HostIdentity>,
    line_ids: Vec<String>,
    distributed_plan_ids: Vec<String>,
    receipts: Vec<TrackStep>,
    actions: Vec<TrackActionObservation>,
    #[serde(default)]
    mask_actions: Vec<MaskActionObservation>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ConstructionTruth {
    host_id: String,
    profile: ConstructionStage,
    build: ConstructionStage,
    image: ConstructionStage,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "disposition", rename_all = "kebab-case", deny_unknown_fields)]
enum ConstructionStage {
    Exact { identity: String },
    Omitted { reason: String },
}

impl ConstructionTruth {
    fn validate(&self) -> bool {
        let stages = [&self.profile, &self.build, &self.image];
        if stages.iter().any(|stage| match stage {
            ConstructionStage::Exact { identity } => !valid_identity(identity),
            ConstructionStage::Omitted { reason } => !valid_narrative(reason),
        }) {
            return false;
        }
        // An exact downstream artifact cannot truthfully exist when its prerequisite
        // construction stage was omitted from this journey.
        !matches!(
            (&self.profile, &self.build, &self.image),
            (
                ConstructionStage::Omitted { .. },
                ConstructionStage::Exact { .. },
                _
            ) | (
                _,
                ConstructionStage::Omitted { .. },
                ConstructionStage::Exact { .. }
            )
        )
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct TrackActionObservation {
    action_id: String,
    concrete_event: String,
    receipt_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct MaskActionObservation {
    action_id: String,
    concrete_event: String,
    face_id: String,
    selected_mask_plot_id: Option<String>,
    plan_id: String,
    selected_route_id: Option<String>,
    show_id: Option<String>,
    receipt_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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
    face_id: Option<String>,
    show_id: Option<String>,
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
    actions: Vec<JourneyAction>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct JourneyAction {
    action: ContractAction,
    bodies: Vec<JourneyBodyCell>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct JourneyBodyCell {
    track_id: String,
    embodiment: String,
    body_id: String,
    mask_plot_id: String,
    construction: Vec<ConstructionTruth>,
    hosts: Vec<HostIdentity>,
    line_ids: Vec<String>,
    distributed_plan_ids: Vec<String>,
    observed: TrackActionObservation,
    receipts: Vec<TrackStep>,
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
    let index = assemble_index(contract, tracks)?;
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
    let projected_tracks = validate_index(&index, &contract)?;
    let mut tracks = Vec::with_capacity(projected_tracks.len());
    for projected in &projected_tracks {
        let track_path = publication_root
            .join(&projected.track_id)
            .join("track.json");
        let retained: BodyTrack = read_bounded_json(&track_path)?;
        if retained.actions != projected.actions
            || projected.receipts.iter().any(|receipt| {
                !retained.receipts.iter().any(|candidate| {
                    serde_json::to_value(candidate).ok() == serde_json::to_value(receipt).ok()
                })
            })
        {
            return Err(format!(
                "retained track '{}' diverges from its index",
                projected.track_id
            ));
        }
        verify_artifacts(&retained, &track_path)?;
        tracks.push(retained);
    }
    validate(&contract, &tracks, &expected_git_commit)?;
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
    let insertion = "<!-- conduit-three-body-flagship@2 --><section class=\"flagship admitted\" aria-labelledby=\"flagship-title\"><div><p class=\"eyebrow\">Accepted three-Body Journey</p><h2 id=\"flagship-title\">The same meaning. Three radically different lives.</h2><p class=\"lede\">Three independently born Bodies traverse one shared semantic contract, each retaining its own machinery, identity, biography, and evidence.</p><p><a class=\"primary\" href=\"current/three-bodies/\">Enter the Journey</a></p></div><div class=\"body-lanes\"><article><b>A</b><h3>ConduitOS</h3><p>Native, freestanding, graphical</p></article><article><b>B</b><h3>Browser</h3><p>DOM, WASM, interactive</p></article><article><b>C</b><h3>Screen-free</h3><p>Spoken, multi-Host, generative</p></article></div><ol class=\"semantic-spine\"><li>Bootstrap</li><li>Birth</li><li>Useful work</li><li>Inspect initial Show</li><li>Wear alternate Mask</li><li>Prefer alternate Mask</li><li>Withdraw selected route</li><li>Inspect no Show</li><li>Add Face Host</li><li>Admit replacement Plan</li><li>Inspect replanned Show</li><li>Doff alternate Mask</li><li>Inspect restored Show</li><li>Break and recover</li><li>Rest and finish</li></ol><p class=\"boundary\"><strong>What this establishes:</strong> semantic portability with independent realization identities. It does not claim equal pixels, prose, timing, placement, or Body identity.</p></section><!-- conduit-three-body-flagship:end -->";
    let start = html
        .find(start_marker)
        .ok_or("journeys gallery entrance lacks its flagship marker")?;
    let end = html[start..]
        .find(end_marker)
        .map(|offset| start + offset + end_marker.len())
        .ok_or("journeys gallery entrance lacks its flagship end marker")?;
    html.replace_range(start..end, insertion);
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

fn assemble_index(
    contract: JourneyContract,
    tracks: Vec<BodyTrack>,
) -> Result<ThreeBodyJourneyIndex, String> {
    let track_actions = tracks
        .iter()
        .map(ordered_actions)
        .collect::<Result<Vec<_>, _>>()?;
    let actions = contract
        .actions
        .iter()
        .map(|action| {
            let bodies = tracks
                .iter()
                .zip(&track_actions)
                .map(|(track, actions)| {
                    let observed = actions[action.action_id.as_str()];
                    JourneyBodyCell {
                        track_id: track.track_id.clone(),
                        embodiment: track.embodiment.clone(),
                        body_id: track.body_id.clone(),
                        mask_plot_id: track.mask_plot_id.clone(),
                        construction: track.construction.clone(),
                        hosts: track.hosts.clone(),
                        line_ids: track.line_ids.clone(),
                        distributed_plan_ids: track.distributed_plan_ids.clone(),
                        observed: observed.clone(),
                        receipts: select_receipts(track, &observed.receipt_ids),
                    }
                })
                .collect();
            JourneyAction {
                action: action.clone(),
                bodies,
            }
        })
        .collect();
    Ok(ThreeBodyJourneyIndex {
        schema: INDEX_SCHEMA.into(),
        disposition: "complete".into(),
        journey_id: contract.journey_id,
        git_commit: contract.git_commit,
        actions,
    })
}

fn ordered_actions(track: &BodyTrack) -> Result<BTreeMap<&str, &TrackActionObservation>, String> {
    let mut by_id = BTreeMap::new();
    for action in &track.actions {
        if by_id.insert(action.action_id.as_str(), action).is_some() {
            return Err(format!(
                "{} has duplicate action {}",
                track.track_id, action.action_id
            ));
        }
    }
    Ok(by_id)
}

fn select_receipts(track: &BodyTrack, ids: &[String]) -> Vec<TrackStep> {
    ids.iter()
        .filter_map(|id| {
            track
                .receipts
                .iter()
                .find(|receipt| &receipt.step_id == id)
                .cloned()
        })
        .collect()
}

fn validate_index(
    index: &ThreeBodyJourneyIndex,
    contract: &JourneyContract,
) -> Result<Vec<BodyTrack>, String> {
    if index.journey_id != contract.journey_id || index.actions.len() != contract.actions.len() {
        return Err("three-Body Journey index diverges from the canonical contract".into());
    }
    for (position, action) in index.actions.iter().enumerate() {
        let body_order = action
            .bodies
            .iter()
            .map(|body| body.track_id.as_str())
            .collect::<Vec<_>>();
        let expected_body_order = index.actions[0]
            .bodies
            .iter()
            .map(|body| body.track_id.as_str())
            .collect::<Vec<_>>();
        if action.action.action_id != contract.actions[position].action_id
            || serde_json::to_value(&action.action).map_err(|e| e.to_string())?
                != serde_json::to_value(&contract.actions[position]).map_err(|e| e.to_string())?
            || action.bodies.len() != REQUIRED_TRACKS
            || body_order != expected_body_order
        {
            return Err("three-Body Journey action is malformed or out of order".into());
        }
    }
    let first = index
        .actions
        .first()
        .ok_or("three-Body Journey index has no actions")?;
    let mut tracks = Vec::with_capacity(REQUIRED_TRACKS);
    for basis in &first.bodies {
        let mut actions = Vec::with_capacity(index.actions.len());
        let mut receipts = Vec::new();
        for action in &index.actions {
            let cell = action
                .bodies
                .iter()
                .find(|cell| cell.track_id == basis.track_id)
                .ok_or("three-Body Journey action omitted a Body cell")?;
            if cell.embodiment != basis.embodiment
                || cell.body_id != basis.body_id
                || cell.mask_plot_id != basis.mask_plot_id
                || cell.construction != basis.construction
                || cell.hosts != basis.hosts
                || cell.line_ids != basis.line_ids
                || cell.distributed_plan_ids != basis.distributed_plan_ids
                || cell.observed.action_id != action.action.action_id
            {
                return Err("three-Body Journey Body cell changed identity or action".into());
            }
            actions.push(cell.observed.clone());
            receipts.extend(cell.receipts.clone());
        }
        tracks.push(BodyTrack {
            schema: TRACK_SCHEMA.into(),
            journey_id: index.journey_id.clone(),
            git_commit: index.git_commit.clone(),
            track_id: basis.track_id.clone(),
            embodiment: basis.embodiment.clone(),
            body_id: basis.body_id.clone(),
            mask_plot_id: basis.mask_plot_id.clone(),
            construction: basis.construction.clone(),
            hosts: basis.hosts.clone(),
            line_ids: basis.line_ids.clone(),
            distributed_plan_ids: basis.distributed_plan_ids.clone(),
            receipts,
            actions,
            mask_actions: Vec::new(),
        });
    }
    Ok(tracks)
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
    let mut mask_plot_ids = BTreeSet::new();
    let mut global_host_ids = BTreeSet::new();
    let mut global_boot_ids = BTreeSet::new();
    let mut plans = BTreeMap::new();
    let mut plays = BTreeMap::new();
    let mut faces = BTreeMap::new();
    let mut shows = BTreeMap::new();
    let mut has_distributed_body = false;
    for track in tracks {
        if track.schema != TRACK_SCHEMA
            || track.journey_id != contract.journey_id
            || track.git_commit != contract.git_commit
            || !track_ids.insert(track.track_id.as_str())
            || !body_ids.insert(track.body_id.as_str())
            || !embodiments.insert(track.embodiment.as_str())
            || !mask_plot_ids.insert(track.mask_plot_id.as_str())
            || !valid_identity(&track.track_id)
            || !valid_identity(&track.embodiment)
            || !valid_identity(&track.body_id)
            || !valid_identity(&track.mask_plot_id)
            || track.construction.len() != track.hosts.len()
            || track.construction.iter().any(|truth| !truth.validate())
            || track.hosts.is_empty()
            || track.hosts.len() > MAXIMUM_HOSTS_PER_BODY
            || track.receipts.is_empty()
            || track.receipts.len() > MAXIMUM_STEPS
            || track.actions.len() != contract.actions.len()
            || track.mask_actions.len() != conduit_presentation::MASK_JOURNEY_ACTIONS.len()
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
        let construction_hosts = track
            .construction
            .iter()
            .map(|truth| truth.host_id.as_str())
            .collect::<BTreeSet<_>>();
        if construction_hosts.len() != track.construction.len() || construction_hosts != host_ids {
            return Err(format!(
                "construction truth does not cover every Host for '{}'",
                track.track_id
            ));
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
            && track.receipts.iter().any(|step| {
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
        let actions = ordered_actions(track)?;
        validate_mask_journey(track)?;
        let expected_action_ids = contract
            .actions
            .iter()
            .map(|action| action.action_id.as_str())
            .collect::<Vec<_>>();
        let observed_action_ids = track
            .actions
            .iter()
            .map(|action| action.action_id.as_str())
            .collect::<Vec<_>>();
        if observed_action_ids != expected_action_ids {
            return Err(format!(
                "{} has a missing, extra, or shuffled public action",
                track.track_id
            ));
        }
        let mut claimed_receipts = BTreeSet::new();
        for required in &contract.actions {
            let observed = actions[required.action_id.as_str()];
            if !valid_narrative(&observed.concrete_event)
                || observed.receipt_ids.is_empty()
                || observed.receipt_ids.iter().collect::<BTreeSet<_>>().len()
                    != observed.receipt_ids.len()
            {
                return Err(format!(
                    "{} has an invalid observation for {}",
                    track.track_id, required.action_id
                ));
            }
            let assertions = observed
                .receipt_ids
                .iter()
                .map(|id| {
                    track
                        .receipts
                        .iter()
                        .find(|receipt| &receipt.step_id == id)
                        .map(|receipt| receipt.assertion.as_str())
                        .ok_or_else(|| {
                            format!(
                                "{} action {} cites an absent receipt",
                                track.track_id, required.action_id
                            )
                        })
                })
                .collect::<Result<BTreeSet<_>, _>>()?;
            if required
                .required_receipt_assertions
                .iter()
                .any(|assertion| !assertions.contains(assertion.as_str()))
            {
                return Err(format!(
                    "{} action {} lacks required semantic receipts",
                    track.track_id, required.action_id
                ));
            }
            claimed_receipts.extend(observed.receipt_ids.iter().map(String::as_str));
        }
        for observed in &track.receipts {
            validate_receipt(track, observed)?;
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
                &mut faces,
                observed.provenance.face_id.as_deref(),
                &track.body_id,
            )?;
            claim_identity(
                &mut shows,
                observed.provenance.show_id.as_deref(),
                &track.body_id,
            )?;
        }
        if claimed_receipts.len() >= track.receipts.len() {
            return Err(format!(
                "{} does not retain richer detailed receipts beyond its public actions",
                track.track_id
            ));
        }
    }
    if !has_distributed_body {
        return Err(
            "at least one body must retain multi-host, Line, and distributed Plan truth".into(),
        );
    }
    Ok(())
}

fn validate_mask_journey(track: &BodyTrack) -> Result<(), String> {
    use conduit_presentation::MaskJourneyAction;

    for (expected, observed) in conduit_presentation::MASK_JOURNEY_ACTIONS
        .iter()
        .zip(&track.mask_actions)
    {
        if observed.action_id != expected.id()
            || !valid_narrative(&observed.concrete_event)
            || !valid_identity(&observed.face_id)
            || !valid_identity(&observed.plan_id)
            || observed
                .selected_mask_plot_id
                .as_deref()
                .is_some_and(|identity| !valid_identity(identity))
            || observed
                .selected_route_id
                .as_deref()
                .is_some_and(|identity| !valid_identity(identity))
            || observed
                .show_id
                .as_deref()
                .is_some_and(|identity| !valid_identity(identity))
            || observed.receipt_ids.is_empty()
            || observed.receipt_ids.iter().collect::<BTreeSet<_>>().len()
                != observed.receipt_ids.len()
            || observed.receipt_ids.iter().any(|receipt_id| {
                !track
                    .receipts
                    .iter()
                    .any(|receipt| receipt.step_id == receipt_id.as_str())
            })
        {
            return Err(format!(
                "{} has invalid or reordered Mask action {}",
                track.track_id, observed.action_id
            ));
        }
    }

    let first = &track.mask_actions[0];
    let mut current_show_ids = BTreeSet::new();
    for action in &track.mask_actions {
        if action
            .show_id
            .as_deref()
            .is_some_and(|show_id| !current_show_ids.insert(show_id))
        {
            return Err(format!(
                "{} reuses a retained Show as current Face truth",
                track.track_id
            ));
        }
    }
    if track
        .mask_actions
        .iter()
        .any(|action| action.face_id != first.face_id)
    {
        return Err(format!(
            "{} changes Face identity during its Mask journey",
            track.track_id
        ));
    }
    let preferred = &track.mask_actions[2];
    if first.show_id.is_none()
        || first.selected_route_id.is_none()
        || first.selected_mask_plot_id.is_none()
        || preferred.plan_id != first.plan_id
        || preferred.show_id.is_none()
        || preferred.selected_route_id.is_none()
        || preferred.selected_route_id == first.selected_route_id
        || preferred.selected_mask_plot_id.is_none()
        || preferred.selected_mask_plot_id == first.selected_mask_plot_id
    {
        return Err(format!(
            "{} does not prove sealed same-Plan Mask selection",
            track.track_id
        ));
    }
    for index in [3_usize, 4, 5, 6] {
        if track.mask_actions[index].show_id.is_some() {
            return Err(format!(
                "{} invents a Show while its selected Mask route is unavailable",
                track.track_id
            ));
        }
    }
    for index in [3_usize, 4, 5] {
        if track.mask_actions[index].plan_id != first.plan_id {
            return Err(format!(
                "{} mutates the active Plan before replacement planning",
                track.track_id
            ));
        }
    }
    let replacement = &track.mask_actions[6];
    let replanned = &track.mask_actions[7];
    if replacement.plan_id == first.plan_id
        || replanned.plan_id != replacement.plan_id
        || replanned.show_id.is_none()
        || replanned.selected_route_id.is_none()
    {
        return Err(format!(
            "{} does not prove a genuine replacement Plan and Show",
            track.track_id
        ));
    }
    let restored = &track.mask_actions[9];
    if restored.plan_id != replacement.plan_id
        || restored.show_id.is_none()
        || restored.selected_route_id.is_none()
        || restored.selected_mask_plot_id != first.selected_mask_plot_id
    {
        return Err(format!(
            "{} does not restore its original worn Mask through sealed replacement truth",
            track.track_id
        ));
    }
    if track.mask_actions[3].action_id != MaskJourneyAction::WithdrawSelectedRoute.id()
        || track.mask_actions[6].action_id != MaskJourneyAction::AdmitReplacementPlan.id()
    {
        return Err(format!(
            "{} does not retain the canonical Mask recovery boundary",
            track.track_id
        ));
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
        || contract.actions.len() != REQUIRED_ACTIONS.len()
    {
        return Err("invalid bounded semantic Journey contract".into());
    }
    let mut action_ids = BTreeSet::new();
    for (position, action) in contract.actions.iter().enumerate() {
        if REQUIRED_ACTIONS.get(position) != Some(&action.action)
            || !action_ids.insert(action.action_id.as_str())
            || !valid_identity(&action.action_id)
            || !valid_narrative(&action.title)
            || !valid_narrative(&action.what_happened)
            || !valid_narrative(&action.what_conduit_established)
            || action.concepts.is_empty()
            || action.concepts.iter().any(|value| !valid_identity(value))
            || action.required_receipt_assertions.is_empty()
            || action
                .required_receipt_assertions
                .iter()
                .any(|value| !valid_identity(value))
            || action.non_claims.is_empty()
            || action.non_claims.iter().any(|value| !valid_identity(value))
        {
            return Err(format!(
                "invalid or reordered semantic Journey action '{}'",
                action.action_id
            ));
        }
    }
    Ok(())
}

fn validate_receipt(track: &BodyTrack, observed: &TrackStep) -> Result<(), String> {
    if !valid_identity(&observed.step_id)
        || !valid_identity(&observed.assertion)
        || observed.disposition != "established"
        || observed.evidence.is_empty()
        || observed.evidence.len() > MAXIMUM_EVIDENCE_PER_STEP
        || observed.evidence.iter().any(|evidence| {
            evidence.evidence_class == "semantic-receipt"
                && evidence.assertion_rung.label().is_empty()
        })
    {
        return Err(format!(
            "{} has invalid detailed receipt {}",
            track.track_id, observed.step_id
        ));
    }
    let classes = observed
        .evidence
        .iter()
        .map(|evidence| evidence.evidence_class.as_str())
        .collect::<BTreeSet<_>>();
    if !classes.contains("semantic-receipt") {
        return Err(format!(
            "{} lacks evidence for {}",
            track.track_id, observed.step_id
        ));
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
        return Err("Body tracks collapsed exact runtime or Face identity".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "evidence_three_body_journey_tests.rs"]
mod tests;
