use super::*;

fn exchange<S, R>(
    stream: &mut S,
    rng: R,
    host: &str,
    path: &str,
    request: &[u8],
    response: &mut [u8],
    expected_response_bytes: usize,
) -> Result<(), WebSocketError>
where
    S: Read + Write,
    R: RngCore,
{
    if request.is_empty() || request.len() + 14 > FRAME_BYTES {
        return Err(WebSocketError::RequestTooLarge);
    }
    if expected_response_bytes == 0 || expected_response_bytes > response.len() {
        return Err(WebSocketError::ResponseTooLarge);
    }
    let mut websocket = BoundedWebSocket::connect(stream, rng, host, path)?;
    websocket.send_binary(request)?;
    let received = websocket.receive_binary(response)?;
    if received != expected_response_bytes {
        return Err(WebSocketError::UnexpectedFrame);
    }
    websocket.close()
}

use core::convert::Infallible;
use rand_core::SeedableRng;

struct Untouched;

struct Scripted<'a> {
    input: &'a [u8],
}

struct CoalescedUpgrade {
    request: [u8; HANDSHAKE_BYTES],
    request_len: usize,
    response: [u8; HANDSHAKE_BYTES],
    response_len: usize,
    response_offset: usize,
}

impl CoalescedUpgrade {
    const fn new() -> Self {
        Self {
            request: [0; HANDSHAKE_BYTES],
            request_len: 0,
            response: [0; HANDSHAKE_BYTES],
            response_len: 0,
            response_offset: 0,
        }
    }

    fn prepare_response(&mut self) {
        let prefix = b"Sec-WebSocket-Key: ";
        let key_start = self.request[..self.request_len]
            .windows(prefix.len())
            .position(|window| window == prefix)
            .expect("client request has a WebSocket key")
            + prefix.len();
        let key_end = self.request[key_start..self.request_len]
            .windows(2)
            .position(|window| window == b"\r\n")
            .expect("WebSocket key header is terminated")
            + key_start;
        let key = core::str::from_utf8(&self.request[key_start..key_end])
            .expect("WebSocket key is ASCII");
        let key = embedded_websocket::WebSocketKey::from(key);
        let mut server = embedded_websocket::WebSocketServer::new_server();
        let mut used = server
            .server_accept(&key, None, &mut self.response)
            .expect("server accepts client key");
        used += server
            .write(
                WebSocketSendMessageType::Binary,
                false,
                b"one-",
                &mut self.response[used..],
            )
            .expect("first fragment fits");
        used += server
            .write(
                WebSocketSendMessageType::Binary,
                true,
                b"two",
                &mut self.response[used..],
            )
            .expect("final fragment fits");
        used += server
            .write(
                WebSocketSendMessageType::Binary,
                true,
                b"next",
                &mut self.response[used..],
            )
            .expect("second message fits");
        self.response_len = used;
    }
}

impl embedded_io::ErrorType for Untouched {
    type Error = Infallible;
}

impl Read for Untouched {
    fn read(&mut self, _: &mut [u8]) -> Result<usize, Self::Error> {
        panic!("invalid admission reached I/O")
    }
}

impl Write for Untouched {
    fn write(&mut self, _: &[u8]) -> Result<usize, Self::Error> {
        panic!("invalid admission reached I/O")
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        panic!("invalid admission reached I/O")
    }
}

impl embedded_io::ErrorType for Scripted<'_> {
    type Error = Infallible;
}

impl Read for Scripted<'_> {
    fn read(&mut self, output: &mut [u8]) -> Result<usize, Self::Error> {
        let count = output.len().min(self.input.len());
        output[..count].copy_from_slice(&self.input[..count]);
        self.input = &self.input[count..];
        Ok(count)
    }
}

