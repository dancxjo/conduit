use clap::{Args, ValueEnum};
use conduit_body::{
    AuthenticatedHostObservation, Body, BodyMembership, MembershipProofId, PartId, Wake,
};
use conduit_core::{bind_active_play, BootId, HostAdvertisement, SignId};
use conduit_plan_lowering::lowering::RemoteCordDirection;
use conduit_semantic_catalog::{
    exact_body_coordination_line_loss, exact_body_coordination_plan, BodyCoordinationPlan,
    BODY_COORDINATION_MAXIMUM_FRAME_BYTES, FOREBRAIN_TO_MOTHERBRAIN_LINE,
    MOTHERBRAIN_TO_FOREBRAIN_LINE,
};
use conduit_std_host::body_coordination::{
    run_forebrain, run_in_process, run_motherbrain, BodyCoordinationReceipt, CoordinationEndpoint,
    CoordinationRole,
};
use conduit_std_host::websocket::NativeWebSocketListener;
use serde::Serialize;
use std::net::SocketAddr;
use std::path::PathBuf;

use crate::cli::GlobalOpts;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum BodyCoordinationMode {
    Conformance,
    Forebrain,
    Motherbrain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum CoordinationLossDirection {
    ForebrainToMotherbrain,
    MotherbrainToForebrain,
}

impl CoordinationLossDirection {
    fn line_id(self) -> &'static str {
        match self {
            Self::ForebrainToMotherbrain => FOREBRAIN_TO_MOTHERBRAIN_LINE,
            Self::MotherbrainToForebrain => MOTHERBRAIN_TO_FOREBRAIN_LINE,
        }
    }
}

#[derive(Args, Debug)]
pub struct BodyCoordinationArgs {
    /// Run deterministic conformance or one exact live Host role.
    #[arg(value_enum)]
    pub mode: BodyCoordinationMode,

    /// Exact forebrain Host Boot identity observed for this run.
    #[arg(long)]
    pub forebrain_boot: String,

    /// Exact motherbrain Host Boot identity observed for this run.
    #[arg(long)]
    pub motherbrain_boot: String,

    /// Planned forebrain listener address for its outbound Line.
    #[arg(long)]
    pub forebrain_bind: Option<SocketAddr>,

    /// Planned motherbrain listener address for its reply Line.
    #[arg(long)]
    pub motherbrain_bind: Option<SocketAddr>,

    /// Explicitly create the two development Part admissions. Reachability is insufficient.
    #[arg(long)]
    pub admit_parts: bool,

    /// Retain this Host's bounded machine-readable receipt.
    #[arg(long)]
    pub evidence_out: Option<PathBuf>,

    /// Deterministically remove one selected Line and retain the required-replan refusal.
    #[arg(long, value_enum)]
    pub induce_line_loss: Option<CoordinationLossDirection>,
}

#[derive(Serialize)]
struct CoordinationEvidence {
    schema: &'static str,
    body: Body,
    wake: Wake,
    membership: BodyMembership,
    receipt: BodyCoordinationReceipt,
}

#[derive(Serialize)]
struct CoordinationLineLossEvidence {
    schema: &'static str,
    body_id: String,
    forebrain_part_id: String,
    motherbrain_part_id: String,
    prior_plan_id: String,
    unavailable_line_id: String,
    outcome: &'static str,
    refusal: String,
    prior_plan_immutable: bool,
    play_started: bool,
    authority: &'static str,
}

struct BodyTruth {
    body: Body,
    wake: Wake,
    membership: BodyMembership,
    forebrain_part: PartId,
    motherbrain_part: PartId,
}

