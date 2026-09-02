//! Host-owned mutation of finite debugger playback state.

use super::{PatchbayHtmlServer, ServerError};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DebuggerTimelineRequest {
    presentation_id: String,
    presentation_revision: u64,
    timeline_revision: u64,
    action: DebuggerTimelineAction,
    #[serde(default)]
    index: Option<usize>,
    #[serde(default)]
    subject: Option<String>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum DebuggerTimelineAction {
    Pause,
    Previous,
    Next,
    JumpLive,
    SelectEvent,
    FilterSubject,
    ClearFilter,
}

impl PatchbayHtmlServer {
    pub(super) fn apply_debugger_timeline(&mut self, body: &[u8]) -> Result<Vec<u8>, ServerError> {
        let request: DebuggerTimelineRequest =
            serde_json::from_slice(body).map_err(|_| ServerError::InvalidRequest)?;
        if request.presentation_id != self.snapshot.presentation.identity.as_str()
            || request.presentation_revision != self.snapshot.presentation.revision
        {
            return Err(ServerError::InvalidRequest);
        }
        let timeline = self
            .snapshot
            .timeline
            .as_mut()
            .ok_or(ServerError::InvalidRequest)?;
        if request.timeline_revision != timeline.revision {
            return Err(ServerError::InvalidRequest);
        }
        let result = match request.action {
            DebuggerTimelineAction::Pause => {
                timeline.pause();
                Ok(())
            }
            DebuggerTimelineAction::Previous => timeline.previous_event(),
            DebuggerTimelineAction::Next => timeline.next_event(),
            DebuggerTimelineAction::JumpLive => {
                timeline.jump_live();
                Ok(())
            }
            DebuggerTimelineAction::SelectEvent => timeline
                .select_event(request.index.ok_or(ServerError::InvalidRequest)?)
                .map(|_| ()),
            DebuggerTimelineAction::FilterSubject => timeline.filter_subject(Some(
                request
                    .subject
                    .as_deref()
                    .ok_or(ServerError::InvalidRequest)?,
            )),
            DebuggerTimelineAction::ClearFilter => timeline.filter_subject(None),
        };
        result
            .map_err(|error| ServerError::Interaction(format!("debugger timeline: {error:?}")))?;
        self.snapshot.timeline_projection = Some(timeline.project(self.snapshot.watches.as_ref()));
        self.encoded_snapshot = self.snapshot.encode()?;
        Ok(self.encoded_snapshot.clone())
    }
}
