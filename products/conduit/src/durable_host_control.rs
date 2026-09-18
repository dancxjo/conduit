//! Authenticated local control plane into the durable installed Host owner.

use conduit_body::{BodyConversationContext, SpawnInvitationClaim, SpawnInvitationSecret};
use conduit_core::{ActivePlayIdentity, HostAdvertisement, Plan};
use conduit_kernel::scheduler::{RemoteIngressOutcome, SchedulerStatus};
use conduit_plan_lowering::lowering::RemoteCordDirection;
use conduit_std_host::{AdmittedRemoteFragment, StdHost};
use conduit_wire::{decode_session_frame, encode_session_frame_into, SessionMessage};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const PROTOCOL: u16 = 1;
const MAXIMUM_CONTROL_FRAME_BYTES: usize = 128 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct DurableHostTruth {
    pub(crate) target_id: String,
    pub(crate) image_content_digest: String,
    pub(crate) advertisement: HostAdvertisement,
}

pub(crate) struct DurableHostRuntime {
    target_id: String,
    image_content_digest: String,
    host: StdHost,
    remote_fragment: Option<AdmittedRemoteFragment>,
    cancellation_signal: Option<PathBuf>,
}

impl DurableHostRuntime {
    pub(crate) fn new(target_id: String, image_content_digest: String, host: StdHost) -> Self {
        Self {
            target_id,
            image_content_digest,
            host,
            remote_fragment: None,
            cancellation_signal: None,
        }
    }

    fn install_cancellation_signal(&mut self, state_dir: &Path) {
        self.cancellation_signal = Some(state_dir.join("remote-cancellation.signal"));
    }

    fn truth(&self) -> DurableHostTruth {
        DurableHostTruth {
            target_id: self.target_id.clone(),
            image_content_digest: self.image_content_digest.clone(),
            advertisement: self.host.advertisement().clone(),
        }
    }

    fn install_body_context(
        &mut self,
        expected_boot_id: &str,
        expected_offer_generation: u64,
        context: &BodyConversationContext,
    ) -> Result<HostAdvertisement, String> {
        let before = self.host.advertisement().clone();
        if before.boot_id.as_str() != expected_boot_id
            || before.offer_generation.0 != expected_offer_generation
        {
            return Err("stale-host-truth".into());
        }
        self.host.install_body_conversation_context(context)?;
        let after = self.host.advertisement().clone();
        if after.host_id != before.host_id || after.boot_id != before.boot_id {
            return Err("body-context-host-identity-changed".into());
        }
        Ok(after)
    }

    fn prepare_remote(
        &mut self,
        expected_boot_id: &str,
        expected_offer_generation: u64,
        plan: &Plan,
    ) -> Result<DurableRemotePreparation, String> {
        let advertisement = self.host.advertisement();
        if advertisement.boot_id.as_str() != expected_boot_id
            || advertisement.offer_generation.0 != expected_offer_generation
        {
            return Err("stale-host-truth".into());
        }
        if self.remote_fragment.is_some() {
            return Err("remote-play-active".into());
        }
        self.clear_cancellation_signal()?;
        if !conduit_core::verify_plan(plan) {
            return Err("invalid-plan".into());
        }
        let fragment = plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == advertisement.host_id)
            .ok_or_else(|| "host-fragment-absent".to_string())?;
        let mut admitted = self.host.prepare_remote_fragment(fragment)?;
        let hello_frames = match encode_remote_hello_frames(&mut admitted) {
            Ok(frames) => frames,
            Err(error) => {
                self.host.release_remote_fragment(admitted)?;
                return Err(error);
            }
        };
        let preparation = DurableRemotePreparation {
            identity: admitted.identity().clone(),
            hello_frames,
        };
        self.remote_fragment = Some(admitted);
        Ok(preparation)
    }

    fn exchange_remote_frame(&mut self, bytes: &[u8]) -> Result<DurableRemoteExchange, String> {
        let admitted = self
            .remote_fragment
            .as_mut()
            .ok_or_else(|| "remote-play-absent".to_string())?;
        let frame = decode_session_frame(
            bytes,
            MAXIMUM_CONTROL_FRAME_BYTES as u32,
            MAXIMUM_CONTROL_FRAME_BYTES as u32,
        )
        .map_err(|error| format!("decode remote session frame: {error:?}"))?;
        let endpoint = admitted
            .sessions()
            .iter()
            .find(|session| session.binding().identity() == frame.identity)
            .map(|session| session.endpoint)
            .ok_or_else(|| "remote-session-absent".to_string())?;
        let message = frame.message;
        let cancellation_signal = self.cancellation_signal.as_deref();
        let active_play_id = admitted.identity().active_play_id.as_str().to_owned();
        let cancelled = || {
            cancellation_signal.is_some_and(|path| {
                fs::read(path).is_ok_and(|bytes| bytes == active_play_id.as_bytes())
            })
        };
        admitted
            .sessions_mut()
            .get_mut(endpoint)
            .ok_or_else(|| "remote-session-absent".to_string())?
            .machine_mut()
            .admit_inbound(frame)
            .map_err(|error| format!("admit remote session frame: {error:?}"))?;
        let mut responses = Vec::new();
        match message {
            SessionMessage::Hello(_) => {
                responses.push(remote_response(admitted, endpoint, SessionMessage::Ready)?);
            }
            SessionMessage::Offered { sequence, payload } => {
                match admitted
                    .runtime_mut()
                    .admit_ingress(endpoint, sequence, payload)?
                {
                    RemoteIngressOutcome::Accepted { sequence: accepted }
                        if accepted == sequence =>
                    {
                        responses.push(remote_response(
                            admitted,
                            endpoint,
                            SessionMessage::Accepted { sequence },
                        )?);
                        drive_remote_fragment(&mut self.host, admitted, &mut responses, cancelled)?;
                        responses.push(remote_response(
                            admitted,
                            endpoint,
                            SessionMessage::Delivered { sequence },
                        )?);
                    }
                    RemoteIngressOutcome::Full {
                        sequence: pressured,
                    } if pressured == sequence => {
                        responses.push(remote_response(
                            admitted,
                            endpoint,
                            SessionMessage::Pressure { sequence },
                        )?);
                    }
                    _ => return Err("remote-ingress-sequence".into()),
                }
            }
            SessionMessage::Accepted { sequence } => {
                let transfer = admitted
                    .runtime_mut()
                    .next_egress(endpoint)?
                    .ok_or_else(|| "remote-egress-absent".to_string())?;
                if transfer.sequence != sequence {
                    return Err("remote-egress-sequence".into());
                }
                admitted.runtime_mut().accept_egress(&transfer)?;
            }
            SessionMessage::Delivered { sequence } => {
                let transfer = admitted
                    .runtime_mut()
                    .next_egress(endpoint)?
                    .ok_or_else(|| "remote-egress-absent".to_string())?;
                if transfer.sequence != sequence {
                    return Err("remote-egress-sequence".into());
                }
                admitted.runtime_mut().deliver_egress(&transfer)?;
                drive_remote_fragment(&mut self.host, admitted, &mut responses, cancelled)?;
            }
            SessionMessage::InputClosed { .. } => {
                admitted.runtime_mut().close_ingress(endpoint)?;
                drive_remote_fragment(&mut self.host, admitted, &mut responses, cancelled)?;
            }
            SessionMessage::Cancelled { code } => {
                admitted.runtime_mut().cancel()?;
                responses.push(remote_response(
                    admitted,
                    endpoint,
                    SessionMessage::Cancelled { code },
                )?);
                let final_sequence = admitted
                    .sessions()
                    .get(endpoint)
                    .ok_or_else(|| "remote-session-absent".to_string())?
                    .machine()
                    .next_sequence();
                responses.push(remote_response(
                    admitted,
                    endpoint,
                    SessionMessage::Terminal {
                        disposition: conduit_wire::SessionTerminalDisposition::Cancelled,
                        final_sequence,
                    },
                )?);
            }
            SessionMessage::Failed { code } => {
                admitted.runtime_mut().cancel()?;
                responses.push(remote_response(
                    admitted,
                    endpoint,
                    SessionMessage::Failed { code },
                )?);
                let final_sequence = admitted
                    .sessions()
                    .get(endpoint)
                    .ok_or_else(|| "remote-session-absent".to_string())?
                    .machine()
                    .next_sequence();
                responses.push(remote_response(
                    admitted,
                    endpoint,
                    SessionMessage::Terminal {
                        disposition: conduit_wire::SessionTerminalDisposition::Failed,
                        final_sequence,
                    },
                )?);
            }
            SessionMessage::Ready | SessionMessage::Pressure { .. } => {
                drive_remote_fragment(&mut self.host, admitted, &mut responses, cancelled)?;
            }
            SessionMessage::Terminal { .. } => {
                let terminal = admitted
                    .sessions()
                    .get(endpoint)
                    .is_some_and(|session| session.machine().is_terminal());
                if !terminal {
                    drive_remote_fragment(&mut self.host, admitted, &mut responses, cancelled)?;
                }
            }
        }
        Ok(DurableRemoteExchange {
            responses,
            active: admitted
                .sessions()
                .get(endpoint)
                .is_some_and(|session| session.machine().is_active()),
        })
    }

    fn release_remote(&mut self) -> Result<(), String> {
        self.clear_cancellation_signal()?;
        let Some(fragment) = self.remote_fragment.take() else {
            return Ok(());
        };
        self.host.release_remote_fragment(fragment)
    }

    fn clear_cancellation_signal(&self) -> Result<(), String> {
        let Some(path) = self.cancellation_signal.as_deref() else {
            return Ok(());
        };
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("clear remote cancellation signal: {error}")),
        }
    }
}

