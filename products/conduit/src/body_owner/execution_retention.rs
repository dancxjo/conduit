//! Keep exact terminal evidence in the owner even if durable publication fails.
use super::Owner;
use conduit_body::BodyLifecycleSession;
use std::path::Path;

impl Owner {
    pub(super) fn ensure_execution_retained(&mut self, root: &Path) -> Result<(), String> {
        if self.execution_persistence_pending {
            self.persist(root)?;
        }
        Ok(())
    }

    pub(super) fn retain_execution(
        &mut self,
        root: &Path,
        next: BodyLifecycleSession,
        receipt: serde_json::Value,
    ) -> Result<(), String> {
        // A failed write is not permission to replay. Subsequent preparation
        // must persist this pending exact result before starting another Play.
        self.execution_persistence_pending = true;
        self.todo_live = None;
        self.thermostat_live = None;
        self.todo_verified = None;
        self.session = next;
        self.last_execution = Some(receipt);
        self.persist(root)
    }
}
