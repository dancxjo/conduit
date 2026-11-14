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
    secure_websocket::{SecureWebSocketError, SecureWebSocketLine},
    websocket::{NativeWebSocketError, NativeWebSocketLine, NativeWebSocketListener},
};
use std::time::Duration;

pub(super) struct Listener(NativeWebSocketListener);
pub(crate) struct Socket {
    line: Carrier,
    binding: LinkBindingId,
    closed: bool,
    input: Box<[u8]>,
    output: Box<[u8]>,
}
#[allow(
    clippy::large_enum_variant,
    reason = "Retain one initialized transport inline for the Socket lifetime."
)]
enum Carrier {
    Native(NativeWebSocketLine),
    Secure(SecureWebSocketLine),
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
                line: Carrier::Native(line),
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
    /// Consume an actual authenticated TLS incarnation. The caller supplies its
    /// exact authorized binding and completes any prior bounded input deadline
    /// only after authentication. This constructor grants no Line or membership.
    pub(super) fn from_secure(
        line: SecureWebSocketLine,
        binding: LinkBindingId,
    ) -> Result<Self, String> {
        let maximum_frame_bytes = line
            .maximum_message_bytes()
            .min(MAX_BROWSER_ADMISSION_FRAME_BYTES);
        if binding.as_str().is_empty() || binding.as_str().len() > 256 || maximum_frame_bytes == 0 {
            return Err("secure participant carrier extent or binding invalid".into());
        }
        Ok(Self {
            line: Carrier::Secure(line),
            binding,
            closed: false,
            input: vec![0; maximum_frame_bytes].into_boxed_slice(),
            output: vec![0; maximum_frame_bytes].into_boxed_slice(),
        })
    }

    /// An authorized lease, not raw traffic, replaces this exact TLS lifetime.
    pub(super) fn replace_input_deadline(
        &mut self,
        deadline: std::time::Instant,
    ) -> Result<(), String> {
        if self.closed {
            return Err("participant carrier closed".into());
        }
        match &mut self.line {
            Carrier::Secure(line) => line.replace_input_deadline(deadline).map_err(debug),
            Carrier::Native(_) => Err("TLS deadline requires an actual secure carrier".into()),
        }
    }

    /// Actual bounded TLS carrier custody is required before offering native
    /// document directions. The owner separately authorizes and selects them.
    pub(crate) fn native_lines(
        &self,
        owner: &conduit_core::HostAdvertisement,
        member: &crate::durable_host::owner::MemberPresentationBasis,
    ) -> Result<(LineOffer, LineOffer), String> {
        let Carrier::Secure(line) = &self.line else {
            return Err("native document Lines require actual TLS carrier".into());
        };
        if self.closed {
            return Err("participant carrier closed".into());
        }
        let pair = crate::durable_host::routed_join::native_lines::offer_pair(
            owner,
            member,
            self.binding.as_str(),
        );
        for offer in [&pair.0, &pair.1] {
            let needed = offer.binding.limits.maximum_frame_bytes as usize;
            if needed > line.maximum_message_bytes()
                || needed > self.input.len()
                || needed > self.output.len()
            {
                return Err("native document Line exceeds initialized carrier buffers".into());
            }
        }
        Ok(pair)
    }

    /// Third logical direction is supplied only while this same TLS socket and
    /// its original initialized bounded buffers remain held and live.
    pub(crate) fn native_graphical_lines(
        &self,
        owner: &conduit_core::HostAdvertisement,
        member: &crate::durable_host::owner::MemberPresentationBasis,
    ) -> Result<(LineOffer, LineOffer, Option<LineOffer>), String> {
        let (face, show) = self.native_lines(owner, member)?;
        let offers = crate::durable_host::routed_join::native_lines::offer_graphical(
            owner,
            member,
            self.binding.as_str(),
        )?;
        if face != offers.0 || show != offers.1 {
            return Err("native graphical carrier changed".into());
        }
        if let Some(offer) = &offers.2 {
            let Carrier::Secure(line) = &self.line else {
                return Err("native interaction requires held TLS carrier".into());
            };
            let needed = offer.binding.limits.maximum_frame_bytes as usize;
            if self.closed
                || needed > line.maximum_message_bytes()
                || needed > self.input.len()
                || needed > self.output.len()
            {
                return Err("native interaction Line exceeds actual carrier custody".into());
            }
        }
        Ok(offers)
    }

    pub(crate) fn binding(&self) -> &LinkBindingId {
        &self.binding
    }