fn remote_response(
    admitted: &mut AdmittedRemoteFragment,
    endpoint: conduit_kernel::RemoteEndpointId,
    message: SessionMessage<'_>,
) -> Result<Vec<u8>, String> {
    let session = admitted
        .sessions_mut()
        .get_mut(endpoint)
        .ok_or_else(|| "remote-session-absent".to_string())?;
    let binding = session.binding().clone();
    let frame = binding.frame(message);
    session
        .machine_mut()
        .admit_outbound(frame)
        .map_err(|error| format!("admit remote session response: {error:?}"))?;
    encode_remote_frame(&binding, frame)
}

fn drive_remote_fragment<F>(
    host: &mut StdHost,
    admitted: &mut AdmittedRemoteFragment,
    responses: &mut Vec<Vec<u8>>,
    cancelled: F,
) -> Result<(), String>
where
    F: Fn() -> bool + Copy,
{
    const MAXIMUM_DRIVE_STEPS: usize = 64;
    for _ in 0..MAXIMUM_DRIVE_STEPS {
        if host.poll_remote_body_conversation_context(admitted)? {
            continue;
        }
        if let Some(request) = admitted.runtime_mut().next_host_request() {
            if admitted
                .runtime_mut()
                .complete_portable_host_operation(request)?
            {
                continue;
            }
            if host.complete_remote_voice_host_operation(admitted, request, cancelled)? {
                continue;
            }
            let work = admitted.runtime().describe_host_request(request)?;
            return Err(format!(
                "remote-host-operation-unsupported:{}",
                work.contract_id.as_str()
            ));
        }
        let endpoints = admitted
            .sessions()
            .iter()
            .filter(|session| session.direction == RemoteCordDirection::Egress)
            .map(|session| session.endpoint)
            .collect::<Vec<_>>();
        for endpoint in endpoints {
            if let Some(transfer) = admitted.runtime_mut().next_egress(endpoint)? {
                responses.push(remote_response(
                    admitted,
                    endpoint,
                    SessionMessage::Offered {
                        sequence: transfer.sequence,
                        payload: &transfer.bytes,
                    },
                )?);
                return Ok(());
            }
        }
        match admitted.runtime_mut().step()? {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Idle => return Ok(()),
            SchedulerStatus::Drained => {
                complete_remote_sessions(admitted, responses)?;
                return Ok(());
            }
            SchedulerStatus::Cancelled => return Err("remote-fragment-cancelled".into()),
        }
    }
    Err("remote-fragment-drive-bound".into())
}

