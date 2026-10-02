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
    let (action_id, happened, established, concepts, assertions) = match action {
        JourneyActionKind::Bootstrap => (
            "journey.bootstrap", "One exact Host boot began a bounded bootstrap from truthful zero-Body state.",
            "Absence and the start of bootstrap were both retained, rather than inferred from later Birth.",
            &["Body", "Host", "Boot"][..], &["body-absent", "bootstrap-started"][..],
        ),
        JourneyActionKind::Birth => (
            "journey.birth", "The bootstrap created a new Body and admitted its first wake.",
            "Birth and usability are exact lifecycle truth for this independent Body.",
            &["Body", "Wake", "Sign"][..], &["body-born", "body-awake"][..],
        ),
        JourneyActionKind::UsefulWork => (
            "journey.useful-work", "The Body exercised already admitted semantic work.",
            "An exact Plan and Play performed useful work; embodiment-specific inspection or revision may remain in detailed receipts.",
            &["Body", "Plot", "Plan", "Play"][..], &["standing-plot-used"][..],
        ),
        JourneyActionKind::MaskInspectInitialShow => mask_action(
            "mask.inspect-initial-show",
            "The Body inspected its exact current Face, Mask route, Plan, and Show.",
            "The initial human-facing realization is producer evidence, not an inferred screenshot label.",
        ),
        JourneyActionKind::MaskWearAlternate => mask_action(
            "mask.wear-alternate",
            "The Body made a second ordinary Mask Plot eligible in its revisioned wardrobe.",
            "Eligibility changed without mutating the immutable body-wide Plan.",
        ),
        JourneyActionKind::MaskPreferAlternate => mask_action(
            "mask.prefer-alternate",
            "The Body preferred and selected an already-sealed alternate Mask route.",
            "Selection changed under the same body-wide Plan; no unsealed route was invented.",
        ),
        JourneyActionKind::MaskWithdrawSelectedRoute => mask_action(
            "mask.withdraw-selected-route",
            "The selected sealed Mask route became unavailable.",
            "The Body retained NoShow rather than silently retaining an unavailable realization.",
        ),
        JourneyActionKind::MaskInspectUnavailableShow => mask_action(
            "mask.inspect-unavailable-show",
            "The Body inspected the interval in which no worn Mask was realizable.",
            "Lack of Show remained distinct from Face truth and from authority to replan.",
        ),
        JourneyActionKind::MaskAddFaceHost => mask_action(
            "mask.add-face-host",
            "A new Host or Boot offered a possible Mask realization.",
            "Availability alone did not mutate the old Plan or create a Show.",
        ),
        JourneyActionKind::MaskAdmitReplacementPlan => mask_action(
            "mask.admit-replacement-plan",
            "Authorized planning admitted a distinct body-wide replacement Plan.",
            "The replacement is a new immutable identity, not disguised same-Plan fallback.",
        ),
        JourneyActionKind::MaskInspectReplannedShow => mask_action(
            "mask.inspect-replanned-show",
            "The Body inspected the Show realized by the replacement Plan.",
            "Face identity remained stable while realization identity changed.",
        ),
        JourneyActionKind::MaskDoffAlternate => mask_action(
            "mask.doff-alternate",
            "The Body removed the alternate Mask from its eligible wardrobe.",
            "A doffed Mask could not remain selected merely because its Back stayed available.",
        ),
        JourneyActionKind::MaskInspectRestoredShow => mask_action(
            "mask.inspect-restored-show",
            "The Body inspected its restored original Mask under the replacement Plan.",
            "The journey ended with exact Mask, route, Plan, and Show correlation.",
        ),
        JourneyActionKind::BreakAndRecover => (
            "journey.break-recover", "The Body retained a real failure or refusal, plus an independently evidenced recovery where one existed.",
            "Failure remained failure; no later success was relabeled as recovery for a different obligation.",
            &["Body", "Refusal", "Plan", "Play"][..], &["fault-observed"][..],
        ),
        JourneyActionKind::RestAndFinish => (
            "journey.rest-finish", "Explicit lifecycle actions ended the wake and then the Body biography.",
            "Lulled and Fulfilled remained distinct, ordered terminal truths.",
            &["Body", "Wake", "Fulfilled"][..], &["body-lulled", "body-fulfilled"][..],
        ),
    };
    ContractAction {
        action_id: action_id.into(),
        action,
        title: action.title().into(),
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

fn mask_action(
    id: &'static str,
    happened: &'static str,
    established: &'static str,
) -> (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
) {
    let assertion = match id {
        "mask.inspect-initial-show" => &["mask-mask.inspect-initial-show"][..],
        "mask.wear-alternate" => &["mask-mask.wear-alternate"][..],
        "mask.prefer-alternate" => &["mask-mask.prefer-alternate"][..],
        "mask.withdraw-selected-route" => &["mask-mask.withdraw-selected-route"][..],
        "mask.inspect-unavailable-show" => &["mask-mask.inspect-unavailable-show"][..],
        "mask.add-face-host" => &["mask-mask.add-face-host"][..],
        "mask.admit-replacement-plan" => &["mask-mask.admit-replacement-plan"][..],
        "mask.inspect-replanned-show" => &["mask-mask.inspect-replanned-show"][..],
        "mask.doff-alternate" => &["mask-mask.doff-alternate"][..],
        "mask.inspect-restored-show" => &["mask-mask.inspect-restored-show"][..],
        _ => &[][..],
    };
    (
        id,
        happened,
        established,
        &["Body", "Face", "Mask", "Plan", "Show"],
        assertion,
    )
}
