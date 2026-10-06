//! Live Host, provider, and Line witnesses for the owner's sealed wardrobe.
use super::{native_mask_route::NativeMaskRoute, Owner};
use conduit_presentation::{CurrentOwnerPresentationRoute, LocalOwnerMaskRouteSeal};

impl Owner {
    pub(crate) fn current_direct_spoken_route<'a>(
        host: &super::OwnerHost,
        cached: Option<&'a LocalOwnerMaskRouteSeal>,
        session: &conduit_body::BodyLifecycleSession,
        face: &conduit_presentation::Presentation,
    ) -> Result<Option<&'a LocalOwnerMaskRouteSeal>, String> {
        let Some(seal) = cached else {
            return Ok(None);
        };
        if host.is_playing() || !host.current().spoken_mask_provider_is_current() {
            return Ok(None);
        }
        let grants = [
            host.current()
                .spoken_mask_artifact_authority_grant("grant/owner-direct-speech/artifact")?,
            host.current().streaming_speech_authority_grant()?,
        ];
        match seal.validate_current_with_grants(session, face, host.advertisement(), &grants) {
            Ok(()) => Ok(Some(seal)),
            Err(
                conduit_presentation::LocalOwnerMaskRouteError::StaleBody
                | conduit_presentation::LocalOwnerMaskRouteError::StaleFace
                | conduit_presentation::LocalOwnerMaskRouteError::StaleHost
                | conduit_presentation::LocalOwnerMaskRouteError::InvalidAuthorityGrant,
            ) => Ok(None),
            Err(error) => Err(format!("direct spoken witness invalid: {error:?}")),
        }
    }

    /// The adapter owns the socket and Show execution; the owner retains only
    /// its exact route witness while that attached provider is still live.
    pub(crate) fn current_attached_terminal_route<'a>(
        host: &super::OwnerHost,
        cached: Option<&'a LocalOwnerMaskRouteSeal>,
        session: &conduit_body::BodyLifecycleSession,
        face: &conduit_presentation::Presentation,
    ) -> Result<Option<&'a LocalOwnerMaskRouteSeal>, String> {
        #[cfg(not(unix))]
        {
            let _ = (host, cached, session, face);
            return Ok(None);
        }
        #[cfg(unix)]
        {
            let Some(seal) = cached else {
                return Ok(None);
            };
            if host.is_playing() || !host.current().terminal_attachment_is_live()? {
                return Ok(None);
            }
            match seal.validate_current(session, face, host.advertisement()) {
                Ok(()) => Ok(Some(seal)),
                Err(
                    conduit_presentation::LocalOwnerMaskRouteError::StaleBody
                    | conduit_presentation::LocalOwnerMaskRouteError::StaleFace
                    | conduit_presentation::LocalOwnerMaskRouteError::StaleHost,
                ) => Ok(None),
                Err(error) => Err(format!("attached terminal witness invalid: {error:?}")),
            }
        }
    }

    pub(crate) fn current_presentation_routes<'a>(
        owner_offer: &'a conduit_core::HostAdvertisement,
        browser: Option<&'a super::participants::BrowserWindow>,
        local: Option<&'a LocalOwnerMaskRouteSeal>,
    ) -> Vec<CurrentOwnerPresentationRoute<'a>> {
        let mut current = Vec::with_capacity(2);
        if let Some(seal) = local {
            current.push(CurrentOwnerPresentationRoute::Local { seal, owner_offer });
        }
        if let Some((seal, mask_host_offer, face_line, return_line, interaction_line)) =
            browser.and_then(super::participants::BrowserWindow::current_mask_route)
        {
            current.push(CurrentOwnerPresentationRoute::Remote {
                seal,
                owner_offer,
                mask_host_offer,
                face_line,
                return_line,
                interaction_line: Some(interaction_line),
            });
        }
        current
    }

    #[cfg(test)]
    pub(super) fn current_presentation_routes_with_native<'a>(
        owner_offer: &'a conduit_core::HostAdvertisement,
        browser: Option<&'a super::participants::BrowserWindow>,
        local: Option<&'a LocalOwnerMaskRouteSeal>,
        native: Option<&'a NativeMaskRoute>,
        session: &conduit_body::BodyLifecycleSession,
        face: &conduit_presentation::Presentation,
        now_millis: u64,
    ) -> Vec<CurrentOwnerPresentationRoute<'a>> {
        Self::current_presentation_routes_with_native_and_speech(
            owner_offer,
            browser,
            local,
            native,
            None,
            session,
            face,
            now_millis,
        )
    }

    pub(super) fn current_presentation_routes_with_native_and_speech<'a>(
        owner_offer: &'a conduit_core::HostAdvertisement,
        browser: Option<&'a super::participants::BrowserWindow>,
        local: Option<&'a LocalOwnerMaskRouteSeal>,
        native: Option<&'a NativeMaskRoute>,
        speech: Option<&'a LocalOwnerMaskRouteSeal>,
        session: &conduit_body::BodyLifecycleSession,
        face: &conduit_presentation::Presentation,
        now_millis: u64,
    ) -> Vec<CurrentOwnerPresentationRoute<'a>> {
        let mut current = Self::current_presentation_routes(owner_offer, browser, local);
        if let Some(seal) = speech {
            current.push(CurrentOwnerPresentationRoute::Local { seal, owner_offer });
        }
        if let Some((seal, mask_host_offer, face_line, return_line)) =
            native.and_then(|route| route.current_witness(session, face, now_millis))
        {
            current.push(CurrentOwnerPresentationRoute::Remote {
                seal,
                owner_offer,
                mask_host_offer,
                face_line,
                return_line,
                interaction_line: None,
            });
        }
        current
    }
}
