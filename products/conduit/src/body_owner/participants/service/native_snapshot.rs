//! Atomic read-only snapshot authorization on an existing active exact window.
use super::*;
use conduit_face::{Face, OwnerFaceSnapshotRequest};
impl Owner {
    pub(crate) fn native_snapshot_in_window(
        &self,
        window_id: &str,
        binding: &LinkBindingId,
        request: &OwnerFaceSnapshotRequest,
    ) -> Result<Face, String> {
        let window = self
            .pending_browser
            .as_ref()
            .ok_or("native snapshot window unavailable")?;
        window.check(window_id)?;
        // Browser presence has its own longer lifetime after admission. This
        // finite one-shot native encounter never inherits that extension.
        if Instant::now() >= window.deadline {
            return Err("native snapshot original window expired".into());
        }
        let WindowState::Active {
            credential,
            observation,
            line_authorization,
            ..
        } = &window.state
        else {
            return Err("native snapshot window is not active".into());
        };
        if !matches!(window.scope, BrowserWindowScope::ExactHost)
            || observation.observed_binding_id != *binding
            || line_authorization.carrier_binding != *binding
            || line_authorization.window_id != window_id
            || line_authorization.credential_id != credential.credential_id.as_str()
            || request.credential_id != credential.credential_id.as_str()
            || request.body_id != credential.body_id
            || request.part_id != credential.part_id
            || request.host_id != credential.host_id
            || request.boot_id != credential.boot_id
        {
            return Err("native snapshot exact carrier or credential mismatch".into());
        }
        // Existing Face validation checks current Part, latest credential and
        // current owner Body basis. No Mask route is selected by this read.
        self.face_snapshot(request)
    }
}
