//! Descriptor-pinned TLS client and finite connection/exchange deadlines.
use super::{
    bounded_config, close, map_socket_error, receive_binary, SecureWebSocketError,
    SocketDeadlineGuard,
};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, WebPkiSupportedAlgorithms};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{
    CertificateError, ClientConfig, ClientConnection, DigitallySignedStruct, Error as TlsError,
    SignatureScheme, StreamOwned,
};
use sha2::{Digest, Sha256};
use std::io::ErrorKind;
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tungstenite::client::IntoClientRequest;
use tungstenite::handshake::HandshakeError;
use tungstenite::protocol::{Message, WebSocket};

/// Native client for one descriptor-pinned authenticated TLS candidate.
///
/// The exact leaf certificate digest is the trust root for this finite
/// rendezvous attempt. It neither grants Body membership nor replaces the
/// one-use session proof exchanged above the encrypted carrier.
pub struct SecureWebSocketClientLine {
    pub(super) socket: WebSocket<StreamOwned<ClientConnection, TcpStream>>,
    maximum_message_bytes: usize,
    _io_deadline: Option<SocketDeadlineGuard>,
}

impl SecureWebSocketClientLine {
    pub fn connect_pinned(
        address: SocketAddr,
        url: &str,
        server_identity: &str,
        certificate_binding_sha256: [u8; 32],
        timeout: Duration,
        maximum_message_bytes: u32,
    ) -> Result<Self, SecureWebSocketError> {
        Self::connect(
            address,
            url,
            server_identity,
            certificate_binding_sha256,
            timeout,
            maximum_message_bytes,
            false,
        )
    }

    /// One discovery/request exchange shares its connection attempt's original
    /// absolute deadline, including later partial frames and control traffic.
    /// This does not admit a continuing session or replenish that deadline.
    pub fn connect_pinned_for_exchange(
        address: SocketAddr,
        url: &str,
        server_identity: &str,
        certificate_binding_sha256: [u8; 32],
        timeout: Duration,
        maximum_message_bytes: u32,
    ) -> Result<Self, SecureWebSocketError> {
        Self::connect(
            address,
            url,
            server_identity,
            certificate_binding_sha256,
            timeout,
            maximum_message_bytes,
            true,
        )
    }

    fn connect(
        address: SocketAddr,
        url: &str,
        server_identity: &str,
        certificate_binding_sha256: [u8; 32],
        timeout: Duration,
        maximum_message_bytes: u32,
        retain_exchange_deadline: bool,
    ) -> Result<Self, SecureWebSocketError> {
        let maximum_message_bytes = usize::try_from(maximum_message_bytes)
            .map_err(|_| SecureWebSocketError::InvalidConfiguration)?;
        if maximum_message_bytes == 0
            || timeout.is_zero()
            || address.ip().is_unspecified()
            || certificate_binding_sha256 == [0; 32]
            || !url.starts_with("wss://")
        {
            return Err(SecureWebSocketError::InvalidConfiguration);
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(SecureWebSocketError::InvalidConfiguration)?;
        let request = url
            .into_client_request()
            .map_err(|_| SecureWebSocketError::InvalidConfiguration)?;
        if request.uri().host() != Some(server_identity) {
            return Err(SecureWebSocketError::InvalidConfiguration);
        }
        let server_name = ServerName::try_from(server_identity.to_owned())
            .map_err(|_| SecureWebSocketError::InvalidConfiguration)?;
        let provider = rustls::crypto::ring::default_provider();
        let verifier = PinnedServerCertificate {
            binding_sha256: certificate_binding_sha256,
            supported: provider.signature_verification_algorithms,
        };
        let client_config = ClientConfig::builder_with_provider(provider.into())
            .with_safe_default_protocol_versions()
            .map_err(|_| SecureWebSocketError::Tls)?
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(verifier))
            .with_no_client_auth();
        let remaining = client_connection_remaining(deadline)?;
        let stream = TcpStream::connect_timeout(&address, remaining)
            .map_err(|error| SecureWebSocketError::Transport(error.kind()))?;
        stream
            .set_nodelay(true)
            .map_err(|error| SecureWebSocketError::Transport(error.kind()))?;
        let remaining = client_connection_remaining(deadline)?;
        stream
            .set_read_timeout(Some(remaining))
            .map_err(|error| SecureWebSocketError::Transport(error.kind()))?;
        stream
            .set_write_timeout(Some(remaining))
            .map_err(|error| SecureWebSocketError::Transport(error.kind()))?;
        // Per-read timeouts cannot bound TLS or WebSocket peers that keep
        // supplying partial records. TCP connect and both handshakes share
        // this original deadline; progress never replenishes it.
        let deadline_guard = SocketDeadlineGuard::start(&stream, deadline).map_err(|error| {
            if error == SecureWebSocketError::AcceptDeadline {
                SecureWebSocketError::Transport(ErrorKind::TimedOut)
            } else {
                error
            }
        })?;
        let connection = ClientConnection::new(Arc::new(client_config), server_name)
            .map_err(|_| SecureWebSocketError::Tls)?;
        let tls = StreamOwned::new(connection, stream);
        let (socket, _) = tungstenite::client::client_with_config(
            url,
            tls,
            Some(bounded_config(maximum_message_bytes)?),
        )
        .map_err(|error| match error {
            HandshakeError::Interrupted(_) | HandshakeError::Failure(_) => {
                SecureWebSocketError::Handshake
            }
        })?;
        // A separately admitted session owns its subsequent lifetime. A short
        // discovery exchange instead keeps the same absolute cutoff through
        // its response; unaccepted bytes cannot replenish the attempt.
        let exchange_deadline = if retain_exchange_deadline {
            Some(deadline_guard)
        } else {
            drop(deadline_guard);
            None
        };
        socket
            .get_ref()
            .sock
            .set_read_timeout(Some(timeout))
            .map_err(|error| SecureWebSocketError::Transport(error.kind()))?;
        socket
            .get_ref()
            .sock
            .set_write_timeout(Some(timeout))
            .map_err(|error| SecureWebSocketError::Transport(error.kind()))?;
        client_connection_remaining(deadline)?;
        Ok(Self {
            socket,
            maximum_message_bytes,
            _io_deadline: exchange_deadline,
        })
    }

