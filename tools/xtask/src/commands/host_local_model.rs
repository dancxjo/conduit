use crate::cli::GlobalOpts;
use conduit_ai::LocalModelKindProfile;
use conduit_body::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyFormPlan, BodyMembership,
    BodyPlayIdentity, MembershipProofId, PartId, ResidentForm, WakeRejectionEvidence,
};
use conduit_core::{
    bind_sign, seal_plan, AuthorityGrantId, BootId, ExpandedFormId, FormIdentity, HostId,
    OfferGeneration,
};
use conduit_std_host::hosted_local_model::{HostedLocalModelAdapter, OllamaDiscovery};
use serde::Serialize;

pub(super) fn inspect(model: &str, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        if !opts.quiet {
            println!("would inspect already-local model {model} without loading it");
        }
        return Ok(());
    }
    let discovery = OllamaDiscovery::discover(model)?;
    if opts.json {
        println!("{}", serde_json::to_string(&discovery)?);
    } else if !opts.quiet {
        println!("LOCAL MODEL DISCOVERED (not initialized or advertised)");
        println!("runtime: {}", discovery.runtime_version);
        println!(
            "model: {} {} ({} bytes)",
            discovery.model_name, discovery.model_content_identity, discovery.model_bytes
        );
        println!(
            "profile: {} {} {} context={}",
            discovery.architecture,
            discovery.parameter_profile,
            discovery.quantization,
            discovery.context_length
        );
    }
    Ok(())
}

pub(super) fn prove(
    model: &str,
    admitted_memory_mib: u32,
    orifina_presenter: bool,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        if !opts.quiet {
            println!(
                "would initialize already-local model {model} with {admitted_memory_mib} MiB admitted"
            );
        }
        return Ok(());
    }
    let adapter = OllamaDiscovery::discover(model)?.initialize(
        admitted_memory_mib,
        vec![
            LocalModelKindProfile::Generate,
            LocalModelKindProfile::ClassifyFiniteLabels,
            LocalModelKindProfile::ExtractValidatedInfo,
            LocalModelKindProfile::InterpretSignEvidence,
            LocalModelKindProfile::PresentSemanticFront,
        ],
    )?;
    let offer = adapter.offer().clone();
    let journey = if orifina_presenter {
        Some(orifina_journey()?)
    } else {
        None
    };
    let presenter_requests = journey
        .as_ref()
        .and_then(|journey| journey.requests.first())
        .map_or_else(Vec::new, |request| {
            conduit_std_host::local_model_proof::presenter_policy_experiment(request.clone())
        });
    let receipt = conduit_std_host::local_model_proof::run(adapter, &presenter_requests)?;
    if opts.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "local_model": receipt,
                "body_journey": journey.map(|journey| journey.receipt),
            }))?
        );
    } else if !opts.quiet {
        println!("LOCAL MODEL INITIALIZED AND WARM");
        println!(
            "implementation: {}/{}",
            conduit_ai::LOCAL_MODEL_IMPLEMENTATION,
            offer.identity.model_content_identity
        );
        println!(
            "limits: input={} output={} work={} memory={}MiB in-flight={} queue={}/{}B cancellation={}",
            offer.limits.work.maximum_input_bytes,
            offer.limits.work.maximum_output_bytes,
            offer.limits.work.maximum_work_units,
            offer.limits.admitted_memory_mib,
            offer.limits.maximum_in_flight,
            offer.limits.maximum_queue_items,
            offer.limits.maximum_queue_bytes,
            offer.limits.cancellation_supported
        );
        println!(
            "Plans: generate={} classify={} extract={} interpret={} present={} house={} completed={}/{}/{}/{}/{}/{}",
            receipt.generate_plan_id,
            receipt.classify_plan_id,
            receipt.extract_plan_id,
            receipt.interpret_plan_id,
            receipt.present_plan_id,
            receipt.house_plan_id,
            receipt.generate_play_completed,
            receipt.classify_play_completed,
            receipt.extract_play_completed,
            receipt.interpret_play_completed,
            receipt.present_play_completed,
            receipt.house_play_completed
        );
        println!(
            "House response: bytes={} sha256={} speech-plan={} speech-outcome={:?}",
            receipt.house_response_bytes,
            receipt.house_response_sha256,
            receipt.house_speech.plan_id,
            receipt.house_speech.outcome
        );
        for presenter in &receipt.presenter_requests {
            println!(
                "Orifina Presenter: request={} policy={} source={}@{} provider={} model={} disposition={:?}",
                presenter.request_identity,
                presenter.policy_revision,
                presenter.source_presentation_identity,
                presenter.source_presentation_revision,
                presenter.manifestation.provider_identity,
                presenter.manifestation.model_identity,
                presenter.manifestation.disposition,
            );
        }
    }
    Ok(())
}

