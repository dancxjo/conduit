//! Exact cross-front Home evidence correlation without collapsing execution identities.

use conduit_home_model::JOURNEY_STEP_IDS;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

const SCHEMA: &str = "conduit.evidence/home-front@1";
const INDEX_SCHEMA: &str = "conduit.evidence/home-cross-front-index@1";
const VOICE_ARTIFACT_SCHEMA: &str = "conduit.home/voice-front@1";
const MAXIMUM_FRONT_ARTIFACT_BYTES: usize = 2 * 1024 * 1024;
const REQUIRED_FRONTS: [&str; 6] = [
    "conduitos",
    "linux-native",
    "windows-native",
    "browser-chromium",
    "browser-firefox",
    "voice-physical",
];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct HomeFrontReceipt {
    schema: String,
    front_id: String,
    proof_class: String,
    step_ids: Vec<String>,
    host_id: String,
    boot_id: String,
    plan_id: String,
    active_play_id: String,
    renderer_id: String,
    manifestation_id: String,
    artifact_path: String,
    artifact_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    voice_physical: Option<VoicePhysicalAcceptance>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VoicePhysicalAcceptance {
    microphone: bool,
    playback: bool,
    intelligible: bool,
    safe_volume: bool,
    attended: bool,
}

#[derive(Debug, Deserialize)]
struct VoiceJourneyArtifact {
    schema: String,
    observed_journey_step_ids: Vec<String>,
    physical_acceptance: VoicePhysicalAcceptance,
}

#[derive(Debug, Serialize)]
struct HomeCrossFrontIndex {
    schema: &'static str,
    disposition: &'static str,
    shared_step_ids: Vec<&'static str>,
    fronts: Vec<HomeFrontReceipt>,
}

pub(super) fn run(
    receipt_paths: Vec<PathBuf>,
    output: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut receipts = receipt_paths
        .iter()
        .map(|path| read_receipt(path))
        .collect::<Result<Vec<_>, _>>()?;
    validate(&receipts)?;
    for (receipt, source) in receipts.iter().zip(&receipt_paths) {
        verify_artifact(receipt, source)?;
    }
    receipts.sort_by_key(|receipt| {
        REQUIRED_FRONTS
            .iter()
            .position(|front| *front == receipt.front_id)
            .unwrap_or(usize::MAX)
    });
    let parent = output.parent().ok_or("Home index output has no parent")?;
    std::fs::create_dir_all(parent)?;
    let bytes = serde_json::to_vec_pretty(&HomeCrossFrontIndex {
        schema: INDEX_SCHEMA,
        disposition: "complete",
        shared_step_ids: JOURNEY_STEP_IDS.to_vec(),
        fronts: receipts,
    })?;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)
        .and_then(|mut file| std::io::Write::write_all(&mut file, &bytes))
        .map_err(|error| format!("create new Home cross-front index: {error}"))?;
    println!("HOME CROSS-FRONT INDEX COMPLETE: {}", output.display());
    Ok(())
}

fn read_receipt(path: &Path) -> Result<HomeFrontReceipt, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("decode {}: {error}", path.display()))
}

