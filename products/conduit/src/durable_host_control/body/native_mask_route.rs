//! Dispatch an explicit admitted peer Mask selection to the sole Body owner.
use super::*;

impl DurableHostRuntime {
    pub(in crate::durable_host_control) fn owned_body_native_mask_route(
        &mut self,
        receipt: &MemberPresentationBasis,
        face_line: &conduit_core::LineOffer,
        return_line: &conduit_core::LineOffer,
        terminal_interaction_line: Option<&conduit_core::LineOffer>,
        native_interaction_line: Option<&conduit_core::LineOffer>,
        spoken_selection: Option<
            &conduit_std_host::direct_spoken_template_mask::RemoteSpokenTemplateSelection,
        >,
        expires_at_millis: u64,
    ) -> Result<conduit_face::RemoteOwnerMaskRouteSeal, String> {
        let HostSource::Body { owner, .. } = &mut self.host else {
            return Err("installed Host does not own a live Body session".into());
        };
        if native_interaction_line.is_some()
            && (terminal_interaction_line.is_some() || spoken_selection.is_some())
        {
            return Err("native-graphical-selection-conflict".into());
        }
        if let Some(line) = native_interaction_line {
            return owner.seal_graphical_native_mask_route(
                receipt,
                face_line,
                return_line,
                Some(line),
                expires_at_millis,
            );
        }
        if let Some(selection) = spoken_selection {
            if terminal_interaction_line.is_some() {
                return Err("remote-spoken-selection-conflict".into());
            }
            owner.seal_remote_spoken_mask_route(
                receipt,
                face_line,
                return_line,
                expires_at_millis,
                selection,
            )
        } else if let Some(interaction_line) = terminal_interaction_line {
            owner.seal_remote_terminal_mask_route(
                receipt,
                face_line,
                return_line,
                interaction_line,
                expires_at_millis,
            )
        } else {
            owner.seal_native_mask_route(receipt, face_line, return_line, expires_at_millis)
        }
    }
}
