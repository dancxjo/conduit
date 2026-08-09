//! Physical R1 proof for the attachment-dependent Pico WebSocket route.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;

use super::firmware::FirmwareIdentity;
use super::r1_signal::{self, UsbSessionIo, WebSocketSessionIo};
use super::transcript::RuntimeTranscriptIdentity;
use super::PicoResult;
use conduit_core::BootId;
use conduit_std_host::pico_usb_source::PicoUsbSource;
use conduit_std_host::usb_cdc::{NativePathCdcCarrier, NativePathCdcLineReader};
use conduit_std_host::websocket::NativeWebSocketCarrier;
use serde::Serialize;

pub(super) fn verify(
    usb: &mut NativePathCdcCarrier,
    clue: &mut NativePathCdcLineReader,
    identity: &FirmwareIdentity,
    runtime: &RuntimeTranscriptIdentity,
) -> PicoResult<()> {
    let mut websocket = connect(usb, clue, identity, runtime)?;
    let (plan_a, _) = recovery_plans()?;
    let source_host = conduit_core::HostId::from(conduit_net::R1_STD_HOST_ID);
    let mut source = PicoUsbSource::prepare_plan(plan_a, &source_host)?;
    source.observe_sink_boot(BootId::from(runtime.boot_id.as_str()))?;
    let mut websocket_io = WebSocketSessionIo::new(&mut websocket);
    r1_signal::handshake(&mut websocket_io, &mut source)?;
    let link_line = clue
        .read_line(Duration::from_secs(3))
        .map_err(|error| format!("timed out reading WebSocket link Sign: {error}"))?;
    verify_link_clue(&link_line, identity, runtime, source.binding())?;
    super::usb_continuity::verify(usb, identity)?;
    websocket
        .close()
        .map_err(|error| format!("failed to close WebSocket carrier: {error:?}"))?;
    println!("==> Physical WebSocket Session, exact route Sign, and simultaneous USB continuity verified");
    Ok(())
}

fn connect(
    usb: &mut NativePathCdcCarrier,
    clue: &mut NativePathCdcLineReader,
    identity: &FirmwareIdentity,
    runtime: &RuntimeTranscriptIdentity,
) -> PicoResult<NativeWebSocketCarrier> {
    usb.send_raw_stream_frame(conduit_net::R1_WEBSOCKET_BASE_QUERY, Duration::from_secs(2))?;
    let mut raw = [0_u8; 2048];
    if usb.receive_raw_stream_frame(&mut raw, Duration::from_secs(3))?
        != conduit_net::R1_WEBSOCKET_BASE_READY
    {
        return Err("Pico returned an unexpected WebSocket Base readiness payload".into());
    }
    usb.send_raw_stream_frame(
        conduit_net::R1_WEBSOCKET_ENDPOINT_CLUE_READY,
        Duration::from_secs(2),
    )?;
    let endpoint_line = clue
        .read_line(Duration::from_secs(3))
        .map_err(|error| format!("timed out reading WebSocket endpoint Sign: {error}"))?;
    let address = verify_endpoint_clue(&endpoint_line, identity, runtime)?;
    let socket_address = SocketAddr::V4(SocketAddrV4::new(address, conduit_net::R1_WEBSOCKET_PORT));
    let url = format!("ws://{socket_address}/conduit");
    let websocket =
        NativeWebSocketCarrier::connect(socket_address, &url, conduit_net::R1_MAXIMUM_FRAME_BYTES)
            .map_err(|error| format!("failed to connect bounded WebSocket carrier: {error:?}"))?;
    Ok(websocket)
}

