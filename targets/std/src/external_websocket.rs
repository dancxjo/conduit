//! Native adapter for the authored `net/websocket/listen` host operation.
//! This is deliberately separate from the Conduit-session WebSocket line.

use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream};
use tungstenite::protocol::{Message, WebSocket, WebSocketConfig};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalPeerId(u16);

impl ExternalPeerId {
    pub fn from_index(index: u16) -> Self {
        Self(index)
    }

    pub fn index(self) -> usize {
        usize::from(self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalWebSocketError {
    InvalidLimit,
    Bind(io::ErrorKind),
    Accept(io::ErrorKind),
    Handshake,
    PeerLimit,
    UnknownPeer,
    OversizedMessage,
    TextMessageRejected,
    Disconnected,
    Protocol,
    Transport(io::ErrorKind),
}

pub struct ExternalWebSocketListener {
    listener: TcpListener,
    maximum_message_bytes: usize,
    peers: Vec<Option<WebSocket<TcpStream>>>,
}

impl ExternalWebSocketListener {
    pub fn bind_loopback(
        maximum_peers: u16,
        maximum_message_bytes: u32,
    ) -> Result<Self, ExternalWebSocketError> {
        Self::bind(
            SocketAddr::from(([127, 0, 0, 1], 0)),
            maximum_peers,
            maximum_message_bytes,
        )
    }

    pub fn bind(
        address: SocketAddr,
        maximum_peers: u16,
        maximum_message_bytes: u32,
    ) -> Result<Self, ExternalWebSocketError> {
        let maximum_message_bytes = usize::try_from(maximum_message_bytes)
            .map_err(|_| ExternalWebSocketError::InvalidLimit)?;
        if !address.ip().is_loopback() || maximum_peers == 0 || maximum_message_bytes == 0 {
            return Err(ExternalWebSocketError::InvalidLimit);
        }
        let listener = TcpListener::bind(address)
            .map_err(|error| ExternalWebSocketError::Bind(error.kind()))?;
        let mut peers = Vec::with_capacity(usize::from(maximum_peers));
        peers.resize_with(usize::from(maximum_peers), || None);
        Ok(Self {
            listener,
            maximum_message_bytes,
            peers,
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr, ExternalWebSocketError> {
        self.listener
            .local_addr()
            .map_err(|error| ExternalWebSocketError::Bind(error.kind()))
    }

    pub fn url(&self) -> Result<String, ExternalWebSocketError> {
        Ok(format!("ws://{}", self.local_addr()?))
    }

    pub fn accept_peer(&mut self) -> Result<ExternalPeerId, ExternalWebSocketError> {
        let index = self
            .peers
            .iter()
            .position(Option::is_none)
            .ok_or(ExternalWebSocketError::PeerLimit)?;
        let (stream, _) = self
            .listener
            .accept()
            .map_err(|error| ExternalWebSocketError::Accept(error.kind()))?;
        let config = WebSocketConfig::default()
            .max_message_size(Some(self.maximum_message_bytes))
            .max_frame_size(Some(self.maximum_message_bytes));
        let socket = tungstenite::accept_with_config(stream, Some(config))
            .map_err(|_| ExternalWebSocketError::Handshake)?;
        self.peers[index] = Some(socket);
        Ok(ExternalPeerId(
            u16::try_from(index).map_err(|_| ExternalWebSocketError::PeerLimit)?,
        ))
    }

    pub fn receive_binary(
        &mut self,
        peer: ExternalPeerId,
        output: &mut [u8],
    ) -> Result<usize, ExternalWebSocketError> {
        let socket = self.peer_mut(peer)?;
        loop {
            match socket.read().map_err(map_socket_error)? {
                Message::Binary(bytes) => {
                    if bytes.len() > output.len() || bytes.len() > self.maximum_message_bytes {
                        return Err(ExternalWebSocketError::OversizedMessage);
                    }
                    output[..bytes.len()].copy_from_slice(&bytes);
                    return Ok(bytes.len());
                }
                Message::Text(_) => return Err(ExternalWebSocketError::TextMessageRejected),
                Message::Ping(bytes) => socket
                    .send(Message::Pong(bytes))
                    .map_err(map_socket_error)?,
                Message::Pong(_) => {}
                Message::Close(_) => {
                    self.peers[peer.index()] = None;
                    return Err(ExternalWebSocketError::Disconnected);
                }
                Message::Frame(_) => return Err(ExternalWebSocketError::Protocol),
            }
        }
    }

    pub fn send_binary(
        &mut self,
        peer: ExternalPeerId,
        bytes: &[u8],
    ) -> Result<(), ExternalWebSocketError> {
        if bytes.len() > self.maximum_message_bytes {
            return Err(ExternalWebSocketError::OversizedMessage);
        }
        self.peer_mut(peer)?
            .send(Message::Binary(bytes.to_vec().into()))
            .map_err(map_socket_error)
    }

    pub fn disconnect(&mut self, peer: ExternalPeerId) -> Result<(), ExternalWebSocketError> {
        let Some(mut socket) = self
            .peers
            .get_mut(peer.index())
            .ok_or(ExternalWebSocketError::UnknownPeer)?
            .take()
        else {
            return Err(ExternalWebSocketError::UnknownPeer);
        };
        socket.close(None).map_err(map_socket_error)
    }

    pub fn next_connected_after(&self, peer: ExternalPeerId) -> Option<ExternalPeerId> {
        (1..=self.peers.len())
            .map(|offset| (peer.index() + offset) % self.peers.len())
            .find(|index| self.peers[*index].is_some())
            .and_then(|index| u16::try_from(index).ok())
            .map(ExternalPeerId::from_index)
    }

    fn peer_mut(
        &mut self,
        peer: ExternalPeerId,
    ) -> Result<&mut WebSocket<TcpStream>, ExternalWebSocketError> {
        self.peers
            .get_mut(peer.index())
            .and_then(Option::as_mut)
            .ok_or(ExternalWebSocketError::UnknownPeer)
    }
}

fn map_socket_error(error: tungstenite::Error) -> ExternalWebSocketError {
    match error {
        tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed => {
            ExternalWebSocketError::Disconnected
        }
        tungstenite::Error::Io(error) => ExternalWebSocketError::Transport(error.kind()),
        tungstenite::Error::Capacity(_) => ExternalWebSocketError::OversizedMessage,
        _ => ExternalWebSocketError::Protocol,
    }
}

#[cfg(test)]
mod tests {
    use super::{ExternalWebSocketError, ExternalWebSocketListener};
    use conduit_net::{MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES, MAXIMUM_EXTERNAL_WEBSOCKET_PEERS};
    use std::net::TcpStream;
    use std::thread;
    use tungstenite::protocol::Message;

    #[test]
    fn two_actual_clients_exchange_in_order_and_one_survives_the_other_closing() {
        let mut server = ExternalWebSocketListener::bind_loopback(
            MAXIMUM_EXTERNAL_WEBSOCKET_PEERS,
            MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES,
        )
        .unwrap();
        let url = server.url().unwrap();
        let connect = |url: String| {
            thread::spawn(move || {
                let stream = TcpStream::connect(url.trim_start_matches("ws://")).unwrap();
                tungstenite::client(url, stream).unwrap().0
            })
        };
        let client_a = connect(url.clone());
        let peer_a = server.accept_peer().unwrap();
        let client_b = connect(url);
        let peer_b = server.accept_peer().unwrap();
        let mut client_a = client_a.join().unwrap();
        let mut client_b = client_b.join().unwrap();
        let mut bytes = [0_u8; MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES as usize];

        client_a
            .send(Message::Binary(b"A".to_vec().into()))
            .unwrap();
        let count = server.receive_binary(peer_a, &mut bytes).unwrap();
        server.send_binary(peer_b, &bytes[..count]).unwrap();
        assert_eq!(client_b.read().unwrap().into_data().as_ref(), b"A");

        client_b
            .send(Message::Binary(b"B".to_vec().into()))
            .unwrap();
        let count = server.receive_binary(peer_b, &mut bytes).unwrap();
        server.send_binary(peer_a, &bytes[..count]).unwrap();
        assert_eq!(client_a.read().unwrap().into_data().as_ref(), b"B");

        client_a.close(None).unwrap();
        assert_eq!(
            server.receive_binary(peer_a, &mut bytes),
            Err(ExternalWebSocketError::Disconnected)
        );
        server.send_binary(peer_b, b"still-live").unwrap();
        assert_eq!(client_b.read().unwrap().into_data().as_ref(), b"still-live");
    }

    #[test]
    fn peer_message_and_shape_limits_fail_explicitly() {
        let mut server = ExternalWebSocketListener::bind_loopback(1, 4).unwrap();
        let url = server.url().unwrap();
        let client = thread::spawn(move || {
            let stream = TcpStream::connect(url.trim_start_matches("ws://")).unwrap();
            tungstenite::client(url, stream).unwrap().0
        });
        let peer = server.accept_peer().unwrap();
        let mut client = client.join().unwrap();
        assert_eq!(server.accept_peer(), Err(ExternalWebSocketError::PeerLimit));
        assert_eq!(
            server.send_binary(peer, b"12345"),
            Err(ExternalWebSocketError::OversizedMessage)
        );
        client.send(Message::Text("bad".into())).unwrap();
        let mut output = [0_u8; 4];
        assert_eq!(
            server.receive_binary(peer, &mut output),
            Err(ExternalWebSocketError::TextMessageRejected)
        );
    }
}
