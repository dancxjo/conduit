//! Explicit TLS-authenticated WebSocket Line for non-loopback rendezvous.
//!
//! This is deliberately separate from [`crate::websocket`]: the local
//! `C1-WS` carrier remains plaintext and loopback-only, while this listener
//! requires operator-supplied TLS identity and an explicit network bind.

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, WebPkiSupportedAlgorithms};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::{
    CertificateError, ClientConfig, ClientConnection, DigitallySignedStruct, Error as TlsError,
    ServerConfig, ServerConnection, SignatureScheme, StreamOwned,
};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, ErrorKind, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};
use tungstenite::client::IntoClientRequest;
use tungstenite::error::Error as TungsteniteError;
use tungstenite::handshake::HandshakeError;
use tungstenite::protocol::frame::coding::CloseCode;
use tungstenite::protocol::{CloseFrame, Message, WebSocket, WebSocketConfig};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SecureWebSocketError {
    InvalidConfiguration,
    Identity(ErrorKind),
    Bind(ErrorKind),
    Accept(ErrorKind),
    AcceptDeadline,
    Tls,
    Handshake,
    Transport(ErrorKind),
    Protocol,
    TextMessageRejected,
    OversizedMessage,
    OutputTooSmall,
    Disconnected,
}

pub struct SecureWebSocketListener {
    listener: TcpListener,
    server_config: Arc<ServerConfig>,
    maximum_message_bytes: usize,
    certificate_binding_sha256: [u8; 32],
}

