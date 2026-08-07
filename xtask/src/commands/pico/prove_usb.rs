//! Typed std -> Pico W USB-CDC session proof & interactive console runner.

use std::time::{Duration, Instant};

use conduit_core::{
    bind_active_play, BootId, ConnectionId, ConnectionProvider, ConnectionProviderInstanceId,
    FragmentId, HostId, KindId, LinkBindingId, LinkEndpoint, LinkEndpointId, LinkLimits, PlanId,
};
use conduit_signal::{encode_signal_fixed, Signal};
#[cfg(unix)]
use conduit_std_host::usb_cdc::{NativePathCdcCarrier, NativePathCdcLineReader, OperatorTerminal};
use conduit_wire::{SessionBinding, SessionMessage};

use super::doctor::repo_root;
use super::firmware::read_identity_manifest;
use super::serial::resolve_dual_ports;
use super::transcript::{self, RuntimeTranscriptIdentity};
use super::{PicoArgs, PicoResult};
use crate::cli::GlobalOpts;

pub fn run_prove_std_pico_usb(
    link_port_opt: Option<&str>,
    evidence_port_opt: Option<&str>,
    interactive: bool,
    pico_args: &PicoArgs,
    opts: &GlobalOpts,
) -> PicoResult<()> {
    if opts.dry_run || pico_args.dry_run {
        println!("==> prove std-pico-usb (dry-run)");
        println!("  firmware build: cargo check --manifest-path firmware/conduit-pico-w-signal/Cargo.toml --target thumbv6m-none-eabi");
        println!("  flash candidate: RPI-RP2 mount copy");
        println!(
            "  link port: {}",
            link_port_opt.unwrap_or("<auto-discover CDC 0>")
        );
        println!(
            "  evidence port: {}",
            evidence_port_opt.unwrap_or("<auto-discover CDC 1>")
        );
        println!("  interactive console: {}", interactive);
        return Ok(());
    }

    // 1. Resolve dual CDC ports if board is already running, or build+flash if not
    let (link_port_path, evidence_port_path) =
        match resolve_dual_ports(link_port_opt, evidence_port_opt) {
            Ok(ports) => {
                println!("==> Detected active Pico W dual CDC ports");
                ports
            }
            Err(_) => {
                println!("==> Active Pico W CDC ports not found. Flashing firmware candidate...");
                super::flash::run_flash(pico_args)?;
                println!("==> Waiting for USB CDC serial ports to enumerate...");
                std::thread::sleep(Duration::from_secs(3));
                resolve_dual_ports(link_port_opt, evidence_port_opt)?
            }
        };

    println!(
        "==> prove std-pico-usb: link port {}, evidence port {}",
        link_port_path.display(),
        evidence_port_path.display()
    );

    // Read expected compiled firmware identity manifest
    let root = repo_root();
    let identity = read_identity_manifest(&root)?;
    if identity.firmware_mode != "usb-remote" {
        return Err(format!(
            "std-to-Pico USB proof requires a usb-remote image, but the current artifact is {}; rebuild with `cargo xtask pico build --usb-remote` and flash it with `cargo xtask pico flash --usb-remote`",
            identity.firmware_mode
        )
        .into());
    }
    println!(
        "==> Loaded build identity: build_id={}, plan_id={}",
        identity.firmware_build_id, identity.generated_image.plan_id
    );

    #[cfg(unix)]
    {
        let mut attempts = 0;
        let (mut evidence_reader, mut carrier, runtime) = loop {
            attempts += 1;
            match (|| -> Result<_, String> {
                // 1. Open CDC 1 and require the kernel to report DTR high.
                let evidence_reader = NativePathCdcLineReader::open(&evidence_port_path)
                    .map_err(|e| format!("Failed to open CDC1 evidence port: {e}"))?;
                println!("==> CDC1 opened; DTR verified high");

                // 2. Open CDC 0 link port and verify DTR there independently.
                let mut carrier = NativePathCdcCarrier::open(&link_port_path, 1024)
                    .map_err(|e| format!("Failed to open CDC0 link port: {e}"))?;
                println!("==> CDC0 opened; DTR verified high");

                // 3. Settle 250 ms to race host open against Pico firmware USB loop
                std::thread::sleep(Duration::from_millis(250));

                // 4. Raw CDC0 Physical Checkpoint
                carrier
                    .send_raw_stream_frame(b"CONDUIT_RAW_CDC0_PROBE", Duration::from_secs(2))
                    .map_err(|e| format!("Failed to send raw CDC0 probe: {e}"))?;
                println!("  [Source] Sent raw CDC0 stream frame probe");

                let mut frame_buf = [0u8; 2048];
                let probe_reply = carrier
                    .receive_raw_stream_frame(&mut frame_buf, Duration::from_secs(3))
                    .map_err(|e| format!("Timed out waiting for CDC0 probe reply: {e}"))?;
                if probe_reply == b"CONDUIT_RAW_CDC0_REPLY" {
                    println!("==> CDC0 raw Pico -> host reply observed");
                } else {
                    return Err("raw CDC0 probe reply payload mismatch".into());
                }

                // 5. Read Pico W boot identity from CDC 1
                let mut reader = evidence_reader;
                let boot_line = reader
                    .read_line(Duration::from_secs(3))
                    .map_err(|e| format!("Timed out reading boot identity from CDC 1: {e}"))?;
                println!("==> CDC1 boot identity received and validated");

                let runtime = transcript::verify_boot(&boot_line, &identity)
                    .map_err(|error| error.to_string())?;

                let gpio_ready = reader
                    .read_line(Duration::from_secs(10))
                    .map_err(|e| format!("Timed out waiting for CYW43 GPIO readiness: {e}"))?;
                if gpio_ready != "CONDUIT_CYW43_GPIO_READY" {
                    return Err(format!("unexpected CDC1 startup record: {gpio_ready}"));
                }
                println!("==> Pico CYW43 GPIO service ready");

                Ok((reader, carrier, runtime))
            })() {
                Ok(res) => break res,
                Err(err) => {
                    if attempts >= 3 {
                        return Err(format!(
                            "Connection attempt failed after {attempts} retries: {err}"
                        )
                        .into());
                    }
                    println!("==> Connection attempt {attempts} failed ({err}); retrying fresh connection in 250 ms...");
                    std::thread::sleep(Duration::from_millis(250));
                }
            }
        };

        println!(
            "==> Observed Pico W runtime link/boot identity: boot_id={}, play_id={}",
            runtime.boot_id, runtime.active_play_id
        );

        // Construct truthful SessionBinding with observed Pico runtime boot/link identity
        let plan_id = PlanId::from(identity.generated_image.plan_id.as_str());
        let source_host_id = HostId::from("host/std");
        let source_boot_id = BootId::from("boot/std");
        let sink_host_id = HostId::from(identity.generated_image.host_id.as_str());
        let sink_boot_id = BootId::from(runtime.boot_id.as_str());

        let source_active_play_id =
            bind_active_play(&plan_id, &source_host_id, &source_boot_id, 0).active_play_id;
        let sink_active_play_id =
            bind_active_play(&plan_id, &sink_host_id, &sink_boot_id, 0).active_play_id;

        let binding = SessionBinding {
            protocol_version: 1,
            plan_id,
            source_fragment_id: FragmentId::from("fragment/std-source"),
            sink_fragment_id: FragmentId::from(identity.generated_image.fragment_id.as_str()),
            source_active_play_id,
            sink_active_play_id,
            connection_id: ConnectionId::from("conn/std-pico-signal"),
            link_binding_id: LinkBindingId::from("link/usb-cdc-0"),
            provider: ConnectionProvider::UsbCdc,
            provider_instance_id: ConnectionProviderInstanceId::from("pico-usb-cdc-0"),
            source: LinkEndpoint {
                host_id: source_host_id,
                boot_id: source_boot_id,
                endpoint_id: LinkEndpointId::from("endpoint/std-out"),
            },
            sink: LinkEndpoint {
                host_id: sink_host_id,
                boot_id: sink_boot_id,
                endpoint_id: LinkEndpointId::from("endpoint/pico-in"),
            },
            value_kind: KindId::from("value/signal"),
            limits: LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 9,
                maximum_buffered_bytes: 1024,
                maximum_frame_bytes: 1024,
            },
        };

        // Create stateful Source-side SessionMachine
        let mut source_machine =
            conduit_wire::SessionMachine::new(binding.clone(), conduit_wire::SessionRole::Source)
                .map_err(|e| format!("Failed to create Source SessionMachine: {e:?}"))?;

        // Handshake Step 1 (Source): Outbound Hello
        let hello = binding.hello_frame();
        source_machine
            .admit_outbound(hello)
            .map_err(|e| format!("Source failed to admit outbound Hello: {e:?}"))?;
        carrier.send_frame(&hello, Duration::from_secs(2))?;
        println!("==> std  -> pico Hello");

        // Handshake Step 2 (Source): Inbound Hello
        let mut frame_buf = [0u8; 2048];
        let start = Instant::now();
        let mut hello_received = false;
        while start.elapsed() < Duration::from_secs(5) {
            match carrier.receive_frame(&mut frame_buf, Duration::from_millis(100)) {
                Ok(res) => {
                    if matches!(res.message, SessionMessage::Hello(_)) {
                        source_machine.admit_inbound(res).map_err(|e| {
                            format!("Source failed to admit inbound Hello from Sink: {e:?}")
                        })?;
                        println!("==> pico -> std  Hello");
                        hello_received = true;
                        break;
                    }
                }
                Err(conduit_std_host::usb_cdc::NativeUsbCdcError::WouldBlock) => {}
                Err(err) => {
                    println!("  [Source SessionMachine] receive_frame error: {:?}", err);
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        if !hello_received {
            return Err("timed out waiting for SessionMessage::Hello from Pico W".into());
        }

        // Handshake Step 3 (Source): Outbound Ready
        let ready_outbound = binding.frame(SessionMessage::Ready);
        source_machine
            .admit_outbound(ready_outbound)
            .map_err(|e| format!("Source failed to admit outbound Ready: {e:?}"))?;
        carrier.send_frame(&ready_outbound, Duration::from_secs(2))?;
        println!("==> std  -> pico Ready");

        // Handshake Step 4 (Source): Inbound Ready
        let start = Instant::now();
        let mut ready_received = false;
        while start.elapsed() < Duration::from_secs(5) {
            match carrier.receive_frame(&mut frame_buf, Duration::from_millis(100)) {
                Ok(res) => {
                    if matches!(res.message, SessionMessage::Ready) {
                        source_machine.admit_inbound(res).map_err(|e| {
                            format!("Source failed to admit inbound Ready from Sink: {e:?}")
                        })?;
                        println!("==> pico -> std  Ready");
                        ready_received = true;
                        break;
                    }
                }
                Err(conduit_std_host::usb_cdc::NativeUsbCdcError::WouldBlock) => {}
                Err(err) => {
                    println!("  [Source SessionMachine] receive_frame error: {:?}", err);
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        if !ready_received {
            return Err("timed out waiting for SessionMessage::Ready from Pico W".into());
        }

        if !source_machine.is_active() {
            return Err("Source SessionMachine is not active after 4-message handshake".into());
        }

        if interactive {
            println!("\n===============================================================");
            println!(" Conduit USB-CDC Pico W Interactive Control (#350)");
            println!(" Link Port:     {}", link_port_path.display());
            println!(" Evidence Port: {}", evidence_port_path.display());
            println!("===============================================================");
            println!(
                " [Press ANY KEY]  -> Instant Button Pulse (Key Down -> LED ON -> Key Up -> LED OFF)"
            );
            println!(" [Press 'q' / ESC] -> Exit interactive session");
            println!("===============================================================\n");

            let mut term = OperatorTerminal::open().map_err(|e| {
                format!("Failed to initialize interactive operator terminal: {}", e)
            })?;
            let mut sequence = 0u64;

            loop {
                let key = match term.read_key(Duration::from_millis(100)) {
                    Ok(Some(k)) => k,
                    Ok(None) => continue,
                    Err(_) => break,
                };

                if key == b'q' || key == b'Q' || key == 3 || key == 27 {
                    println!("\n==> Exiting Pico W USB-CDC interactive session...");
                    break;
                }

                // 1. KEY DOWN: level = true (Pico LED ON)
                println!(
                    "  [KEY DOWN] Key 0x{:02x} -> Sent Signal seq {} (level: true) -> Pico LED ON",
                    key, sequence
                );
                send_and_verify_item(
                    &mut source_machine,
                    &mut carrier,
                    &mut evidence_reader,
                    &binding,
                    sequence,
                    true,
                    &identity,
                    &runtime,
                )?;

                // Hold button pulse for 250ms
                std::thread::sleep(Duration::from_millis(250));

                // 2. KEY UP: level = false (Pico LED OFF)
                sequence += 1;
                println!(
                    "  [KEY UP  ] Released -> Sent Signal seq {} (level: false) -> Pico LED OFF",
                    sequence
                );
                send_and_verify_item(
                    &mut source_machine,
                    &mut carrier,
                    &mut evidence_reader,
                    &binding,
                    sequence,
                    false,
                    &identity,
                    &runtime,
                )?;

                sequence += 1;
            }

            super::session_completion::complete(
                &mut source_machine,
                &mut carrier,
                &mut evidence_reader,
                &binding,
                sequence,
                &identity,
                &runtime,
            )?;
            println!("==> Pico W USB-CDC interactive session completed.");
            return Ok(());
        }

        println!("==> Streaming 16 Signal items over physical USB CDC link...");
        for sequence in 0..16u64 {
            let level = (sequence % 2) == 1;
            send_and_verify_item(
                &mut source_machine,
                &mut carrier,
                &mut evidence_reader,
                &binding,
                sequence,
                level,
                &identity,
                &runtime,
            )?;
        }

        super::session_completion::complete(
            &mut source_machine,
            &mut carrier,
            &mut evidence_reader,
            &binding,
            16,
            &identity,
            &runtime,
        )?;
        println!("==> Pico W USB-CDC proof completed successfully.");
    }

    Ok(())
}

#[cfg(unix)]
fn send_and_verify_item(
    source_machine: &mut conduit_wire::SessionMachine,
    carrier: &mut NativePathCdcCarrier,
    evidence_reader: &mut NativePathCdcLineReader,
    binding: &SessionBinding,
    sequence: u64,
    level: bool,
    identity: &super::firmware::FirmwareIdentity,
    runtime: &RuntimeTranscriptIdentity,
) -> PicoResult<()> {
    let signal = Signal { sequence, level };
    let payload = encode_signal_fixed(&signal);
    let offer = binding.frame(SessionMessage::Offered {
        sequence,
        payload: &payload,
    });

    // 1. Admit outbound Offered & send frame over CDC 0
    source_machine
        .admit_outbound(offer)
        .map_err(|e| format!("Source failed to admit outbound Offered: {e:?}"))?;
    carrier.send_frame(&offer, Duration::from_secs(2))?;

    // 2. Wait for Accepted(sequence) over CDC 0
    let mut frame_buf = [0u8; 2048];
    let start = Instant::now();
    let mut accepted = false;
    while start.elapsed() < Duration::from_secs(2) {
        match carrier.receive_frame(&mut frame_buf, Duration::from_millis(100)) {
            Ok(res) => {
                if matches!(res.message, SessionMessage::Accepted { sequence: s } if s == sequence)
                {
                    source_machine
                        .admit_inbound(res)
                        .map_err(|e| format!("Source failed to admit inbound Accepted: {e:?}"))?;
                    accepted = true;
                    break;
                }
            }
            Err(conduit_std_host::usb_cdc::NativeUsbCdcError::WouldBlock) => {}
            Err(err) => {
                return Err(format!("receive_frame error waiting for Accepted: {err:?}").into());
            }
        }
    }
    if !accepted {
        return Err(format!("timed out waiting for SessionMessage::Accepted {sequence}").into());
    }

    // 3. Read receipt line from CDC 1
    let line = evidence_reader
        .read_line(Duration::from_secs(2))
        .map_err(|e| {
            format!("timed out reading receipt for sequence {sequence} from CDC 1: {e}")
        })?;
    transcript::verify_receipt(&line, sequence as usize, level, identity, runtime)?;
    println!("  [RECEIPT ] <- CDC 1: {line}");

    // 4. Wait for Delivered(sequence) over CDC 0
    let start_del = Instant::now();
    let mut delivered = false;
    while start_del.elapsed() < Duration::from_secs(2) {
        match carrier.receive_frame(&mut frame_buf, Duration::from_millis(100)) {
            Ok(res) => {
                if matches!(res.message, SessionMessage::Delivered { sequence: s } if s == sequence)
                {
                    source_machine
                        .admit_inbound(res)
                        .map_err(|e| format!("Source failed to admit inbound Delivered: {e:?}"))?;
                    delivered = true;
                    break;
                }
            }
            Err(conduit_std_host::usb_cdc::NativeUsbCdcError::WouldBlock) => {}
            Err(err) => {
                return Err(format!("receive_frame error waiting for Delivered: {err:?}").into());
            }
        }
    }
    if !delivered {
        return Err(format!("timed out waiting for SessionMessage::Delivered {sequence}").into());
    }

    Ok(())
}
