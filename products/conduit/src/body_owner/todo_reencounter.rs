//! Re-encounter a retained Todo selection on a fresh installed Host Boot.

use super::Owner;
use conduit_core::{ResourceContentRequirement, TerminalDisposition};
use conduit_todo_plot::TodoState;
use std::path::Path;

impl Owner {
    pub(crate) fn has_retained_verified_todo_read(&self) -> bool {
        self.resident_name.as_deref() == Some("todo/checkpoint-restore")
            && self.todo_verified_read_receipt().is_some()
    }

    /// A fresh Boot has no valid display cache or current Play. The retained
    /// receipt identifies the write, but only a new admitted Host read may
    /// recover its exact content for this Boot's Face.
    pub(crate) fn reencounter_committed_todo(
        &mut self,
        state_root: &Path,
        checkpoint_root: &Path,
        selected_write: &ResourceContentRequirement,
        maximum_millis: u64,
    ) -> Result<TodoState, String> {
        let prior = self
            .todo_verified_read_receipt()
            .ok_or("Todo re-encounter has no verified prior read")?;
        if prior["body_id"] != self.session.evidence().body_id.as_str()
            || prior["selected_content"]["identity"] != serde_json::json!(selected_write.identity)
            || prior["selected_content"]["version"] != serde_json::json!(selected_write.version)
            || prior["read_terminal"] != serde_json::json!(TerminalDisposition::Completed)
            || !prior["read_terminal_sign"]["sign_id"].is_string()
            || prior["read_terminal_sign"]["active_play_id"] != prior["read_play"]["active_play_id"]
            || prior["restored_fore_sha256"] != prior["write"]["committed_fore_sha256"]
            || prior["read_failure"] != serde_json::Value::Null
            || prior["read_cleanup_failure"] != serde_json::Value::Null
        {
            return Err("Todo re-encounter selection differs from retained read".into());
        }
        let write_receipt = prior["write"].clone();
        self.read_selected_todo(
            state_root,
            checkpoint_root,
            selected_write,
            None,
            write_receipt,
            maximum_millis,
        )
    }
}
