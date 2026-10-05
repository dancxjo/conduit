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
        })
    }
    pub(super) fn receive(
        &mut self,
        timeout: Duration,
    ) -> Result<(BrowserAdmissionIngress, u32), String> {
        self.line
            .set_read_timeout(Some(timeout.max(Duration::from_millis(1))))
            .map_err(debug)?;
        let bytes = self.line.receive_binary(&mut self.input).map_err(debug)?;
        Ok((
            decode_browser_admission_frame(&self.input[..bytes]).map_err(debug)?,
            bytes as u32,
        ))
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
