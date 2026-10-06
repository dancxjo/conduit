use crate::cli::GlobalOpts;
use conduit_ai::LocalModelKindProfile;
use conduit_body::{BodyBiographyEvidence, ResidentPlot, WakeRejectionEvidence};
use conduit_core::{AuthorityGrantId, BootId, HostId};
#[path = "host_local_model/orifina_body.rs"]
mod orifina_body;
use conduit_std_host::hosted_local_model::{HostedLocalModelAdapter, OllamaDiscovery};
use orifina_body::{
    admit_orifina_companion, orifina_body, proof_error, start_orifina, tutorial_request,
};
use serde::Serialize;

pub(super) fn inspect(
    model: &str,
    ollama_endpoint: &str,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        if !opts.quiet {
            println!("would inspect already-local model {model} without loading it");
        }
        return Ok(());
    }
    let discovery = OllamaDiscovery::discover_at(ollama_endpoint, model)?;
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
    ollama_endpoint: &str,
    admitted_memory_mib: u32,
    orifina_presenter: bool,
    journey_documentary: bool,
    speech: &super::host_speech::SpeechOptions,
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
    let speech = speech.discover()?;
    let adapter = OllamaDiscovery::discover_at(ollama_endpoint, model)?.initialize(
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
    let presenter_requests = journey.as_ref().map_or_else(Vec::new, |journey| {
        if journey_documentary {
            journey.requests.clone()
        } else {
            journey.requests.first().cloned().into_iter().collect()
        }
    });
    let receipt = conduit_std_host::local_model_proof::run(
        adapter,
        &presenter_requests,
        &conduit_tongues::specimen_language_request(),
    )?;
    if let Some(journey) = journey.as_ref().filter(|_| journey_documentary) {
        if let Err(error) =
            super::host_local_model_journey::write(journey, &receipt, speech.as_ref())
        {
            // Preserve the actual provider outcomes even when a later
            // documentary requirement cannot be met. This is no sealed track.
            if opts.json {
                println!(
                    "{}",
                    serde_json::to_string(&serde_json::json!({
                        "local_model": receipt,
                        "documentary_error": error.to_string(),
                    }))?
                );
            }
            return Err(error);
        }
    }
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
            offer.limits.work.maximum_input_bytes(),
            offer.limits.work.maximum_output_bytes(),
            offer.limits.work.maximum_work_units(),
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

pub(super) struct PreparedOrifinaJourney {
    pub(super) requests: Vec<conduit_presentation::GenerativePresenterRequest>,
    pub(super) receipt: OrifinaJourneyReceipt,
}

#[derive(Serialize)]
pub(super) struct OrifinaJourneyReceipt {
    pub(super) schema: &'static str,
    pub(super) body_id: String,
    pub(super) host_ids: Vec<String>,
    pub(super) boot_ids: Vec<String>,
    pub(super) plan_ids: Vec<String>,
    pub(super) play_ids: Vec<String>,
    pub(super) workload_revision: u64,
    pub(super) fault_reason: &'static str,
    pub(super) repaired: bool,
    pub(super) fulfilled: bool,
    pub(super) biography: BodyBiographyEvidence,
}

fn orifina_journey() -> Result<PreparedOrifinaJourney, Box<dyn std::error::Error>> {
    let host = HostId::from("host/orifina-local-model");
    let boot = BootId::from("boot/orifina-local-model");
    let mut body = orifina_body()?;
    let mut requests = vec![conduit_tutorial_plot::generative_request(
        &body,
        "request/workspace/orifina/provider-proof".into(),
        1,
        conduit_tutorial_plot::TutorialPlayback::Lulled,
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
        conduit_tutorial_plot::TutorialPlayback::Playing,
    )?);
    body.lull(&host, &boot, Some(&first))
        .map_err(|error| proof_error("lull initial Orifina Play", error))?;
    body.admit_plot(
        0,
        ResidentPlot::new(
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
        conduit_tutorial_plot::TutorialPlayback::Lulled,
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
            checked_plot_ids: failed
                .plan
                .plots
                .iter()
                .map(|plot| plot.plot.checked_plot_id.clone())
                .collect(),
        }],
    )
    .map_err(|error| proof_error("retain Orifina refusal", error))?;
    requests.push(tutorial_request(
        &body,
        "fault",
        4,
        conduit_tutorial_plot::TutorialPlayback::Refused,
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
        conduit_tutorial_plot::TutorialPlayback::Playing,
    )?);
    body.lull(&host, &boot, Some(&repaired))
        .map_err(|error| proof_error("lull repaired Orifina Play", error))?;
    requests.push(tutorial_request(
        &body,
        "lulled",
        6,
        conduit_tutorial_plot::TutorialPlayback::Lulled,
    )?);
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
        7,
        conduit_tutorial_plot::TutorialPlayback::Completed,
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
