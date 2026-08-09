//! Physical same-Plan continuation from WebSocket to USB CDC.

use std::time::Duration;

use conduit_core::{
    bind_active_play, BootId, ClueId, ConnectionBase, LinkAvailability, LinkBinding,
};
use conduit_std_host::pico_control_source::PicoControlSource;
use conduit_std_host::r1_control::{R1ControlPeer, R1InputEvent};
use conduit_std_host::usb_cdc::{NativePathCdcCarrier, NativePathCdcLineReader};
use conduit_wire::{
    decode_session_checkpoint, encode_session_checkpoint_into, SessionMessage, SessionResumeAction,
};
use serde::Serialize;

use super::super::firmware::FirmwareIdentity;
use super::super::r1_control_session;
use super::super::r1_signal::{R1SessionIo, UsbSessionIo, WebSocketSessionIo};
use super::super::transcript::RuntimeTranscriptIdentity;
use super::super::PicoResult;

pub(crate) fn verify_plan_c_continuation(
    usb: &mut NativePathCdcCarrier,
    clue: &mut NativePathCdcLineReader,
    identity: &FirmwareIdentity,
    runtime: &RuntimeTranscriptIdentity,
    interactive: bool,
) -> PicoResult<()> {
    if !interactive {
        return Err("physical R1 Plan C network-loss proof requires --interactive".into());
    }
    let plan = conduit_system_continuity::exact_r1_control_plan(
        BootId::from(conduit_net::R1_PICO_BOOT_ID),
        conduit_system_continuity::R1SignalRouteSet::WebSocketThenUsb,
    )?
    .plan;
    let mut websocket = super::connect_with_query(
        usb,
        clue,
        identity,
        runtime,
        conduit_net::R1_PLAN_C_WEBSOCKET_BASE_QUERY,
    )?;
    super::super::usb_continuity::verify(usb, identity)?;

    let source_host = conduit_core::HostId::from(conduit_net::R1_STD_HOST_ID);
    let source_boot = BootId::from(conduit_net::R1_STD_BOOT_ID);
    let mut source = PicoControlSource::prepare_plan(plan.clone(), &source_host)?;
    source.observe_sink_boot(BootId::from(runtime.boot_id.as_str()))?;
    let initial_binding = source.binding().clone();

    let body = conduit_body::Body::born(
        plan.source_document_id.clone(),
        plan.checked_form_id.clone(),
        0,
        ClueId::from("r1/physical/plan-c-body-born"),
    )
    .map_err(lifecycle_error)?;
    let (body, wake) = body
        .wake(0, ClueId::from("r1/physical/plan-c-woke"))
        .map_err(lifecycle_error)?;
    let wake = wake
        .plan_ready(&plan, ClueId::from("r1/physical/plan-c-ready"))
        .map_err(lifecycle_error)?;
    let play = bind_active_play(&plan.plan_id, &source_host, &source_boot, 0);
    let wake = wake
        .play_started(&play, ClueId::from("r1/physical/plan-c-play-started"))
        .map_err(lifecycle_error)?;

    let (sequence, payload) = {
        let mut websocket_io = WebSocketSessionIo::new(&mut websocket);
        r1_control_session::handshake(&mut websocket_io, &mut source)?;
        let link_line = clue
            .read_line(Duration::from_secs(3))
            .map_err(|error| format!("timed out reading Plan C WebSocket link Sign: {error}"))?;
        super::verify_link_clue(&link_line, identity, runtime, source.binding())?;
        let (sequence, payload) = source.offer_input(R1InputEvent {
            peer: R1ControlPeer::Terminal,
            peer_sequence: 0,
            level: true,
        })?;
        let binding = source.binding().clone();
        let offered = binding.frame(SessionMessage::Offered {
            sequence,
            payload: &payload,
        });
        source.admit_outbound(offered)?;

        println!("==> Plan C retained offer {sequence}; remove real Wi-Fi/network availability, then press Enter");
        let mut confirmation = String::new();
        std::io::stdin().read_line(&mut confirmation)?;
        match websocket_io.send(&offered) {
            Err(error) if error.to_string().starts_with("WebSocket ") => {}
            Err(error) => {
                return Err(format!(
                    "Plan C failed without an exact WebSocket transport-unavailable result: {error}"
                )
                .into())
            }
            Ok(()) => {
                return Err(
                    "WebSocket Plan C remained writable after the declared physical fault".into(),
                )
            }
        }
        (sequence, payload)
    };
    drop(websocket);

    let mut source_checkpoint = [0_u8; 2048];
    let checkpoint_len =
        encode_session_checkpoint_into(source.checkpoint_offer(), &mut source_checkpoint, 2048)
            .map_err(wire_error)?;
    usb.send_raw_stream_frame(&source_checkpoint[..checkpoint_len], Duration::from_secs(2))?;
    let mut sink_checkpoint = [0_u8; 2048];
    let sink_raw = usb.receive_raw_stream_frame(&mut sink_checkpoint, Duration::from_secs(3))?;
    let sink_offer = decode_session_checkpoint(sink_raw, 2048).map_err(wire_error)?;
    let usb_link = usb_candidate(&plan)?;
    let acceptance = source.resume_with_link(&usb_link, sink_offer)?;
    if acceptance.action != SessionResumeAction::ReplayOffered(sequence)
        || !acceptance.same_plan_continues
    {
        return Err(
            "Plan C checkpoint reconciliation did not require the exact pending replay".into(),
        );
    }
    if source.binding().plan_id != initial_binding.plan_id
        || source.binding().source_active_play_id != initial_binding.source_active_play_id
        || source.binding().sink_active_play_id != initial_binding.sink_active_play_id
        || source.binding().attachment.base != ConnectionBase::UsbCdc
    {
        return Err("Plan C or Play identity changed during USB continuation".into());
    }
    let wake = wake
        .same_plan_observed(
            &plan.plan_id,
            ClueId::from("r1/physical/plan-c-usb-selected"),
        )
        .map_err(lifecycle_error)?;

    let mut usb_io = UsbSessionIo::new(usb);
    r1_control_session::handshake(&mut usb_io, &mut source)?;
    let merged = r1_control_session::replay_offered(
        &mut usb_io,
        &mut source,
        sequence,
        &payload,
        &mut |found| {
            let line = clue
                .read_line(Duration::from_secs(3))
                .map_err(|error| format!("missing replayed Plan C LED Sign: {error}"))?;
            super::super::r1_signal_transcript::verify_receipt(
                &line, &plan, found, identity, runtime,
            )
        },
    )?;
    super::super::r1_live_control::emit_physical_input_sign(&plan, &merged)?;
    for input in super::control_inputs().into_iter().skip(1) {
        let merged =
            r1_control_session::deliver_input(&mut usb_io, &mut source, input, &mut |found| {
                let line = clue
                    .read_line(Duration::from_secs(3))
                    .map_err(|error| format!("missing continued Plan C LED Sign: {error}"))?;
                super::super::r1_signal_transcript::verify_receipt(
                    &line, &plan, found, identity, runtime,
                )
            })?;
        super::super::r1_live_control::emit_physical_input_sign(&plan, &merged)?;
    }
    let delivered = r1_control_session::finish(&mut usb_io, &mut source)?;
    if delivered != 6 {
        return Err("continued Plan C did not deliver the exact six deliberate inputs".into());
    }
    let terminal = clue
        .read_line(Duration::from_secs(3))
        .map_err(|error| format!("missing continued Plan C terminal Sign: {error}"))?;
    super::super::r1_signal_transcript::verify_terminal(&terminal, &plan, identity, runtime)?;

    let lifecycle = super::super::r1_lifecycle::lull_and_wake(
        &body,
        &wake,
        source.is_terminal(),
        1,
        super::super::r1_lifecycle::R1LullClues {
            wake_lulled: ClueId::from("r1/physical/plan-c-wake-lulled"),
            body_retained: ClueId::from("r1/physical/plan-c-body-retained"),
            later_wake: ClueId::from("r1/physical/plan-c-later-wake"),
        },
    )?;

    let outcome = PhysicalPlanCContinuationOutcome {
        schema: "conduit.r1/same-plan-continuation-hil@1",
        proof_class: "physical-cross-host",
        fault: "operator-confirmed-real-wifi-or-network-unavailability",
        body_id: body.body_id.as_str(),
        wake_id: wake.wake_id.as_str(),
        pico_host_id: conduit_net::R1_PICO_HOST_ID,
        pico_boot_id: runtime.boot_id.as_str(),
        plan_id: source.binding().plan_id.as_str(),
        source_play_id: source.binding().source_active_play_id.as_str(),
        sink_play_id: source.binding().sink_active_play_id.as_str(),
        planner_requested: false,
        websocket_ready_before_fault: true,
        usb_cdc_ready_before_fault: true,
        websocket_unavailable_after_fault: true,
        initially_selected_base: "websocket",
        continued_selected_base: "usb-cdc",
        websocket_link_binding_id: conduit_net::R1_WEBSOCKET_LINK_BINDING_ID,
        usb_cdc_link_binding_id: conduit_net::R1_USB_LINK_BINDING_ID,
        reconciliation: "replay-offered",
        same_plan_continues: true,
        delivered_values: delivered,
        lifecycle: &lifecycle,
        branch_c_physical_acceptance: true,
    };
    println!("{}", serde_json::to_string(&outcome)?);
    println!("==> Physical Plan C same-Plan/same-Play continuation completed");
    Ok(())
}