struct PreparedOrifinaJourney {
    requests: Vec<conduit_presentation::GenerativePresenterRequest>,
    receipt: OrifinaJourneyReceipt,
}

#[derive(Serialize)]
struct OrifinaJourneyReceipt {
    schema: &'static str,
    body_id: String,
    host_ids: Vec<String>,
    boot_ids: Vec<String>,
    plan_ids: Vec<String>,
    play_ids: Vec<String>,
    workload_revision: u64,
    fault_reason: &'static str,
    repaired: bool,
    fulfilled: bool,
    biography: BodyBiographyEvidence,
}

fn orifina_journey() -> Result<PreparedOrifinaJourney, Box<dyn std::error::Error>> {
    let host = HostId::from("host/orifina-local-model");
    let boot = BootId::from("boot/orifina-local-model");
    let mut body = orifina_body()?;
    let mut requests = vec![conduit_workspace_model::tutorial::generative_request(
        &body,
        "request/workspace/orifina/provider-proof".into(),
        1,
        conduit_workspace_model::tutorial::TutorialPlayback::Lulled,
    )
    .map_err(|error| proof_error("build Workspace Orifina request", error))?];
    let mut plan_ids = Vec::new();
    let mut play_ids = Vec::new();

    let first = start_orifina(&mut body, &host, &boot, 1)?;
    plan_ids.push(
        body.realization()
            .expect("started realization")
            .plan
            .plan_id
            .as_str()
            .to_owned(),
    );
    play_ids.push(first.active_play_id.as_str().to_owned());
    requests.push(tutorial_request(
        &body,
        "playing",
        2,
        conduit_workspace_model::tutorial::TutorialPlayback::Playing,
    )?);
    body.lull(&host, &boot, Some(&first))
        .map_err(|error| proof_error("lull initial Orifina Play", error))?;
    body.admit_form(
        0,
        ResidentForm::new(
            "source/orifina-notes".into(),
            "checked/orifina-notes".into(),
        ),
        &host,
        &boot,
    )
    .map_err(|error| proof_error("revise Orifina workload", error))?;
    admit_orifina_companion(&mut body)?;
    requests.push(tutorial_request(
        &body,
        "revised",
        3,
        conduit_workspace_model::tutorial::TutorialPlayback::Lulled,
    )?);

    let failed = body
        .propose(orifina_plans(&body), &host, &boot)
        .map_err(|error| proof_error("propose refused Orifina Play", error))?
        .clone();
    plan_ids.push(failed.plan.plan_id.as_str().to_owned());
    body.fail(
        &host,
        &boot,
        vec![WakeRejectionEvidence {
            reason_code: "model.presenter-temporarily-unavailable".into(),
            category: "Availability".into(),
            stage: "Body execution".into(),
            resource: "llm/present-semantic-front".into(),
            required: 1,
            available: 0,
            host_id: host.clone(),
            boot_id: boot.clone(),
            plan_id: Some(failed.plan.plan_id),
            checked_form_ids: failed
                .plan
                .forms
                .iter()
                .map(|form| form.form.checked_form_id.clone())
                .collect(),
        }],
    )
    .map_err(|error| proof_error("retain Orifina refusal", error))?;
    requests.push(tutorial_request(
        &body,
        "fault",
        4,
        conduit_workspace_model::tutorial::TutorialPlayback::Refused,
    )?);

    let repaired = start_orifina(&mut body, &host, &boot, 2)?;
    plan_ids.push(
        body.realization()
            .expect("repaired realization")
            .plan
            .plan_id
            .as_str()
            .to_owned(),
    );
    play_ids.push(repaired.active_play_id.as_str().to_owned());
    requests.push(tutorial_request(
        &body,
        "repaired",
        5,
        conduit_workspace_model::tutorial::TutorialPlayback::Playing,
    )?);
    body.lull(&host, &boot, Some(&repaired))
        .map_err(|error| proof_error("lull repaired Orifina Play", error))?;
    body.fulfill(
        &host,
        &boot,
        AuthorityGrantId::from("grant/orifina/operator-finish"),
        "operator/orifina-proof".into(),
    )
    .map_err(|error| proof_error("fulfill Orifina Body", error))?;
    requests.push(tutorial_request(
        &body,
        "fulfilled",
        6,
        conduit_workspace_model::tutorial::TutorialPlayback::Completed,
    )?);
    let evidence = body.evidence().clone();
    Ok(PreparedOrifinaJourney {
        requests,
        receipt: OrifinaJourneyReceipt {
            schema: "conduit.evidence/orifina-body-journey@1",
            body_id: evidence.body_id.as_str().to_owned(),
            host_ids: vec![host.as_str().to_owned(), "host/orifina-companion".into()],
            boot_ids: vec![boot.as_str().to_owned(), "boot/orifina-companion".into()],
            plan_ids,
            play_ids,
            workload_revision: evidence.body.workload_revision,
            fault_reason: "model.presenter-temporarily-unavailable",
            repaired: evidence
                .wakes
                .iter()
                .any(|wake| matches!(wake.lifecycle, conduit_body::WakeLifecycle::Failed))
                && evidence
                    .wakes
                    .iter()
                    .rev()
                    .any(|wake| matches!(wake.lifecycle, conduit_body::WakeLifecycle::Lulled)),
            fulfilled: matches!(
                evidence.body.state,
                conduit_body::BodyState::Fulfilled { .. }
            ),
            biography: evidence,
        },
    })
}