fn complete_remote_sessions(
    admitted: &mut AdmittedRemoteFragment,
    responses: &mut Vec<Vec<u8>>,
) -> Result<(), String> {
    let endpoints = admitted
        .sessions()
        .iter()
        .map(|session| (session.endpoint, session.direction))
        .collect::<Vec<_>>();
    let mut completions = Vec::with_capacity(endpoints.len());
    for (endpoint, direction) in endpoints {
        let checkpoint = admitted
            .sessions()
            .get(endpoint)
            .ok_or_else(|| "remote-session-absent".to_string())?
            .machine()
            .checkpoint();
        if direction == RemoteCordDirection::Egress {
            if !admitted.runtime_mut().egress_terminal(endpoint)? {
                return Err("remote-egress-not-terminal".into());
            }
        } else if !checkpoint.input_closed {
            return Err("remote-ingress-not-closed".into());
        }
        completions.push((endpoint, direction, checkpoint.next_sequence));
    }
    for (endpoint, direction, final_sequence) in completions {
        if direction == RemoteCordDirection::Egress {
            responses.push(remote_response(
                admitted,
                endpoint,
                SessionMessage::InputClosed { final_sequence },
            )?);
        }
        responses.push(remote_response(
            admitted,
            endpoint,
            SessionMessage::Terminal {
                disposition: conduit_wire::SessionTerminalDisposition::Completed,
                final_sequence,
            },
        )?);
    }
    Ok(())
}

fn encode_remote_hello_frames(
    admitted: &mut AdmittedRemoteFragment,
) -> Result<Vec<Vec<u8>>, String> {
    let mut hello_frames = Vec::with_capacity(admitted.sessions().len());
    let endpoints = admitted
        .sessions()
        .iter()
        .map(|session| session.endpoint)
        .collect::<Vec<_>>();
    for endpoint in endpoints {
        let session = admitted
            .sessions_mut()
            .get_mut(endpoint)
            .ok_or_else(|| "remote session disappeared during preparation".to_string())?;
        let binding = session.binding().clone();
        let hello = binding.hello_frame();
        session
            .machine_mut()
            .admit_outbound(hello)
            .map_err(|error| format!("admit remote session grant: {error:?}"))?;
        hello_frames.push(encode_remote_frame(&binding, hello)?);
    }
    Ok(hello_frames)
}

fn encode_remote_frame(
    binding: &conduit_wire::SessionBinding,
    frame: conduit_wire::SessionFrame<'_>,
) -> Result<Vec<u8>, String> {
    let frame_bound = usize::try_from(binding.attachment.limits.maximum_frame_bytes)
        .map_err(|_| "remote session frame bound overflow".to_string())?;
    if frame_bound > MAXIMUM_CONTROL_FRAME_BYTES {
        return Err("remote-session-frame-bound".into());
    }
    let mut bytes = vec![0; frame_bound];
    let length = encode_session_frame_into(
        frame,
        &mut bytes,
        binding.limits.maximum_payload_bytes,
        binding.attachment.limits.maximum_frame_bytes,
    )
    .map_err(|error| format!("encode remote session frame: {error:?}"))?;
    bytes.truncate(length);
    Ok(bytes)
}

