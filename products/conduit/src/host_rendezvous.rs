//! One-use, code-addressed attachment of an already-running std Host.
//!
//! The rendezvous code only locates and authenticates a finite carrier
//! session. Body membership remains an explicit invitation proof completed by
//! the Body-side admission manager.

use conduit_body::{BodyConversationContext, MembershipCredential, SpawnInvitationClaim};
use conduit_core::{HostAdvertisement, PoolRealizationEnvelope, PoolRealizationObservation};
use conduit_std_host::websocket::{
    NativeWebSocketError, NativeWebSocketLine, NativeWebSocketListener,
};
use conduit_wire::{decode_session_frame, SessionMessage};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Read, Write};
use std::path::Path;
use std::time::Duration;

use crate::cli::RendezvousCarrier;

mod secure;
pub(crate) use secure::SecureNetworkOptions;

const PROTOCOL: u16 = 1;
const MAXIMUM_FRAME_BYTES: usize = 96 * 1024;
const MAXIMUM_ID_BYTES: usize = 192;
const CODE_PREFIX: &str = "C1-WS";
const SERIAL_CODE_PREFIX: &str = "C1-SERIAL";

trait RendezvousLine {
    fn receive(&mut self) -> Result<Vec<u8>, String>;
    fn send(&mut self, bytes: &[u8]) -> Result<(), String>;
    fn close(&mut self) -> Result<(), String>;
    fn enter_retained_idle(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn poll_receive(&mut self, _timeout: Duration) -> Result<Option<Vec<u8>>, String> {
        Ok(Some(self.receive()?))
    }
    fn supports_interruptible_exchange(&self) -> bool {
        false
    }
}

impl RendezvousLine for NativeWebSocketLine {
    fn receive(&mut self) -> Result<Vec<u8>, String> {
        let mut bytes = vec![0_u8; MAXIMUM_FRAME_BYTES];
        let length = self
            .receive_binary(&mut bytes)
            .map_err(debug("receive rendezvous frame"))?;
        bytes.truncate(length);
        Ok(bytes)
    }

    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.send_binary(bytes)
            .map_err(debug("send rendezvous frame"))
    }

    fn close(&mut self) -> Result<(), String> {
        NativeWebSocketLine::close(self).map_err(debug("close rendezvous Line"))
    }

    fn enter_retained_idle(&mut self) -> Result<(), String> {
        self.set_read_timeout(None)
            .map_err(debug("remove retained rendezvous read timeout"))
    }

