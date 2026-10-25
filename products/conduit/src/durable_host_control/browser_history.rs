//! Browser history selection remains on the serialized authenticated Body owner.
use super::*;
impl DurableHostRuntime {
    pub(super) fn owned_body_browser_history_mask(
        &mut self,
        window: &str,
        binding: &LinkBindingId,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
        look: &[u8],
    ) -> Result<RemoteOwnerMaskRouteSeal, String> {
        let HostSource::Body { owner, .. } = &mut self.host else {
            return Err("installed Host does not own a live Body session".into());
        };
        owner.select_browser_history_mask(window, binding, request, show, look)
    }
}
