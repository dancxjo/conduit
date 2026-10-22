//! Admission of finite presentation snapshots from the independent Mask controller.
use super::*;
use conduit_core::{BootId, HostId};
use conduitos_mask_core::control::{Operation, Scope as ControlScope, View};
impl Owner {
    pub(crate) fn control(
        &mut self,
        host: &HostId,
        boot: &BootId,
        operation: Operation,
    ) -> Result<(bool, View), Refusal> {
        let result = (|| {
            let scope = ControlScope {
                session: self.service()?.instance(),
                host: host.clone(),
                boot: boot.clone(),
            };
            let command = Command::Control {
                expected: self.control_revision,
                scope: scope.clone(),
                operation,
            };
            let bytes = self.service()?.command(&command)?;
            let Reply::Control { changed, snapshot } = data::decode(&bytes)? else {
                return Err(Refusal::Malformed);
            };
            let expected = if changed {
                self.control_revision
                    .checked_add(1)
                    .ok_or(Refusal::Capacity)?
            } else {
                self.control_revision
            };
            if snapshot.scope != scope
                || snapshot.revision != expected
                || snapshot.view.details_page >= 16
                || changed != operation.changes()?
            {
                return Err(Refusal::Malformed);
            }
            self.control_revision = expected;
            Ok((changed, snapshot.view))
        })();
        if result.is_err() {
            self.terminal();
        }
        result
    }
}
