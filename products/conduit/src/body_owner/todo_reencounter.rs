//! Re-encounter a retained Todo selection on a fresh installed Host Boot.

use super::Owner;
use conduit_core::{ResourceAccessMode, ResourceContentRequirement, TerminalDisposition};
use conduit_todo_plot::TodoState;
use std::path::Path;

impl Owner {
    /// Recovery evidence is distinct from a current Face's verified read. A
    /// failed command may retain one preceding witness, but only an explicit
    /// matching selection and a fresh admitted read can make it display truth.
    fn retained_todo_read_for_reencounter(&self) -> Option<&serde_json::Value> {
        let receipt = self.last_execution.as_ref()?;
        if let Some(read) = self.todo_verified_read_receipt() {
            return Some(read);
        }
        let read = &receipt["retained_verified_read"];
        (receipt["schema"] == "conduit.todo/next-checkpoint-receipt@1"
            && matches!(
                serde_json::from_value::<TerminalDisposition>(receipt["terminal"].clone()),
                Ok(TerminalDisposition::Failed { .. } | TerminalDisposition::Cancelled { .. })
            )
            && receipt["terminal_sign"]["sign_id"].is_string()
            && receipt["terminal_sign"]["active_play_id"] == receipt["play"]["active_play_id"]
            && receipt["committed_fore_count"] == 0
            && read["schema"] == "conduit.todo/verified-read-receipt@1"
            && read["verified"] == true
            && read["body_id"] == receipt["body_id"]
            && read["write"]["retained_verified_read"].is_null())
        .then_some(read)
    }

    pub(crate) fn has_retained_verified_todo_read(&self) -> bool {
        let Some(read) = self.retained_todo_read_for_reencounter() else {
            return false;
        };
        match self.resident_name.as_deref() {
            Some("todo/checkpoint-restore") => true,
            // An interrupted next write retains the preceding verified read.
            // Re-encounter only an explicit selection of that published version.
            Some("todo/checkpoint-once") => {
                self.host.advertisement().resources.iter().any(|resource| {
                    resource.class_id.as_str() == "resource/todo-checkpoint@1"
                        && resource.content.as_ref().is_some_and(|content| {
                            read["selected_content"]["identity"]
                                == serde_json::json!(content.contract.identity)
                                && read["selected_content"]["version"]
                                    == serde_json::json!(content.contract.version)
                        })
                })
            }
            _ => false,
        }
    }

    pub(super) fn todo_read_resident_matches(&self, fresh_boot: bool) -> bool {
        match self.resident_name.as_deref() {
            Some("todo/checkpoint-restore") => fresh_boot,
            Some("todo/checkpoint-once") => !fresh_boot || self.has_retained_verified_todo_read(),
            _ => false,
        }
    }

    pub(crate) fn has_retained_failed_todo_read(&self) -> bool {
        self.resident_name.as_deref() == Some("todo/checkpoint-restore")
            && self.last_execution.as_ref().is_some_and(|receipt| {
                receipt["schema"] == "conduit.todo/verified-read-receipt@1"
                    && receipt["verified"] == false
            })
    }

    /// A failed post-write read is not a verified Face. A fresh Boot may retry
    /// only the exact selected generation from the retained successful write;
    /// the new Play still has to read and verify it before Todo is projected.
    pub(crate) fn retry_retained_failed_todo_read(
        &mut self,
        state_root: &Path,
        checkpoint_root: &Path,
        selected_write: &ResourceContentRequirement,
        maximum_millis: u64,
    ) -> Result<TodoState, String> {
        let prior = self
            .last_execution
            .as_ref()
            .filter(|receipt| {
                self.has_retained_failed_todo_read()
                    && receipt["body_id"] == self.session.evidence().body_id.as_str()
                    && receipt["read_failure"].is_string()
                    && receipt["restored_fore_sha256"].is_null()
            })
            .ok_or("Todo failed read has no exact retained refusal")?;
        let write = &prior["write"];
        let mut selected_read = selected_write.clone();
        selected_read.access = ResourceAccessMode::ReadPublished;
        selected_read.publication_slots = 0;
        let write_plot_id = crate::plot_source::parse(include_str!(
            "../../../../plots/todo/checkpoint-once.conduit"
        ))?
        .expand_entry_for_authoring()?
        .expanded
        .checked_plot_id;
        if prior["selected_content"] != serde_json::json!(selected_read)
            || (write["schema"] != "conduit.todo/next-checkpoint-receipt@1"
                && write["schema"] != "conduit.todo/first-checkpoint-receipt@1")
            || prior["read_terminal"] == serde_json::json!(TerminalDisposition::Completed)
            || prior["read_play"]["body_id"] != self.session.evidence().body_id.as_str()
            || prior["read_play"]["plan_id"] != prior["read_plan_id"]
            || !prior["read_terminal_sign"]["sign_id"].is_string()
            || prior["read_terminal_sign"]["active_play_id"] != prior["read_play"]["active_play_id"]
            || write["body_id"] != self.session.evidence().body_id.as_str()
            || write["checkpoint_namespace"]["body_id"] != self.session.evidence().body_id.as_str()
            || write["checkpoint_namespace"]["write_plot_id"] != write_plot_id.as_str()
            || write["selected_content"] != serde_json::json!(selected_write)
            || write["terminal"] != serde_json::json!(TerminalDisposition::Completed)
            || !write["failure"].is_null()
            || !write["cleanup_failure"].is_null()
            || write["committed_fore_count"] != 1
            || !write["committed_fore_sha256"]
                .as_str()
                .is_some_and(super::super::super::valid_digest)
            || !write["terminal_sign"]["sign_id"].is_string()
            || write["terminal_sign"]["active_play_id"] != write["play"]["active_play_id"]
            || write["play"]["body_id"] != self.session.evidence().body_id.as_str()
            || write["play"]["plan_id"] != write["plan_id"]
        {
            return Err("Todo failed read differs from exact published write".into());
        }
        let write = write.clone();
        self.read_selected_todo(
            state_root,
            checkpoint_root,
            selected_write,
            None,
            write,
            maximum_millis,
        )
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
            .retained_todo_read_for_reencounter()
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