#[derive(Debug, Clone)]
pub(crate) struct DurableRemotePreparation {
    pub(crate) identity: ActivePlayIdentity,
    pub(crate) hello_frames: Vec<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub(crate) struct DurableRemoteExchange {
    pub(crate) responses: Vec<Vec<u8>>,
    pub(crate) active: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct DurableJoinProof {
    pub(crate) advertisement: HostAdvertisement,
    pub(crate) invitation_id: String,
    pub(crate) body_id: String,
    pub(crate) nonce: [u8; 32],
    pub(crate) signature: Vec<u8>,
    pub(crate) observed_at_millis: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum Request {
    Status {
        protocol: u16,
        token: Vec<u8>,
    },
    Join {
        protocol: u16,
        token: Vec<u8>,
        expected_boot_id: String,
        expected_offer_generation: u64,
        claim: SpawnInvitationClaim,
        secret: Vec<u8>,
    },
    InstallBodyContext {
        protocol: u16,
        token: Vec<u8>,
        expected_boot_id: String,
        expected_offer_generation: u64,
        context: BodyConversationContext,
    },
    PrepareRemote {
        protocol: u16,
        token: Vec<u8>,
        expected_boot_id: String,
        expected_offer_generation: u64,
        plan: Box<Plan>,
    },
    ExchangeRemote {
        protocol: u16,
        token: Vec<u8>,
        frame: Vec<u8>,
    },
    ReleaseRemote {
        protocol: u16,
        token: Vec<u8>,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum Response {
    Status {
        protocol: u16,
        target_id: String,
        image_content_digest: String,
        advertisement: HostAdvertisement,
    },
    Join {
        protocol: u16,
        advertisement: HostAdvertisement,
        invitation_id: String,
        body_id: String,
        nonce: [u8; 32],
        signature: Vec<u8>,
        observed_at_millis: u64,
    },
    BodyContextInstalled {
        protocol: u16,
        advertisement: HostAdvertisement,
    },
    RemotePrepared {
        protocol: u16,
        identity: ActivePlayIdentity,
        hello_frames: Vec<Vec<u8>>,
    },
    RemoteExchanged {
        protocol: u16,
        responses: Vec<Vec<u8>>,
        active: bool,
    },
    RemoteReleased {
        protocol: u16,
    },
    Refused {
        protocol: u16,
        code: String,
    },
}

pub(crate) fn ensure_secret(state_dir: &Path) -> Result<(), String> {
    let path = state_dir.join("control.token");
    if path.exists() {
        return read_secret(&path).map(|_| ());
    }
    let mut token = [0_u8; 32];
    getrandom::fill(&mut token).map_err(|error| format!("create local control token: {error}"))?;
    if token == [0; 32] {
        return Err("system randomness returned a weak local control token".into());
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(&path)
        .and_then(|mut file| file.write_all(&token))
        .map_err(|error| format!("retain local control token: {error}"))?;
    token.fill(0);
    Ok(())
}

pub(crate) fn signal_remote_cancellation(
    state_dir: &Path,
    active_play_id: &str,
) -> Result<(), String> {
    let temporary = state_dir.join("remote-cancellation.signal.pending");
    let destination = state_dir.join("remote-cancellation.signal");
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(&temporary)
        .and_then(|mut file| {
            file.write_all(active_play_id.as_bytes())?;
            file.sync_all()
        })
        .and_then(|()| fs::rename(&temporary, &destination))
        .map_err(|error| format!("signal exact remote Play cancellation: {error}"))
}

#[cfg(unix)]
pub(crate) fn serve(state_dir: &Path, mut runtime: DurableHostRuntime) -> Result<(), String> {
    use std::os::unix::{fs::PermissionsExt, net::UnixListener};

    runtime.install_cancellation_signal(state_dir);
    let socket = state_dir.join("control.sock");
    if socket.exists() {
        fs::remove_file(&socket)
            .map_err(|error| format!("remove stale control endpoint: {error}"))?;
    }
    let listener = UnixListener::bind(&socket)
        .map_err(|error| format!("bind durable Host control endpoint: {error}"))?;
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("restrict durable Host control endpoint: {error}"))?;
    let mut token = read_secret(&state_dir.join("control.token"))?;
    for incoming in listener.incoming() {
        let mut stream = incoming.map_err(|error| format!("accept local Host control: {error}"))?;
        let response = handle(read_frame(&mut stream)?, &token, &mut runtime);
        write_frame(&mut stream, &response)?;
    }
    token.fill(0);
    Ok(())
}

#[cfg(not(unix))]
pub(crate) fn serve(_state_dir: &Path, _runtime: DurableHostRuntime) -> Result<(), String> {
    Err("no reviewed local durable Host control carrier exists on this platform".into())
}

#[cfg(unix)]
pub(crate) fn current(state_dir: &Path) -> Result<DurableHostTruth, String> {
    use std::os::unix::net::UnixStream;
    let mut token = read_secret(&state_dir.join("control.token"))?;
    let mut stream = UnixStream::connect(state_dir.join("control.sock"))
        .map_err(|error| format!("connect to durable Host service: {error}"))?;
    write_frame(
        &mut stream,
        &Request::Status {
            protocol: PROTOCOL,
            token: token.to_vec(),
        },
    )?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|error| format!("finish durable Host status request: {error}"))?;
    token.fill(0);
    match read_frame::<_, Response>(&mut stream)? {
        Response::Status {
            protocol: PROTOCOL,
            target_id,
            image_content_digest,
            advertisement,
        } => Ok(DurableHostTruth {
            target_id,
            image_content_digest,
            advertisement,
        }),
        Response::Refused { code, .. } => Err(format!("durable Host refused status: {code}")),
        _ => Err("durable Host returned the wrong control response".into()),
    }
}

#[cfg(not(unix))]
pub(crate) fn current(_state_dir: &Path) -> Result<DurableHostTruth, String> {
    Err("no reviewed local durable Host control carrier exists on this platform".into())
}

#[cfg(unix)]
pub(crate) fn join(
    state_dir: &Path,
    expected: &HostAdvertisement,
    claim: SpawnInvitationClaim,
    secret: Vec<u8>,
) -> Result<DurableJoinProof, String> {
    use std::os::unix::net::UnixStream;
    let mut token = read_secret(&state_dir.join("control.token"))?;
    let mut stream = UnixStream::connect(state_dir.join("control.sock"))
        .map_err(|error| format!("connect to durable Host service: {error}"))?;
    let mut request = Request::Join {
        protocol: PROTOCOL,
        token: token.to_vec(),
        expected_boot_id: expected.boot_id.as_str().into(),
        expected_offer_generation: expected.offer_generation.0,
        claim,
        secret,
    };
    write_sensitive_frame(&mut stream, &request)?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|error| format!("finish durable Host join request: {error}"))?;
    token.fill(0);
    if let Request::Join { secret, token, .. } = &mut request {
        secret.fill(0);
        token.fill(0);
    }
    match read_frame::<_, Response>(&mut stream)? {
        Response::Join {
            protocol: PROTOCOL,
            advertisement,
            invitation_id,
            body_id,
            nonce,
            signature,
            observed_at_millis,
        } if advertisement == *expected => Ok(DurableJoinProof {
            advertisement,
            invitation_id,
            body_id,
            nonce,
            signature,
            observed_at_millis,
        }),
        Response::Refused { code, .. } => Err(format!("durable Host refused join: {code}")),
        _ => Err("durable Host changed identity while completing rendezvous".into()),
    }
}

#[cfg(unix)]
pub(crate) fn install_body_context(
    state_dir: &Path,
    expected: &HostAdvertisement,
    context: BodyConversationContext,
) -> Result<HostAdvertisement, String> {
    use std::os::unix::net::UnixStream;
    let mut token = read_secret(&state_dir.join("control.token"))?;
    let mut stream = UnixStream::connect(state_dir.join("control.sock"))
        .map_err(|error| format!("connect to durable Host service: {error}"))?;
    let mut request = Request::InstallBodyContext {
        protocol: PROTOCOL,
        token: token.to_vec(),
        expected_boot_id: expected.boot_id.as_str().into(),
        expected_offer_generation: expected.offer_generation.0,
        context,
    };
    write_sensitive_frame(&mut stream, &request)?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|error| format!("finish durable Host Body context request: {error}"))?;
    token.fill(0);
    if let Request::InstallBodyContext { token, .. } = &mut request {
        token.fill(0);
    }
    match read_frame::<_, Response>(&mut stream)? {
        Response::BodyContextInstalled {
            protocol: PROTOCOL,
            advertisement,
        } => Ok(advertisement),
        Response::Refused { code, .. } => {
            Err(format!("durable Host refused Body context: {code}"))
        }
        _ => Err("durable Host returned the wrong Body context response".into()),
    }
}

#[cfg(not(unix))]
pub(crate) fn install_body_context(
    _state_dir: &Path,
    _expected: &HostAdvertisement,
    _context: BodyConversationContext,
) -> Result<HostAdvertisement, String> {
    Err("no reviewed local durable Host control carrier exists on this platform".into())
}

#[cfg(unix)]
pub(crate) fn prepare_remote(
    state_dir: &Path,
    expected: &HostAdvertisement,
    plan: Plan,
) -> Result<DurableRemotePreparation, String> {
    use std::os::unix::net::UnixStream;
    let mut token = read_secret(&state_dir.join("control.token"))?;
    let mut stream = UnixStream::connect(state_dir.join("control.sock"))
        .map_err(|error| format!("connect to durable Host service: {error}"))?;
    let mut request = Request::PrepareRemote {
        protocol: PROTOCOL,
        token: token.to_vec(),
        expected_boot_id: expected.boot_id.as_str().into(),
        expected_offer_generation: expected.offer_generation.0,
        plan: Box::new(plan),
    };
    write_sensitive_frame(&mut stream, &request)?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|error| format!("finish durable Host remote preparation: {error}"))?;
    token.fill(0);
    if let Request::PrepareRemote { token, .. } = &mut request {
        token.fill(0);
    }
    match read_frame::<_, Response>(&mut stream)? {
        Response::RemotePrepared {
            protocol: PROTOCOL,
            identity,
            hello_frames,
        } => Ok(DurableRemotePreparation {
            identity,
            hello_frames,
        }),
        Response::Refused { code, .. } => {
            Err(format!("durable Host refused remote preparation: {code}"))
        }
        _ => Err("durable Host returned the wrong remote preparation response".into()),
    }
}

#[cfg(not(unix))]
pub(crate) fn prepare_remote(
    _state_dir: &Path,
    _expected: &HostAdvertisement,
    _plan: Plan,
) -> Result<DurableRemotePreparation, String> {
    Err("no reviewed local durable Host control carrier exists on this platform".into())
}

#[cfg(unix)]
pub(crate) fn release_remote(state_dir: &Path) -> Result<(), String> {
    use std::os::unix::net::UnixStream;
    let mut token = read_secret(&state_dir.join("control.token"))?;
    let mut stream = UnixStream::connect(state_dir.join("control.sock"))
        .map_err(|error| format!("connect to durable Host service: {error}"))?;
    let mut request = Request::ReleaseRemote {
        protocol: PROTOCOL,
        token: token.to_vec(),
    };
    write_sensitive_frame(&mut stream, &request)?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|error| format!("finish durable Host remote release: {error}"))?;
    token.fill(0);
    if let Request::ReleaseRemote { token, .. } = &mut request {
        token.fill(0);
    }
    match read_frame::<_, Response>(&mut stream)? {
        Response::RemoteReleased { protocol: PROTOCOL } => Ok(()),
        Response::Refused { code, .. } => {
            Err(format!("durable Host refused remote release: {code}"))
        }
        _ => Err("durable Host returned the wrong remote release response".into()),
    }
}