    pub fn send_binary(&mut self, bytes: &[u8]) -> Result<(), SecureWebSocketError> {
        if bytes.len() > self.maximum_message_bytes {
            return Err(SecureWebSocketError::OversizedMessage);
        }
        self.socket
            .send(Message::Binary(bytes.to_vec().into()))
            .map_err(map_socket_error)
    }

    pub fn receive_binary(&mut self, output: &mut [u8]) -> Result<usize, SecureWebSocketError> {
        receive_binary(&mut self.socket, self.maximum_message_bytes, output)
    }

    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> Result<(), SecureWebSocketError> {
        self.socket
            .get_ref()
            .sock
            .set_read_timeout(timeout)
            .map_err(|error| SecureWebSocketError::Transport(error.kind()))
    }

    /// Bound the established socket by a separately authorized finite lease,
    /// including partial frames and blocked writes. The caller must validate
    /// renewal authority before replacing this deadline, never on raw input.
    /// Failure to prepare the new guard leaves the old guard in place.
    pub fn replace_io_deadline(&mut self, deadline: Instant) -> Result<(), SecureWebSocketError> {
        let guard = SocketDeadlineGuard::start(&self.socket.get_ref().sock, deadline)?;
        self._io_deadline = Some(guard);
        Ok(())
    }

    pub fn close(&mut self) -> Result<(), SecureWebSocketError> {
        close(&mut self.socket)
    }
}

fn client_connection_remaining(deadline: Instant) -> Result<Duration, SecureWebSocketError> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or(SecureWebSocketError::Transport(ErrorKind::TimedOut))
}

#[derive(Debug)]
pub(super) struct PinnedServerCertificate {
    pub(super) binding_sha256: [u8; 32],
    pub(super) supported: WebPkiSupportedAlgorithms,
}

impl ServerCertVerifier for PinnedServerCertificate {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        let actual: [u8; 32] = Sha256::digest(end_entity.as_ref()).into();
        if actual != self.binding_sha256 {
            return Err(TlsError::InvalidCertificate(
                CertificateError::ApplicationVerificationFailure,
            ));
        }
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signed: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls12_signature(message, certificate, signed, &self.supported)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signed: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls13_signature(message, certificate, signed, &self.supported)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.supported.supported_schemes()
    }
}
