//! Preserve the explicitly selected presentation through each finite return.
use super::{Grant, MaskSelection};
use conduit_core::HostAdvertisement;
use conduit_face::RemoteOwnerMaskRouteSeal;
use conduit_std_host::secure_websocket::SecureWebSocketListener;
use std::path::Path;

impl Grant {
    pub(crate) fn seal_mask_route(
        &self,
        listener: &SecureWebSocketListener,
        state_dir: &Path,
        owner: &HostAdvertisement,
        sequence: u8,
    ) -> Result<RemoteOwnerMaskRouteSeal, String> {
        let binding = super::super::native_lines::binding_reference(
            listener,
            &self.binding_reference(sequence),
        );
        let (face, show, interaction) = match self.mask {
            MaskSelection::Native => {
                let (face, show) =
                    super::super::native_lines::offer_pair(owner, &self.receipt, &binding);
                (face, show, None)
            }
            MaskSelection::Terminal => {
                let (face, show, interaction) =
                    super::super::native_lines::offer_terminal(owner, &self.receipt, &binding);
                (face, show, Some(interaction))
            }
        };
        crate::durable_host_control::select_native_mask_route(
            state_dir,
            self.receipt.clone(),
            face,
            show,
            interaction,
            self.expires_at_millis,
        )
    }
}
