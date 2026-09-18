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
        steps: REQUIRED_MILESTONES
            .iter()
            .copied()
            .map(canonical_step)
            .collect(),
    }
}

fn canonical_step(milestone: JourneyMilestone) -> ContractStep {
    let (step_id, title, happened, established, concepts, rung, provenance) = match milestone {
        JourneyMilestone::BodyAbsent => (
            "body.absent",
            "Before the Body",
            "The reviewed Host began without a Body.",
            "No Body identity existed before this track's birth operation.",
            &["Body", "Host", "Boot"][..],
            EvidenceRung::RuntimeReceipt,
            &[ProvenanceField::Host, ProvenanceField::Boot][..],
        ),
        JourneyMilestone::BootstrapStarted => (
            "bootstrap.started",
            "Bootstrap began",
            "The Host started the bounded Body bootstrap flow.",
            "Bootstrap was an explicit operation on one exact Host boot.",
            &["Host", "Boot", "Body"][..],
            EvidenceRung::RuntimeReceipt,
            &[ProvenanceField::Host, ProvenanceField::Boot][..],
        ),
        JourneyMilestone::BodyBorn => (
            "body.born",
            "A Body was born",
            "The accepted birth operation created this track's Body.",
            "One new Body identity and biography began.",
            &["Body", "Host", "Boot", "Sign"][..],
            EvidenceRung::BodyBiography,
            &[
                ProvenanceField::Body,
                ProvenanceField::Host,
                ProvenanceField::Boot,
                ProvenanceField::Sign,
            ][..],
        ),
        JourneyMilestone::BodyWoken => (
            "body.awake",
            "The Body woke",
            "An admitted Wake made the Body active.",
            "The Body became Awake through an exact lifecycle transition.",
            &["Body", "Wake", "Sign"][..],
            EvidenceRung::BodyBiography,
            &[ProvenanceField::Body, ProvenanceField::Sign][..],
        ),
        JourneyMilestone::FormUsed => (
            "form.used",
            "A standing Form was used",
            "Later input exercised already admitted work.",
            "One exact Plan and Play retained the standing Form across use.",
            &["Body", "Form", "Plan", "Play"][..],
            EvidenceRung::RuntimeReceipt,
            &[
                ProvenanceField::Body,
                ProvenanceField::Plan,
                ProvenanceField::Play,
                ProvenanceField::Sign,
            ][..],
        ),
        JourneyMilestone::BodyInspected => (
            "body.inspected",
            "The Body inspected itself",
            "The resident Body Surface presented current realization truth.",
            "The Presentation and Manifestation described this exact Body.",
            &["Body", "Presentation", "Manifestation"][..],
            EvidenceRung::SemanticPresentation,
            &[
                ProvenanceField::Body,
                ProvenanceField::Presentation,
                ProvenanceField::Manifestation,
            ][..],
        ),
        JourneyMilestone::WorkloadRevised => (
            "workload.revised",
            "The workload changed",
            "An ordinary semantic operation admitted another Form.",
            "The Body retained a new exact workload revision.",
            &["Body", "Form", "Plan", "Sign"][..],
            EvidenceRung::BodyBiography,
            &[
                ProvenanceField::Body,
                ProvenanceField::Plan,
                ProvenanceField::Sign,
            ][..],
        ),
        JourneyMilestone::HostAdded => (
            "host.added",
            "Another Host was added",
            "The Body completed the reviewed Add Host flow.",
            "An exact additional Host boot became retained Body membership.",
            &["Body", "Host", "Boot", "Plan"][..],
            EvidenceRung::RuntimeReceipt,
            &[
                ProvenanceField::Body,
                ProvenanceField::Host,
                ProvenanceField::Boot,
                ProvenanceField::Plan,
                ProvenanceField::Sign,
            ][..],
        ),
        JourneyMilestone::FaultObserved => (
            "fault.observed",
            "A real fault was observed",
            "Ordinary execution retained an honest failure or refusal.",
            "The Body did not reinterpret failure as success or completion.",
            &["Body", "Sign", "Refusal"][..],
            EvidenceRung::StreamDisposition,
            &[ProvenanceField::Body, ProvenanceField::Sign][..],
        ),
        JourneyMilestone::BodyRepaired => (
            "body.repaired",
            "The Body was repaired",
            "An ordinary repair restored the required work.",
            "New runtime evidence established recovery from the retained fault.",
            &["Body", "Plan", "Play", "Sign"][..],
            EvidenceRung::RuntimeReceipt,
            &[
                ProvenanceField::Body,
                ProvenanceField::Plan,
                ProvenanceField::Play,
                ProvenanceField::Sign,
            ][..],
        ),
        JourneyMilestone::BodyContinued => (
            "body.long-running",
            "The Body continued",
            "The repaired Body remained available for later admitted input.",
            "Finite admitted storage and work did not imply short-lived execution.",
            &["Body", "Play", "Sign"][..],
            EvidenceRung::RuntimeReceipt,
            &[
                ProvenanceField::Body,
                ProvenanceField::Play,
                ProvenanceField::Sign,
            ][..],
        ),
        JourneyMilestone::BodyLulled => (
            "body.lulled",
            "The Body lulled",
            "An explicit lifecycle action ended the current Wake.",
            "The Body became Lulled without being Fulfilled.",
            &["Body", "Wake", "Sign"][..],
            EvidenceRung::LifecycleAction,
            &[ProvenanceField::Body, ProvenanceField::Sign][..],
        ),
        JourneyMilestone::BodyFulfilled => (
            "body.fulfilled",
            "The Body was fulfilled",
            "A selected terminal lifecycle action closed the Body biography.",
            "Fulfilled was irreversible and distinct from readiness or generated prose.",
            &["Body", "Fulfilled", "Sign"][..],
            EvidenceRung::FulfilledTransition,
            &[ProvenanceField::Body, ProvenanceField::Sign][..],
        ),
    };
    ContractStep {
        step_id: step_id.into(),
        milestone: Some(milestone),
        title: title.into(),
        what_happened: happened.into(),
        what_conduit_established: established.into(),
        concepts: concepts.iter().map(|value| (*value).into()).collect(),
        required_assertion: milestone.required_assertion().into(),
        required_assertion_rung: rung,
        allowed_dispositions: vec!["established".into()],
        required_evidence_classes: vec!["semantic-receipt".into()],
        required_provenance: provenance.to_vec(),
        non_claims: vec![
            "not-body-identity-equality".into(),
            "not-pixel-wording-or-timing-equality".into(),
        ],
    }
}
