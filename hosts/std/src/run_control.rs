use conduit_core::ActivePlayId;
use std::sync::{Arc, Mutex};

const MAXIMUM_CONTROL_REQUEST_ID_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunControlRequestId(String);

impl RunControlRequestId {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if value.is_empty() || value.len() > MAXIMUM_CONTROL_REQUEST_ID_BYTES {
            return Err("run control request identity must contain 1..=128 bytes".into());
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunControlDisposition {
    Accepted,
    RejectedAlreadyRequested,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunControlReceipt {
    pub request_id: RunControlRequestId,
    pub active_play_id: ActivePlayId,
    pub disposition: RunControlDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedRunControlRequest {
    pub request_id: RunControlRequestId,
    pub disposition: RunControlDisposition,
}

#[derive(Debug, Clone, Default)]
pub struct RunControl {
    requested: Arc<Mutex<Option<RunControlRequestId>>>,
}

impl RunControl {
    pub fn request_stop(
        &self,
        request_id: RunControlRequestId,
    ) -> Result<(), RejectedRunControlRequest> {
        let mut requested = self.requested.lock().expect("run control lock poisoned");
        if requested.is_some() {
            return Err(RejectedRunControlRequest {
                request_id,
                disposition: RunControlDisposition::RejectedAlreadyRequested,
            });
        }
        *requested = Some(request_id);
        Ok(())
    }

    pub(crate) fn requested_stop(&self) -> Option<RunControlRequestId> {
        self.requested
            .lock()
            .expect("run control lock poisoned")
            .clone()
    }
}
