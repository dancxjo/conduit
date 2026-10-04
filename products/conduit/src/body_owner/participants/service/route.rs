//! Owner selection of the ordinary browser Mask on its current admitted carrier.
use super::*;
use conduit_core::{
    AuthorityGrantId, BaseImplementationId, BaseInstanceId, CredentialReferenceId,
    LineAvailability, LineAvailabilitySign, LineId, LineOffer, LinkAuthorityReference, LinkBinding,
    LinkCredentialReference, LinkEndpoint, LinkEndpointId, LinkLimits, SignId,
};
use conduit_presentation::{MaskShow, OwnerFaceSnapshotRequest, RemoteOwnerMaskRouteSeal};

const WEBSOCKET: &conduit_host_browser_make::BrowserLineRealizationDescriptor =
    &conduit_host_browser_make::BROWSER_LINE_REALIZATIONS[0];

impl Owner {
    /// Called only by the worker holding the accepted browser socket. The
    /// serialized owner verifies its exact observed binding again and retains
    /// the selected route until replacement or browser leave.
    pub(crate) fn browser_mask_route(
        &mut self,
        window_id: &str,
        credential: &MembershipCredential,
        binding: &LinkBindingId,
    ) -> Result<RemoteOwnerMaskRouteSeal, String> {
        let mut window = self.pending_browser.take().ok_or("window-not-active")?;
        let result = (|| {
            window.check(window_id).map_err(|_| "window-not-active")?;
            let WindowState::Active {
                credential: active,
                observation,
                route,
            } = &mut window.state
            else {
                return Err("window-not-active".into());
            };
            if active != credential || observation.observed_binding_id != *binding {
                return Err("browser-route-carrier-mismatch".into());
            }
            let current = self.session.evidence().membership.parts.iter().any(|part| {
                part.part_id == credential.part_id
                    && part.current.as_ref().is_some_and(|host| {
                        host.host_id == credential.host_id
                            && host.boot_id == credential.boot_id
                            && host.offer_generation == observation.advertisement.offer_generation
                    })
            });
            let admitted = self.admissions.as_ref().is_some_and(|manager| {
                manager
                    .receipts
                    .iter()
                    .rev()
                    .find(|receipt| receipt.credential.part_id == credential.part_id)
                    .is_some_and(|receipt| receipt.credential == *credential)
            });
            if !current || !admitted || credential.body_id != self.session.evidence().body_id {
                return Err("browser-route-part-unavailable".into());
            }
            let browser = &observation.advertisement;
            if !browser
                .capabilities
                .contains(&conduit_browser_mask_offer::offer())
            {
                return Err("browser-route-mask-back-unavailable".into());
            }
            let owner = self.host.advertisement();
            let planned = conduit_browser_mask_offer::planned_mask(
                browser,
                conduit_browser_mask_offer::MASK_SOURCE,
                "browser-graphical",
            )?;
            let face = self.local_face_snapshot()?;
            let forward = carrier_line("face", binding, window_id, credential, owner, browser);
            let backward = carrier_line("return", binding, window_id, credential, browser, owner);
            let selected = RemoteOwnerMaskRouteSeal::seal_current(
                &self.session,
                &face,
                owner,
                browser,
                &planned,
                &forward,
                &backward,
            )
            .map_err(|error| format!("browser-route-refused:{error:?}"))?;
            *route = Some(Box::new(selected.clone()));
            Ok(selected)
        })();
        self.pending_browser = Some(window);
        result
    }

    pub(crate) fn validate_browser_mask_show(
        &self,
        window_id: &str,
        binding: &LinkBindingId,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
    ) -> Result<(), String> {
        let window = self.pending_browser.as_ref().ok_or("window-not-active")?;
        window.check(window_id).map_err(|_| "window-not-active")?;
        let WindowState::Active {
            credential,
            observation,
            route: Some(route),
        } = &window.state
        else {
            return Err("browser-mask-route-not-selected".into());
        };
        if observation.observed_binding_id != *binding
            || request.credential_id != credential.credential_id.as_str()
            || request.body_id != credential.body_id
            || request.part_id != credential.part_id
            || request.host_id != credential.host_id
            || request.boot_id != credential.boot_id
        {
            return Err("browser-route-carrier-mismatch".into());
        }
        let face = self.face_snapshot(request)?;
        let owner = self.host.advertisement();
        let browser = &observation.advertisement;
        let forward = carrier_line("face", binding, window_id, credential, owner, browser);
        let backward = carrier_line("return", binding, window_id, credential, browser, owner);
        route
            .validate_available_show(
                &self.session,
                &face,
                owner,
                browser,
                &forward,
                &backward,
                show,
            )
            .map_err(|error| format!("browser-mask-show-refused:{error:?}"))
    }
}

fn carrier_line(
    direction: &str,
    carrier: &LinkBindingId,
    window_id: &str,
    credential: &MembershipCredential,
    source: &HostAdvertisement,
    sink: &HostAdvertisement,
) -> LineOffer {
    let carrier = carrier.as_str();
    let line_id = LineId::from(format!("line/browser-mask/{carrier}/{direction}"));
    let binding_id = LinkBindingId::from(format!("binding/browser-mask/{carrier}/{direction}"));
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
                endpoint_id: LinkEndpointId::from(format!("endpoint/{carrier}/{direction}/source")),
            },
            sink: LinkEndpoint {
                host_id: sink.host_id.clone(),
                boot_id: sink.boot_id.clone(),
                endpoint_id: LinkEndpointId::from(format!("endpoint/{carrier}/{direction}/sink")),
            },
            base: BaseImplementationId::from(WEBSOCKET.base_implementation_id),
            base_instance_id: BaseInstanceId::from(format!("base-instance/{carrier}")),
            credential: LinkCredentialReference::Opaque(CredentialReferenceId::from(
                credential.credential_id.as_str(),
            )),
            authority: LinkAuthorityReference::Grant(AuthorityGrantId::from(window_id)),
            limits: LinkLimits {
                maximum_in_flight_items: WEBSOCKET.maximum_in_flight_items,
                maximum_payload_bytes: WEBSOCKET.maximum_payload_bytes,
                maximum_buffered_bytes: WEBSOCKET.maximum_buffered_bytes,
                maximum_frame_bytes: WEBSOCKET.maximum_frame_bytes,
            },
        },
        contract: WEBSOCKET.contract,
    }
}
