//! Deterministic Body lifecycle fixtures for the local-model Orifina proof.
use conduit_body::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyMembership, BodyPlayIdentity,
    BodyPlotPlan, MembershipProofId, PartId, ResidentPlot,
};
use conduit_core::{
    bind_sign, seal_plan, BootId, ExpandedPlotId, HostId, OfferGeneration, PlotIdentity,
};

pub(super) fn orifina_body(
) -> Result<conduit_body::BodyLifecycleSession, Box<dyn std::error::Error>> {
    let plot = ResidentPlot::new("source/morse".into(), "checked/morse".into());
    let body = Body::born(
        plot.source_document_id,
        plot.checked_plot_id,
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
    conduit_body::BodyLifecycleSession::open(evidence)
        .map_err(|error| proof_error("open Orifina Workspace Body", error))
}

pub(super) fn orifina_plans(body: &conduit_body::BodyLifecycleSession) -> Vec<BodyPlotPlan> {
    body.evidence()
        .body
        .workset
        .plots()
        .iter()
        .map(|plot| BodyPlotPlan {
            plot: plot.clone(),
            plan: seal_plan(
                PlotIdentity {
                    source_document_id: plot.source_document_id.clone(),
                    checked_plot_id: plot.checked_plot_id.clone(),
                    expanded_plot_id: ExpandedPlotId::from("expanded/orifina-proof"),
                },
                vec![],
            ),
        })
        .collect()
}

pub(super) fn start_orifina(
    body: &mut conduit_body::BodyLifecycleSession,
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

pub(super) fn admit_orifina_companion(
    body: &mut conduit_body::BodyLifecycleSession,
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
    *body = conduit_body::BodyLifecycleSession::open(evidence)
        .map_err(|error| proof_error("reopen distributed Orifina Body", error))?;
    Ok(())
}

pub(super) fn tutorial_request(
    body: &conduit_body::BodyLifecycleSession,
    stage: &str,
    revision: u64,
    playback: conduit_tutorial_plot::TutorialPlayback,
) -> Result<conduit_presentation::GenerativePresenterRequest, Box<dyn std::error::Error>> {
    conduit_tutorial_plot::generative_request(
        body,
        format!("request/workspace/orifina/journey/{stage}"),
        revision,
        playback,
    )
    .map_err(|error| proof_error("build Workspace Orifina request", error))
}

pub(super) fn proof_error(stage: &str, error: impl core::fmt::Debug) -> Box<dyn std::error::Error> {
    std::io::Error::other(format!("{stage}: {error:?}")).into()
}
