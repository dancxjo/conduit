//! Bounded loopback carrier for the existing browser admission protocol.
use super::{
    nonce,
    service::{BrowserAdmittedSnapshot, BrowserCarrierLineEvidence},
};
use conduit_core::{
    BaseImplementationId, BaseInstanceId, CredentialReferenceId, LineAvailability,
    LineAvailabilitySign, LineId, LineOffer, LinkAuthorityReference, LinkBinding, LinkBindingId,
    LinkCredentialReference, LinkEndpoint, LinkEndpointId, LinkLimits, SignId,
};
use conduit_std_host::{
    browser_admission::{
        decode_browser_admission_frame, encode_browser_admission_frame, BrowserAdmissionEgress,
        BrowserAdmissionIngress, MAX_BROWSER_ADMISSION_FRAME_BYTES,
    },
    websocket::{NativeWebSocketError, NativeWebSocketLine, NativeWebSocketListener},
};
use std::time::Duration;

pub(super) struct Listener(NativeWebSocketListener);
pub(super) struct Socket {
    line: NativeWebSocketLine,
    binding: LinkBindingId,
    closed: bool,
    input: Box<[u8]>,
    output: Box<[u8]>,
}
impl Listener {
    pub(super) fn bind() -> Result<Self, String> {
        NativeWebSocketListener::bind_loopback(MAX_BROWSER_ADMISSION_FRAME_BYTES as u32)
            .map(Self)
            .map_err(debug)
    }
    pub(super) fn url(&self) -> Result<String, String> {
        self.0.url().map_err(debug)
    }
    pub(super) fn accept(
        &self,
        timeout: Duration,
        binding_prefix: &str,
    ) -> Result<Option<Socket>, String> {
        match self.0.accept_with_timeout(timeout) {
            Ok(line) => Ok(Some(Socket {
                line,
                binding: LinkBindingId::from(format!("{binding_prefix}/{}", hex(&nonce()?))),
                closed: false,
                input: vec![0; MAX_BROWSER_ADMISSION_FRAME_BYTES].into_boxed_slice(),
                output: vec![0; MAX_BROWSER_ADMISSION_FRAME_BYTES].into_boxed_slice(),
            })),
            Err(NativeWebSocketError::AcceptDeadline) => Ok(None),
            Err(error) => Err(debug(error)),
        }
    }
}
impl Socket {
    pub(super) fn binding(&self) -> &LinkBindingId {
        &self.binding
    }

    /// Issued only while this accepted WebSocket incarnation is retained.
    /// The serialized owner checks the authorization and both directions.
    pub(super) fn line_evidence(
        &self,
        snapshot: &BrowserAdmittedSnapshot,
        window_id: &str,
    ) -> Result<BrowserCarrierLineEvidence, String> {
        let authorization = snapshot
            .line_authorization
            .as_ref()
            .ok_or("carrier-line-authorization-missing")?;
        let owner = snapshot
            .owner_advertisement
            .as_deref()
            .ok_or("carrier-owner-offer-missing")?;
        let browser = snapshot
            .browser_advertisement
            .as_deref()
            .ok_or("carrier-browser-offer-missing")?;
        if self.closed
            || authorization.window_id != window_id
            || authorization.carrier_binding != self.binding
            || authorization.credential_id != snapshot.credential.credential_id.as_str()
        {
            return Err("carrier-line-authorization-stale".into());
        }
        let descriptor = &conduit_host_browser_make::BROWSER_LINE_REALIZATIONS[0];
        if self.line.maximum_message_bytes() < descriptor.maximum_frame_bytes as usize {
            return Err("carrier-line-frame-limit".into());
        }
        let offer = |direction: &str,
                     source: &conduit_core::HostAdvertisement,
                     sink: &conduit_core::HostAdvertisement,
                     grant: &conduit_core::AuthorityGrantId| {
            let carrier = self.binding.as_str();
            let line_id = LineId::from(format!("line/browser-mask/{carrier}/{direction}"));
            let binding_id =
                LinkBindingId::from(format!("binding/browser-mask/{carrier}/{direction}"));
            LineOffer {
                availability: LineAvailabilitySign {
                    line_id: line_id.clone(),
                    binding_id: binding_id.clone(),
                    availability: LineAvailability::Ready,
                    sign_id: SignId::from(format!("sign/browser-mask/{carrier}/{direction}/ready")),
                },
                line_id,
                binding: LinkBinding {
                    binding_id,
                    source: LinkEndpoint {
                        host_id: source.host_id.clone(),
                        boot_id: source.boot_id.clone(),
                        endpoint_id: LinkEndpointId::from(format!(
                            "endpoint/{carrier}/{direction}/source"
                        )),
                    },
                    sink: LinkEndpoint {
                        host_id: sink.host_id.clone(),
                        boot_id: sink.boot_id.clone(),
                        endpoint_id: LinkEndpointId::from(format!(
                            "endpoint/{carrier}/{direction}/sink"
                        )),
                    },
                    base: BaseImplementationId::from(descriptor.base_implementation_id),
                    base_instance_id: BaseInstanceId::from(format!("base-instance/{carrier}")),
                    credential: LinkCredentialReference::Opaque(CredentialReferenceId::from(
                        snapshot.credential.credential_id.as_str(),
                    )),
                    authority: LinkAuthorityReference::Grant(grant.clone()),
                    limits: LinkLimits {
                        maximum_in_flight_items: descriptor.maximum_in_flight_items,
                        maximum_payload_bytes: descriptor.maximum_payload_bytes,
                        maximum_buffered_bytes: descriptor.maximum_buffered_bytes,
                        maximum_frame_bytes: descriptor.maximum_frame_bytes,
                    },
                },
                contract: descriptor.contract,
            }
        };
        Ok(BrowserCarrierLineEvidence {
            authorization: authorization.clone(),
            face: offer("face", owner, browser, &authorization.face_grant_id),
            returned: offer("return", browser, owner, &authorization.return_grant_id),
            interaction: offer(
                "interaction",
                browser,
                owner,
                &authorization.interaction_grant_id,
            ),
        })
    }
    pub(super) fn receive(
        &mut self,
        timeout: Duration,
    ) -> Result<(BrowserAdmissionIngress, u32), String> {
        self.receive_polled(timeout)?
            .ok_or_else(|| "browser admission receive timed out".into())
    }

