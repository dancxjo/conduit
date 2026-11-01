//! Retain rejected live Fore through worker quiescence; never record or retire it.
use super::native::LiveFore;
use std::sync::mpsc::Receiver;

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
    /// Call exactly once after the actual producer has joined. The receiver
    /// must be the original sync_channel(1), so at most one trailing value
    /// remains. This helper never probes beyond that original finite slot.
    pub(super) fn capture_trailing(&mut self, receiver: &Receiver<LiveFore>) {
        if self.trailing.is_none() {
            if let Ok(value) = receiver.try_recv() {
                self.trailing = Some(value);
            }
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
        "canonical_hex":canonical,
        "canonical_length":fore.length,
        "extent_valid":fore.length <= fore.bytes.len(),
        "source_selection":fore.scope.selection,
        "native_play":fore.scope.native_play,
    })
}
#[cfg(test)]
#[path = "fore_refusal/tests.rs"]
mod tests;