impl SecureWebSocketListener {
    pub fn bind(
        address: SocketAddr,
        certificate_pem: &Path,
        private_key_pem: &Path,
        maximum_message_bytes: u32,
        authorize_network: bool,
    ) -> Result<Self, SecureWebSocketError> {
        let maximum_message_bytes = usize::try_from(maximum_message_bytes)
            .map_err(|_| SecureWebSocketError::InvalidConfiguration)?;
        if maximum_message_bytes == 0
            || !authorize_network
            || address.ip().is_loopback()
            || address.port() == 0
        {
            return Err(SecureWebSocketError::InvalidConfiguration);
        }
        let certificates = read_certificates(certificate_pem)?;
        let certificate_binding_sha256 = Sha256::digest(certificates[0].as_ref()).into();
        let private_key = read_private_key(private_key_pem)?;
        let provider = rustls::crypto::ring::default_provider();
        let server_config = ServerConfig::builder_with_provider(provider.into())
            .with_safe_default_protocol_versions()
            .map_err(|_| SecureWebSocketError::Tls)?
            .with_no_client_auth()
            .with_single_cert(certificates, private_key)
            .map_err(|_| SecureWebSocketError::Tls)?;
        let listener =
            TcpListener::bind(address).map_err(|error| SecureWebSocketError::Bind(error.kind()))?;
        Ok(Self {
            listener,
            server_config: Arc::new(server_config),
            maximum_message_bytes,
            certificate_binding_sha256,
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr, SecureWebSocketError> {
        self.listener
            .local_addr()
            .map_err(|error| SecureWebSocketError::Bind(error.kind()))
    }

    pub fn certificate_binding_sha256(&self) -> [u8; 32] {
        self.certificate_binding_sha256
    }

    pub fn accept_with_timeout(
        &self,
        timeout: Duration,
    ) -> Result<SecureWebSocketLine, SecureWebSocketError> {
        if timeout.is_zero() {
            return Err(SecureWebSocketError::InvalidConfiguration);
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(SecureWebSocketError::InvalidConfiguration)?;
        self.listener
            .set_nonblocking(true)
            .map_err(|error| SecureWebSocketError::Accept(error.kind()))?;
        let stream = loop {
            match self.listener.accept() {
                Ok((stream, _peer)) => break stream,
                Err(error)
                    if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) =>
                {
                    if Instant::now() >= deadline {
                        self.listener
                            .set_nonblocking(false)
                            .map_err(|error| SecureWebSocketError::Accept(error.kind()))?;
                        return Err(SecureWebSocketError::AcceptDeadline);
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => {
                    self.listener
                        .set_nonblocking(false)
                        .map_err(|restore| SecureWebSocketError::Accept(restore.kind()))?;
                    return Err(SecureWebSocketError::Accept(error.kind()));
                }
            }
        };
        self.listener
            .set_nonblocking(false)
            .map_err(|error| SecureWebSocketError::Accept(error.kind()))?;
        stream
            .set_nodelay(true)
            .map_err(|error| SecureWebSocketError::Transport(error.kind()))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(|error| SecureWebSocketError::Transport(error.kind()))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(|error| SecureWebSocketError::Transport(error.kind()))?;
        // A socket read timeout is per read. A peer can otherwise trickle a
        // handshake or partial frame indefinitely while each read succeeds.
        let deadline_guard = SocketDeadlineGuard::start(&stream, deadline)?;
        let connection = ServerConnection::new(Arc::clone(&self.server_config))
            .map_err(|_| SecureWebSocketError::Tls)?;
        let tls = StreamOwned::new(connection, stream);
        let socket =
            tungstenite::accept_with_config(tls, Some(bounded_config(self.maximum_message_bytes)?))
                .map_err(|error| match error {
                    HandshakeError::Interrupted(_) | HandshakeError::Failure(_) => {
                        SecureWebSocketError::Handshake
                    }
                })?;
        if Instant::now() >= deadline {
            return Err(SecureWebSocketError::AcceptDeadline);
        }
        Ok(SecureWebSocketLine {
            socket,
            maximum_message_bytes: self.maximum_message_bytes,
            input_deadline_guard: Some(deadline_guard),
        })
    }
}

pub struct SecureWebSocketLine {
    socket: WebSocket<StreamOwned<ServerConnection, TcpStream>>,
    maximum_message_bytes: usize,
    input_deadline_guard: Option<SocketDeadlineGuard>,
}

/// Enforce one absolute lifetime across accept, TLS/WebSocket handshake, and
/// every later frame. The watchdog owns only a clone of this exact socket and
/// is joined when the Line ends; it cannot affect another accepted Line.
struct SocketDeadlineGuard {
    cancel: mpsc::Sender<()>,
    watchdog: Option<thread::JoinHandle<()>>,
}

impl SocketDeadlineGuard {
    fn start(stream: &TcpStream, deadline: Instant) -> Result<Self, SecureWebSocketError> {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or(SecureWebSocketError::AcceptDeadline)?;
        let shutdown_stream = stream
            .try_clone()
            .map_err(|error| SecureWebSocketError::Transport(error.kind()))?;
        let (cancel, receiver) = mpsc::channel();
        let watchdog = thread::Builder::new()
            .name("conduit-wss-deadline".into())
            .spawn(move || {
                if matches!(
                    receiver.recv_timeout(remaining),
                    Err(mpsc::RecvTimeoutError::Timeout)
                ) {
                    let _ = shutdown_stream.shutdown(Shutdown::Both);
                }
            })
            .map_err(|error| SecureWebSocketError::Transport(error.kind()))?;
        Ok(Self {
            cancel,
            watchdog: Some(watchdog),
        })
    }
}

impl Drop for SocketDeadlineGuard {
    fn drop(&mut self) {
        let _ = self.cancel.send(());
        if let Some(watchdog) = self.watchdog.take() {
            let _ = watchdog.join();
        }
    }
}

/// Native client for one descriptor-pinned authenticated TLS candidate.
///
/// The exact leaf certificate digest is the trust root for this finite
/// rendezvous attempt. It neither grants Body membership nor replaces the
/// one-use session proof exchanged above the encrypted carrier.
pub struct SecureWebSocketClientLine {
    socket: WebSocket<StreamOwned<ClientConnection, TcpStream>>,
    maximum_message_bytes: usize,
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
        let stream = TcpStream::connect_timeout(&address, timeout)
            .map_err(|error| SecureWebSocketError::Transport(error.kind()))?;
        stream
            .set_nodelay(true)
            .map_err(|error| SecureWebSocketError::Transport(error.kind()))?;
        stream
            .set_read_timeout(Some(timeout))
            .map_err(|error| SecureWebSocketError::Transport(error.kind()))?;
        stream
            .set_write_timeout(Some(timeout))
            .map_err(|error| SecureWebSocketError::Transport(error.kind()))?;
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
        Ok(Self {
            socket,
            maximum_message_bytes,
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

    pub fn close(&mut self) -> Result<(), SecureWebSocketError> {
        close(&mut self.socket)
    }
}

#[derive(Debug)]
struct PinnedServerCertificate {
    binding_sha256: [u8; 32],
    supported: WebPkiSupportedAlgorithms,
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

impl SecureWebSocketLine {
    /// A caller may release the input deadline only after it has received and
    /// admitted a complete request before that deadline. Bounded actor work
    /// and its response may then finish after the grant's submission cutoff.
    pub fn complete_bounded_input(&mut self) {
        self.input_deadline_guard = None;
    }
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> Result<(), SecureWebSocketError> {
        self.socket
            .get_ref()
            .sock
            .set_read_timeout(timeout)
            .map_err(|error| SecureWebSocketError::Transport(error.kind()))
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

    pub fn close(&mut self) -> Result<(), SecureWebSocketError> {
        close(&mut self.socket)
    }
}

fn receive_binary<S: Read + Write>(
    socket: &mut WebSocket<S>,
    maximum_message_bytes: usize,
    output: &mut [u8],
) -> Result<usize, SecureWebSocketError> {
    loop {
        match socket.read().map_err(map_socket_error)? {
            Message::Binary(bytes) => {
                if bytes.len() > maximum_message_bytes {
                    return Err(SecureWebSocketError::OversizedMessage);
                }
                if output.len() < bytes.len() {
                    return Err(SecureWebSocketError::OutputTooSmall);
                }
                output[..bytes.len()].copy_from_slice(&bytes);
                return Ok(bytes.len());
            }
            Message::Text(_) => return Err(SecureWebSocketError::TextMessageRejected),
            Message::Ping(_) | Message::Pong(_) => {
                socket.flush().map_err(map_socket_error)?;
            }
            Message::Close(_) => return Err(SecureWebSocketError::Disconnected),
            Message::Frame(_) => return Err(SecureWebSocketError::Protocol),
        }
    }
}

fn close<S: Read + Write>(socket: &mut WebSocket<S>) -> Result<(), SecureWebSocketError> {
    socket
        .close(Some(CloseFrame {
            code: CloseCode::Normal,
            reason: "conduit-terminal".into(),
        }))
        .map_err(map_socket_error)
}

fn read_certificates(path: &Path) -> Result<Vec<CertificateDer<'static>>, SecureWebSocketError> {
    let file = File::open(path).map_err(|error| SecureWebSocketError::Identity(error.kind()))?;
    let certificates = rustls_pemfile::certs(&mut BufReader::new(file))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| SecureWebSocketError::Identity(error.kind()))?;
    if certificates.is_empty() {
        return Err(SecureWebSocketError::InvalidConfiguration);
    }
    Ok(certificates)
}

fn read_private_key(path: &Path) -> Result<PrivateKeyDer<'static>, SecureWebSocketError> {
    let file = File::open(path).map_err(|error| SecureWebSocketError::Identity(error.kind()))?;
    rustls_pemfile::private_key(&mut BufReader::new(file))
        .map_err(|error| SecureWebSocketError::Identity(error.kind()))?
        .ok_or(SecureWebSocketError::InvalidConfiguration)
}

fn bounded_config(maximum_message_bytes: usize) -> Result<WebSocketConfig, SecureWebSocketError> {
    let maximum_write_buffer = maximum_message_bytes
        .checked_add(32)
        .ok_or(SecureWebSocketError::InvalidConfiguration)?;
    Ok(WebSocketConfig::default()
        .read_buffer_size(maximum_message_bytes)
        .write_buffer_size(0)
        .max_write_buffer_size(maximum_write_buffer)
        .max_message_size(Some(maximum_message_bytes))
        .max_frame_size(Some(maximum_message_bytes)))
}

fn map_socket_error(error: TungsteniteError) -> SecureWebSocketError {
    match error {
        TungsteniteError::ConnectionClosed | TungsteniteError::AlreadyClosed => {
            SecureWebSocketError::Disconnected
        }
        TungsteniteError::Io(error) => SecureWebSocketError::Transport(error.kind()),
        TungsteniteError::Capacity(_) => SecureWebSocketError::OversizedMessage,
        TungsteniteError::Tls(_) => SecureWebSocketError::Tls,
        _ => SecureWebSocketError::Protocol,
    }
}

#[cfg(test)]
mod tests;
