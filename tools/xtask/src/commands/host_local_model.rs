use crate::cli::GlobalOpts;
use conduit_ai::LocalModelKindProfile;
use conduit_body::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyMembership, MembershipProofId,
    PartId, ResidentForm,
};
use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_std_host::hosted_local_model::{HostedLocalModelAdapter, OllamaDiscovery};

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
    let presenter_requests = if orifina_presenter {
        let intended = conduit_workspace_model::tutorial::generative_request(
            &orifina_body()?,
            "request/workspace/orifina/provider-proof".into(),
            1,
            conduit_workspace_model::tutorial::TutorialPlayback::Lulled,
        )
        .map_err(|error| proof_error("build Workspace Orifina request", error))?;
        conduit_std_host::local_model_proof::presenter_policy_experiment(intended)
    } else {
        Vec::new()
    };
    let receipt = conduit_std_host::local_model_proof::run(adapter, &presenter_requests)?;
    if opts.json {
        println!("{}", serde_json::to_string(&receipt)?);
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
                host_id: HostId::from("host/orifina-provider-proof"),
                boot_id: BootId::from("boot/orifina-provider-proof"),
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

fn proof_error(stage: &str, error: impl core::fmt::Debug) -> Box<dyn std::error::Error> {
    std::io::Error::other(format!("{stage}: {error:?}")).into()
}
