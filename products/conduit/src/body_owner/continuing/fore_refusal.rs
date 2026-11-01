//! Retain rejected live Fore through worker quiescence; never record or retire it.
use super::native::LiveFore;
use std::sync::mpsc::{Receiver, TryRecvError};

pub(super) struct ForeRefusal {
    first: LiveFore,
    error: String,
    trailing: Option<LiveFore>,
}
impl ForeRefusal {
    pub(super) fn new(first: LiveFore, error: String) -> Self {
        Self {
            first,
            error,
            trailing: None,
        }
    }
    /// Call only after the producer has actually joined. The original channel
    /// is sync_channel(1), so at most one trailing value can remain. An anomalous
    /// extra value is returned affinely, never silently discarded or acknowledged.
    pub(super) fn capture_trailing(
        &mut self,
        receiver: &Receiver<LiveFore>,
    ) -> Result<(), LiveFore> {
        match receiver.try_recv() {
            Ok(value) => {
                if self.trailing.is_some() {
                    return Err(value);
                }
                self.trailing = Some(value);
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => return Ok(()),
        }
        match receiver.try_recv() {
            Ok(extra) => Err(extra),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => Ok(()),
        }
    }
    pub(super) fn diagnostics(&self) -> serde_json::Value {
        serde_json::json!({
            "custody":"rejected-typed-fore-not-recorded",
            "error":self.error,
            "durable_recording_acknowledged":false,
            "producer_retirement_authorized":false,
            "first":document(&self.first),
            "trailing":self.trailing.as_ref().map(document),
        })
    }
}
fn document(fore: &LiveFore) -> serde_json::Value {
    let length = fore.length.min(fore.bytes.len());
    let canonical: String = fore.bytes[..length]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    serde_json::json!({
        "sequence":fore.sequence,
        "canonical_bytes":canonical,
        "canonical_length":fore.length,
        "extent_valid":fore.length <= fore.bytes.len(),
        "source_selection":fore.scope.selection,
        "native_play":fore.scope.native_play,
    })
}
#[cfg(test)]
#[path = "fore_refusal/tests.rs"]
mod tests;
