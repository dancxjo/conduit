use std::io::ErrorKind;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream};
use std::time::{Duration, Instant};

use tungstenite::client::client_with_config;
use tungstenite::error::Error as TungsteniteError;
use tungstenite::handshake::HandshakeError;
use tungstenite::protocol::frame::coding::CloseCode;
use tungstenite::protocol::{CloseFrame, Message, WebSocket, WebSocketConfig};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum NativeWebSocketError {
    InvalidLimit,
    Bind(ErrorKind),
    Accept(ErrorKind),
    AcceptDeadline,
    Handshake,
    Transport(ErrorKind),
    Protocol,
    TextMessageRejected,
    OversizedMessage,
    OutputTooSmall,
    Disconnected,
}

pub struct NativeWebSocketListener {
    listener: TcpListener,
    maximum_message_bytes: usize,
    expected_peer: IpAddr,
}

impl NativeWebSocketListener {
    pub fn bind_loopback(maximum_message_bytes: u32) -> Result<Self, NativeWebSocketError> {
        let maximum_message_bytes = usize::try_from(maximum_message_bytes)
            .map_err(|_| NativeWebSocketError::InvalidLimit)?;
        if maximum_message_bytes == 0 {
            return Err(NativeWebSocketError::InvalidLimit);
        }
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
            .map_err(|error| NativeWebSocketError::Bind(error.kind()))?;
        Ok(Self {
            listener,
            maximum_message_bytes,
            expected_peer: IpAddr::V4(Ipv4Addr::LOCALHOST),
        })
    }