fn orifina_body() -> Result<conduit_workspace_model::WorkspaceBody, Box<dyn std::error::Error>> {
    let form = ResidentForm::new("source/morse".into(), "checked/morse".into());
    let body = Body::born(
        form.source_document_id,
        form.checked_form_id,
        1,
        "sign/orifina-provider-proof/birth".into(),
    )
    .map_err(|error| proof_error("birth Orifina proof Body", error))?;
    let mut membership = BodyMembership::new(body.body_id.clone())
        .map_err(|error| proof_error("initialize Orifina membership", error))?;
    let mut evidence = BodyBiographyEvidence::born(
        body.clone(),
        membership.clone(),
        "Orifina provider proof".into(),
    )
    .map_err(|error| proof_error("initialize Orifina biography", error))?;
    let part = PartId::bind(&body.body_id, "provider-proof", 1)
        .map_err(|error| proof_error("bind Orifina proof part", error))?;
    let proof = MembershipProofId::bind("proof/orifina-provider")
        .map_err(|error| proof_error("bind Orifina membership proof", error))?;
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            "sign/orifina-provider-proof/admit".into(),
        )
        .map_err(|error| proof_error("admit Orifina proof part", error))?;
    let present = membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: HostId::from("host/orifina-local-model"),
                boot_id: BootId::from("boot/orifina-local-model"),
                offer_generation: OfferGeneration(1),
                proof_id: proof,
                sequence: 1,
            },
            "sign/orifina-provider-proof/present".into(),
        )
        .map_err(|error| proof_error("observe Orifina proof Host", error))?;
    evidence
        .append_membership_events(membership, &[(admitted, 2), (present, 3)])
        .map_err(|error| proof_error("retain Orifina membership evidence", error))?;
    conduit_workspace_model::WorkspaceBody::open(evidence)
        .map_err(|error| proof_error("open Orifina Workspace Body", error))
}