pub fn run(
    args: BodyCoordinationArgs,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    if !args.admit_parts {
        return Err("body coordination requires explicit --admit-parts; reachability cannot create membership".into());
    }
    let forebrain_boot = BootId::from(args.forebrain_boot.as_str());
    let motherbrain_boot = BootId::from(args.motherbrain_boot.as_str());
    let base_instance = match (args.forebrain_bind, args.motherbrain_bind) {
        (Some(forebrain), Some(motherbrain)) => {
            format!("wifi/interbrain/{forebrain}/{motherbrain}")
        }
        (None, None) if args.mode == BodyCoordinationMode::Conformance => {
            "wifi/interbrain/conformance".to_string()
        }
        _ => return Err("live coordination requires both exact bind addresses".into()),
    };
    let exact = exact_body_coordination_plan(forebrain_boot, motherbrain_boot, &base_instance)?;
    let truth = body_truth(&exact, args.induce_line_loss.is_none())?;
    if opts.dry_run {
        println!(
            "coordination-plan body={} plan={} forebrain={} motherbrain={} lines=2 cords=2 authority=none",
            truth.body.body_id.as_str(),
            exact.plan.plan_id.as_str(),
            exact.forebrain.boot_id.as_str(),
            exact.motherbrain.boot_id.as_str(),
        );
        return Ok(());
    }

    if let Some(direction) = args.induce_line_loss {
        if args.mode != BodyCoordinationMode::Conformance {
            return Err("--induce-line-loss is a deterministic conformance action only".into());
        }
        let loss = exact_body_coordination_line_loss(
            exact.forebrain.boot_id.clone(),
            exact.motherbrain.boot_id.clone(),
            &base_instance,
            direction.line_id(),
        )?;
        if loss.plan_id != exact.plan.plan_id || !loss.replan_required {
            return Err("Line loss did not preserve the prior Plan or require replanning".into());
        }
        let receipt = [CoordinationLineLossEvidence {
            schema: "conduit.pete/body-coordination-line-loss@1",
            body_id: truth.body.body_id.as_str().into(),
            forebrain_part_id: truth.forebrain_part.as_str().into(),
            motherbrain_part_id: truth.motherbrain_part.as_str().into(),
            prior_plan_id: loss.plan_id.as_str().into(),
            unavailable_line_id: loss.unavailable_line_id.as_str().into(),
            outcome: "replan_required",
            refusal: loss.refusal,
            prior_plan_immutable: true,
            play_started: false,
            authority: "none",
        }];
        emit(&receipt, args.evidence_out.as_ref(), opts)?;
        return Ok(());
    }

    match args.mode {
        BodyCoordinationMode::Conformance => {
            let mut forebrain = CoordinationEndpoint::prepare(&exact, &exact.forebrain.host_id)?;
            let mut motherbrain =
                CoordinationEndpoint::prepare(&exact, &exact.motherbrain.host_id)?;
            run_in_process(&mut forebrain, &mut motherbrain)?;
            let receipts = [
                evidence(&exact, &truth, CoordinationRole::Forebrain, &forebrain)?,
                evidence(&exact, &truth, CoordinationRole::Motherbrain, &motherbrain)?,
            ];
            emit(&receipts, args.evidence_out.as_ref(), opts)?;
        }
        BodyCoordinationMode::Forebrain => {
            let forebrain_bind = args.forebrain_bind.expect("live addresses checked");
            let motherbrain_bind = args.motherbrain_bind.expect("live addresses checked");
            let listener = NativeWebSocketListener::bind_planned(
                forebrain_bind,
                motherbrain_bind.ip(),
                BODY_COORDINATION_MAXIMUM_FRAME_BYTES,
            )
            .map_err(|error| format!("bind forebrain Line: {error:?}"))?;
            if !opts.quiet {
                println!("coordination-ready role=forebrain bind={forebrain_bind}");
            }
            let mut endpoint = CoordinationEndpoint::prepare(&exact, &exact.forebrain.host_id)?;
            run_forebrain(&mut endpoint, listener, motherbrain_bind)?;
            emit(
                &[evidence(
                    &exact,
                    &truth,
                    CoordinationRole::Forebrain,
                    &endpoint,
                )?],
                args.evidence_out.as_ref(),
                opts,
            )?;
        }
        BodyCoordinationMode::Motherbrain => {
            let forebrain_bind = args.forebrain_bind.expect("live addresses checked");
            let motherbrain_bind = args.motherbrain_bind.expect("live addresses checked");
            let listener = NativeWebSocketListener::bind_planned(
                motherbrain_bind,
                forebrain_bind.ip(),
                BODY_COORDINATION_MAXIMUM_FRAME_BYTES,
            )
            .map_err(|error| format!("bind motherbrain Line: {error:?}"))?;
            if !opts.quiet {
                println!("coordination-ready role=motherbrain bind={motherbrain_bind}");
            }
            let mut endpoint = CoordinationEndpoint::prepare(&exact, &exact.motherbrain.host_id)?;
            run_motherbrain(&mut endpoint, listener, forebrain_bind)?;
            emit(
                &[evidence(
                    &exact,
                    &truth,
                    CoordinationRole::Motherbrain,
                    &endpoint,
                )?],
                args.evidence_out.as_ref(),
                opts,
            )?;
        }
    }
    Ok(())
}

fn body_truth(
    exact: &BodyCoordinationPlan,
    start_play: bool,
) -> Result<BodyTruth, Box<dyn std::error::Error>> {
    let body = Body::born(
        exact.plan.source_document_id.clone(),
        exact.plan.checked_form_id.clone(),
        1,
        SignId::from("pete-orinthrop/body-born"),
    )
    .map_err(debug)?;
    let (body, wake) = body
        .wake(1, SignId::from("pete-orinthrop/body-woke"))
        .map_err(debug)?;
    let wake = wake
        .plan_ready(
            &exact.plan,
            SignId::from("pete-orinthrop/coordination-plan-ready"),
        )
        .map_err(debug)?;
    // The current Body lifecycle anchors a distributed Play at its steward
    // Host. Each fragment still retains its own exact Host/Boot ActivePlay.
    let steward_play = bind_active_play(
        &exact.plan.plan_id,
        &exact.forebrain.host_id,
        &exact.forebrain.boot_id,
        0,
    );
    let wake = if start_play {
        wake.play_started(
            &steward_play,
            SignId::from("pete-orinthrop/coordination-play-started"),
        )
        .map_err(debug)?
    } else {
        wake
    };
    let mut membership = BodyMembership::new(body.body_id.clone()).map_err(debug)?;
    let forebrain_part = admit(&body, &mut membership, &exact.forebrain, "forebrain", 1)?;
    let motherbrain_part = admit(&body, &mut membership, &exact.motherbrain, "motherbrain", 2)?;
    membership.validate().map_err(debug)?;
    Ok(BodyTruth {
        body,
        wake,
        membership,
        forebrain_part,
        motherbrain_part,
    })
}