pub(super) fn verify_new_plan_recovery(
    usb: &mut NativePathCdcCarrier,
    clue: &mut NativePathCdcLineReader,
    identity: &FirmwareIdentity,
    runtime: &RuntimeTranscriptIdentity,
    interactive: bool,
) -> PicoResult<()> {
    if !interactive {
        return Err("physical R1 network-loss proof requires --interactive".into());
    }
    let mut websocket = connect(usb, clue, identity, runtime)?;
    super::usb_continuity::verify(usb, identity)?;

    let (plan_a, plan_b) = recovery_plans()?;
    let plan_a_connection = remote_connection(&plan_a)?;
    let plan_b_connection = remote_connection(&plan_b)?;
    if plan_a_connection.connection_id != plan_b_connection.connection_id
        || plan_a_connection.source_placement_id != plan_b_connection.source_placement_id
        || plan_a_connection.source_port_id != plan_b_connection.source_port_id
        || plan_a_connection.sink_placement_id != plan_b_connection.sink_placement_id
        || plan_a_connection.sink_port_id != plan_b_connection.sink_port_id
    {
        return Err("replacement Plan changed the semantic Cord identity".into());
    }
    let cord_connection_id = plan_a_connection.connection_id.clone();
    let source_host = conduit_core::HostId::from(conduit_net::R1_STD_HOST_ID);
    let source_boot = BootId::from(conduit_net::R1_STD_BOOT_ID);
    let mut recovery = conduit_system_continuity::R1NewPlanRecovery::begin(
        plan_a.clone(),
        conduit_core::GearId::from("show"),
        1,
        1,
        source_host.clone(),
        source_boot.clone(),
        0,
        conduit_system_continuity::R1RecoveryStartClues {
            birth: conduit_core::ClueId::from("r1/physical/body-born"),
            wake: conduit_core::ClueId::from("r1/physical/body-woke"),
            plan_ready: conduit_core::ClueId::from("r1/physical/plan-a-ready"),
            play_started: conduit_core::ClueId::from("r1/physical/play-a-started"),
        },
    )
    .map_err(|error| format!("failed to begin physical R1 recovery record: {error:?}"))?;
    let mut source_a = PicoUsbSource::prepare_plan(plan_a.clone(), &source_host)?;
    source_a.observe_sink_boot(BootId::from(runtime.boot_id.as_str()))?;
    {
        let mut websocket_io = WebSocketSessionIo::new(&mut websocket);
        r1_signal::handshake(&mut websocket_io, &mut source_a)?;
        let link_line = clue
            .read_line(Duration::from_secs(3))
            .map_err(|error| format!("timed out reading WebSocket link Sign: {error}"))?;
        verify_link_clue(&link_line, identity, runtime, source_a.binding())?;
        for _ in 0..2 {
            if !r1_signal::deliver_next(&mut websocket_io, &mut source_a, &mut |sequence| {
                let line = clue
                    .read_line(Duration::from_secs(3))
                    .map_err(|error| format!("missing Plan A physical LED Sign: {error}"))?;
                super::r1_signal_transcript::verify_receipt(
                    &line, &plan_a, sequence, identity, runtime,
                )
            })? {
                return Err("Plan A ended before physical LED on/off control".into());
            }
        }

        println!("==> Plan A delivered physical LED off/on over WebSocket");
        println!("==> Remove real Wi-Fi/network availability now, then press Enter");
        let mut confirmation = String::new();
        std::io::stdin().read_line(&mut confirmation)?;
        match r1_signal::deliver_next(&mut websocket_io, &mut source_a, &mut |_| Ok(())) {
            Err(error) if error.to_string().starts_with("WebSocket ") => source_a.cancel()?,
            Err(error) => {
                return Err(format!(
                    "Plan A failed without an exact WebSocket transport-unavailable result: {error}"
                )
                .into())
            }
            Ok(_) => {
                return Err(
                    "WebSocket Plan A remained usable after the declared physical fault".into(),
                )
            }
        }
    }
    drop(websocket);

    recovery
        .observe_route_unavailable(
            conduit_core::LinkObservation {
                binding_id: conduit_core::LinkBindingId::from(
                    conduit_net::R1_WEBSOCKET_LINK_BINDING_ID,
                ),
                availability: conduit_core::LinkAvailability::Unavailable,
                clue_id: conduit_core::ClueId::from("r1/physical/websocket-line-unavailable"),
            },
            conduit_core::ClueId::from("r1/physical/play-a-unsatisfied"),
        )
        .map_err(|error| format!("failed recording physical Line loss: {error:?}"))?;
    recovery
        .install_replacement(
            plan_b.clone(),
            source_host.clone(),
            source_boot.clone(),
            source_host.clone(),
            source_boot,
            0,
            conduit_system_continuity::R1ReplacementClues {
                request: conduit_core::ClueId::from("r1/physical/replan-requested"),
                planned: conduit_core::ClueId::from("r1/physical/plan-b-planned"),
                superseded: conduit_core::ClueId::from("r1/physical/plan-a-superseded"),
                realized: conduit_core::ClueId::from("r1/physical/plan-b-realized"),
                play_started: conduit_core::ClueId::from("r1/physical/play-b-started"),
            },
        )
        .map_err(|error| format!("failed recording physical replacement Plan: {error:?}"))?;

    let mut source_b = PicoUsbSource::prepare_plan(plan_b.clone(), &source_host)?;
    source_b.observe_sink_boot(BootId::from(runtime.boot_id.as_str()))?;
    let mut usb_io = UsbSessionIo::new(usb);
    let delivered = r1_signal::run_to_completion(&mut usb_io, &mut source_b, &mut |sequence| {
        let line = clue
            .read_line(Duration::from_secs(3))
            .map_err(|error| format!("missing Plan B physical LED Sign: {error}"))?;
        super::r1_signal_transcript::verify_receipt(&line, &plan_b, sequence, identity, runtime)
    })?;
    if delivered != 16 {
        return Err("USB Plan B did not deliver the exact sixteen-value Signal".into());
    }
    let terminal = clue
        .read_line(Duration::from_secs(3))
        .map_err(|error| format!("missing Plan B terminal Sign: {error}"))?;
    super::r1_signal_transcript::verify_terminal(&terminal, &plan_b, identity, runtime)?;
    let plan_b_id = recovery
        .plan_b()
        .ok_or("physical recovery record has no Plan B")?
        .plan_id
        .clone();
    let play_b_id = recovery
        .play_b()
        .ok_or("physical recovery record has no Play B")?
        .active_play_id
        .clone();
    recovery
        .record_led_result(
            conduit_core::HostId::from(conduit_net::R1_PICO_HOST_ID),
            BootId::from(runtime.boot_id.as_str()),
            plan_b_id,
            play_b_id,
            conduit_core::ClueId::from("r1/physical/plan-b-led-result"),
            true,
        )
        .map_err(|error| format!("failed recording physical Plan B LED result: {error:?}"))?;
    let outcome = PhysicalNewPlanRecoveryOutcome {
        schema: "conduit.r1/new-plan-recovery-hil@1",
        proof_class: "physical-cross-host",
        fault: "operator-confirmed-real-wifi-or-network-unavailability",
        body_id: recovery.body().body_id.as_str(),
        wake_id: recovery.wake().wake_id.as_str(),
        pico_host_id: conduit_net::R1_PICO_HOST_ID,
        pico_boot_id: runtime.boot_id.as_str(),
        checked_form_id: plan_a.checked_form_id.as_str(),
        cord_connection_id: cord_connection_id.as_str(),
        plan_a_id: recovery.plan_a().plan_id.as_str(),
        play_a_id: recovery.play_a().active_play_id.as_str(),
        plan_b_id: recovery.plan_b().expect("checked above").plan_id.as_str(),
        play_b_id: recovery
            .play_b()
            .expect("checked above")
            .active_play_id
            .as_str(),
        planner_requested: true,
        websocket_ready_before_fault: true,
        usb_cdc_ready_before_fault: true,
        websocket_unavailable_after_fault: true,
        initially_selected_base: "websocket",
        replacement_selected_base: "usb-cdc",
        plan_a_base: "websocket",
        plan_a_link_binding_id: conduit_net::R1_WEBSOCKET_LINK_BINDING_ID,
        plan_a_base_instance_id: conduit_net::R1_WEBSOCKET_BASE_INSTANCE_ID,
        plan_b_base: "usb-cdc",
        plan_b_link_binding_id: conduit_net::R1_USB_LINK_BINDING_ID,
        plan_b_base_instance_id: conduit_net::R1_USB_BASE_INSTANCE_ID,
        control_events: recovery.events(),
        led_results: recovery.led_results(),
        branch_a_physical_acceptance: true,
    };
    println!("{}", serde_json::to_string(&outcome)?);
    println!("==> Physical WebSocket-to-USB new-Plan recovery execution completed");
    Ok(())
}

