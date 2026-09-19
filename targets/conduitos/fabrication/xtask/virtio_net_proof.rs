use std::{
    fs,
    io::{Read, Write},
    net::{Shutdown, TcpListener},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::cli::GlobalOpts;

use super::{build, image, profile::Paths, report::ArtifactRole, ConduitosArch, ConduitosError};

const PREFIX: &str = "CONDUIT_VIRTIO_NET_SIGN ";
const REQUEST: &[u8] = b"CONDUIT TCP PING\n";
const RESPONSE: &[u8] = b"CONDUIT TCP PONG\n";

pub(super) fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    build::execute_virtio_net_proof(opts)?
        .artifact_role
        .require(ArtifactRole::ArchitectureProofAppliance)?;
    image::assemble_architecture_proof(ConduitosArch::X86_64, opts)?;
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-virtio-net-proof",
            "dry-run cannot manufacture device traffic",
        ));
    }
    let paths = Paths::new(ConduitosArch::X86_64)?;
    let serial_path = paths.target.join("virtio-net-proof.log");
    let packet_path = paths.target.join("virtio-net-proof.pcap");
    let _ = fs::remove_file(&serial_path);
    let _ = fs::remove_file(&packet_path);
    let serial = format!("file:{}", serial_path.to_string_lossy());
    let packet_trace = format!(
        "filter-dump,id=conduit-net-trace,netdev=conduit-net,file={}",
        packet_path.to_string_lossy()
    );
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| ConduitosError::refusal("virtio-tcp-listen-failed", error.to_string()))?;
    listener.set_nonblocking(true).map_err(|error| {
        ConduitosError::refusal("virtio-tcp-listen-config-failed", error.to_string())
    })?;
    let host_port = listener
        .local_addr()
        .map_err(|error| {
            ConduitosError::refusal("virtio-tcp-listen-address-failed", error.to_string())
        })?
        .port();
    let server = thread::spawn(move || serve_once(listener));
    let netdev = format!(
        "user,id=conduit-net,restrict=on,guestfwd=tcp:10.0.2.100:9000-tcp:127.0.0.1:{host_port}"
    );
    let mut child = Command::new("qemu-system-x86_64")
        .args([
            "-M",
            "q35",
            "-cpu",
            "max",
            "-m",
            "64M",
            "-smp",
            "1",
            "-display",
            "none",
            "-monitor",
            "none",
            "-serial",
            &serial,
            "-no-reboot",
            "-netdev",
            &netdev,
            "-device",
            "virtio-net-pci,netdev=conduit-net,disable-modern=on,rx_queue_size=256,tx_queue_size=256,mac=52:54:00:12:34:56",
            "-object",
            &packet_trace,
            "-device",
            "isa-debug-exit,iobase=0xf4,iosize=0x04",
            "-cdrom",
        ])
        .arg(&paths.iso)
        .args(["-boot", "d"])
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| ConduitosError::refusal("missing-qemu", error.to_string()))?;
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|error| {
            ConduitosError::refusal("virtio-net-proof-wait-failed", error.to_string())
        })? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ConduitosError::refusal(
                "virtio-net-proof-timeout",
                fs::read_to_string(&serial_path).unwrap_or_default(),
            ));
        }
        thread::sleep(Duration::from_millis(10));
    };
    let transcript = fs::read_to_string(&serial_path).unwrap_or_default();
    if status.code() != Some(33) {
        let stderr = child
            .wait_with_output()
            .map(|output| String::from_utf8_lossy(&output.stderr).into_owned())
            .unwrap_or_default();
        return Err(ConduitosError::refusal(
            "virtio-net-proof-guest-failed",
            format!("{status}; {stderr}; {transcript}"),
        ));
    }
    server
        .join()
        .map_err(|_| {
            ConduitosError::refusal("virtio-tcp-server-panicked", "server thread panicked")
        })?
        .map_err(|reason| ConduitosError::refusal("virtio-tcp-server-failed", reason))?;
    let signs = transcript
        .lines()
        .filter_map(|line| line.strip_prefix(PREFIX))
        .collect::<Vec<_>>();
    if signs.len() != 1 {
        return Err(ConduitosError::refusal(
            "virtio-net-proof-sign-count",
            format!("expected one Sign, found {}; {transcript}", signs.len()),
        ));
    }
    let sign: serde_json::Value = serde_json::from_str(signs[0]).map_err(|error| {
        ConduitosError::refusal("virtio-net-proof-sign-invalid", error.to_string())
    })?;
    if sign["status"] != "completed"
        || sign["proof_class"] != "freestanding-emulator"
        || sign["device"] != "virtio-net-pci-transitional"
        || sign["mac"] != "52:54:00:12:34:56"
        || sign["queue_entries"] != 256
        || sign["entropy_provider_generation"] != 1
        || sign["entropy_requests"] != 1
        || sign["schema"] != "conduit.conduitos/virtio-tcp-proof@1"
        || sign["remote_ip"] != "10.0.2.100"
        || sign["remote_port"] != 9000
        || sign["tcp_transmitted_bytes"] != REQUEST.len() as u64
        || sign["tcp_received_bytes"] != RESPONSE.len() as u64
        || sign["tcp_claimed"] != true
        || sign["tls_claimed"] != false
        || sign["websocket_claimed"] != false
        || sign["bounded"] != true
    {
        return Err(ConduitosError::refusal(
            "virtio-net-proof-sign-mismatch",
            sign.to_string(),
        ));
    }
    if !opts.quiet && !opts.json {
        println!("PROVED x86_64 ConduitOS bounded VirtIO-net TCP exchange");
    }
    Ok(())
}

fn serve_once(listener: TcpListener) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err("timed out waiting for the guest TCP session".into());
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(error.to_string()),
        }
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| error.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| error.to_string())?;
    let mut request = [0; REQUEST.len()];
    stream
        .read_exact(&mut request)
        .map_err(|error| error.to_string())?;
    if request != REQUEST {
        return Err(format!("unexpected request: {request:?}"));
    }
    stream
        .write_all(RESPONSE)
        .map_err(|error| error.to_string())?;
    stream
        .shutdown(Shutdown::Write)
        .map_err(|error| error.to_string())?;
    Ok(())
}