#[cfg(unix)]
pub(crate) fn exchange_remote(
    state_dir: &Path,
    frame: Vec<u8>,
) -> Result<DurableRemoteExchange, String> {
    use std::os::unix::net::UnixStream;
    let mut token = read_secret(&state_dir.join("control.token"))?;
    let mut stream = UnixStream::connect(state_dir.join("control.sock"))
        .map_err(|error| format!("connect to durable Host service: {error}"))?;
    let mut request = Request::ExchangeRemote {
        protocol: PROTOCOL,
        token: token.to_vec(),
        frame,
    };
    write_sensitive_frame(&mut stream, &request)?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|error| format!("finish durable Host remote exchange: {error}"))?;
    token.fill(0);
    if let Request::ExchangeRemote { token, frame, .. } = &mut request {
        token.fill(0);
        frame.fill(0);
    }
    match read_frame::<_, Response>(&mut stream)? {
        Response::RemoteExchanged {
            protocol: PROTOCOL,
            responses,
            active,
        } => Ok(DurableRemoteExchange { responses, active }),
        Response::Refused { code, .. } => {
            Err(format!("durable Host refused remote exchange: {code}"))
        }
        _ => Err("durable Host returned the wrong remote exchange response".into()),
    }
}

#[cfg(not(unix))]
pub(crate) fn exchange_remote(
    _state_dir: &Path,
    mut frame: Vec<u8>,
) -> Result<DurableRemoteExchange, String> {
    frame.fill(0);
    Err("no reviewed local durable Host control carrier exists on this platform".into())
}

#[cfg(not(unix))]
pub(crate) fn release_remote(_state_dir: &Path) -> Result<(), String> {
    Err("no reviewed local durable Host control carrier exists on this platform".into())
}

#[cfg(not(unix))]
pub(crate) fn join(
    _state_dir: &Path,
    _expected: &HostAdvertisement,
    _claim: SpawnInvitationClaim,
    mut secret: Vec<u8>,
) -> Result<DurableJoinProof, String> {
    secret.fill(0);
    Err("no reviewed local durable Host control carrier exists on this platform".into())
}

fn handle(mut request: Request, token: &[u8; 32], runtime: &mut DurableHostRuntime) -> Response {
    let offered = match &mut request {
        Request::Status { token, .. }
        | Request::Join { token, .. }
        | Request::InstallBodyContext { token, .. }
        | Request::PrepareRemote { token, .. }
        | Request::ExchangeRemote { token, .. }
        | Request::ReleaseRemote { token, .. } => token,
    };
    let authenticated = constant_time_equal(offered, token);
    offered.fill(0);
    if !authenticated {
        return refused("unauthorized");
    }
    let truth = runtime.truth();
    match request {
        Request::Status { protocol, .. } if protocol == PROTOCOL => Response::Status {
            protocol: PROTOCOL,
            target_id: truth.target_id.clone(),
            image_content_digest: truth.image_content_digest.clone(),
            advertisement: truth.advertisement.clone(),
        },
        Request::Join {
            protocol,
            expected_boot_id,
            expected_offer_generation,
            claim,
            mut secret,
            ..
        } if protocol == PROTOCOL => {
            let result = create_join(
                &truth,
                &expected_boot_id,
                expected_offer_generation,
                &claim,
                &secret,
            );
            secret.fill(0);
            result.unwrap_or_else(|code| refused(&code))
        }
        Request::InstallBodyContext {
            protocol,
            expected_boot_id,
            expected_offer_generation,
            context,
            ..
        } if protocol == PROTOCOL => runtime
            .install_body_context(&expected_boot_id, expected_offer_generation, &context)
            .map(|advertisement| Response::BodyContextInstalled {
                protocol: PROTOCOL,
                advertisement,
            })
            .unwrap_or_else(|code| refused(&code)),
        Request::PrepareRemote {
            protocol,
            expected_boot_id,
            expected_offer_generation,
            plan,
            ..
        } if protocol == PROTOCOL => runtime
            .prepare_remote(&expected_boot_id, expected_offer_generation, &plan)
            .map(|preparation| Response::RemotePrepared {
                protocol: PROTOCOL,
                identity: preparation.identity,
                hello_frames: preparation.hello_frames,
            })
            .unwrap_or_else(|code| refused(&code)),
        Request::ExchangeRemote {
            protocol,
            mut frame,
            ..
        } if protocol == PROTOCOL => {
            let result = runtime.exchange_remote_frame(&frame);
            frame.fill(0);
            result
                .map(|exchange| Response::RemoteExchanged {
                    protocol: PROTOCOL,
                    responses: exchange.responses,
                    active: exchange.active,
                })
                .unwrap_or_else(|code| refused(&code))
        }
        Request::ReleaseRemote { protocol, .. } if protocol == PROTOCOL => runtime
            .release_remote()
            .map(|()| Response::RemoteReleased { protocol: PROTOCOL })
            .unwrap_or_else(|code| refused(&code)),
        _ => refused("protocol"),
    }
}

