//! Host-owned mutation of finite debugger Watch presentation state.

use super::{PatchbayHtmlServer, ServerError};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DebuggerWatchRequest {
    presentation_id: String,
    presentation_revision: u64,
    watch_revision: u64,
    action: DebuggerWatchAction,
    subject: String,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum DebuggerWatchAction {
    Add,
    Remove,
    Focus,
    ClearHistory,
}

impl PatchbayHtmlServer {
    pub(super) fn apply_debugger_watch(&mut self, body: &[u8]) -> Result<Vec<u8>, ServerError> {
        let request: DebuggerWatchRequest =
            serde_json::from_slice(body).map_err(|_| ServerError::InvalidRequest)?;
        if request.presentation_id != self.snapshot.presentation.identity.as_str()
            || request.presentation_revision != self.snapshot.presentation.revision
        {
            return Err(ServerError::InvalidRequest);
        }
        let debugger = self.snapshot.debugger.clone();
        let watches = self
            .snapshot
            .watches
            .as_mut()
            .ok_or(ServerError::InvalidRequest)?;
        if request.watch_revision != watches.revision {
            return Err(ServerError::InvalidRequest);
        }
        let result = match request.action {
            DebuggerWatchAction::Add => watches.add(&request.subject).and_then(|()| {
                debugger.as_ref().map_or(Ok(()), |debugger| {
                    watches
                        .capture_current(&request.subject, debugger)
                        .map(|_| ())
                })
            }),
            DebuggerWatchAction::Remove => watches.remove(&request.subject).map(|_| ()),
            DebuggerWatchAction::Focus => watches.focus(&request.subject),
            DebuggerWatchAction::ClearHistory => watches.clear_history(&request.subject),
        };
        result.map_err(|error| ServerError::Interaction(format!("debugger Watch: {error:?}")))?;
        self.encoded_snapshot = self.snapshot.encode()?;
        Ok(self.encoded_snapshot.clone())
    }
}
