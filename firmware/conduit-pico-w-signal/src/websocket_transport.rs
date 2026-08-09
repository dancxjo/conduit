//! Fixed-buffer RFC 6455 binary-message carrier over one Embassy TCP socket.

use embassy_net::tcp::TcpSocket;
use embedded_websocket::{
    read_http_header, Error as FrameError, WebSocketReceiveMessageType,
    WebSocketSendMessageType, WebSocketServer,
};

const HTTP_BYTES: usize = 1_024;
const FRAME_OVERHEAD_BYTES: usize = 14;
const FRAME_BYTES: usize = conduit_net::R1_MAXIMUM_FRAME_BYTES as usize;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum WebSocketTransportError {
    Disconnected,
    Tcp,
    Http,
    WrongPath,
    Frame,
    Text,
    Oversized,
}

pub struct WebSocketTransport {
    websocket: WebSocketServer,
    pending: [u8; FRAME_BYTES + FRAME_OVERHEAD_BYTES],
    pending_len: usize,
}

impl WebSocketTransport {
    pub async fn accept(socket: &mut TcpSocket<'_>) -> Result<Self, WebSocketTransportError> {
        let mut request = [0_u8; HTTP_BYTES];
        let mut request_len = 0;
        loop {
            if request_len == request.len() {
                return Err(WebSocketTransportError::Oversized);
            }
            let read = socket
                .read(&mut request[request_len..])
                .await
                .map_err(|_| WebSocketTransportError::Tcp)?;
            if read == 0 {
                return Err(WebSocketTransportError::Disconnected);
            }
            request_len += read;
            let mut headers = [httparse::EMPTY_HEADER; 16];
            let mut parsed = httparse::Request::new(&mut headers);
            let header_len = match parsed
                .parse(&request[..request_len])
                .map_err(|_| WebSocketTransportError::Http)?
            {
                httparse::Status::Partial => continue,
                httparse::Status::Complete(length) => length,
            };
            if parsed.method != Some("GET") || parsed.path != Some("/conduit") {
                return Err(WebSocketTransportError::WrongPath);
            }
            let context = read_http_header(
                parsed
                    .headers
                    .iter()
                    .map(|header| (header.name, header.value)),
            )
            .map_err(|_| WebSocketTransportError::Http)?
            .ok_or(WebSocketTransportError::Http)?;
            let mut websocket = WebSocketServer::new_server();
            let mut response = [0_u8; HTTP_BYTES];
            let response_len = websocket
                .server_accept(&context.sec_websocket_key, None, &mut response)
                .map_err(|_| WebSocketTransportError::Http)?;
            write_all(socket, &response[..response_len]).await?;
            socket.flush().await.map_err(|_| WebSocketTransportError::Tcp)?;
            let mut transport = Self {
                websocket,
                pending: [0; FRAME_BYTES + FRAME_OVERHEAD_BYTES],
                pending_len: request_len - header_len,
            };
            transport.pending[..transport.pending_len]
                .copy_from_slice(&request[header_len..request_len]);
            return Ok(transport);
        }
    }

    pub async fn receive_binary(
        &mut self,
        socket: &mut TcpSocket<'_>,
        output: &mut [u8],
    ) -> Result<usize, WebSocketTransportError> {
        let mut output_len = 0;
        loop {
            if self.pending_len == 0 {
                self.read_more(socket).await?;
            }
            let result = match self
                .websocket
                .read(&self.pending[..self.pending_len], &mut output[output_len..])
            {
                Ok(result) => result,
                Err(FrameError::ReadFrameIncomplete) => {
                    self.read_more(socket).await?;
                    continue;
                }
                Err(_) => return Err(WebSocketTransportError::Frame),
            };
            if result.len_from == 0 && result.len_to == 0 {
                return Err(WebSocketTransportError::Frame);
            }
            self.pending.copy_within(result.len_from..self.pending_len, 0);
            self.pending_len -= result.len_from;
            output_len = output_len
                .checked_add(result.len_to)
                .ok_or(WebSocketTransportError::Oversized)?;
            if output_len > output.len() {
                return Err(WebSocketTransportError::Oversized);
            }
            match result.message_type {
                WebSocketReceiveMessageType::Binary if result.end_of_message => {
                    return Ok(output_len)
                }
                WebSocketReceiveMessageType::Binary => {}
                WebSocketReceiveMessageType::Text => return Err(WebSocketTransportError::Text),
                _ => return Err(WebSocketTransportError::Frame),
            }
        }
    }

    pub async fn send_binary(
        &mut self,
        socket: &mut TcpSocket<'_>,
        payload: &[u8],
    ) -> Result<(), WebSocketTransportError> {
        if payload.len() > FRAME_BYTES {
            return Err(WebSocketTransportError::Oversized);
        }
        let mut frame = [0_u8; FRAME_BYTES + FRAME_OVERHEAD_BYTES];
        let length = self
            .websocket
            .write(WebSocketSendMessageType::Binary, true, payload, &mut frame)
            .map_err(|_| WebSocketTransportError::Frame)?;
        write_all(socket, &frame[..length]).await?;
        socket.flush().await.map_err(|_| WebSocketTransportError::Tcp)
    }

    pub async fn wait_for_disconnect(&mut self, socket: &mut TcpSocket<'_>) {
        let mut byte = [0_u8; 1];
        while matches!(socket.read(&mut byte).await, Ok(1)) {}
    }

    async fn read_more(
        &mut self,
        socket: &mut TcpSocket<'_>,
    ) -> Result<(), WebSocketTransportError> {
        if self.pending_len == self.pending.len() {
            return Err(WebSocketTransportError::Oversized);
        }
        let read = socket
            .read(&mut self.pending[self.pending_len..])
            .await
            .map_err(|_| WebSocketTransportError::Tcp)?;
        if read == 0 {
            return Err(WebSocketTransportError::Disconnected);
        }
        self.pending_len += read;
        Ok(())
    }
}

async fn write_all(
    socket: &mut TcpSocket<'_>,
    mut bytes: &[u8],
) -> Result<(), WebSocketTransportError> {
    while !bytes.is_empty() {
        let written = socket
            .write(bytes)
            .await
            .map_err(|_| WebSocketTransportError::Tcp)?;
        if written == 0 {
            return Err(WebSocketTransportError::Disconnected);
        }
        bytes = &bytes[written..];
    }
    Ok(())
}
