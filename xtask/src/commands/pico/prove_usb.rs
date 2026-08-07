//! Typed std -> Pico W USB-CDC session proof & interactive console runner.

use std::io::{BufRead, BufReader};
use std::time::{Duration, Instant};

use conduit_core::{
    bind_active_play, BootId, ConnectionId, ConnectionProvider, ConnectionProviderInstanceId,
    FragmentId, HostId, KindId, LinkBindingId, LinkEndpoint, LinkEndpointId, LinkLimits, PlanId,
};
use conduit_signal::{encode_signal_fixed, Signal};
#[cfg(unix)]
use conduit_std_host::usb_cdc::{NativePathCdcCarrier, OperatorTerminal};
use conduit_wire::{SessionBinding, SessionFrame, SessionMessage};

use super::doctor::repo_root;
use super::firmware::read_identity_manifest;
use super::serial::resolve_dual_ports;
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
    println!(
        "==> Loaded build identity: build_id={}, plan_id={}",
        identity.firmware_build_id, identity.generated_image.plan_id
    );

    // 2. Open CDC 1 evidence port in background thread
    let evidence_file = std::fs::OpenOptions::new()
        .read(true)
        .open(&evidence_port_path)
        .map_err(|e| {
            format!(
                "Failed to open evidence port {}: {}",
                evidence_port_path.display(),
                e
            )
        })?;

    let (evidence_tx, evidence_rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(evidence_file);
        let mut line = String::new();
        while reader.read_line(&mut line).unwrap_or(0) > 0 {
            let trimmed = line.trim().to_string();
            if !trimmed.is_empty() {
                let _ = evidence_tx.send(trimmed);
            }
            line.clear();
        }
    });

    println!("==> Starting 5 second blocking wait for Pico W boot identity from CDC 1...");
    let boot_line = evidence_rx
        .recv_timeout(Duration::from_secs(5))
        .map_err(|_| {
            "timed out reading Pico boot identity from CDC 1 (5 seconds elapsed without data)"
        })?;

    println!(
        "==> Received boot line ({} bytes): {}",
        boot_line.len(),
        boot_line
    );

    let boot_record: serde_json::Value = serde_json::from_str(&boot_line)?;
    let runtime_boot_id = boot_record["runtime_boot_id"]
        .as_str()
        .ok_or("missing runtime_boot_id in Pico boot identity record")?
        .to_string();
    let runtime_active_play_id = boot_record["runtime_active_play_id"]
        .as_str()
        .ok_or("missing runtime_active_play_id in Pico boot identity record")?
        .to_string();

    println!(
        "==> Observed Pico W runtime link/boot identity: boot_id={}, play_id={}",
        runtime_boot_id, runtime_active_play_id
    );

    // Open CDC 0 native path carrier
    #[cfg(unix)]
    let mut carrier = NativePathCdcCarrier::open(&link_port_path, 1024).map_err(|e| {
        format!(
            "Failed to open native CDC port {}: {}",
            link_port_path.display(),
            e
        )
    })?;

    // Construct truthful SessionBinding with observed Pico runtime boot/link identity
    let plan_id = PlanId::from(identity.generated_image.plan_id.as_str());
    let source_host_id = HostId::from("host/std");
    let source_boot_id = BootId::from("boot/std");
    let sink_host_id = HostId::from(identity.generated_image.host_id.as_str());
    let sink_boot_id = BootId::from(runtime_boot_id.as_str());

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

    #[cfg(unix)]
    {
        // 3. Raw CDC0 Physical Checkpoint before SessionMachine
        println!("==> Executing raw CDC0 bidirectional physical checkpoint...");
        carrier.send_raw_stream_frame(b"CONDUIT_RAW_CDC0_PROBE", Duration::from_secs(2))?;
        println!("  [Source] Sent raw CDC0 stream frame probe");

        let mut frame_buf = [0u8; 2048];
        let probe_reply =
            carrier.receive_raw_stream_frame(&mut frame_buf, Duration::from_secs(5))?;
        if probe_reply == b"CONDUIT_RAW_CDC0_REPLY" {
            println!("==> CDC0 raw bidirectional checkpoint passed");
        } else {
            return Err("raw CDC0 probe reply payload mismatch".into());
        }
    }

    // Create stateful Source-side SessionMachine
    let mut source_machine =
        conduit_wire::SessionMachine::new(binding.clone(), conduit_wire::SessionRole::Source)
            .map_err(|e| format!("Failed to create Source SessionMachine: {e:?}"))?;

    println!("==> Initiating stateful 4-message SessionMachine handshake over USB CDC 0...");

    #[cfg(unix)]
    {
        // Handshake Step 1 (Source): Admit outbound Hello on Source SessionMachine and send over CDC 0
        let hello = binding.hello_frame();
        source_machine
            .admit_outbound(hello)
            .map_err(|e| format!("Source failed to admit outbound Hello: {e:?}"))?;
        carrier.send_frame(&hello, Duration::from_secs(2))?;
        println!("  [Source SessionMachine] Sent outbound SessionFrame::Hello");

        // Handshake Step 2 (Source): Receive inbound Hello from Sink over CDC 0
        let mut frame_buf = [0u8; 2048];
        let start = Instant::now();
        let mut hello_received = false;
        while start.elapsed() < Duration::from_secs(5) {
            match carrier.receive_frame(&mut frame_buf, Duration::from_millis(100)) {
                Ok(res) => {
                    println!(
                        "  [Source SessionMachine] Received frame: {:?}",
                        res.message
                    );
                    if matches!(res.message, SessionMessage::Hello(_)) {
                        source_machine.admit_inbound(res).map_err(|e| {
                            format!("Source failed to admit inbound Hello from Sink: {e:?}")
                        })?;
                        println!("  [Source SessionMachine] Admitted inbound Hello from Sink");
                        hello_received = true;
                        break;
                    }
                }
                Err(conduit_std_host::usb_cdc::NativeUsbCdcError::WouldBlock) => {}
                Err(err) => {
                    println!("  [Source SessionMachine] receive_frame error: {:?}", err);
                }
            }

            while let Ok(evidence_line) = evidence_rx.try_recv() {
                println!("  [Pico evidence CDC 1]: {}", evidence_line);
            }

            std::thread::sleep(Duration::from_millis(50));
        }

        if !hello_received {
            return Err("timed out waiting for SessionMessage::Hello from Pico W".into());
        }

        // Handshake Step 3 (Source): Admit outbound Ready on Source SessionMachine and send over CDC 0
        let ready_outbound = binding.frame(SessionMessage::Ready);
        source_machine
            .admit_outbound(ready_outbound)
            .map_err(|e| format!("Source failed to admit outbound Ready: {e:?}"))?;
        carrier.send_frame(&ready_outbound, Duration::from_secs(2))?;
        println!("  [Source SessionMachine] Sent outbound SessionMessage::Ready");

        // Handshake Step 4 (Source): Receive inbound Ready from Sink over CDC 0
        let start = Instant::now();
        let mut ready_received = false;
        while start.elapsed() < Duration::from_secs(5) {
            match carrier.receive_frame(&mut frame_buf, Duration::from_millis(100)) {
                Ok(res) => {
                    println!(
                        "  [Source SessionMachine] Received frame: {:?}",
                        res.message
                    );
                    if matches!(res.message, SessionMessage::Ready) {
                        source_machine.admit_inbound(res).map_err(|e| {
                            format!("Source failed to admit inbound Ready from Sink: {e:?}")
                        })?;
                        println!("  [Source SessionMachine] Admitted inbound Ready from Sink");
                        ready_received = true;
                        break;
                    }
                }
                Err(conduit_std_host::usb_cdc::NativeUsbCdcError::WouldBlock) => {}
                Err(err) => {
                    println!("  [Source SessionMachine] receive_frame error: {:?}", err);
                }
            }

            while let Ok(evidence_line) = evidence_rx.try_recv() {
                println!("  [Pico evidence CDC 1]: {}", evidence_line);
            }

            std::thread::sleep(Duration::from_millis(50));
        }

        if !ready_received {
            return Err("timed out waiting for SessionMessage::Ready from Pico W".into());
        }

        if !source_machine.is_active() {
            return Err("Source SessionMachine is not active after 4-message handshake".into());
        }

        println!(
            "==> Real two-sided SessionMachine handshake complete (source_machine.is_active() == true)"
        );

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
                    &evidence_rx,
                    &binding,
                    sequence,
                    true,
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
                    &evidence_rx,
                    &binding,
                    sequence,
                    false,
                )?;

                sequence += 1;
            }

            let terminal = SessionFrame {
                identity: binding.identity(),
                message: SessionMessage::Terminal {
                    disposition: conduit_wire::SessionTerminalDisposition::Completed,
                    final_sequence: sequence,
                },
            };
            let _ = carrier.send_frame(&terminal, Duration::from_secs(1));
            println!("==> Pico W USB-CDC interactive session completed.");
            return Ok(());
        }

        println!("==> Streaming 16 Signal items over physical USB CDC link...");
        for sequence in 0..16u64 {
            let level = (sequence % 2) == 1;
            send_and_verify_item(
                &mut source_machine,
                &mut carrier,
                &evidence_rx,
                &binding,
                sequence,
                level,
            )?;
        }

        let terminal = SessionFrame {
            identity: binding.identity(),
            message: SessionMessage::Terminal {
                disposition: conduit_wire::SessionTerminalDisposition::Completed,
                final_sequence: 16,
            },
        };
        let _ = carrier.send_frame(&terminal, Duration::from_secs(1));
        println!("==> Pico W USB-CDC proof completed successfully.");
    }

    Ok(())
}