#[derive(Serialize)]
struct PhysicalNewPlanRecoveryOutcome<'a> {
    schema: &'static str,
    proof_class: &'static str,
    fault: &'static str,
    body_id: &'a str,
    wake_id: &'a str,
    pico_host_id: &'static str,
    pico_boot_id: &'a str,
    checked_form_id: &'a str,
    cord_connection_id: &'a str,
    plan_a_id: &'a str,
    play_a_id: &'a str,
    plan_b_id: &'a str,
    play_b_id: &'a str,
    planner_requested: bool,
    websocket_ready_before_fault: bool,
    usb_cdc_ready_before_fault: bool,
    websocket_unavailable_after_fault: bool,
    initially_selected_base: &'static str,
    replacement_selected_base: &'static str,
    plan_a_base: &'static str,
    plan_a_link_binding_id: &'static str,
    plan_a_base_instance_id: &'static str,
    plan_b_base: &'static str,
    plan_b_link_binding_id: &'static str,
    plan_b_base_instance_id: &'static str,
    control_events: &'a [conduit_core::ControlLoopEvent],
    led_results: &'a [conduit_system_continuity::R1LedResultClue],
    branch_a_physical_acceptance: bool,
}

fn recovery_plans() -> PicoResult<(conduit_core::Plan, conduit_core::Plan)> {
    let planned_boot = BootId::from(conduit_net::R1_PICO_BOOT_ID);
    let plan_a = conduit_system_continuity::exact_r1_signal_plan(
        planned_boot.clone(),
        conduit_system_continuity::R1SignalRouteSet::WebSocketOnly,
    )?;
    let plan_b = conduit_system_continuity::exact_r1_signal_plan(
        planned_boot,
        conduit_system_continuity::R1SignalRouteSet::UsbOnly,
    )?;
    Ok((plan_a.plan, plan_b.plan))
}

