//! Acknowledged configuration intent at the actual returned-owner boundary.
//! Admission/evidence/workspace guards remain in the parent before this call.
use super::NativeRoot;
impl NativeRoot {
    pub(super) fn accept_acknowledged_network_configuration(
        &mut self,
        now: u64,
    ) -> Result<(), &'static [u8]> {
        if self.network.recovery.accepts_configuration() {
            let acknowledged = self
                .credential_writer
                .try_lock()
                .is_ok_and(|mailbox| mailbox.has_acknowledged());
            // New acknowledged AP input wins over an expired cooldown.
            if acknowledged
                || crate::native_credentials::committed_revision()
                    > self.network.recovery.revision()
            {
                if self.credentials.is_none() || self.network.return_unselected_ready().is_err() {
                    return Err(b"configuration-owner-pending");
                }
                let configuration =
                    match crate::native_credentials::prepare_configuration(&mut self.credentials) {
                        Ok(configuration) => configuration,
                        Err("credential-unavailable" | "credential-not-acknowledged") => {
                            return Err(b"waiting");
                        }
                        Err(_) => return Err(b"refused"),
                    };
                let revision = match crate::native_credentials::committed_revision() {
                    0 => match self.network.recovery.revision().checked_add(1) {
                        Some(revision) => revision,
                        None => return Err(b"configuration-exhausted"),
                    },
                    revision => revision,
                };
                if self
                    .network
                    .stage_unselected_configuration(configuration)
                    .is_err()
                {
                    return Err(b"refused");
                }
                if self.network.recovery.configuration(revision, now).is_err() {
                    return Err(b"configuration-refused");
                }
            }
        }
        Ok(())
    }
}