#[cfg(unix)]
fn send_and_verify_item(
    source_machine: &mut conduit_wire::SessionMachine,
    carrier: &mut NativePathCdcCarrier,
    evidence_rx: &std::sync::mpsc::Receiver<String>,
    binding: &SessionBinding,
    sequence: u64,
    level: bool,
) -> PicoResult<()> {
    let signal = Signal { sequence, level };
    let payload = encode_signal_fixed(&signal);
    let offer = binding.frame(SessionMessage::Offered {
        sequence,
        payload: &payload,
    });

    // 1. Admit outbound Offered & send frame
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

        while let Ok(line) = evidence_rx.try_recv() {
            if line.starts_with('{') {
                println!("  [RECEIPT ] <- CDC 1: {}", line);
            } else {
                println!("  [Pico log] <- CDC 1: {}", line);
            }
        }

        std::thread::sleep(Duration::from_millis(10));
    }
    if !accepted {
        return Err(format!("timed out waiting for SessionMessage::Accepted {sequence}").into());
    }

    // 3. Read receipt line from CDC 1
    let start_ev = Instant::now();
    let mut receipt_received = false;
    while start_ev.elapsed() < Duration::from_secs(2) {
        while let Ok(line) = evidence_rx.try_recv() {
            if line.starts_with('{') {
                println!("  [RECEIPT ] <- CDC 1: {}", line);
                receipt_received = true;
            } else {
                println!("  [Pico log] <- CDC 1: {}", line);
            }
        }
        if receipt_received {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

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

        while let Ok(line) = evidence_rx.try_recv() {
            if line.starts_with('{') {
                println!("  [RECEIPT ] <- CDC 1: {}", line);
            } else {
                println!("  [Pico log] <- CDC 1: {}", line);
            }
        }

        std::thread::sleep(Duration::from_millis(10));
    }
    if !delivered {
        return Err(format!("timed out waiting for SessionMessage::Delivered {sequence}").into());
    }

    Ok(())
}
