use std::{
    fs,
    io::{Read, Write},
    net::{Shutdown, TcpListener},
    process::{Command, Stdio},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use crate::cli::GlobalOpts;
use conduitos::virtio_tls_fixture::PINNED_CERTIFICATE_DER;
use rustls::{
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
    ServerConfig, ServerConnection, StreamOwned,
};

use super::{build, image, profile::Paths, report::ArtifactRole, ConduitosArch, ConduitosError};

const PREFIX: &str = "CONDUIT_VIRTIO_NET_SIGN ";
const REQUEST: &[u8] = b"CONDUIT TCP PING\n";
const RESPONSE: &[u8] = b"CONDUIT TCP PONG\n";
const PRIVATE_KEY_DER: &[u8] = &[
    0x30, 0x81, 0x87, 0x02, 0x01, 0x00, 0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02,
    0x01, 0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07, 0x04, 0x6d, 0x30, 0x6b, 0x02,
    0x01, 0x01, 0x04, 0x20, 0x22, 0x93, 0x96, 0xc2, 0x6e, 0xe3, 0x82, 0x7f, 0x73, 0x17, 0xfd, 0xf8,
    0x0b, 0x34, 0xfb, 0x5e, 0xae, 0x10, 0x4b, 0xcc, 0x7e, 0xb1, 0xf0, 0xa9, 0xe7, 0x48, 0xfc, 0x59,
    0x12, 0x84, 0xa0, 0x51, 0xa1, 0x44, 0x03, 0x42, 0x00, 0x04, 0x8e, 0x09, 0x9b, 0x49, 0xcd, 0x99,
    0x18, 0x09, 0xcc, 0x9c, 0x0e, 0xa2, 0x5b, 0x56, 0xce, 0x37, 0x25, 0x2c, 0xfa, 0xcd, 0x05, 0x08,
    0xfb, 0x58, 0xce, 0x4b, 0xc6, 0xa4, 0xa1, 0x7a, 0x15, 0xcd, 0xc8, 0x8b, 0x46, 0x76, 0xaf, 0x1c,
    0xf4, 0x4f, 0xd8, 0x9d, 0xc1, 0xb6, 0xbd, 0x06, 0xcf, 0x51, 0x8c, 0x6a, 0x66, 0x40, 0xb9, 0x5e,
    0xd1, 0x71, 0x12, 0xd0, 0xce, 0xe5, 0xd3, 0x19, 0x60, 0xff,
];

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
        || sign["schema"] != "conduit.conduitos/virtio-tls-proof@1"
        || sign["remote_ip"] != "10.0.2.100"
        || sign["remote_port"] != 9000
        || sign["server_name"] != "relay.conduit.invalid"
        || sign["certificate_sha256"]
            != "b58b58d2cfc273d464dd6dfaa5eacc8d5b0b404b236839af0360f78caebe7648"
        || sign["plaintext_transmitted_bytes"] != REQUEST.len() as u64
        || sign["plaintext_received_bytes"] != RESPONSE.len() as u64
        || sign["tcp_claimed"] != true
        || sign["tls_claimed"] != true
        || sign["websocket_claimed"] != false
        || sign["bounded"] != true
    {
        return Err(ConduitosError::refusal(
            "virtio-net-proof-sign-mismatch",
            sign.to_string(),
        ));
    }
    if !opts.quiet && !opts.json {
        println!("PROVED x86_64 ConduitOS bounded pinned TLS exchange");
    }
    Ok(())
}

fn serve_once(listener: TcpListener) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(20);
    let stream = loop {
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
    let provider = rustls::crypto::ring::default_provider();
    let config = ServerConfig::builder_with_provider(Arc::new(provider))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|error| error.to_string())?
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(PINNED_CERTIFICATE_DER.to_vec())],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(PRIVATE_KEY_DER.to_vec())),
        )
        .map_err(|error| error.to_string())?;
    let connection = ServerConnection::new(Arc::new(config)).map_err(|error| error.to_string())?;
    let mut tls = StreamOwned::new(connection, stream);
    let mut request = [0; REQUEST.len()];
    tls.read_exact(&mut request)
        .map_err(|error| error.to_string())?;
    if request != REQUEST {
        return Err(format!("unexpected request: {request:?}"));
    }
    tls.write_all(RESPONSE).map_err(|error| error.to_string())?;
    tls.flush().map_err(|error| error.to_string())?;
    tls.conn.send_close_notify();
    tls.flush().map_err(|error| error.to_string())?;
    tls.sock
        .shutdown(Shutdown::Write)
        .map_err(|error| error.to_string())?;
    Ok(())
}