fn orifina_plans(body: &conduit_workspace_model::WorkspaceBody) -> Vec<BodyFormPlan> {
    body.evidence()
        .body
        .workset
        .forms()
        .iter()
        .map(|form| BodyFormPlan {
            form: form.clone(),
            plan: seal_plan(
                FormIdentity {
                    source_document_id: form.source_document_id.clone(),
                    checked_form_id: form.checked_form_id.clone(),
                    expanded_form_id: ExpandedFormId::from("expanded/orifina-proof"),
                },
                vec![],
            ),
        })
        .collect()
}

fn start_orifina(
    body: &mut conduit_workspace_model::WorkspaceBody,
    host: &HostId,
    boot: &BootId,
    sequence: u64,
) -> Result<BodyPlayIdentity, Box<dyn std::error::Error>> {
    let proposal = body
        .propose(orifina_plans(body), host, boot)
        .map_err(|error| proof_error("propose Orifina Play", error))?
        .clone();
    let play = BodyPlayIdentity::bind(&proposal.plan, sequence);
    let sign = |index| bind_sign(host, boot, Some(&play.active_play_id), index).sign_id;
    let wake = proposal
        .wake
        .body_plan_ready(&proposal.plan, sign(0))
        .and_then(|wake| wake.body_play_started(&proposal.plan, &play, sign(1)))
        .map_err(|error| proof_error("start Orifina Play", error))?;
    body.started(host, boot, play.clone(), wake)
        .map_err(|error| proof_error("retain Orifina Play", error))?;
    Ok(play)
}

fn admit_orifina_companion(
    body: &mut conduit_workspace_model::WorkspaceBody,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut evidence = body.evidence().clone();
    let mut membership = evidence.membership.clone();
    let part = PartId::bind(&evidence.body_id, "companion", 2)
        .map_err(|error| proof_error("bind Orifina companion", error))?;
    let proof = MembershipProofId::bind("proof/orifina-companion")
        .map_err(|error| proof_error("bind Orifina companion proof", error))?;
    let admitted = membership
        .admit(
            &evidence.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            "sign/orifina-companion/admitted".into(),
        )
        .map_err(|error| proof_error("admit Orifina companion", error))?;
    let joined = membership
        .observe_present(
            &evidence.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: "host/orifina-companion".into(),
                boot_id: "boot/orifina-companion".into(),
                offer_generation: OfferGeneration(1),
                proof_id: proof,
                sequence: 1,
            },
            "sign/orifina-companion/joined".into(),
        )
        .map_err(|error| proof_error("join Orifina companion", error))?;
    let sequence = evidence
        .records
        .last()
        .map_or(1, |record| record.sequence + 1);
    evidence
        .append_membership_events(membership, &[(admitted, sequence), (joined, sequence + 1)])
        .map_err(|error| proof_error("retain Orifina companion", error))?;
    *body = conduit_workspace_model::WorkspaceBody::open(evidence)
        .map_err(|error| proof_error("reopen distributed Orifina Body", error))?;
    Ok(())
}

fn tutorial_request(
    body: &conduit_workspace_model::WorkspaceBody,
    stage: &str,
    revision: u64,
    playback: conduit_workspace_model::tutorial::TutorialPlayback,
) -> Result<conduit_presentation::GenerativePresenterRequest, Box<dyn std::error::Error>> {
    conduit_workspace_model::tutorial::generative_request(
        body,
        format!("request/workspace/orifina/journey/{stage}").into(),
        revision,
        playback,
    )
    .map_err(|error| proof_error("build Workspace Orifina request", error))
}

fn proof_error(stage: &str, error: impl core::fmt::Debug) -> Box<dyn std::error::Error> {
    std::io::Error::other(format!("{stage}: {error:?}")).into()
}
