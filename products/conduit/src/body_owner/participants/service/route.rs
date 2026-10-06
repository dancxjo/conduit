//! Owner selection of the ordinary browser Mask on its current admitted carrier.
use super::*;
use conduit_core::{
    BaseImplementationId, BaseInstanceId, CredentialReferenceId, LineAvailability,
    LinkAuthorityReference, LinkCredentialReference, LinkLimits,
};
use conduit_presentation::{
    FaceInteraction, MaskShow, MaskWardrobeAction, OwnerFaceSnapshotRequest,
    RemoteOwnerMaskRouteError, RemoteOwnerMaskRouteSeal,
};

const WEBSOCKET: &conduit_host_browser_make::BrowserLineRealizationDescriptor =
    &conduit_host_browser_make::BROWSER_LINE_REALIZATIONS[0];

impl Owner {
    /// Membership can change the Face after the browser has sealed its route.
    /// Rebind only an intact, still-attached browser route to that new Face;
    /// `browser_mask_route` rechecks the actual carrier, Part, offers, and Lines
    /// and invalidates the old Show before replacing the owner Plan.
    pub(crate) fn refresh_browser_mask_route_for_current_face(&mut self) -> Result<(), String> {
        let face = self.local_face_snapshot()?;
        let Some(window) = self.pending_browser.as_ref() else {
            return Ok(());
        };
        let Some((seal, browser_offer, face_line, return_line, interaction_line)) =
            window.current_mask_route()
        else {
            return Ok(());
        };
        match seal.validate_current_with_interaction(
            &self.session,
            &face,
            self.host.advertisement(),
            browser_offer,
            face_line,
            return_line,
            interaction_line,
        ) {
            Ok(()) => return Ok(()),
            // An accepted workset replacement advances the same Body's
            // workload revision as well as its Face. Re-seal only through
            // browser_mask_route, which rechecks the current credential,
            // attached carrier, Host offers, and exact Lines.
            Err(RemoteOwnerMaskRouteError::StaleFace | RemoteOwnerMaskRouteError::StaleBody) => {}
            Err(error) => return Err(format!("browser route cannot refresh: {error:?}")),
        }
        let WindowState::Active {
            credential,
            observation,
            line_evidence: Some(evidence),
            ..
        } = &window.state
        else {
            return Err("browser route lost its carrier evidence".into());
        };
        let window_id = window.id.clone();
        let credential = credential.clone();
        let binding = observation.observed_binding_id.clone();
        let evidence = (**evidence).clone();
        self.browser_mask_route(&window_id, &credential, &binding, Some(&evidence))?;
        Ok(())
    }

    pub(crate) fn browser_wardrobe_report(
        &mut self,
        window_id: &str,
        binding: &LinkBindingId,
        request: &OwnerFaceSnapshotRequest,
        owner_plan_id: Option<&conduit_core::PlanId>,
        basis_revision: u64,
        action: Option<MaskWardrobeAction>,
    ) -> Result<serde_json::Value, String> {
        {
            let window = self.pending_browser.as_ref().ok_or("window-not-active")?;
            window.check(window_id).map_err(|_| "window-not-active")?;
            let WindowState::Active {
                credential,
                observation,
                line_authorization,
                line_evidence: Some(lines),
                route: Some(route),
                ..
            } = &window.state
            else {
                return Err("browser-wardrobe-face-route-unavailable".into());
            };
            if observation.observed_binding_id != *binding
                || request.credential_id != credential.credential_id.as_str()
                || request.body_id != credential.body_id
                || request.part_id != credential.part_id
                || request.host_id != credential.host_id
                || request.boot_id != credential.boot_id
            {
                return Err("browser-wardrobe-carrier-mismatch".into());
            }
            let current = self.session.evidence().membership.parts.iter().any(|part| {
                part.part_id == credential.part_id
                    && part.current.as_ref().is_some_and(|host| {
                        host.host_id == credential.host_id
                            && host.boot_id == credential.boot_id
                            && host.offer_generation == observation.advertisement.offer_generation
                    })
            });
            if !current {
                return Err("browser-wardrobe-part-unavailable".into());
            }
            let face = self.face_snapshot(request)?;
            validate_carrier_evidence(
                lines,
                line_authorization,
                window_id,
                binding,
                credential,
                self.host.advertisement(),
                &observation.advertisement,
            )?;
            route
                .validate_current_with_interaction(
                    &self.session,
                    &face,
                    self.host.advertisement(),
                    &observation.advertisement,
                    &lines.face,
                    &lines.returned,
                    &lines.interaction,
                )
                .map_err(|error| format!("browser-wardrobe-route-stale:{error:?}"))?;
        }
        if action.is_some()
            && (self.session.evidence().body.state != BodyState::Lulled
                || self.session.realization().is_some())
        {
            return Err("owner-wardrobe-requires-lulled-body".into());
        }
        self.owner_wardrobe_report(owner_plan_id, basis_revision, action)
    }