fn verify_artifact(receipt: &HomeFrontReceipt, source: &Path) -> Result<(), String> {
    let relative = Path::new(&receipt.artifact_path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
    {
        return Err(format!(
            "{} artifact path must stay beside its receipt",
            receipt.front_id
        ));
    }
    let artifact = source
        .parent()
        .ok_or_else(|| format!("{} receipt has no parent", receipt.front_id))?
        .join(relative);
    let bytes = std::fs::read(&artifact).map_err(|error| {
        format!(
            "read {} artifact {}: {error}",
            receipt.front_id,
            artifact.display()
        )
    })?;
    if bytes.is_empty() || bytes.len() > MAXIMUM_FRONT_ARTIFACT_BYTES {
        return Err(format!(
            "{} artifact violates its byte bound",
            receipt.front_id
        ));
    }
    let digest = format!("sha256:{:x}", Sha256::digest(&bytes));
    if digest != receipt.artifact_sha256 {
        return Err(format!("{} artifact digest changed", receipt.front_id));
    }
    if receipt.front_id == "voice-physical" {
        verify_complete_voice_journey(&bytes)?;
    }
    Ok(())
}

fn verify_complete_voice_journey(bytes: &[u8]) -> Result<(), String> {
    let artifact: VoiceJourneyArtifact = serde_json::from_slice(bytes)
        .map_err(|error| format!("decode physical Voice journey artifact: {error}"))?;
    let expected = JOURNEY_STEP_IDS
        .iter()
        .map(|step| (*step).to_owned())
        .collect::<Vec<_>>();
    if artifact.schema != VOICE_ARTIFACT_SCHEMA
        || artifact.observed_journey_step_ids != expected
        || !artifact.physical_acceptance.microphone
        || !artifact.physical_acceptance.playback
        || !artifact.physical_acceptance.intelligible
        || !artifact.physical_acceptance.safe_volume
        || !artifact.physical_acceptance.attended
    {
        return Err(
            "physical Voice artifact does not prove the complete attended Home journey".into(),
        );
    }
    Ok(())
}

fn validate(receipts: &[HomeFrontReceipt]) -> Result<(), String> {
    if receipts.len() != REQUIRED_FRONTS.len() {
        return Err(format!(
            "Home cross-front index requires exactly {} receipts",
            REQUIRED_FRONTS.len()
        ));
    }
    let expected_steps = JOURNEY_STEP_IDS
        .iter()
        .map(|step| (*step).to_owned())
        .collect::<Vec<_>>();
    let mut fronts = BTreeSet::new();
    let mut host_boots = BTreeSet::new();
    let mut plan_plays = BTreeSet::new();
    let mut manifestations = BTreeSet::new();
    for receipt in receipts {
        if receipt.schema != SCHEMA {
            return Err(format!("{} has the wrong receipt schema", receipt.front_id));
        }
        if !REQUIRED_FRONTS.contains(&receipt.front_id.as_str())
            || !fronts.insert(&receipt.front_id)
        {
            return Err(format!(
                "unknown or duplicate Home front {}",
                receipt.front_id
            ));
        }
        if receipt.step_ids != expected_steps {
            return Err(format!(
                "{} does not carry the exact shared Home journey",
                receipt.front_id
            ));
        }
        for (name, value) in [
            ("proof_class", &receipt.proof_class),
            ("host_id", &receipt.host_id),
            ("boot_id", &receipt.boot_id),
            ("plan_id", &receipt.plan_id),
            ("active_play_id", &receipt.active_play_id),
            ("renderer_id", &receipt.renderer_id),
            ("manifestation_id", &receipt.manifestation_id),
            ("artifact_path", &receipt.artifact_path),
        ] {
            if value.is_empty() || value.len() > 512 {
                return Err(format!("{} has invalid {name}", receipt.front_id));
            }
        }
        if !valid_sha256(&receipt.artifact_sha256) {
            return Err(format!(
                "{} has an invalid artifact digest",
                receipt.front_id
            ));
        }
        if !host_boots.insert((&receipt.host_id, &receipt.boot_id)) {
            return Err("Home fronts collapsed distinct Host/Boot identity".into());
        }
        if !plan_plays.insert((&receipt.plan_id, &receipt.active_play_id)) {
            return Err("Home fronts collapsed distinct Plan/Play identity".into());
        }
        if !manifestations.insert((&receipt.renderer_id, &receipt.manifestation_id)) {
            return Err("Home fronts collapsed distinct renderer manifestation".into());
        }
        if receipt.front_id == "voice-physical" {
            let physical = receipt
                .voice_physical
                .as_ref()
                .ok_or("physical Voice receipt lacks attended acceptance")?;
            if !physical.microphone
                || !physical.playback
                || !physical.intelligible
                || !physical.safe_volume
                || !physical.attended
            {
                return Err("physical Voice acceptance is incomplete".into());
            }
        } else if receipt.voice_physical.is_some() {
            return Err(format!(
                "{} must not claim physical Voice acceptance",
                receipt.front_id
            ));
        }
    }
    if fronts
        .iter()
        .map(|front| front.as_str())
        .collect::<BTreeSet<_>>()
        != REQUIRED_FRONTS.into_iter().collect()
    {
        return Err("Home cross-front index is missing a required front".into());
    }
    Ok(())
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt(front_id: &str, index: usize) -> HomeFrontReceipt {
        HomeFrontReceipt {
            schema: SCHEMA.into(),
            front_id: front_id.into(),
            proof_class: "fixture".into(),
            step_ids: JOURNEY_STEP_IDS.iter().map(|step| (*step).into()).collect(),
            host_id: format!("host-{index}"),
            boot_id: format!("boot-{index}"),
            plan_id: format!("plan-{index}"),
            active_play_id: format!("play-{index}"),
            renderer_id: format!("renderer-{index}"),
            manifestation_id: format!("manifestation-{index}"),
            artifact_path: format!("front-{index}.json"),
            artifact_sha256: format!("sha256:{:064x}", index + 1),
            voice_physical: (front_id == "voice-physical").then_some(VoicePhysicalAcceptance {
                microphone: true,
                playback: true,
                intelligible: true,
                safe_volume: true,
                attended: true,
            }),
        }
    }

    fn complete() -> Vec<HomeFrontReceipt> {
        REQUIRED_FRONTS
            .iter()
            .enumerate()
            .map(|(index, front)| receipt(front, index))
            .collect()
    }

    #[test]
    fn exact_distinct_fronts_form_one_complete_index() {
        validate(&complete()).unwrap();
    }

    #[test]
    fn identity_collapse_is_refused() {
        let mut receipts = complete();
        receipts[1].host_id = receipts[0].host_id.clone();
        receipts[1].boot_id = receipts[0].boot_id.clone();
        assert!(validate(&receipts).is_err());
    }

    #[test]
    fn voice_cannot_be_complete_without_attended_physical_acceptance() {
        let mut receipts = complete();
        receipts[5].voice_physical.as_mut().unwrap().safe_volume = false;
        assert_eq!(
            validate(&receipts).unwrap_err(),
            "physical Voice acceptance is incomplete"
        );
    }

    #[test]
    fn artifact_digest_and_containment_are_verified() {
        let root =
            std::env::temp_dir().join(format!("conduit-home-cross-front-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("receipt.json");
        let artifact = root.join("front.json");
        std::fs::write(&source, b"{}").unwrap();
        std::fs::write(&artifact, b"exact artifact").unwrap();
        let mut value = receipt("conduitos", 0);
        value.artifact_path = "front.json".into();
        value.artifact_sha256 = format!("sha256:{:x}", Sha256::digest(b"exact artifact"));
        verify_artifact(&value, &source).unwrap();
        value.artifact_path = "../front.json".into();
        assert!(verify_artifact(&value, &source).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn one_turn_voice_artifact_cannot_claim_the_complete_journey() {
        let artifact = serde_json::json!({
            "schema": VOICE_ARTIFACT_SCHEMA,
            "journey_step_ids": JOURNEY_STEP_IDS,
            "physical_scope": "one attended push-to-talk command",
            "physical_acceptance": {
                "microphone": true,
                "playback": true,
                "intelligible": true,
                "safe_volume": true,
                "attended": true
            }
        });
        assert!(verify_complete_voice_journey(&serde_json::to_vec(&artifact).unwrap()).is_err());
    }

    #[test]
    fn exact_observed_voice_journey_is_admitted() {
        let artifact = serde_json::json!({
            "schema": VOICE_ARTIFACT_SCHEMA,
            "observed_journey_step_ids": JOURNEY_STEP_IDS,
            "physical_acceptance": {
                "microphone": true,
                "playback": true,
                "intelligible": true,
                "safe_volume": true,
                "attended": true
            }
        });
        verify_complete_voice_journey(&serde_json::to_vec(&artifact).unwrap()).unwrap();
    }
}