    /// Tungstenite retains partial frames across read timeouts. A pending owner
    /// operation uses this poll without consuming a frame or renewing a lease.
    pub(super) fn receive_polled(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<(BrowserAdmissionIngress, u32)>, String> {
        self.line
            .set_read_timeout(Some(timeout.max(Duration::from_millis(1))))
            .map_err(debug)?;
        let bytes = match self.line.receive_binary(&mut self.input) {
            Ok(bytes) => bytes,
            Err(NativeWebSocketError::Transport(
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut,
            )) => return Ok(None),
            Err(error) => return Err(debug(error)),
        };
        Ok(Some((
            decode_browser_admission_frame(&self.input[..bytes]).map_err(debug)?,
            bytes as u32,
        )))
    }
    pub(super) fn send(&mut self, frame: &BrowserAdmissionEgress) -> Result<(), String> {
        let length = encode_browser_admission_frame(frame, &mut self.output).map_err(debug)?;
        self.line.send_binary(&self.output[..length]).map_err(debug)
    }
    pub(super) fn close(&mut self) {
        self.closed = true;
        let _ = self.line.close();
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn debug(error: impl std::fmt::Debug) -> String {
    format!("browser admission carrier: {error:?}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpStream,
        sync::mpsc,
    };

    #[test]
    fn partial_frame_survives_pending_operation_poll_timeout() {
        let listener = Listener::bind().unwrap();
        let url = listener.url().unwrap();
        let expected: BrowserAdmissionIngress = serde_json::from_str(r#"{"kind":"presence-leave","protocol":1,"credential_id":"credential/test","body_id":"body/test","part_id":"part/test","host_id":"host/test","boot_id":"boot/test","sequence":2}"#).unwrap();
        let data = serde_json::to_vec(&expected).unwrap();
        let (partial, ready) = mpsc::sync_channel(1);
        let (finish, continue_frame) = mpsc::sync_channel(1);
        let client = std::thread::spawn(move || {
            let (authority, path) = url.strip_prefix("ws://").unwrap().split_once('/').unwrap();
            let mut tcp = TcpStream::connect(authority).unwrap();
            tcp.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            write!(tcp, "GET /{path} HTTP/1.1\r\nHost: {authority}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n").unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                assert!(header.len() < 2048);
                let mut byte = [0];
                tcp.read_exact(&mut byte).unwrap();
                header.push(byte[0]);
            }
            assert!(header.starts_with(b"HTTP/1.1 101"));
            let mask = [3, 1, 4, 1];
            let mut encoded = vec![0x82];
            if data.len() < 126 {
                encoded.push(0x80 | data.len() as u8);
            } else {
                encoded.push(0x80 | 126);
                encoded.extend_from_slice(&(data.len() as u16).to_be_bytes());
            }
            encoded.extend_from_slice(&mask);
            encoded.extend(data.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
            let split = encoded.len() / 2;
            tcp.write_all(&encoded[..split]).unwrap();
            partial.send(()).unwrap();
            continue_frame.recv().unwrap();
            tcp.write_all(&encoded[split..]).unwrap();
        });
        let mut socket = listener
            .accept(Duration::from_secs(2), "poll-test")
            .unwrap()
            .unwrap();
        ready.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(socket
            .receive_polled(Duration::from_millis(20))
            .unwrap()
            .is_none());
        finish.send(()).unwrap();
        let (actual, _) = socket.receive(Duration::from_secs(2)).unwrap();
        assert_eq!(actual, expected);
        client.join().unwrap();
    }
}
