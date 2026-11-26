//! Larger response eligibility follows an exact current admitted Mask route.
use super::*;

impl Owner {
    pub(crate) fn browser_face_response_bytes(
        &self,
        request: &OwnerFaceSnapshotRequest,
        face: &conduit_face::Face,
    ) -> Option<usize> {
        self.validate_face_request(request).ok()?;
        let window = self.pending_browser.as_ref()?;
        let WindowState::Active { credential, .. } = &window.state else {
            return None;
        };
        if request.credential_id != credential.credential_id.as_str()
            || request.body_id != credential.body_id
            || request.part_id != credential.part_id
            || request.host_id != credential.host_id
            || request.boot_id != credential.boot_id
        {
            return None;
        }
        let (route, browser, face_line, returned, interaction) = window.current_mask_route()?;
        if !conduit_browser_mask_offer::full_profile::selected(&route.planned_mask)
            || !browser
                .capabilities
                .contains(&conduit_browser_mask_offer::full_profile::offer())
            || face_line.binding.limits.maximum_payload_bytes
                != conduit_host_browser_make::OWNER_FACE_FULL_LINE.maximum_payload_bytes
        {
            return None;
        }
        route
            .validate_current_with_interaction(
                &self.session,
                face,
                self.host.advertisement(),
                browser,
                face_line,
                returned,
                interaction,
            )
            .ok()?;
        Some(conduit_host_browser_make::OWNER_FACE_FULL_MAXIMUM_RESPONSE_BYTES as usize)
    }
}