fn create_join(
    truth: &DurableHostTruth,
    expected_boot_id: &str,
    expected_offer_generation: u64,
    claim: &SpawnInvitationClaim,
    secret: &[u8],
) -> Result<Response, String> {
    if truth.advertisement.boot_id.as_str() != expected_boot_id
        || truth.advertisement.offer_generation.0 != expected_offer_generation
    {
        return Err("stale-host-truth".into());
    }
    let now = now_millis().map_err(|_| "clock".to_string())?;
    claim.inspect(now).map_err(|_| "invitation".to_string())?;
    let secret: [u8; 32] = secret
        .try_into()
        .map_err(|_| "invitation-secret".to_string())?;
    let secret = SpawnInvitationSecret::from_csprng_bytes(secret)
        .map_err(|_| "invitation-secret".to_string())?;
    let transcript = claim.signing_transcript(
        &truth.advertisement.host_id,
        &truth.advertisement.boot_id,
        truth.advertisement.offer_generation,
    );
    Ok(Response::Join {
        protocol: PROTOCOL,
        advertisement: truth.advertisement.clone(),
        invitation_id: claim.invitation_id.as_str().into(),
        body_id: claim.body_id.as_str().into(),
        nonce: claim.nonce,
        signature: secret.sign(&transcript).to_vec(),
        observed_at_millis: now,
    })
}

fn refused(code: &str) -> Response {
    Response::Refused {
        protocol: PROTOCOL,
        code: code.into(),
    }
}

fn read_secret(path: &Path) -> Result<[u8; 32], String> {
    let bytes = fs::read(path).map_err(|error| format!("read local control token: {error}"))?;
    bytes
        .try_into()
        .map_err(|_| "local control token has the wrong finite bound".into())
}