    /// Issued only while this accepted WebSocket incarnation is retained.
    /// The serialized owner checks the authorization and both directions.
    pub(super) fn line_evidence(
        &self,
        snapshot: &BrowserAdmittedSnapshot,
        window_id: &str,
    ) -> Result<BrowserCarrierLineEvidence, String> {
        let Carrier::Native(line) = &self.line else {
            return Err("secure carrier requires its actual TLS Line evidence".into());
        };
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
        if line.maximum_message_bytes() < descriptor.maximum_frame_bytes as usize {
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
        if self.closed {
            return Err("participant carrier closed".into());
        }
        let timeout = Some(timeout.max(Duration::from_millis(1)));
        let bytes = match &mut self.line {
            Carrier::Native(line) => {
                line.set_read_timeout(timeout).map_err(debug)?;
                match line.receive_binary(&mut self.input) {
                    Ok(bytes) => bytes,
                    Err(NativeWebSocketError::Transport(
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut,
                    )) => return Ok(None),
                    Err(error) => return Err(debug(error)),
                }
            }
            Carrier::Secure(line) => {
                line.set_read_timeout(timeout).map_err(debug)?;
                match line.receive_binary(&mut self.input) {
                    Ok(bytes) => bytes,
                    Err(SecureWebSocketError::Transport(
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut,
                    )) => return Ok(None),
                    Err(error) => return Err(debug(error)),
                }
            }
        };
        Ok(Some((
            decode_browser_admission_frame(&self.input[..bytes]).map_err(debug)?,
            bytes as u32,
        )))
    }
    pub(super) fn send(&mut self, frame: &BrowserAdmissionEgress) -> Result<(), String> {
        let length = encode_browser_admission_frame(frame, &mut self.output).map_err(debug)?;
        if self.closed {
            return Err("participant carrier closed".into());
        }
        match &mut self.line {
            Carrier::Native(line) => line.send_binary(&self.output[..length]).map_err(debug),
            Carrier::Secure(line) => line.send_binary(&self.output[..length]).map_err(debug),
        }
    }
    pub(super) fn close(&mut self) {
        self.closed = true;
        match &mut self.line {
            Carrier::Native(line) => {
                let _ = line.close();
            }
            Carrier::Secure(line) => {
                let _ = line.close();
            }
        }
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
    #[test]
    fn pinned_tls_socket_preserves_binding_and_poll_timeout() {
        use conduit_std_host::secure_websocket::{
            SecureWebSocketClientLine, SecureWebSocketListener,
        };
        use rcgen::{generate_simple_self_signed, CertifiedKey};
        let CertifiedKey { cert, signing_key } =
            generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let directory =
            std::env::temp_dir().join(format!("conduit-participant-tls-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let certificate = directory.join("certificate.pem");
        let private_key = directory.join("private-key.pem");
        std::fs::write(&certificate, cert.pem()).unwrap();
        std::fs::write(&private_key, signing_key.serialize_pem()).unwrap();
        let reservation = std::net::TcpListener::bind("0.0.0.0:0").unwrap();
        let address = reservation.local_addr().unwrap();
        drop(reservation);
        let listener = SecureWebSocketListener::bind(
            address,
            &certificate,
            &private_key,
            MAX_BROWSER_ADMISSION_FRAME_BYTES as u32,
            true,
        )
        .unwrap();
        let address =
            std::net::SocketAddr::from(([127, 0, 0, 1], listener.local_addr().unwrap().port()));
        let pin = listener.certificate_binding_sha256();
        let (finish, ready) = mpsc::sync_channel(1);
        let client = std::thread::spawn(move || {
            let mut line = SecureWebSocketClientLine::connect_pinned(
                address,
                &format!("wss://localhost:{}/conduit", address.port()),
                "localhost",
                pin,
                Duration::from_secs(2),
                MAX_BROWSER_ADMISSION_FRAME_BYTES as u32,
            )
            .unwrap();
            ready.recv_timeout(Duration::from_secs(2)).unwrap();
            line.send_binary(br#"{"kind":"presence-leave","protocol":1,"credential_id":"credential/test","body_id":"body/test","part_id":"part/test","host_id":"host/test","boot_id":"boot/test","sequence":2}"#).unwrap();
        });
        let mut accepted = listener
            .accept_with_timeout(Duration::from_secs(2))
            .unwrap();
        accepted.complete_bounded_input();
        let binding: LinkBindingId = "fixture/exact-tls-incarnation".into();
        let mut socket = Socket::from_secure(accepted, binding.clone()).unwrap();
        assert_eq!(socket.binding(), &binding);
        assert!(socket
            .receive_polled(Duration::from_millis(20))
            .unwrap()
            .is_none());
        finish.send(()).unwrap();
        let (frame, _) = socket.receive(Duration::from_secs(2)).unwrap();
        assert!(matches!(
            frame,
            BrowserAdmissionIngress::PresenceLeave { sequence: 2, .. }
        ));
        socket.close();
        assert!(socket.receive_polled(Duration::from_millis(1)).is_err());
        client.join().unwrap();
        drop(socket);
        drop(listener);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
