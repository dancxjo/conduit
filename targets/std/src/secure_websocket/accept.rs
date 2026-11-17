//! Listener wait and accepted initial-I/O deadlines; neither is renewed by bytes.

use super::{
    bounded_config, SecureWebSocketError, SecureWebSocketLine, SecureWebSocketListener,
    SocketDeadlineGuard,
};
use rustls::{ServerConnection, StreamOwned};
use std::io::ErrorKind;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tungstenite::handshake::HandshakeError;

impl SecureWebSocketListener {
    /// One total deadline covers listening, TLS/WebSocket handshake and initial
    /// application input. Listening consumes this budget, preserving the original API.
    pub fn accept_with_timeout(
        &self,
        timeout: Duration,
    ) -> Result<SecureWebSocketLine, SecureWebSocketError> {
        let deadline = finite_deadline(Instant::now(), timeout)?;
        self.accept_until(deadline, None)
    }

    /// Bound listening separately from the accepted peer's initial I/O.
    /// The latter begins once at TCP acceptance and covers TLS/WebSocket plus
    /// initial application input; received bytes never renew it. Total wait is
    /// bounded by listen_timeout + initial_io_timeout, apart from OS scheduling.
    pub fn accept_with_timeouts(
        &self,
        listen_timeout: Duration,
        initial_io_timeout: Duration,
    ) -> Result<SecureWebSocketLine, SecureWebSocketError> {
        let listen_deadline = finite_deadline(Instant::now(), listen_timeout)?;
        // Validate the full declared lifetime before waiting for a peer.
        finite_deadline(listen_deadline, initial_io_timeout)?;
        self.accept_until(listen_deadline, Some(initial_io_timeout))
    }

    // None retains the original total deadline. Some selects one fresh,
    // independently declared initial-I/O budget at TCP acceptance only.
    fn accept_until(
        &self,
        listen_deadline: Instant,
        initial_io_timeout: Option<Duration>,
    ) -> Result<SecureWebSocketLine, SecureWebSocketError> {
        self.listener
            .set_nonblocking(true)
            .map_err(|error| SecureWebSocketError::Accept(error.kind()))?;
        let (stream, accepted_at) = loop {
            match self.listener.accept() {
                Ok((stream, _peer)) => break (stream, Instant::now()),
                Err(error)
                    if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) =>
                {
                    if Instant::now() >= listen_deadline {
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
        if accepted_at >= listen_deadline {
            return Err(SecureWebSocketError::AcceptDeadline);
        }
        let deadline = match initial_io_timeout {
            Some(timeout) => finite_deadline(accepted_at, timeout)?,
            None => listen_deadline,
        };
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

fn finite_deadline(started: Instant, timeout: Duration) -> Result<Instant, SecureWebSocketError> {
    if timeout.is_zero() {
        return Err(SecureWebSocketError::InvalidConfiguration);
    }
    started
        .checked_add(timeout)
        .ok_or(SecureWebSocketError::InvalidConfiguration)
}