impl Write for Scripted<'_> {
    fn write(&mut self, input: &[u8]) -> Result<usize, Self::Error> {
        Ok(input.len())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl embedded_io::ErrorType for CoalescedUpgrade {
    type Error = Infallible;
}

impl Read for CoalescedUpgrade {
    fn read(&mut self, output: &mut [u8]) -> Result<usize, Self::Error> {
        if self.response_len == 0 {
            self.prepare_response();
        }
        let remaining = &self.response[self.response_offset..self.response_len];
        let count = output.len().min(remaining.len());
        output[..count].copy_from_slice(&remaining[..count]);
        self.response_offset += count;
        Ok(count)
    }
}

impl Write for CoalescedUpgrade {
    fn write(&mut self, input: &[u8]) -> Result<usize, Self::Error> {
        if self.response_len != 0 {
            return Ok(input.len());
        }
        let available = self.request.len() - self.request_len;
        let count = input.len().min(available);
        self.request[self.request_len..self.request_len + count].copy_from_slice(&input[..count]);
        self.request_len += count;
        Ok(count)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[test]
fn descriptors_and_storage_refuse_before_stream_use() {
    let mut response = [0; 1];
    let make_rng = || rand_chacha::ChaCha20Rng::from_seed([7; 32]);
    assert_eq!(
        exchange(
            &mut Untouched,
            make_rng(),
            "",
            "/conduit",
            b"x",
            &mut response,
            1,
        ),
        Err(WebSocketError::InvalidDescriptor)
    );
    assert_eq!(
        exchange(
            &mut Untouched,
            make_rng(),
            "relay",
            "relative",
            b"x",
            &mut response,
            1,
        ),
        Err(WebSocketError::InvalidDescriptor)
    );
    assert_eq!(
        exchange(
            &mut Untouched,
            make_rng(),
            "relay",
            "/conduit",
            b"",
            &mut response,
            1,
        ),
        Err(WebSocketError::RequestTooLarge)
    );
    assert_eq!(
        exchange(
            &mut Untouched,
            make_rng(),
            "relay",
            "/conduit",
            b"x",
            &mut response,
            0,
        ),
        Err(WebSocketError::ResponseTooLarge)
    );
}

#[test]
fn rejected_and_lost_upgrades_remain_distinct() {
    let mut response = [0; 1];
    let rng = rand_chacha::ChaCha20Rng::from_seed([8; 32]);
    let mut rejected = Scripted {
        input: b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n",
    };
    assert_eq!(
        exchange(
            &mut rejected,
            rng,
            "relay",
            "/conduit",
            b"x",
            &mut response,
            1,
        ),
        Err(WebSocketError::HandshakeAuthentication)
    );

    let rng = rand_chacha::ChaCha20Rng::from_seed([9; 32]);
    let mut lost = Scripted { input: b"" };
    assert_eq!(
        exchange(&mut lost, rng, "relay", "/conduit", b"x", &mut response, 1,),
        Err(WebSocketError::HandshakeRead)
    );
}

#[test]
fn coalesced_upgrade_fragmented_message_and_next_message_reuse_fixed_storage() {
    let mut stream = CoalescedUpgrade::new();
    let rng = rand_chacha::ChaCha20Rng::from_seed([10; 32]);
    let mut websocket = BoundedWebSocket::connect(&mut stream, rng, "relay", "/conduit")
        .expect("coalesced upgrade succeeds");
    assert_eq!(
        websocket.received, 17,
        "all coalesced frames remain buffered"
    );
    assert_eq!(
        &websocket.receive[..websocket.received],
        b"\x02\x04one-\x80\x03two\x82\x04next"
    );
    let mut first = [0; 7];
    assert_eq!(websocket.receive_binary(&mut first), Ok(7));
    assert_eq!(&first, b"one-two");
    let mut second = [0; 4];
    assert_eq!(websocket.receive_binary(&mut second), Ok(4));
    assert_eq!(&second, b"next");
    websocket.close().expect("bounded close fits");
}

#[test]
fn native_admission_sized_binary_message_fits_the_finite_frame() {
    let mut stream = CoalescedUpgrade::new();
    let rng = rand_chacha::ChaCha20Rng::from_seed([11; 32]);
    let mut websocket =
        BoundedWebSocket::connect(&mut stream, rng, "owner", "/conduit").expect("upgrade succeeds");
    assert!(websocket.send_binary(&[7; 5288]).is_ok());
    assert_eq!(
        websocket.send_binary(&[7; MAXIMUM_BINARY_MESSAGE_BYTES + 1]),
        Err(WebSocketError::RequestTooLarge)
    );
}