fn read_frame<R: Read, T: for<'de> Deserialize<'de>>(reader: &mut R) -> Result<T, String> {
    let mut bytes = Vec::with_capacity(4096);
    reader
        .take((MAXIMUM_CONTROL_FRAME_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read local control frame: {error}"))?;
    if bytes.is_empty() || bytes.len() > MAXIMUM_CONTROL_FRAME_BYTES {
        bytes.fill(0);
        return Err("local control frame violates its finite bound".into());
    }
    let value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("decode local control frame: {error}"));
    bytes.fill(0);
    value
}

fn write_frame<W: Write, T: Serialize>(writer: &mut W, value: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    if bytes.len() > MAXIMUM_CONTROL_FRAME_BYTES {
        return Err("local control response violates its finite bound".into());
    }
    writer
        .write_all(&bytes)
        .map_err(|error| format!("write local control frame: {error}"))
}

fn write_sensitive_frame<W: Write, T: Serialize>(writer: &mut W, value: &T) -> Result<(), String> {
    let mut bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    if bytes.len() > MAXIMUM_CONTROL_FRAME_BYTES {
        bytes.fill(0);
        return Err("local control response violates its finite bound".into());
    }
    let result = writer
        .write_all(&bytes)
        .map_err(|error| format!("write local control frame: {error}"));
    bytes.fill(0);
    result
}

fn constant_time_equal(offered: &[u8], expected: &[u8; 32]) -> bool {
    let mut difference = offered.len() ^ expected.len();
    for (index, expected_byte) in expected.iter().copied().enumerate() {
        difference |= usize::from(offered.get(index).copied().unwrap_or(0) ^ expected_byte);
    }
    difference == 0
}

fn now_millis() -> Result<u64, String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    u64::try_from(millis).map_err(|_| "system clock exceeds control representation".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_body::{
        Body, BodyConversationContext, HostPresenceClock, HostPresenceClockScale, HostPresenceTable,
    };
    use conduit_core::{
        process_owned_line_offer_with_limits, BaseImplementationId, BootId, GearId, HostId,
        LineScope, LineSecurity, LinkLimits, OfferGeneration,
    };
    use conduit_planner::{PlacementChoice, PlacementChoices};
    use conduit_std_host::{StdHost, StdHostConfig};
    use std::{collections::BTreeMap, path::PathBuf};

    fn runtime() -> DurableHostRuntime {
        let host = StdHost::new_with_config(StdHostConfig {
            host_id: HostId::from("host/durable-fixture"),
            boot_id: BootId::from("boot/durable-fixture"),
            offer_generation: OfferGeneration(7),
        });
        DurableHostRuntime::new(
            "std/x86_64/computer".into(),
            format!("sha256:{}", "a".repeat(64)),
            host,
        )
    }

    fn claim() -> SpawnInvitationClaim {
        serde_json::from_value(serde_json::json!({
            "invitation_id":"invitation/durable",
            "body_id":"body/durable",
            "nonce":vec![17;32],
            "expires_at_millis":4_000_000_000_000_u64
        }))
        .unwrap()
    }

    fn context() -> BodyConversationContext {
        let body = Body::born(
            conduit_core::SourceDocumentId::from("source/orifinia"),
            conduit_core::CheckedFormId::from("checked/orifinia"),
            1,
            conduit_core::SignId::from("sign/orifinia/born"),
        )
        .unwrap();
        let (body, wake) = body
            .wake(1, conduit_core::SignId::from("sign/orifinia/wake"))
            .unwrap();
        let presence = HostPresenceTable::new(
            body.body_id.clone(),
            HostPresenceClock::new(
                "clock/orifinia".into(),
                HostPresenceClockScale::Milliseconds,
                1,
                0,
            )
            .unwrap(),
            30_000,
        )
        .unwrap();
        BodyConversationContext::from_current_truth(
            "Orifinia Dawnheart",
            &body,
            &wake,
            &presence,
            None,
            &[],
        )
        .unwrap()
    }

    fn remote_plan() -> Plan {
        let form = crate::form_source::load(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../forms/hello/main.conduit"),
        )
        .unwrap();
        let form = form.expand_entry().unwrap();
        let source = crate::std_websocket_line::host(crate::std_websocket_line::SOURCE_HOST);
        let sink = crate::std_websocket_line::host(crate::std_websocket_line::SINK_HOST);
        let mut offer = process_owned_line_offer_with_limits(
            "body-line/remote-control-test",
            "body-line/remote-control-test/binding",
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
            "body-line/remote-control-test/instance",
            source.advertisement(),
            sink.advertisement(),
            LinkLimits {
                maximum_in_flight_items: 4,
                maximum_payload_bytes: 4_096,
                maximum_buffered_bytes: 16_384,
                maximum_frame_bytes: 8_192,
            },
        );
        offer.contract.scope = LineScope::LocalNetwork;
        offer.contract.security = LineSecurity::PlaintextNetwork;
        let advertisements = [source.advertisement().clone(), sink.advertisement().clone()];
        let placements = PlacementChoices {
            by_gear: form
                .gears
                .iter()
                .map(|gear| {
                    let host = if gear.kind_id.as_str()
                        == conduit_semantic_catalog::TEXT_PRESENTATION_KIND
                    {
                        &advertisements[1]
                    } else {
                        &advertisements[0]
                    };
                    let capability = host
                        .capabilities
                        .iter()
                        .find(|offer| offer.kind_id == gear.kind_id)
                        .unwrap();
                    (
                        GearId::from(gear.gear_id.as_str()),
                        PlacementChoice {
                            host_id: host.host_id.clone(),
                            capability_id: capability.capability_id.clone(),
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>(),
        };
        crate::product_execution::ProductExecutionContext::new(
            advertisements.into(),
            vec![
                crate::product_execution::ProductRuntime::std(source),
                crate::product_execution::ProductRuntime::std(sink),
            ],
            vec![
                BaseImplementationId::from("conduit.base/local@1"),
                BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
            ],
            vec![offer],
            Vec::new(),
        )
        .unwrap()
        .plan_with_placements(&form, &placements)
        .unwrap()
    }

    #[test]
    fn durable_owner_signs_only_its_exact_current_boot_and_generation() {
        let mut runtime = runtime();
        let truth = runtime.truth();
        let token = [23_u8; 32];
        let response = handle(
            Request::Join {
                protocol: PROTOCOL,
                token: token.to_vec(),
                expected_boot_id: truth.advertisement.boot_id.as_str().into(),
                expected_offer_generation: 7,
                claim: claim(),
                secret: vec![29; 32],
            },
            &token,
            &mut runtime,
        );
        let Response::Join {
            advertisement,
            signature,
            ..
        } = response
        else {
            panic!("current durable Host did not issue join proof")
        };
        assert_eq!(advertisement, truth.advertisement);
        assert_eq!(signature.len(), 64);

        let stale = handle(
            Request::Join {
                protocol: PROTOCOL,
                token: token.to_vec(),
                expected_boot_id: "boot/stale".into(),
                expected_offer_generation: 7,
                claim: claim(),
                secret: vec![29; 32],
            },
            &token,
            &mut runtime,
        );
        assert!(matches!(stale, Response::Refused { ref code, .. } if code == "stale-host-truth"));
    }

    #[test]
    fn unauthorized_helper_cannot_obtain_or_substitute_advertisement() {
        let mut runtime = runtime();
        let response = handle(
            Request::Status {
                protocol: PROTOCOL,
                token: vec![0; 32],
            },
            &[23; 32],
            &mut runtime,
        );
        assert!(matches!(response, Response::Refused { ref code, .. } if code == "unauthorized"));
    }

    #[test]
    fn durable_runtime_retains_live_host_truth_instead_of_a_frozen_advertisement() {
        let mut runtime = runtime();
        let before = runtime.truth().advertisement;
        runtime
            .host
            .install_body_conversation_context(&context())
            .unwrap();
        let after = runtime.truth().advertisement;

        assert_eq!(after.host_id, before.host_id);
        assert_eq!(after.boot_id, before.boot_id);
        assert_eq!(after.offer_generation.0, before.offer_generation.0 + 1);
        assert!(after.capabilities.iter().any(|offer| {
            offer.implementation.implementation_id.as_str()
                == conduit_std_offers::BODY_CONVERSATION_CONTEXT_STD_IMPLEMENTATION
        }));
    }

    #[test]
    fn cancellation_signal_names_one_exact_active_play_and_is_cleared_by_its_owner() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let state_dir = std::env::temp_dir().join(format!(
            "conduit-remote-cancellation-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&state_dir).unwrap();
        signal_remote_cancellation(&state_dir, "play/orifinia/voice-7").unwrap();
        assert_eq!(
            fs::read(state_dir.join("remote-cancellation.signal")).unwrap(),
            b"play/orifinia/voice-7"
        );

        let mut runtime = runtime();
        runtime.install_cancellation_signal(&state_dir);
        runtime.clear_cancellation_signal().unwrap();
        assert!(!state_dir.join("remote-cancellation.signal").exists());
        fs::remove_dir(state_dir).unwrap();
    }

    #[test]
    fn durable_runtime_activates_and_drives_exact_remote_play_until_explicit_release() {
        let host = crate::std_websocket_line::host(crate::std_websocket_line::SOURCE_HOST);
        let truth = host.advertisement().clone();
        let mut runtime = DurableHostRuntime::new(
            "std/x86_64/computer".into(),
            format!("sha256:{}", "b".repeat(64)),
            host,
        );
        let plan = remote_plan();

        let prepared = runtime
            .prepare_remote(truth.boot_id.as_str(), truth.offer_generation.0, &plan)
            .unwrap();
        assert_eq!(prepared.identity.host_id, truth.host_id);
        assert_eq!(prepared.identity.boot_id, truth.boot_id);
        assert_eq!(prepared.identity.plan_id, plan.plan_id);
        assert!(!prepared.hello_frames.is_empty());
        let ready = runtime
            .exchange_remote_frame(&prepared.hello_frames[0])
            .unwrap();
        assert!(!ready.active);
        assert_eq!(ready.responses.len(), 1);
        let ready_frame = &ready.responses[0];
        assert!(matches!(
            decode_session_frame(
                ready_frame,
                MAXIMUM_CONTROL_FRAME_BYTES as u32,
                MAXIMUM_CONTROL_FRAME_BYTES as u32,
            )
            .unwrap()
            .message,
            SessionMessage::Ready
        ));
        let active = runtime.exchange_remote_frame(ready_frame).unwrap();
        assert!(active.active);
        assert_eq!(active.responses.len(), 1);
        let accepted_frame = {
            let offered = decode_session_frame(
                &active.responses[0],
                MAXIMUM_CONTROL_FRAME_BYTES as u32,
                MAXIMUM_CONTROL_FRAME_BYTES as u32,
            )
            .unwrap();
            assert!(matches!(
                offered.message,
                SessionMessage::Offered {
                    sequence: 0,
                    payload: b"HELLO, WORLD."
                }
            ));
            let mut encoded = vec![0; MAXIMUM_CONTROL_FRAME_BYTES];
            let length = encode_session_frame_into(
                conduit_wire::SessionFrame {
                    identity: offered.identity,
                    message: SessionMessage::Accepted { sequence: 0 },
                },
                &mut encoded,
                MAXIMUM_CONTROL_FRAME_BYTES as u32,
                MAXIMUM_CONTROL_FRAME_BYTES as u32,
            )
            .unwrap();
            encoded.truncate(length);
            encoded
        };
        let accepted = runtime.exchange_remote_frame(&accepted_frame).unwrap();
        assert!(accepted.active);
        assert!(accepted.responses.is_empty());
        let delivered_frame = {
            let accepted = decode_session_frame(
                &accepted_frame,
                MAXIMUM_CONTROL_FRAME_BYTES as u32,
                MAXIMUM_CONTROL_FRAME_BYTES as u32,
            )
            .unwrap();
            let mut encoded = vec![0; MAXIMUM_CONTROL_FRAME_BYTES];
            let length = encode_session_frame_into(
                conduit_wire::SessionFrame {
                    identity: accepted.identity,
                    message: SessionMessage::Delivered { sequence: 0 },
                },
                &mut encoded,
                MAXIMUM_CONTROL_FRAME_BYTES as u32,
                MAXIMUM_CONTROL_FRAME_BYTES as u32,
            )
            .unwrap();
            encoded.truncate(length);
            encoded
        };
        let delivered = runtime.exchange_remote_frame(&delivered_frame).unwrap();
        assert!(!delivered.active);
        assert_eq!(delivered.responses.len(), 2);
        let closed = decode_session_frame(
            &delivered.responses[0],
            MAXIMUM_CONTROL_FRAME_BYTES as u32,
            MAXIMUM_CONTROL_FRAME_BYTES as u32,
        )
        .unwrap();
        assert!(matches!(
            closed.message,
            SessionMessage::InputClosed { final_sequence: 1 }
        ));
        let terminal = decode_session_frame(
            &delivered.responses[1],
            MAXIMUM_CONTROL_FRAME_BYTES as u32,
            MAXIMUM_CONTROL_FRAME_BYTES as u32,
        )
        .unwrap();
        assert!(matches!(
            terminal.message,
            SessionMessage::Terminal {
                disposition: conduit_wire::SessionTerminalDisposition::Completed,
                final_sequence: 1
            }
        ));
        let mut peer_terminal = vec![0; MAXIMUM_CONTROL_FRAME_BYTES];
        let length = encode_session_frame_into(
            conduit_wire::SessionFrame {
                identity: terminal.identity,
                message: SessionMessage::Terminal {
                    disposition: conduit_wire::SessionTerminalDisposition::Completed,
                    final_sequence: 1,
                },
            },
            &mut peer_terminal,
            MAXIMUM_CONTROL_FRAME_BYTES as u32,
            MAXIMUM_CONTROL_FRAME_BYTES as u32,
        )
        .unwrap();
        peer_terminal.truncate(length);
        let terminal = runtime.exchange_remote_frame(&peer_terminal).unwrap();
        assert!(!terminal.active);
        assert!(terminal.responses.is_empty());
        assert_eq!(
            runtime
                .prepare_remote(truth.boot_id.as_str(), truth.offer_generation.0, &plan)
                .unwrap_err(),
            "remote-play-active"
        );

        runtime.release_remote().unwrap();
        let prepared = runtime
            .prepare_remote(truth.boot_id.as_str(), truth.offer_generation.0, &plan)
            .unwrap();
        let hello = decode_session_frame(
            &prepared.hello_frames[0],
            MAXIMUM_CONTROL_FRAME_BYTES as u32,
            MAXIMUM_CONTROL_FRAME_BYTES as u32,
        )
        .unwrap();
        let ready = runtime
            .exchange_remote_frame(&prepared.hello_frames[0])
            .unwrap()
            .responses
            .remove(0);
        runtime.exchange_remote_frame(&ready).unwrap();
        let mut cancelled = vec![0; MAXIMUM_CONTROL_FRAME_BYTES];
        let length = encode_session_frame_into(
            conduit_wire::SessionFrame {
                identity: hello.identity,
                message: SessionMessage::Cancelled { code: 9 },
            },
            &mut cancelled,
            MAXIMUM_CONTROL_FRAME_BYTES as u32,
            MAXIMUM_CONTROL_FRAME_BYTES as u32,
        )
        .unwrap();
        cancelled.truncate(length);
        let cancellation = runtime.exchange_remote_frame(&cancelled).unwrap();
        assert!(!cancellation.active);
        assert_eq!(cancellation.responses.len(), 2);
        assert!(matches!(
            decode_session_frame(
                &cancellation.responses[0],
                MAXIMUM_CONTROL_FRAME_BYTES as u32,
                MAXIMUM_CONTROL_FRAME_BYTES as u32,
            )
            .unwrap()
            .message,
            SessionMessage::Cancelled { code: 9 }
        ));
        let terminal = decode_session_frame(
            &cancellation.responses[1],
            MAXIMUM_CONTROL_FRAME_BYTES as u32,
            MAXIMUM_CONTROL_FRAME_BYTES as u32,
        )
        .unwrap();
        assert!(matches!(
            terminal.message,
            SessionMessage::Terminal {
                disposition: conduit_wire::SessionTerminalDisposition::Cancelled,
                final_sequence: 0
            }
        ));
        let mut peer_terminal = vec![0; MAXIMUM_CONTROL_FRAME_BYTES];
        let length = encode_session_frame_into(
            conduit_wire::SessionFrame {
                identity: terminal.identity,
                message: SessionMessage::Terminal {
                    disposition: conduit_wire::SessionTerminalDisposition::Cancelled,
                    final_sequence: 0,
                },
            },
            &mut peer_terminal,
            MAXIMUM_CONTROL_FRAME_BYTES as u32,
            MAXIMUM_CONTROL_FRAME_BYTES as u32,
        )
        .unwrap();
        peer_terminal.truncate(length);
        assert!(runtime
            .exchange_remote_frame(&peer_terminal)
            .unwrap()
            .responses
            .is_empty());
        runtime.release_remote().unwrap();
    }
}
