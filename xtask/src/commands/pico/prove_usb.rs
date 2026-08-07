//! Typed std -> Pico W USB-CDC session proof & interactive console runner.

use std::io::{BufRead, BufReader, Write as _};
use std::time::{Duration, Instant};

use conduit_core::{
    ActivePlayId, BootId, ConnectionId, ConnectionProvider, ConnectionProviderInstanceId,
    FragmentId, HostId, KindId, LinkBindingId, LinkEndpoint, LinkEndpointId, LinkLimits, PlanId,
};
use conduit_signal::{encode_signal_fixed, Signal};
use conduit_std_host::usb_cdc::NativeUsbCdcCarrier;
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
    let (link_port_path, evidence_port_path) = match resolve_dual_ports(
        link_port_opt,
        evidence_port_opt,
    ) {
        Ok(ports) => {
            println!("==> Detected active Pico W dual CDC ports");
            ports
        }
        Err(_) => {
            super::firmware::run_build(pico_args)?;
            super::flash::run_flash(pico_args)?;

            println!("==> Waiting for USB CDC serial ports to enumerate...");
            let deadline = Instant::now() + Duration::from_secs(15);
            let mut resolved = None;
            while Instant::now() < deadline {
                if let Ok(ports) = resolve_dual_ports(link_port_opt, evidence_port_opt) {
                    resolved = Some(ports);
                    break;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
            resolved.ok_or_else(|| {
                    Box::<dyn std::error::Error>::from(
                        "timed out waiting for Pico W USB CDC ports to enumerate (/dev/ttyACM0, /dev/ttyACM1)",
                    )
                })?
        }
    };

    println!(
        "==> prove std-pico-usb: link port {}, evidence port {}",
        link_port_path.display(),
        evidence_port_path.display()
    );

    // Set baud/raw mode on both CDC serial ports
    for p in [&link_port_path, &evidence_port_path] {
        let _ = std::process::Command::new("stty")
            .args([
                "-F",
                p.to_str().ok_or("port path not UTF-8")?,
                "115200",
                "cs8",
                "-cstopb",
                "-parenb",
                "raw",
                "-echo",
            ])
            .status();
    }

    let identity = read_identity_manifest(&repo_root())?;

    // Open CDC 1 for receipt observation
    let evidence_file = std::fs::OpenOptions::new()
        .read(true)
        .open(&evidence_port_path)?;
    let mut evidence_reader = BufReader::new(evidence_file);

    // Read initial boot identity line from CDC 1
    let mut boot_line = String::new();
    evidence_reader.read_line(&mut boot_line)?;
    let boot_record: serde_json::Value = serde_json::from_str(boot_line.trim())?;
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

    // Open CDC 0 link session
    let link_file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&link_port_path)?;

    let mut carrier = NativeUsbCdcCarrier::new(link_file.try_clone()?, link_file, 512)?;

    // Construct truthful SessionBinding with observed Pico runtime boot/link identity
    let binding = SessionBinding {
        protocol_version: 1,
        plan_id: PlanId::from(identity.generated_image.plan_id.as_str()),
        source_fragment_id: FragmentId::from("fragment/std-source"),
        sink_fragment_id: FragmentId::from(identity.generated_image.fragment_id.as_str()),
        source_active_play_id: ActivePlayId::from("play/std-host"),
        sink_active_play_id: ActivePlayId::from(runtime_active_play_id.as_str()),
        connection_id: ConnectionId::from("conn/std-pico-signal"),
        link_binding_id: LinkBindingId::from("link/usb-cdc-0"),
        provider: ConnectionProvider::UsbCdc,
        provider_instance_id: ConnectionProviderInstanceId::from("pico-usb-cdc-0"),
        source: LinkEndpoint {
            host_id: HostId::from("host/std"),
            boot_id: BootId::from("boot/std"),
            endpoint_id: LinkEndpointId::from("endpoint/std-out"),
        },
        sink: LinkEndpoint {
            host_id: HostId::from(identity.generated_image.host_id.as_str()),
            boot_id: BootId::from(runtime_boot_id.as_str()),
            endpoint_id: LinkEndpointId::from("endpoint/pico-in"),
        },
        value_kind: KindId::from("value/signal"),
        limits: LinkLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: 9,
            maximum_buffered_bytes: 9,
            maximum_frame_bytes: 512,
        },
    };

    // Session frame exchange: Hello -> Ready -> Offered items
    let hello = SessionFrame {
        identity: binding.identity(),
        message: SessionMessage::Hello(conduit_wire::SessionHello {
            provider: ConnectionProvider::UsbCdc,
            source: conduit_wire::SessionEndpoint {
                host_id: "host/std",
                boot_id: "boot/std",
                endpoint_id: "endpoint/std-out",
            },
            sink: conduit_wire::SessionEndpoint {
                host_id: identity.generated_image.host_id.as_str(),
                boot_id: runtime_boot_id.as_str(),
                endpoint_id: "endpoint/pico-in",
            },
            value_kind: "value/signal",
            limits: binding.limits,
        }),
    };

    carrier.send_frame(&hello)?;
    let mut frame_buf = [0u8; 512];
    let ready_res = carrier.receive_frame(&mut frame_buf)?;
    if !matches!(ready_res.message, SessionMessage::Ready) {
        return Err("expected Ready session response from Pico W".into());
    }

    if interactive {
        println!("\n===============================================================");
        println!(" Conduit USB-CDC Pico W Interactive Control (#350)");
        println!(" Link Port:     {}", link_port_path.display());
        println!(" Evidence Port: {}", evidence_port_path.display());
        println!("===============================================================");
        println!(" [Press ENTER]     -> Toggle Pico W onboard LED ON/OFF");
        println!(" [Type 'q' + ENTER] -> Exit interactive session");
        println!("===============================================================\n");

        let mut sequence = 0u64;
        let mut level = false;
        let mut stdin_lines = BufReader::new(std::io::stdin()).lines();

        loop {
            print!("> ");
            let _ = std::io::stdout().flush();
            let line = match stdin_lines.next() {
                Some(Ok(l)) => l,
                _ => break,
            };

            let trimmed = line.trim();
            if trimmed == "q" || trimmed == "quit" || trimmed == "exit" {
                println!("==> Closing Pico W USB-CDC session...");
                break;
            }

            level = !level;
            let signal = Signal { sequence, level };
            let payload = encode_signal_fixed(&signal);

            let offer = SessionFrame {
                identity: binding.identity(),
                message: SessionMessage::Offered {
                    sequence,
                    payload: &payload,
                },
            };

            carrier.send_frame(&offer)?;
            println!(
                "  [-> USB CDC] Sent Signal sequence {} -> Pico LED {}",
                sequence,
                if level { "ON" } else { "OFF" }
            );

            let mut receipt_line = String::new();
            if evidence_reader.read_line(&mut receipt_line)? > 0 {
                println!("  [<- CDC 1] Receipt: {}", receipt_line.trim());
            }

            sequence += 1;
        }

        let terminal = SessionFrame {
            identity: binding.identity(),
            message: SessionMessage::Terminal {
                disposition: conduit_wire::SessionTerminalDisposition::Completed,
                final_sequence: sequence,
            },
        };
        let _ = carrier.send_frame(&terminal);
        println!("==> Pico W USB-CDC interactive session completed.");
        return Ok(());
    }

    println!("==> Streaming 16 Signal items over physical USB CDC link...");
    for sequence in 0..16u64 {
        let level = (sequence % 2) == 1;
        let signal = Signal { sequence, level };
        let payload = encode_signal_fixed(&signal);

        let offer = SessionFrame {
            identity: binding.identity(),
            message: SessionMessage::Offered {
                sequence,
                payload: &payload,
            },
        };

        carrier.send_frame(&offer)?;

        let mut receipt_line = String::new();
        evidence_reader.read_line(&mut receipt_line)?;
        let receipt_record: serde_json::Value = serde_json::from_str(receipt_line.trim())?;
        if receipt_record["sequence"].as_u64() != Some(sequence) {
            return Err(format!("sequence mismatch in receipt: expected {sequence}").into());
        }
    }

    println!("==> Physical std -> Pico W USB-CDC remote session acceptance passed 100%!");
    Ok(())
}