    /// Bind one exact planned non-loopback endpoint and admit only the planned
    /// peer address. Host/Boot/session identity is still established above
    /// this transport check; an IP match grants no membership or authority.
    pub fn bind_planned(
        address: SocketAddr,
        expected_peer: IpAddr,
        maximum_message_bytes: u32,
    ) -> Result<Self, NativeWebSocketError> {
        let maximum_message_bytes = usize::try_from(maximum_message_bytes)
            .map_err(|_| NativeWebSocketError::InvalidLimit)?;
        if maximum_message_bytes == 0
            || address.ip().is_unspecified()
            || address.port() == 0
            || expected_peer.is_unspecified()
            || expected_peer.is_multicast()
        {
            return Err(NativeWebSocketError::InvalidLimit);
        }
        let listener =
            TcpListener::bind(address).map_err(|error| NativeWebSocketError::Bind(error.kind()))?;
        Ok(Self {
            listener,
            maximum_message_bytes,
            expected_peer,
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr, NativeWebSocketError> {
        self.listener
            .local_addr()
            .map_err(|error| NativeWebSocketError::Bind(error.kind()))
    }

    pub fn url(&self) -> Result<String, NativeWebSocketError> {
        let address = self.local_addr()?;
        Ok(format!("ws://{address}/conduit"))
    }

    pub fn accept(&self) -> Result<NativeWebSocketLine, NativeWebSocketError> {
        self.accept_until(None)
    }

    /// Admit one peer before an exact finite deadline. The listener is made
    /// nonblocking only for this bounded admission window and is restored
    /// before the accepted stream enters the WebSocket handshake.
    pub fn accept_with_timeout(
        &self,
        timeout: Duration,
    ) -> Result<NativeWebSocketLine, NativeWebSocketError> {
        if timeout.is_zero() {
            return Err(NativeWebSocketError::InvalidLimit);
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(NativeWebSocketError::InvalidLimit)?;
        self.accept_until(Some(deadline))
    }

    fn accept_until(
        &self,
        deadline: Option<Instant>,
    ) -> Result<NativeWebSocketLine, NativeWebSocketError> {
        self.listener
            .set_nonblocking(deadline.is_some())
            .map_err(|error| NativeWebSocketError::Accept(error.kind()))?;
        let (stream, peer) = loop {
            match self.listener.accept() {
                Ok(accepted) => break accepted,
                Err(error)
                    if deadline.is_some()
                        && matches!(
                            error.kind(),
                            ErrorKind::WouldBlock | ErrorKind::Interrupted
                        ) =>
                {
                    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                        self.listener
                            .set_nonblocking(false)
                            .map_err(|error| NativeWebSocketError::Accept(error.kind()))?;
                        return Err(NativeWebSocketError::AcceptDeadline);
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => {
                    self.listener
                        .set_nonblocking(false)
                        .map_err(|restore| NativeWebSocketError::Accept(restore.kind()))?;
                    return Err(NativeWebSocketError::Accept(error.kind()));
                }
            }
        };
        self.listener
            .set_nonblocking(false)
            .map_err(|error| NativeWebSocketError::Accept(error.kind()))?;
        if peer.ip() != self.expected_peer {
            return Err(NativeWebSocketError::Protocol);
        }
        stream
            .set_nodelay(true)
            .map_err(|error| NativeWebSocketError::Transport(error.kind()))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(|error| NativeWebSocketError::Transport(error.kind()))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(|error| NativeWebSocketError::Transport(error.kind()))?;
        let config = bounded_config(self.maximum_message_bytes)?;
        let socket =
            tungstenite::accept_with_config(stream, Some(config)).map_err(|error| match error {
                HandshakeError::Interrupted(_) | HandshakeError::Failure(_) => {
                    NativeWebSocketError::Handshake
                }
            })?;
        Ok(NativeWebSocketLine {
            socket,
            maximum_message_bytes: self.maximum_message_bytes,
        })
    }
}

pub struct NativeWebSocketLine {
    socket: WebSocket<TcpStream>,
    maximum_message_bytes: usize,
}

impl NativeWebSocketLine {
    pub fn connect(
        address: SocketAddr,
        url: &str,
        maximum_message_bytes: u32,
    ) -> Result<Self, NativeWebSocketError> {
        let maximum_message_bytes = usize::try_from(maximum_message_bytes)
            .map_err(|_| NativeWebSocketError::InvalidLimit)?;
        if maximum_message_bytes == 0 || address.ip().is_unspecified() {
            return Err(NativeWebSocketError::InvalidLimit);
        }
        let stream = TcpStream::connect_timeout(&address, Duration::from_secs(5))
            .map_err(|error| NativeWebSocketError::Transport(error.kind()))?;
        stream
            .set_nodelay(true)
            .map_err(|error| NativeWebSocketError::Transport(error.kind()))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(|error| NativeWebSocketError::Transport(error.kind()))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(|error| NativeWebSocketError::Transport(error.kind()))?;
        let config = bounded_config(maximum_message_bytes)?;
        let (socket, _) =
            client_with_config(url, stream, Some(config)).map_err(|error| match error {
                HandshakeError::Interrupted(_) | HandshakeError::Failure(_) => {
                    NativeWebSocketError::Handshake
                }
            })?;
        Ok(Self {
            socket,
            maximum_message_bytes,
        })
    }

    pub fn maximum_message_bytes(&self) -> usize {
        self.maximum_message_bytes
    }

    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> Result<(), NativeWebSocketError> {
        self.socket
            .get_ref()
            .set_read_timeout(timeout)
            .map_err(|error| NativeWebSocketError::Transport(error.kind()))
    }

    pub fn send_binary(&mut self, bytes: &[u8]) -> Result<(), NativeWebSocketError> {
        if bytes.len() > self.maximum_message_bytes {
            return Err(NativeWebSocketError::OversizedMessage);
        }
        self.socket
            .send(Message::Binary(bytes.to_vec().into()))
            .map_err(map_socket_error)
    }

    pub fn receive_binary(&mut self, output: &mut [u8]) -> Result<usize, NativeWebSocketError> {
        loop {
            let message = self.socket.read().map_err(map_socket_error)?;
            match message {
                Message::Binary(bytes) => {
                    if bytes.len() > self.maximum_message_bytes {
                        return Err(NativeWebSocketError::OversizedMessage);
                    }
                    if output.len() < bytes.len() {
                        return Err(NativeWebSocketError::OutputTooSmall);
                    }
                    output[..bytes.len()].copy_from_slice(&bytes);
                    return Ok(bytes.len());
                }
                Message::Text(_) => return Err(NativeWebSocketError::TextMessageRejected),
                Message::Ping(_) | Message::Pong(_) => {
                    self.socket.flush().map_err(map_socket_error)?;
                }
                Message::Close(_) => return Err(NativeWebSocketError::Disconnected),
                Message::Frame(_) => return Err(NativeWebSocketError::Protocol),
            }
        }
    }

    pub fn close(&mut self) -> Result<(), NativeWebSocketError> {
        self.socket
            .close(Some(CloseFrame {
                code: CloseCode::Normal,
                reason: "conduit-terminal".into(),
            }))
            .map_err(map_socket_error)
    }
}

fn bounded_config(maximum_message_bytes: usize) -> Result<WebSocketConfig, NativeWebSocketError> {
    let maximum_write_buffer = maximum_message_bytes
        .checked_add(32)
        .ok_or(NativeWebSocketError::InvalidLimit)?;
    Ok(WebSocketConfig::default()
        .read_buffer_size(maximum_message_bytes)
        .write_buffer_size(0)
        .max_write_buffer_size(maximum_write_buffer)
        .max_message_size(Some(maximum_message_bytes))
        .max_frame_size(Some(maximum_message_bytes)))
}

fn map_socket_error(error: TungsteniteError) -> NativeWebSocketError {
    match error {
        TungsteniteError::ConnectionClosed | TungsteniteError::AlreadyClosed => {
            NativeWebSocketError::Disconnected
        }
        TungsteniteError::Io(error) => NativeWebSocketError::Transport(error.kind()),
        TungsteniteError::Capacity(_) => NativeWebSocketError::OversizedMessage,
        _ => NativeWebSocketError::Protocol,
    }
}

#[cfg(test)]
mod tests {
    use super::{NativeWebSocketError, NativeWebSocketLine, NativeWebSocketListener};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream};
    use std::thread;
    use tungstenite::protocol::Message;

    #[test]
    fn unavailable_and_unspecified_client_endpoints_fail_distinctly() {
        let unspecified = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 8765));
        assert_eq!(
            NativeWebSocketLine::connect(unspecified, "ws://0.0.0.0:8765/conduit", 16).map(|_| ()),
            Err(NativeWebSocketError::InvalidLimit)
        );

        let unavailable =
            TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).expect("reserve address");
        let address = unavailable.local_addr().expect("reserved address");
        drop(unavailable);
        assert!(matches!(
            NativeWebSocketLine::connect(address, &format!("ws://{address}/conduit"), 16),
            Err(NativeWebSocketError::Transport(_))
        ));
    }

    #[test]
    fn planned_listener_rejects_unspecified_bind_and_peer_facts() {
        let unspecified = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 8765));
        assert!(matches!(
            NativeWebSocketListener::bind_planned(unspecified, IpAddr::V4(Ipv4Addr::LOCALHOST), 16),
            Err(NativeWebSocketError::InvalidLimit)
        ));
        let exact = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8765));
        assert!(matches!(
            NativeWebSocketListener::bind_planned(exact, IpAddr::V4(Ipv4Addr::UNSPECIFIED), 16),
            Err(NativeWebSocketError::InvalidLimit)
        ));
    }

    #[test]
    fn planned_listener_admission_has_a_finite_deadline() {
        let listener = NativeWebSocketListener::bind_loopback(16).expect("loopback binds");
        let started = std::time::Instant::now();
        assert_eq!(
            listener
                .accept_with_timeout(std::time::Duration::from_millis(25))
                .map(|_| ()),
            Err(NativeWebSocketError::AcceptDeadline)
        );
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
    }

    #[test]
    fn actual_loopback_rfc6455_is_binary_only_and_message_bounded() {
        let listener = NativeWebSocketListener::bind_loopback(16).expect("loopback binds");
        let address = listener.local_addr().expect("address");
        let url = listener.url().expect("url");
        assert!(address.ip().is_loopback());
        assert_ne!(address.port(), 0);
        let server = thread::spawn(move || {
            let mut line = listener.accept().expect("RFC 6455 accepts");
            assert_eq!(line.maximum_message_bytes(), 16);
            assert_eq!(
                line.send_binary(&[0; 17]),
                Err(NativeWebSocketError::OversizedMessage)
            );
            let mut input = [0_u8; 16];
            let length = line.receive_binary(&mut input).expect("binary arrives");
            assert_eq!(&input[..length], b"actual-binary");
            line.send_binary(b"bounded-reply").expect("reply sends");
            line.close().expect("normal close sends");
        });
        let stream = TcpStream::connect(address).expect("client connects");
        let (mut client, _) = tungstenite::client(url, stream).expect("client upgrades");
        client
            .send(Message::binary(&b"actual-binary"[..]))
            .expect("binary sends");
        assert_eq!(
            client.read().expect("binary reply"),
            Message::binary(&b"bounded-reply"[..])
        );
        assert!(client.read().expect("normal close").is_close());
        server.join().expect("server exits");

        let listener = NativeWebSocketListener::bind_loopback(16).expect("loopback binds");
        let address = listener.local_addr().expect("address");
        let url = listener.url().expect("url");
        let server = thread::spawn(move || {
            let mut line = listener.accept().expect("RFC 6455 accepts");
            assert_eq!(
                line.receive_binary(&mut [0_u8; 16]),
                Err(NativeWebSocketError::TextMessageRejected)
            );
        });
        let stream = TcpStream::connect(address).expect("client connects");
        let (mut client, _) = tungstenite::client(url, stream).expect("client upgrades");
        client
            .send(Message::text("not-binary"))
            .expect("text reaches rejection boundary");
        server.join().expect("server rejects text");
    }

    #[test]
    fn disconnect_before_readiness_and_with_an_in_flight_frame_are_distinct() {
        let listener = NativeWebSocketListener::bind_loopback(16).expect("loopback binds");
        let address = listener.local_addr().expect("address");
        let url = listener.url().expect("url");
        let server = thread::spawn(move || {
            let mut line = listener.accept().expect("RFC 6455 accepts");
            assert_eq!(
                line.receive_binary(&mut [0_u8; 16]),
                Err(NativeWebSocketError::Disconnected)
            );
        });
        let stream = TcpStream::connect(address).expect("client connects");
        let (mut client, _) = tungstenite::client(url, stream).expect("client upgrades");
        client.close(None).expect("disconnect before readiness");
        server.join().expect("server classifies early disconnect");

        let listener = NativeWebSocketListener::bind_loopback(16).expect("loopback binds");
        let address = listener.local_addr().expect("address");
        let url = listener.url().expect("url");
        let server = thread::spawn(move || {
            let mut line = listener.accept().expect("RFC 6455 accepts");
            let mut input = [0_u8; 16];
            let length = line.receive_binary(&mut input).expect("frame arrives");
            assert_eq!(&input[..length], b"offered");
            assert_eq!(
                line.receive_binary(&mut input),
                Err(NativeWebSocketError::Disconnected)
            );
        });
        let stream = TcpStream::connect(address).expect("client connects");
        let (mut client, _) = tungstenite::client(url, stream).expect("client upgrades");
        client
            .send(Message::binary(&b"offered"[..]))
            .expect("in-flight frame sends");
        client.close(None).expect("disconnect in flight");
        server
            .join()
            .expect("server classifies in-flight disconnect");
    }
}