fn admit(
    body: &Body,
    membership: &mut BodyMembership,
    host: &HostAdvertisement,
    durable_subject: &str,
    sequence: u64,
) -> Result<PartId, Box<dyn std::error::Error>> {
    let part = PartId::bind(&body.body_id, durable_subject, sequence).map_err(debug)?;
    let proof = MembershipProofId::bind(&format!(
        "operator-explicit-development-admission/{durable_subject}"
    ))
    .map_err(debug)?;
    membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            SignId::from(format!("pete-orinthrop/{durable_subject}/admitted")),
        )
        .map_err(debug)?;
    membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                offer_generation: host.offer_generation,
                proof_id: proof,
                sequence: 1,
            },
            SignId::from(format!("pete-orinthrop/{durable_subject}/present")),
        )
        .map_err(debug)?;
    Ok(part)
}

fn evidence(
    exact: &BodyCoordinationPlan,
    truth: &BodyTruth,
    role: CoordinationRole,
    endpoint: &CoordinationEndpoint,
) -> Result<CoordinationEvidence, Box<dyn std::error::Error>> {
    let (part, peer_part, host, peer) = match role {
        CoordinationRole::Forebrain => (
            &truth.forebrain_part,
            &truth.motherbrain_part,
            &exact.forebrain,
            &exact.motherbrain,
        ),
        CoordinationRole::Motherbrain => (
            &truth.motherbrain_part,
            &truth.forebrain_part,
            &exact.motherbrain,
            &exact.forebrain,
        ),
    };
    let outbound = endpoint.binding(RemoteCordDirection::Egress);
    let inbound = endpoint.binding(RemoteCordDirection::Ingress);
    let expected_play = bind_active_play(&exact.plan.plan_id, &host.host_id, &host.boot_id, 0);
    if endpoint.identity().active_play_id != expected_play.active_play_id {
        return Err("kernel ActivePlay identity mismatch".into());
    }
    let receipt = BodyCoordinationReceipt {
        schema: BodyCoordinationReceipt::SCHEMA.into(),
        role,
        body_id: truth.body.body_id.as_str().into(),
        part_id: part.as_str().into(),
        peer_part_id: peer_part.as_str().into(),
        host_id: host.host_id.as_str().into(),
        boot_id: host.boot_id.as_str().into(),
        peer_host_id: peer.host_id.as_str().into(),
        peer_boot_id: peer.boot_id.as_str().into(),
        plan_id: exact.plan.plan_id.as_str().into(),
        fragment_id: endpoint.fragment().fragment_id.as_str().into(),
        active_play_id: expected_play.active_play_id.as_str().into(),
        outbound_cord_id: endpoint.cord(RemoteCordDirection::Egress).0,
        inbound_cord_id: endpoint.cord(RemoteCordDirection::Ingress).0,
        outbound_line_id: outbound.attachment.line_id.as_str().into(),
        inbound_line_id: inbound.attachment.line_id.as_str().into(),
        base_instance_id: outbound.attachment.base_instance_id.as_str().into(),
        offered: true,
        accepted: true,
        delivered: true,
        input_closed: true,
        terminal: "completed".into(),
        received: endpoint.received().into(),
        authority: "none".into(),
    };
    receipt.validate()?;
    Ok(CoordinationEvidence {
        schema: "conduit.pete/body-coordination-evidence@1",
        body: truth.body.clone(),
        wake: truth.wake.clone(),
        membership: truth.membership.clone(),
        receipt,
    })
}

fn emit<T: Serialize>(
    evidence: &[T],
    output: Option<&PathBuf>,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = serde_json::to_vec_pretty(evidence)?;
    if let Some(path) = output {
        std::fs::write(path, &bytes)?;
    }
    if opts.json || (output.is_none() && !opts.quiet) {
        println!("{}", String::from_utf8(bytes)?);
    }
    Ok(())
}

fn debug(error: impl core::fmt::Debug) -> Box<dyn std::error::Error> {
    format!("{error:?}").into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_admission_binds_two_parts_without_authority() {
        let exact = exact_body_coordination_plan(
            BootId::from("forebrain/boot-1"),
            BootId::from("motherbrain/boot-1"),
            "wifi/interbrain/conformance",
        )
        .unwrap();
        let truth = body_truth(&exact, true).unwrap();
        assert_eq!(truth.membership.parts.len(), 2);
        assert!(truth.membership.parts.iter().all(|part| part.is_present()));
        assert_ne!(truth.forebrain_part, truth.motherbrain_part);
    }
}