    fn poll_receive(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, String> {
        self.set_read_timeout(Some(timeout))
            .map_err(debug("set rendezvous poll timeout"))?;
        let mut bytes = vec![0_u8; MAXIMUM_FRAME_BYTES];
        let result = self.receive_binary(&mut bytes);
        self.set_read_timeout(None)
            .map_err(debug("restore retained rendezvous idle timeout"))?;
        match result {
            Ok(length) => {
                bytes.truncate(length);
                Ok(Some(bytes))
            }
            Err(NativeWebSocketError::Transport(
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut,
            )) => Ok(None),
            Err(error) => Err(format!("poll rendezvous frame: {error:?}")),
        }
    }

    fn supports_interruptible_exchange(&self) -> bool {
        true
    }
}

struct SerialStreamLine<R, W> {
    reader: R,
    writer: W,
}

impl<R: BufRead, W: Write> RendezvousLine for SerialStreamLine<R, W> {
    fn receive(&mut self) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::with_capacity(1024);
        self.reader
            .by_ref()
            .take((MAXIMUM_FRAME_BYTES + 2) as u64)
            .read_until(b'\n', &mut bytes)
            .map_err(|error| format!("receive serial rendezvous frame: {error}"))?;
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
        }
        if bytes.last() == Some(&b'\r') {
            bytes.pop();
        }
        if bytes.is_empty() || bytes.len() > MAXIMUM_FRAME_BYTES {
            return Err("serial rendezvous frame violates its finite bound".into());
        }
        Ok(bytes)
    }

    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.writer
            .write_all(bytes)
            .and_then(|_| self.writer.write_all(b"\n"))
            .and_then(|_| self.writer.flush())
            .map_err(|error| format!("send serial rendezvous frame: {error}"))
    }

    fn close(&mut self) -> Result<(), String> {
        self.writer
            .flush()
            .map_err(|error| format!("close serial rendezvous Line: {error}"))
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum Ingress {
    Hello {
        protocol: u16,
        session_secret: Vec<u8>,
    },
    Invite {
        protocol: u16,
        session_secret: Vec<u8>,
        spore_id: String,
        image_id: String,
        claim: SpawnInvitationClaim,
        secret: Vec<u8>,
    },
    Admitted {
        protocol: u16,
        credential: MembershipCredential,
    },
    BodyContext {
        protocol: u16,
        context: BodyConversationContext,
    },
    ObserveLocalModelPool {
        protocol: u16,
        realization: PoolRealizationEnvelope,
    },
    Close {
        protocol: u16,
    },
    PrepareRemote {
        protocol: u16,
        plan: Box<conduit_core::Plan>,
    },
    ReleaseRemote {
        protocol: u16,
    },
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum Egress<'a> {
    Host {
        protocol: u16,
        friendly_label: &'a str,
        target_id: &'a str,
        image_content_digest: &'a str,
        advertisement: &'a HostAdvertisement,
        lines: [&'a str; 1],
    },
    Join {
        protocol: u16,
        spore_id: &'a str,
        image_id: &'a str,
        advertisement: &'a HostAdvertisement,
        invitation_id: &'a str,
        body_id: &'a str,
        host_id: &'a str,
        boot_id: &'a str,
        nonce: [u8; 32],
        signature: Vec<u8>,
        observed_at_millis: u64,
    },
    AdmissionRetained {
        protocol: u16,
        body_id: &'a str,
        part_id: &'a str,
    },
    BodyContextInstalled {
        protocol: u16,
        body_id: &'a str,
        basis_revision: u64,
    },
    LocalModelPoolObserved {
        protocol: u16,
        observation: &'a PoolRealizationObservation,
    },
    RemotePrepared {
        protocol: u16,
        identity: &'a conduit_core::ActivePlayIdentity,
        hello_frames: &'a [Vec<u8>],
    },
    RemoteReleased {
        protocol: u16,
    },
    Refused {
        protocol: u16,
        code: &'a str,
    },
}

pub(crate) fn serve(
    state_dir: &Path,
    carrier: RendezvousCarrier,
    timeout_seconds: u64,
    secure: SecureNetworkOptions,
) -> Result<(), String> {
    match carrier {
        RendezvousCarrier::Websocket => serve_websocket(state_dir, timeout_seconds),
        RendezvousCarrier::SecureWebsocket => secure::serve(state_dir, timeout_seconds, secure),
        RendezvousCarrier::Serial => serve_serial(state_dir),
    }
}

fn serve_websocket(state_dir: &Path, timeout_seconds: u64) -> Result<(), String> {
    let listener = NativeWebSocketListener::bind_loopback(MAXIMUM_FRAME_BYTES as u32)
        .map_err(debug("bind rendezvous Line"))?;
    let address = listener
        .local_addr()
        .map_err(debug("read rendezvous address"))?;
    let mut session_secret = [0_u8; 32];
    getrandom::fill(&mut session_secret)
        .map_err(|error| format!("create rendezvous secret: {error}"))?;
    if session_secret == [0; 32] {
        return Err("system randomness returned a weak rendezvous secret".into());
    }
    let code = encode_code(address.port(), &session_secret);
    println!("Rendezvous code: {code}");
    println!("Enter this one-use code in Crèche → Add Host → Already running.");
    println!("Waiting up to {timeout_seconds} seconds for the initial local WebSocket connection…");

    let result = (|| {
        let mut line = listener
            .accept_with_timeout(Duration::from_secs(timeout_seconds))
            .map_err(debug("accept rendezvous Line"))?;
        println!("Initial Host connection accepted; rendezvous deadline cleared. Retaining the Line until explicit close or loss.");
        run_session(
            &mut line,
            state_dir,
            &session_secret,
            "conduit-line/loopback-websocket@1",
        )
    })();
    session_secret.fill(0);
    if result.is_ok() {
        println!("Retained Host Line closed cleanly.");
    }
    result
}

fn serve_serial(state_dir: &Path) -> Result<(), String> {
    let mut session_secret = [0_u8; 32];
    getrandom::fill(&mut session_secret)
        .map_err(|error| format!("create rendezvous secret: {error}"))?;
    if session_secret == [0; 32] {
        return Err("system randomness returned a weak rendezvous secret".into());
    }
    eprintln!(
        "Rendezvous code: {SERIAL_CODE_PREFIX}-{}",
        hex(&session_secret)
    );
    eprintln!("Enter this one-use code in Crèche, then choose the attached serial port.");
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let result = run_session(
        &mut SerialStreamLine {
            reader: stdin.lock(),
            writer: stdout.lock(),
        },
        state_dir,
        &session_secret,
        "conduit-line/serial-text@1",
    );
    session_secret.fill(0);
    if result.is_ok() {
        eprintln!("Host invitation proof sent; Crèche still decides admission.");
    }
    result
}

fn run_session(
    line: &mut impl RendezvousLine,
    state_dir: &Path,
    session_secret: &[u8; 32],
    line_id: &'static str,
) -> Result<(), String> {
    let truth = crate::durable_host_control::current(state_dir)?;
    run_session_with_join(
        line,
        state_dir,
        session_secret,
        line_id,
        truth,
        |expected, claim, secret| {
            crate::durable_host_control::join(state_dir, expected, claim, secret)
        },
    )
}

fn run_session_with_join(
    line: &mut impl RendezvousLine,
    state_dir: &Path,
    session_secret: &[u8; 32],
    line_id: &'static str,
    truth: crate::durable_host_control::DurableHostTruth,
    join_host: impl FnOnce(
        &HostAdvertisement,
        SpawnInvitationClaim,
        Vec<u8>,
    ) -> Result<crate::durable_host_control::DurableJoinProof, String>,
) -> Result<(), String> {
    match receive(line)? {
        Ingress::Hello {
            protocol,
            session_secret: mut offered,
        } => {
            let accepted = protocol == PROTOCOL && authenticate(&mut offered, session_secret);
            offered.fill(0);
            if !accepted {
                send(
                    line,
                    &Egress::Refused {
                        protocol: PROTOCOL,
                        code: "rendezvous-secret",
                    },
                )?;
                return Err("rendezvous hello did not prove the code secret".into());
            }
        }
        _ => return Err("rendezvous did not begin with hello".into()),
    }
    send(
        line,
        &Egress::Host {
            protocol: PROTOCOL,
            friendly_label: "This running computer",
            target_id: &truth.target_id,
            image_content_digest: &truth.image_content_digest,
            advertisement: &truth.advertisement,
            lines: [line_id],
        },
    )?;

    let (spore_id, image_id, claim, mut secret) = match receive(line)? {
        Ingress::Invite {
            protocol,
            session_secret: mut offered,
            spore_id,
            image_id,
            claim,
            secret,
        } => {
            let accepted = protocol == PROTOCOL && authenticate(&mut offered, session_secret);
            offered.fill(0);
            if !accepted {
                send(
                    line,
                    &Egress::Refused {
                        protocol: PROTOCOL,
                        code: "rendezvous-secret",
                    },
                )?;
                return Err("rendezvous invitation did not prove the code secret".into());
            }
            (spore_id, image_id, claim, secret)
        }
        _ => return Err("rendezvous expected one Body invitation".into()),
    };
    bounded_id(&spore_id, "spore")?;
    bounded_id(&image_id, "IMAGE")?;
    let join = join_host(&truth.advertisement, claim, core::mem::take(&mut secret))?;
    let joined_body_id = join.body_id.clone();
    send(
        line,
        &Egress::Join {
            protocol: PROTOCOL,
            spore_id: &spore_id,
            image_id: &image_id,
            advertisement: &join.advertisement,
            invitation_id: &join.invitation_id,
            body_id: &join.body_id,
            host_id: join.advertisement.host_id.as_str(),
            boot_id: join.advertisement.boot_id.as_str(),
            nonce: join.nonce,
            signature: join.signature,
            observed_at_millis: join.observed_at_millis,
        },
    )?;
    // Admission completes the bounded handshake. A retained Host Line may then
    // remain honestly idle indefinitely; only explicit joined-session polling
    // installs a short read deadline, and it restores this idle state.
    line.enter_retained_idle()?;
    let mut membership_retained = false;
    let mut body_context_installed = false;
    let mut remote_prepared = None;
    loop {
        match receive_joined(line)? {
            JoinedIngress::Control(Ingress::Admitted {
                protocol,
                credential,
            }) if protocol == PROTOCOL && !membership_retained => {
                crate::durable_host::retain_rendezvous_membership(
                    state_dir,
                    &credential,
                    &joined_body_id,
                    &truth.advertisement,
                )?;
                send(
                    line,
                    &Egress::AdmissionRetained {
                        protocol: PROTOCOL,
                        body_id: credential.body_id.as_str(),
                        part_id: credential.part_id.as_str(),
                    },
                )?;
                membership_retained = true;
            }
            JoinedIngress::Control(Ingress::BodyContext {
                protocol,
                context,
            }) if protocol == PROTOCOL && membership_retained => {
                if context.body_id.as_str() != joined_body_id {
                    return Err("joined Host Body context belongs to another Body".into());
                }
                let body_id = context.body_id.as_str().to_owned();
                let basis_revision = context.basis.revision;
                let advertisement = crate::durable_host_control::install_body_context(
                    state_dir,
                    &truth.advertisement,
                    context,
                )?;
                if advertisement != truth.advertisement {
                    return Err(
                        "provider-ready Body context publication changed current Host offers".into(),
                    );
                }
                send(
                    line,
                    &Egress::BodyContextInstalled {
                        protocol: PROTOCOL,
                        body_id: &body_id,
                        basis_revision,
                    },
                )?;
                body_context_installed = true;
            }
            JoinedIngress::Control(Ingress::ObserveLocalModelPool {
                protocol,
                realization,
            }) if protocol == PROTOCOL && membership_retained => {
                let observation = crate::durable_host_control::observe_local_model_pool(
                    state_dir,
                    realization,
                )?;
                send(
                    line,
                    &Egress::LocalModelPoolObserved {
                        protocol: PROTOCOL,
                        observation: &observation,
                    },
                )?;
            }
            JoinedIngress::Control(Ingress::PrepareRemote { protocol, plan })
                if protocol == PROTOCOL && membership_retained && body_context_installed =>
            {
                if remote_prepared.is_some() {
                    return Err("joined Host Line already owns one remote Play".into());
                }
                let preparation = crate::durable_host_control::prepare_remote(
                    state_dir,
                    &truth.advertisement,
                    *plan,
                )?;
                send(
                    line,
                    &Egress::RemotePrepared {
                        protocol: PROTOCOL,
                        identity: &preparation.identity,
                        hello_frames: &preparation.hello_frames,
                    },
                )?;
                remote_prepared = Some(preparation.identity);
            }
            JoinedIngress::Frame(frame) if remote_prepared.is_some() => {
                exchange_joined_frame(
                    line,
                    state_dir,
                    remote_prepared.as_ref().expect("guarded remote identity"),
                    &frame,
                )?;
            }
            JoinedIngress::Control(Ingress::ReleaseRemote { protocol })
                if protocol == PROTOCOL && remote_prepared.is_some() =>
            {
                crate::durable_host_control::release_remote(state_dir)?;
                send(line, &Egress::RemoteReleased { protocol: PROTOCOL })?;
                remote_prepared = None;
            }
            JoinedIngress::Control(Ingress::Close { protocol }) if protocol == PROTOCOL => {
                if remote_prepared.is_some() {
                    crate::durable_host_control::release_remote(state_dir)?;
                }
                break;
            }
            _ => {
                return Err(
                    "joined Host Line expected admission retention, Body context, remote preparation, or explicit close"
                        .into(),
                )
            }
        }
    }
    let _ = line.close();
    Ok(())
}

fn exchange_joined_frame(
    line: &mut impl RendezvousLine,
    state_dir: &Path,
    active_play: &conduit_core::ActivePlayIdentity,
    frame: &[u8],
) -> Result<(), String> {
    if !line.supports_interruptible_exchange() {
        let exchange = crate::durable_host_control::exchange_remote(state_dir, frame.to_vec())?;
        for response in exchange.responses {
            line.send(&response)?;
        }
        return Ok(());
    }
    let first = decode_session_frame(
        frame,
        MAXIMUM_FRAME_BYTES as u32,
        MAXIMUM_FRAME_BYTES as u32,
    )
    .map_err(|error| format!("decode joined Host session frame: {error:?}"))?;
    let identity = first.identity;
    std::thread::scope(|scope| {
        let worker =
            scope.spawn(|| crate::durable_host_control::exchange_remote(state_dir, frame.to_vec()));
        let mut cancellation = None;
        while !worker.is_finished() {
            let Some(candidate) = line.poll_receive(Duration::from_millis(25))? else {
                continue;
            };
            if cancellation.is_some() {
                return Err("joined Host Line sent more than one frame during cancellation".into());
            }
            let decoded = decode_session_frame(
                &candidate,
                MAXIMUM_FRAME_BYTES as u32,
                MAXIMUM_FRAME_BYTES as u32,
            )
            .map_err(|error| format!("decode joined Host cancellation: {error:?}"))?;
            if decoded.identity != identity
                || !matches!(decoded.message, SessionMessage::Cancelled { .. })
            {
                return Err("joined Host Line changed session while an exchange was active".into());
            }
            crate::durable_host_control::signal_remote_cancellation(
                state_dir,
                active_play.active_play_id.as_str(),
            )?;
            cancellation = Some(candidate);
        }
        let exchanged = worker
            .join()
            .map_err(|_| "durable Host exchange worker panicked".to_string())?;
        match exchanged {
            Ok(exchange) => {
                for response in exchange.responses {
                    line.send(&response)?;
                }
            }
            Err(error) if cancellation.is_some() && error.contains("remote-fragment-cancelled") => {
            }
            Err(error) => return Err(error),
        }
        if let Some(frame) = cancellation {
            let exchange = crate::durable_host_control::exchange_remote(state_dir, frame)?;
            for response in exchange.responses {
                line.send(&response)?;
            }
        }
        Ok(())
    })
}

// Keep the bounded control envelope inline: boxing it would add an avoidable
// allocation to every retained-Line control message merely to equalize this
// private dispatch enum's variant sizes.
#[allow(clippy::large_enum_variant)]
enum JoinedIngress {
    Control(Ingress),
    Frame(Vec<u8>),
}

fn receive_joined(line: &mut impl RendezvousLine) -> Result<JoinedIngress, String> {
    let mut bytes = line.receive()?;
    if bytes.starts_with(b"CNDS") {
        return Ok(JoinedIngress::Frame(bytes));
    }
    let decoded = serde_json::from_slice(&bytes)
        .map(JoinedIngress::Control)
        .map_err(|error| format!("decode joined Host Line frame: {error}"));
    bytes.fill(0);
    decoded
}

fn receive(line: &mut impl RendezvousLine) -> Result<Ingress, String> {
    let mut bytes = line.receive()?;
    let decoded =
        serde_json::from_slice(&bytes).map_err(|error| format!("decode rendezvous frame: {error}"));
    bytes.fill(0);
    decoded
}

fn send(line: &mut impl RendezvousLine, frame: &Egress<'_>) -> Result<(), String> {
    let bytes =
        serde_json::to_vec(frame).map_err(|error| format!("encode rendezvous frame: {error}"))?;
    if bytes.len() > MAXIMUM_FRAME_BYTES {
        return Err("rendezvous response exceeds its finite bound".into());
    }
    line.send(&bytes)
}

fn encode_code(port: u16, secret: &[u8; 32]) -> String {
    format!("{CODE_PREFIX}-{port:04X}-{}", hex(secret))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02X}")).collect()
}

fn bounded_id(value: &str, name: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > MAXIMUM_ID_BYTES || value.chars().any(char::is_whitespace)
    {
        return Err(format!(
            "{name} identity is missing or outside its finite bound"
        ));
    }
    Ok(())
}

fn constant_time_equal(offered: &[u8], expected: &[u8; 32]) -> bool {
    let mut difference = offered.len() ^ expected.len();
    for (index, expected_byte) in expected.iter().copied().enumerate() {
        difference |= usize::from(offered.get(index).copied().unwrap_or(0) ^ expected_byte);
    }
    difference == 0
}

fn authenticate(offered: &mut [u8], expected: &[u8; 32]) -> bool {
    constant_time_equal(offered, expected)
}

fn debug<T: core::fmt::Debug>(context: &'static str) -> impl FnOnce(T) -> String {
    move |error| format!("{context}: {error:?}")
}

#[cfg(test)]
#[path = "host_rendezvous/protected_tests.rs"]
mod protected_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    struct MemoryLine {
        incoming: VecDeque<Vec<u8>>,
    }

    impl RendezvousLine for MemoryLine {
        fn receive(&mut self) -> Result<Vec<u8>, String> {
            self.incoming
                .pop_front()
                .ok_or_else(|| "empty memory Line".to_string())
        }

        fn send(&mut self, _bytes: &[u8]) -> Result<(), String> {
            Ok(())
        }

        fn close(&mut self) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn retained_websocket_idle_removes_the_handshake_read_deadline() {
        let listener = NativeWebSocketListener::bind_loopback(64).expect("loopback listener binds");
        let address = listener.local_addr().expect("loopback address");
        let url = listener.url().expect("loopback url");
        let client = std::thread::spawn(move || {
            let mut line =
                NativeWebSocketLine::connect(address, &url, 64).expect("client connects");
            std::thread::sleep(Duration::from_millis(50));
            line.send_binary(b"retained-idle")
                .expect("client sends after idle interval");
        });
        let mut line = listener.accept().expect("server accepts");
        line.set_read_timeout(Some(Duration::from_millis(10)))
            .expect("short handshake deadline installs");
        RendezvousLine::enter_retained_idle(&mut line)
            .expect("retained joined Line removes handshake deadline");
        let mut bytes = [0_u8; 64];
        let length = line
            .receive_binary(&mut bytes)
            .expect("retained joined Line survives beyond old deadline");
        assert_eq!(&bytes[..length], b"retained-idle");
        client.join().expect("client thread completes");
    }

    #[test]
    fn code_is_bounded_and_carries_no_host_or_body_identity() {
        let code = encode_code(4173, &[0xab; 32]);
        assert_eq!(code, format!("C1-WS-104D-{}", "AB".repeat(32)));
        assert!(code.len() < 96);
    }

    #[test]
    fn secret_comparison_checks_length_and_content() {
        assert!(constant_time_equal(&[7; 32], &[7; 32]));
        assert!(!constant_time_equal(&[7; 31], &[7; 32]));
        assert!(!constant_time_equal(&[7; 32], &[8; 32]));
    }

    #[test]
    fn serial_stream_line_retains_one_bounded_frame_per_newline() {
        let mut output = Vec::new();
        let mut line = SerialStreamLine {
            reader: std::io::Cursor::new(b"{\"kind\":\"hello\"}\r\n"),
            writer: &mut output,
        };
        assert_eq!(line.receive().unwrap(), b"{\"kind\":\"hello\"}");
        line.send(b"{\"kind\":\"host\"}").unwrap();
        assert_eq!(output, b"{\"kind\":\"host\"}\n");
    }

    #[test]
    fn joined_line_distinguishes_canonical_session_frames_from_control() {
        let mut line = MemoryLine {
            incoming: VecDeque::from([
                b"CNDS\x04\x02".to_vec(),
                serde_json::to_vec(&serde_json::json!({"kind":"close","protocol":1})).unwrap(),
            ]),
        };
        assert!(matches!(
            receive_joined(&mut line).unwrap(),
            JoinedIngress::Frame(bytes) if bytes == b"CNDS\x04\x02"
        ));
        assert!(matches!(
            receive_joined(&mut line).unwrap(),
            JoinedIngress::Control(Ingress::Close { protocol: PROTOCOL })
        ));
    }

    #[test]
    fn joined_line_decodes_one_bounded_model_pool_observation_request() {
        let request = serde_json::json!({
            "kind": "observe-local-model-pool",
            "protocol": PROTOCOL,
            "realization": {
                "host_id": "host/model-a",
                "boot_id": "boot/model-a/1",
                "offer_generation": 2,
                "capability_id": "capability/model-a/generate",
                "implementation_id": "std/local-model@1",
                "artifact_id": "model/sha256-a",
                "member_capacity": 1,
                "resources": []
            }
        });
        let mut line = MemoryLine {
            incoming: VecDeque::from([serde_json::to_vec(&request).unwrap()]),
        };
        assert!(matches!(
            receive_joined(&mut line).unwrap(),
            JoinedIngress::Control(Ingress::ObserveLocalModelPool {
                protocol: PROTOCOL,
                realization,
            }) if realization.host_id.as_str() == "host/model-a"
                && realization.boot_id.as_str() == "boot/model-a/1"
        ));
    }
}
