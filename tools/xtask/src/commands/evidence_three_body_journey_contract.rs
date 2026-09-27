//! Canonical shared semantic contract for the three-Body Journey.

use super::*;

pub(super) fn write(commit: String, output: PathBuf) -> Result<(), String> {
    if !valid_commit(&commit) {
        return Err("three-Body Journey contract requires an exact commit".into());
    }
    let contract = canonical(&commit);
    validate_contract(&contract, &commit)
        .map_err(|error| format!("invalid canonical three-Body Journey contract: {error}"))?;
    let parent = output
        .parent()
        .ok_or("three-Body Journey contract output has no parent")?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("create three-Body Journey contract directory: {error}"))?;
    let bytes = serde_json::to_vec_pretty(&contract)
        .map_err(|error| format!("encode three-Body Journey contract: {error}"))?;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)
        .and_then(|mut file| std::io::Write::write_all(&mut file, &bytes))
        .map_err(|error| format!("create three-Body Journey contract: {error}"))
}

pub(super) fn canonical(commit: &str) -> JourneyContract {
    JourneyContract {
        schema: CONTRACT_SCHEMA.into(),
        journey_id: "orifina/tutorial@1".into(),
        git_commit: commit.into(),
        actions: REQUIRED_ACTIONS
            .iter()
            .copied()
            .map(canonical_action)
            .collect(),
    }
}

fn canonical_action(action: JourneyActionKind) -> ContractAction {
    let (action_id, title, happened, established, concepts, assertions) = match action {
        JourneyActionKind::Bootstrap => (
            "journey.bootstrap", "Begin with no Body", "One exact Host boot began a bounded bootstrap from truthful zero-Body state.",
            "Absence and the start of bootstrap were both retained, rather than inferred from later Birth.",
            &["Body", "Host", "Boot"][..], &["body-absent", "bootstrap-started"][..],
        ),
        JourneyActionKind::Birth => (
            "journey.birth", "Birth and become usable", "The bootstrap created a new Body and admitted its first wake.",
            "Birth and usability are exact lifecycle truth for this independent Body.",
            &["Body", "Wake", "Sign"][..], &["body-born", "body-awake"][..],
        ),
        JourneyActionKind::UsefulWork => (
            "journey.useful-work", "Do useful work", "The Body exercised already admitted semantic work.",
            "An exact Plan and Play performed useful work; embodiment-specific inspection or revision may remain in detailed receipts.",
            &["Body", "Form", "Plan", "Play"][..], &["standing-form-used"][..],
        ),
        JourneyActionKind::BreakAndRecover => (
            "journey.break-recover", "Break and recovery outcome", "The Body retained a real failure or refusal, plus an independently evidenced recovery where one existed.",
            "Failure remained failure; no later success was relabeled as recovery for a different obligation.",
            &["Body", "Refusal", "Plan", "Play"][..], &["fault-observed"][..],
        ),
        JourneyActionKind::RestAndFinish => (
            "journey.rest-finish", "Rest and finish", "Explicit lifecycle actions ended the wake and then the Body biography.",
            "Lulled and Fulfilled remained distinct, ordered terminal truths.",
            &["Body", "Wake", "Fulfilled"][..], &["body-lulled", "body-fulfilled"][..],
        ),
    };
    ContractAction {
        action_id: action_id.into(),
        action,
        title: title.into(),
        what_happened: happened.into(),
        what_conduit_established: established.into(),
        concepts: concepts.iter().map(|value| (*value).into()).collect(),
        required_receipt_assertions: assertions.iter().map(|value| (*value).into()).collect(),
        non_claims: vec![
            "not-body-identity-equality".into(),
            "not-pixel-wording-or-timing-equality".into(),
        ],
    }
}