fn lifecycle_error(error: conduit_body::BodyLifecycleError) -> Box<dyn std::error::Error> {
    format!("Plan C Body lifecycle rejected: {error:?}").into()
}

fn wire_error(error: conduit_wire::WireError) -> Box<dyn std::error::Error> {
    format!("Plan C checkpoint wire rejected: {error:?}").into()
}

fn usb_candidate(plan: &conduit_core::Plan) -> PicoResult<LinkBinding> {
    let connection = super::remote_connection(plan)?;
    let candidate = connection
        .route_candidates
        .iter()
        .find(|candidate| candidate.base == ConnectionBase::UsbCdc)
        .ok_or("Plan C has no sealed USB CDC candidate")?;
    Ok(LinkBinding {
        binding_id: candidate.binding_id.clone(),
        source: candidate.source.clone(),
        sink: candidate.sink.clone(),
        base: candidate.base,
        base_instance_id: candidate.base_instance_id.clone(),
        availability: LinkAvailability::Ready,
        credential: candidate.credential.clone(),
        authority: candidate.authority.clone(),
        limits: candidate.limits,
    })
}

#[derive(Serialize)]
struct PhysicalPlanCContinuationOutcome<'a> {
    schema: &'static str,
    proof_class: &'static str,
    fault: &'static str,
    body_id: &'a str,
    wake_id: &'a str,
    pico_host_id: &'static str,
    pico_boot_id: &'a str,
    plan_id: &'a str,
    source_play_id: &'a str,
    sink_play_id: &'a str,
    planner_requested: bool,
    websocket_ready_before_fault: bool,
    usb_cdc_ready_before_fault: bool,
    websocket_unavailable_after_fault: bool,
    initially_selected_base: &'static str,
    continued_selected_base: &'static str,
    websocket_link_binding_id: &'static str,
    usb_cdc_link_binding_id: &'static str,
    reconciliation: &'static str,
    same_plan_continues: bool,
    delivered_values: u64,
    lifecycle: &'a super::super::r1_lifecycle::R1LullSign,
    branch_c_physical_acceptance: bool,
}
