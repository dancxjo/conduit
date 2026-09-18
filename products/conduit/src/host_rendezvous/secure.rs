//! Opt-in secure network carrier for the shared running-Host session.

use conduit_body::{
    RendezvousAuthentication, RendezvousCandidate, RendezvousLineFamily,
    RunningHostRendezvousDescriptor,
};
use conduit_std_host::secure_websocket::{
    SecureWebSocketError, SecureWebSocketLine, SecureWebSocketListener,
};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{debug, run_session, RendezvousLine, MAXIMUM_FRAME_BYTES};

pub(crate) struct SecureNetworkOptions {
    pub(crate) bind: Option<String>,
    pub(crate) public_url: Option<String>,
    pub(crate) tls_cert: Option<PathBuf>,
    pub(crate) tls_key: Option<PathBuf>,
    pub(crate) authorize_network: bool,
}

impl RendezvousLine for SecureWebSocketLine {
    fn receive(&mut self) -> Result<Vec<u8>, String> {
        let mut bytes = vec![0_u8; MAXIMUM_FRAME_BYTES];
        let length = self
            .receive_binary(&mut bytes)
            .map_err(debug("receive secure rendezvous frame"))?;
        bytes.truncate(length);
        Ok(bytes)
    }

    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.send_binary(bytes)
            .map_err(debug("send secure rendezvous frame"))
    }

    fn close(&mut self) -> Result<(), String> {
        SecureWebSocketLine::close(self).map_err(debug("close secure rendezvous Line"))
    }

    fn enter_retained_idle(&mut self) -> Result<(), String> {
        self.set_read_timeout(None)
            .map_err(debug("remove secure rendezvous read timeout"))
    }

    fn poll_receive(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, String> {
        self.set_read_timeout(Some(timeout))
            .map_err(debug("set secure rendezvous poll timeout"))?;
        let mut bytes = vec![0_u8; MAXIMUM_FRAME_BYTES];
        let result = self.receive_binary(&mut bytes);
        self.set_read_timeout(None)
            .map_err(debug("restore secure rendezvous idle timeout"))?;
        match result {
            Ok(length) => {
                bytes.truncate(length);
                Ok(Some(bytes))
            }
            Err(SecureWebSocketError::Transport(
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut,
            )) => Ok(None),
            Err(error) => Err(format!("poll secure rendezvous frame: {error:?}")),
        }
    }

    fn supports_interruptible_exchange(&self) -> bool {
        true
    }
}

pub(crate) fn serve(
    state_dir: &Path,
    timeout_seconds: u64,
    options: SecureNetworkOptions,
) -> Result<(), String> {
    if !options.authorize_network {
        return Err("secure LAN rendezvous requires --authorize-network".into());
    }
    let bind = options
        .bind
        .ok_or_else(|| "secure LAN rendezvous requires --bind".to_string())?
        .parse::<SocketAddr>()
        .map_err(|_| "secure LAN rendezvous --bind is not an exact socket address".to_string())?;
    let public_url = options
        .public_url
        .ok_or_else(|| "secure LAN rendezvous requires --public-url".to_string())?;
    if !public_url.starts_with("wss://") || public_url.len() > 256 {
        return Err("secure LAN rendezvous --public-url must be one bounded wss URL".into());
    }
    let certificate = options
        .tls_cert
        .ok_or_else(|| "secure LAN rendezvous requires --tls-cert".to_string())?;
    let private_key = options
        .tls_key
        .ok_or_else(|| "secure LAN rendezvous requires --tls-key".to_string())?;
    let listener =
        SecureWebSocketListener::bind(bind, &certificate, &private_key, MAXIMUM_FRAME_BYTES as u32)
            .map_err(debug("bind secure rendezvous Line"))?;
    let actual = listener
        .local_addr()
        .map_err(debug("read secure rendezvous address"))?;
    if actual != bind {
        return Err("secure LAN rendezvous did not bind the exact authorized socket".into());
    }
    let mut session_secret = [0_u8; 32];
    getrandom::fill(&mut session_secret)
        .map_err(|error| format!("create rendezvous secret: {error}"))?;
    if session_secret == [0; 32] {
        return Err("system randomness returned a weak rendezvous secret".into());
    }
    let now_millis: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock precedes the Unix epoch".to_string())?
        .as_millis()
        .try_into()
        .map_err(|_| "system clock is outside the rendezvous representation".to_string())?;
    let expires_at_millis = now_millis
        .checked_add(timeout_seconds.saturating_mul(1_000))
        .ok_or_else(|| "secure rendezvous expiry overflowed".to_string())?;
    let server_identity = public_url
        .strip_prefix("wss://")
        .and_then(|remainder| remainder.split(['/', ':']).next())
        .filter(|identity| !identity.is_empty())
        .ok_or_else(|| "secure LAN rendezvous URL omitted a server identity".to_string())?;
    let descriptor = RunningHostRendezvousDescriptor::new(
        vec![RendezvousCandidate {
            candidate_id: "candidate/secure-lan".into(),
            line_family: RendezvousLineFamily::AuthenticatedTlsStream,
            reachability: public_url.clone(),
            authentication: RendezvousAuthentication {
                server_identity: server_identity.into(),
                transport_binding_sha256: listener.certificate_binding_sha256(),
            },
            expires_at_millis,
            maximum_attempts: 1,
            attempt_timeout_millis: timeout_seconds.min(30).saturating_mul(1_000) as u32,
        }],
        session_secret,
        now_millis,
    )
    .map_err(|error| format!("construct secure rendezvous descriptor: {error:?}"))?;
    println!(
        "Rendezvous descriptor: {}",
        serde_json::to_string(&descriptor)
            .map_err(|error| format!("encode secure rendezvous descriptor: {error}"))?
    );
    println!("The remote browser must trust the configured TLS identity.");
    println!("Waiting up to {timeout_seconds} seconds for one authenticated connection…");
    let result = (|| {
        let mut line = listener
            .accept_with_timeout(Duration::from_secs(timeout_seconds))
            .map_err(debug("accept secure rendezvous Line"))?;
        run_session(
            &mut line,
            state_dir,
            &session_secret,
            "conduit-line/authenticated-tls-stream@1",
        )
    })();
    session_secret.fill(0);
    result
}
