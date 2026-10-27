//! Bounded input waiting and the selected profile's admitted idle disposition.
use super::*;

impl BodyLiveForeQueue {
    pub(crate) fn wait_if_empty(&self, observed: u64) -> Result<(), String> {
        let state = self.state.lock().expect("live Fore queue lock poisoned");
        if !state.queued.is_empty() || state.close_requested {
            return Ok(());
        }
        let remaining = self.wait_timeout.map(|timeout| {
            timeout.saturating_sub(state.started_at.expect("waiting Play started").elapsed())
        });
        drop(state);
        if let Some(remaining) = remaining {
            let timed_out = self
                .control
                .wait_for_activity_or_stop_for(observed, remaining);
            if timed_out {
                let mut state = self.state.lock().expect("live Fore queue lock poisoned");
                if state.queued.is_empty()
                    && !state.close_requested
                    && !self.control.stop_requested()
                {
                    if let Some(request) = state.idle_stop.take() {
                        drop(state);
                        // The exact document profile pre-admitted this bounded
                        // control request. Kernel cancellation owns its terminal.
                        let _ = self.control.request_stop(request);
                        return Ok(());
                    }
                    return Err("waiting checkpoint command timed out without an effect".into());
                }
            }
        } else {
            self.control.wait_for_activity_or_stop(observed);
        }
        Ok(())
    }
}