    /// Called only by the worker holding the accepted browser socket. The
    /// serialized owner verifies its exact observed binding again and retains
    /// the selected route until replacement or browser leave.
    pub(crate) fn browser_mask_route(
        &mut self,
        window_id: &str,
        credential: &MembershipCredential,
        binding: &LinkBindingId,
        evidence: Option<&BrowserCarrierLineEvidence>,
    ) -> Result<RemoteOwnerMaskRouteSeal, String> {
        let mut window = self.pending_browser.take().ok_or("window-not-active")?;
        let result = (|| -> Result<RemoteOwnerMaskRouteSeal, String> {
            window.check(window_id).map_err(|_| "window-not-active")?;
            let WindowState::Active {
                credential: active,
                observation,
                line_authorization,
                line_evidence,
                route,
                acknowledged_show,
                ..
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
            let evidence = evidence.ok_or("browser-line-evidence-missing")?;
            validate_carrier_evidence(
                evidence,
                line_authorization,
                window_id,
                binding,
                credential,
                owner,
                browser,
            )?;
            let planned = conduit_browser_mask_offer::planned_owner_face_show_interaction_mask(
                browser,
                owner,
                &evidence.face,
                &evidence.returned,
                &evidence.interaction,
                conduit_browser_mask_offer::MASK_SOURCE,
                "browser-graphical",
            )?;
            let face = self.local_face_snapshot()?;
            let selected = RemoteOwnerMaskRouteSeal::seal_current_with_interaction(
                &self.session,
                &face,
                owner,
                browser,
                &planned,
                &evidence.face,
                &evidence.returned,
                &evidence.interaction,
            )
            .map_err(|error| format!("browser-route-refused:{error:?}"))?;
            *route = Some(Box::new(selected.clone()));
            *line_evidence = Some(Box::new(evidence.clone()));
            *acknowledged_show = None;
            Ok(selected)
        })();
        self.pending_browser = Some(window);
        let selected = result?;
        self.admit_browser_presentation_route(&selected)?;
        Ok(selected)
    }

    pub(crate) fn validate_browser_mask_show(
        &self,
        window_id: &str,
        binding: &LinkBindingId,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
    ) -> Result<(), String> {
        self.validate_browser_mask_route_show(window_id, binding, request, show)?;
        let Some(BrowserWindow {
            state: WindowState::Active {
                acknowledged_show, ..
            },
            ..
        }) = self.pending_browser.as_ref()
        else {
            return Err("window-not-active".into());
        };
        if acknowledged_show.as_deref() != Some(show) {
            return Err("browser-mask-show-not-acknowledged".into());
        }
        Ok(())
    }

    pub(crate) fn validate_browser_mask_interaction(
        &mut self,
        window_id: &str,
        binding: &LinkBindingId,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<(), String> {
        self.validate_browser_mask_show(window_id, binding, request, show)?;
        let Some(BrowserWindow {
            state: WindowState::Active {
                route: Some(route), ..
            },
            ..
        }) = self.pending_browser.as_ref()
        else {
            return Err("browser-mask-route-not-selected".into());
        };
        route
            .validate_interaction_payload(interaction.encode().len())
            .map_err(|error| format!("browser-mask-interaction-refused:{error:?}"))?;
        let face = self.local_face_snapshot()?;
        let local = Self::current_attached_terminal_route(
            &self.host,
            self.attached_terminal_route.as_ref(),
            &self.session,
            &face,
        )?;
        let current = Self::current_presentation_routes_with_native(
            self.host.advertisement(),
            self.pending_browser.as_ref(),
            local,
            self.pending_native_mask.as_ref(),
            &self.session,
            &face,
            super::super::super::super::super::current_time_millis()?,
        );
        let selected = self
            .presentation_wardrobe
            .as_mut()
            .ok_or("owner presentation wardrobe is not admitted")?
            .selected_show(&self.session, &face, &current)
            .map_err(super::super::super::presentation_wardrobe_runtime::wardrobe_error)?;
        if selected != show {
            return Err("stale owner Mask Show".into());
        }
        Ok(())
    }

    fn validate_browser_mask_route_show(
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
            line_authorization,
            line_evidence,
            route: Some(route),
            ..
        } = &window.state
        else {
            return Err("browser-mask-route-not-selected".into());
        };
        let evidence = line_evidence
            .as_deref()
            .ok_or("browser-line-evidence-lost")?;
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
        validate_carrier_evidence(
            evidence,
            line_authorization,
            window_id,
            binding,
            credential,
            owner,
            browser,
        )?;
        route
            .validate_available_show_with_interaction(
                &self.session,
                &face,
                owner,
                browser,
                &evidence.face,
                &evidence.returned,
                &evidence.interaction,
                show,
            )
            .map_err(|error| format!("browser-mask-show-refused:{error:?}"))?;
        Ok(())
    }

    pub(crate) fn acknowledge_browser_mask_show(
        &mut self,
        window_id: &str,
        binding: &LinkBindingId,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
    ) -> Result<(), String> {
        self.validate_browser_mask_route_show(window_id, binding, request, show)?;
        let window = self.pending_browser.as_ref().ok_or("window-not-active")?;
        let seal = match &window.state {
            WindowState::Active {
                route: Some(route), ..
            } => (**route).clone(),
            _ => return Err("browser-mask-route-not-selected".into()),
        };
        self.acknowledge_selected_browser_show(&seal, show)?;
        let window = self.pending_browser.as_mut().ok_or("window-not-active")?;
        let WindowState::Active {
            acknowledged_show, ..
        } = &mut window.state
        else {
            return Err("window-not-active".into());
        };
        *acknowledged_show = Some(Box::new(show.clone()));
        Ok(())
    }
}

fn validate_carrier_evidence(
    evidence: &BrowserCarrierLineEvidence,
    issued: &BrowserLineAuthorization,
    window_id: &str,
    binding: &LinkBindingId,
    credential: &MembershipCredential,
    owner: &HostAdvertisement,
    browser: &HostAdvertisement,
) -> Result<(), String> {
    if &evidence.authorization != issued
        || &issued.carrier_binding != binding
        || issued.credential_id != credential.credential_id.as_str()
        || issued.window_id != window_id
    {
        return Err("browser-line-authority-missing-or-stale".into());
    }
    let expected_base = BaseImplementationId::from(WEBSOCKET.base_implementation_id);
    let expected_instance = BaseInstanceId::from(format!("base-instance/{}", binding.as_str()));
    let expected_credential = LinkCredentialReference::Opaque(CredentialReferenceId::from(
        credential.credential_id.as_str(),
    ));
    let limits = LinkLimits {
        maximum_in_flight_items: WEBSOCKET.maximum_in_flight_items,
        maximum_payload_bytes: WEBSOCKET.maximum_payload_bytes,
        maximum_buffered_bytes: WEBSOCKET.maximum_buffered_bytes,
        maximum_frame_bytes: WEBSOCKET.maximum_frame_bytes,
    };
    for (direction, offer, source, sink, grant) in [
        (
            "face",
            &evidence.face,
            owner,
            browser,
            &issued.face_grant_id,
        ),
        (
            "return",
            &evidence.returned,
            browser,
            owner,
            &issued.return_grant_id,
        ),
        (
            "interaction",
            &evidence.interaction,
            browser,
            owner,
            &issued.interaction_grant_id,
        ),
    ] {
        if grant.as_str().is_empty()
            || !offer.validate_sign_identity()
            || offer.availability.availability != LineAvailability::Ready
            || offer.line_id.as_str()
                != format!("line/browser-mask/{}/{direction}", binding.as_str())
            || offer.binding.binding_id.as_str()
                != format!("binding/browser-mask/{}/{direction}", binding.as_str())
            || offer.binding.source.host_id != source.host_id
            || offer.binding.source.boot_id != source.boot_id
            || offer.binding.sink.host_id != sink.host_id
            || offer.binding.sink.boot_id != sink.boot_id
            || offer.binding.source.endpoint_id.as_str().is_empty()
            || offer.binding.sink.endpoint_id.as_str().is_empty()
            || offer.binding.source.endpoint_id.as_str()
                != format!("endpoint/{}/{direction}/source", binding.as_str())
            || offer.binding.sink.endpoint_id.as_str()
                != format!("endpoint/{}/{direction}/sink", binding.as_str())
            || offer.availability.sign_id.as_str()
                != format!("sign/browser-mask/{}/{direction}/ready", binding.as_str())
            || offer.binding.base != expected_base
            || offer.binding.base_instance_id != expected_instance
            || offer.binding.credential != expected_credential
            || offer.binding.authority != LinkAuthorityReference::Grant(grant.clone())
            || offer.binding.limits != limits
            || offer.contract != WEBSOCKET.contract
        {
            return Err("browser-line-evidence-mismatch".into());
        }
    }
    let offers = [&evidence.face, &evidence.returned, &evidence.interaction];
    if offers.iter().enumerate().any(|(index, left)| {
        offers.iter().skip(index + 1).any(|right| {
            left.line_id == right.line_id || left.binding.binding_id == right.binding.binding_id
        })
    }) {
        return Err("browser-line-evidence-mismatch".into());
    }
    Ok(())
}
