//! Shared browser/native admission pressure handling, without changing grants.
use super::{state, Owner};
use std::path::Path;

impl Owner {
    pub(super) fn retain_admission_capacity(&mut self, root: &Path) -> Result<(), String> {
        let Some(manager) = self.admissions.as_ref() else {
            return Ok(());
        };
        if manager.receipts.len() < conduit_body::MAX_ADMISSION_RECEIPTS {
            return Ok(());
        }
        let next = state::retire_admissions(
            root,
            &mut self.session,
            self.last_execution.as_ref(),
            manager,
        )?;
        self.admissions = Some(next);
        Ok(())
    }
}
