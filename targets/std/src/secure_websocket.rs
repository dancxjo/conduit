//! Explicit TLS-authenticated WebSocket Line for non-loopback rendezvous.
//!
//! This is deliberately separate from [`crate::websocket`]: the local
//! `C1-WS` carrier remains plaintext and loopback-only, while this listener
//! requires operator-supplied TLS identity and an explicit network bind.

use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::{ServerConfig, ServerConnection, StreamOwned};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, ErrorKind};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
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
    ) -> Result<Self, SecureWebSocketError> {
        let maximum_message_bytes = usize::try_from(maximum_message_bytes)
            .map_err(|_| SecureWebSocketError::InvalidConfiguration)?;
        if maximum_message_bytes == 0
            || address.ip().is_loopback()
            || address.ip().is_unspecified()
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
        Ok(SecureWebSocketLine {
            socket,
            maximum_message_bytes: self.maximum_message_bytes,
        })
    }
}

pub struct SecureWebSocketLine {
    socket: WebSocket<StreamOwned<ServerConnection, TcpStream>>,
    maximum_message_bytes: usize,
}

impl SecureWebSocketLine {
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
        loop {
            match self.socket.read().map_err(map_socket_error)? {
                Message::Binary(bytes) => {
                    if bytes.len() > self.maximum_message_bytes {
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
                    self.socket.flush().map_err(map_socket_error)?;
                }
                Message::Close(_) => return Err(SecureWebSocketError::Disconnected),
                Message::Frame(_) => return Err(SecureWebSocketError::Protocol),
            }
        }
    }

    pub fn close(&mut self) -> Result<(), SecureWebSocketError> {
        self.socket
            .close(Some(CloseFrame {
                code: CloseCode::Normal,
                reason: "conduit-terminal".into(),
            }))
            .map_err(map_socket_error)
    }
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
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddrV4};

    #[test]
    fn secure_remote_listener_cannot_rebrand_loopback() {
        let result = SecureWebSocketListener::bind(
            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 443)),
            Path::new("missing-cert.pem"),
            Path::new("missing-key.pem"),
            1024,
        );
        assert!(matches!(
            result,
            Err(SecureWebSocketError::InvalidConfiguration)
        ));
    }
}