fn remote_connection(plan: &conduit_core::Plan) -> PicoResult<&conduit_core::PlannedConnection> {
    plan.fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .find(|connection| !connection.route_candidates.is_empty())
        .ok_or_else(|| "R1 Plan has no remote Cord realization".into())
}

fn verify_endpoint_clue(
    line: &str,
    identity: &FirmwareIdentity,
    runtime: &RuntimeTranscriptIdentity,
) -> PicoResult<Ipv4Addr> {
    let record: serde_json::Value = serde_json::from_str(line)
        .map_err(|error| format!("malformed WebSocket endpoint Sign: {error}"))?;
    verify_fields(
        &record,
        &[
            ("schema", "conduit.network/websocket-endpoint-clue@1"),
            ("firmware_build_id", identity.firmware_build_id.as_str()),
            ("host_id", identity.generated_image.host_id.as_str()),
            ("runtime_boot_id", runtime.boot_id.as_str()),
            ("attachment_id", "r1/pico-network-attachment-1"),
            ("interface_pool_id", conduit_net::R1_WIFI_STATION_POOL_ID),
            (
                "base_instance_id",
                conduit_net::R1_WEBSOCKET_BASE_INSTANCE_ID,
            ),
            (
                "sink_endpoint_id",
                conduit_net::R1_PICO_WEBSOCKET_ENDPOINT_ID,
            ),
        ],
    )?;
    if record["port"].as_u64() != Some(u64::from(conduit_net::R1_WEBSOCKET_PORT))
        || record["maximum_frame_bytes"].as_u64()
            != Some(u64::from(conduit_net::R1_MAXIMUM_FRAME_BYTES))
    {
        return Err("WebSocket endpoint Sign bounds mismatched".into());
    }
    let octets = record["ipv4"]
        .as_array()
        .ok_or("WebSocket endpoint IPv4 is absent")?;
    if octets.len() != 4 {
        return Err("WebSocket endpoint IPv4 is malformed".into());
    }
    let mut address = [0_u8; 4];
    for (target, value) in address.iter_mut().zip(octets) {
        *target = u8::try_from(
            value
                .as_u64()
                .ok_or("WebSocket endpoint IPv4 is malformed")?,
        )?;
    }
    let address = Ipv4Addr::from(address);
    if address.is_unspecified() || address.is_loopback() || address.is_multicast() {
        return Err("WebSocket endpoint is not a usable LAN address".into());
    }
    Ok(address)
}

fn verify_link_clue(
    line: &str,
    identity: &FirmwareIdentity,
    runtime: &RuntimeTranscriptIdentity,
    binding: &conduit_wire::SessionBinding,
) -> PicoResult<()> {
    let record: serde_json::Value = serde_json::from_str(line)
        .map_err(|error| format!("malformed WebSocket link Sign: {error}"))?;
    verify_fields(
        &record,
        &[
            ("schema", "conduit.network/websocket-link-clue@1"),
            ("firmware_build_id", identity.firmware_build_id.as_str()),
            ("host_id", identity.generated_image.host_id.as_str()),
            ("runtime_boot_id", runtime.boot_id.as_str()),
            (
                "websocket_active_play_id",
                binding.sink_active_play_id.as_str(),
            ),
            ("attachment_id", "r1/pico-network-attachment-1"),
            ("usb_link_binding_id", conduit_net::R1_USB_LINK_BINDING_ID),
            (
                "websocket_link_binding_id",
                conduit_net::R1_WEBSOCKET_LINK_BINDING_ID,
            ),
            (
                "base_instance_id",
                conduit_net::R1_WEBSOCKET_BASE_INSTANCE_ID,
            ),
            (
                "source_endpoint_id",
                conduit_net::R1_STD_WEBSOCKET_ENDPOINT_ID,
            ),
            (
                "sink_endpoint_id",
                conduit_net::R1_PICO_WEBSOCKET_ENDPOINT_ID,
            ),
            ("clue_id", conduit_net::R1_WEBSOCKET_ROUTE_CLUE_ID),
        ],
    )?;
    if record["handshake"].as_bool() != Some(true)
        || record["maximum_frame_bytes"].as_u64()
            != Some(u64::from(conduit_net::R1_MAXIMUM_FRAME_BYTES))
    {
        return Err("WebSocket link Sign handshake or bound mismatched".into());
    }
    Ok(())
}

fn verify_fields(record: &serde_json::Value, expected: &[(&str, &str)]) -> PicoResult<()> {
    for (field, value) in expected {
        if record[*field].as_str() != Some(*value) {
            return Err(format!("WebSocket Sign field `{field}` mismatched").into());
        }
    }
    Ok(())
}
