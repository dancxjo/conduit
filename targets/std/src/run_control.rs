use conduit_core::ActivePlayId;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

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
    state: Arc<(Mutex<RunControlState>, Condvar)>,
}

#[derive(Debug, Default)]
struct RunControlState {
    requested: Option<RunControlRequestId>,
    accepted: bool,
    quiescent: bool,
    activity_generation: u64,
}

impl RunControl {
    pub fn request_stop(
        &self,
        request_id: RunControlRequestId,
    ) -> Result<(), RejectedRunControlRequest> {
        let mut state = self.state.0.lock().expect("run control lock poisoned");
        if state.requested.is_some() || state.accepted {
            return Err(RejectedRunControlRequest {
                request_id,
                disposition: RunControlDisposition::RejectedAlreadyRequested,
            });
        }
        state.requested = Some(request_id);
        self.state.1.notify_all();
        Ok(())
    }

    pub(crate) fn same_source(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.state, &other.state)
    }

    /// Wakes a live Fore runner after a bounded action or close is admitted.
    pub(crate) fn signal_activity(&self) {
        let mut state = self.state.0.lock().expect("run control lock poisoned");
        // At most the admitted finite Fore items, close, and terminal signals
        // occur in a Play; wrapping cannot alias a still-waiting observation.
        state.activity_generation = state.activity_generation.wrapping_add(1);
        self.state.1.notify_all();
    }

    pub(crate) fn activity_generation(&self) -> u64 {
        self.state
            .0
            .lock()
            .expect("run control lock poisoned")
            .activity_generation
    }

    pub(crate) fn wait_for_activity_or_stop(&self, observed: u64) {
        let state = self.state.0.lock().expect("run control lock poisoned");
        drop(
            self.state
                .1
                .wait_while(state, |state| {
                    state.activity_generation == observed
                        && state.requested.is_none()
                        && !state.accepted
                })
                .expect("run control lock poisoned"),
        );
    }

    /// Host-local safety bound for a waiting one-command Fore. This does not
    /// invent a semantic deadline or retry an effect.
    pub(crate) fn wait_for_activity_or_stop_for(&self, observed: u64, timeout: Duration) -> bool {
        let state = self.state.0.lock().expect("run control lock poisoned");
        let (state, result) = self
            .state
            .1
            .wait_timeout_while(state, timeout, |state| {
                state.activity_generation == observed
                    && state.requested.is_none()
                    && !state.accepted
            })
            .expect("run control lock poisoned");
        result.timed_out()
            && state.activity_generation == observed
            && state.requested.is_none()
            && !state.accepted
    }

    /// Observe cancellation during a Host effect without consuming the exact
    /// request that the runner must acknowledge in its lifecycle evidence.
    pub fn stop_requested(&self) -> bool {
        let state = self.state.0.lock().expect("run control lock poisoned");
        state.requested.is_some() || state.accepted
    }

    pub(crate) fn requested_stop(&self) -> Option<RunControlRequestId> {
        let mut state = self.state.0.lock().expect("run control lock poisoned");
        let requested = state.requested.take();
        if requested.is_some() {
            state.accepted = true;
        }
        requested
    }

    pub(crate) fn mark_quiescent(&self) {
        let mut state = self.state.0.lock().expect("run control lock poisoned");
        state.quiescent = true;
        self.state.1.notify_all();
    }

    /// Wait for exact runner evidence that this play reached quiescence.
    pub fn wait_until_quiescent(&self, timeout: Duration) -> bool {
        let state = self.state.0.lock().expect("run control lock poisoned");
        if state.quiescent {
            return true;
        }
        self.state
            .1
            .wait_timeout_while(state, timeout, |state| !state.quiescent)
            .expect("run control lock poisoned")
            .0
            .quiescent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepted_stop_identity_is_taken_once_and_still_rejects_duplicates() {
        let control = RunControl::default();
        control
            .request_stop(RunControlRequestId::new("first").unwrap())
            .unwrap();

        assert!(control.stop_requested());
        assert!(
            control.stop_requested(),
            "Host observation must not consume the request"
        );
        assert_eq!(control.requested_stop().unwrap().as_str(), "first");
        assert!(control.stop_requested());
        assert_eq!(control.requested_stop(), None);
        assert_eq!(
            control
                .request_stop(RunControlRequestId::new("second").unwrap())
                .unwrap_err()
                .disposition,
            RunControlDisposition::RejectedAlreadyRequested
        );
    }

    #[test]
    fn activity_between_observation_and_wait_is_not_lost() {
        let control = RunControl::default();
        let observed = control.activity_generation();
        control.signal_activity();
        control.wait_for_activity_or_stop(observed);
        assert_ne!(control.activity_generation(), observed);
    }

    #[test]
    fn stop_notifies_an_idle_waiter_without_polling() {
        let control = RunControl::default();
        let observed = control.activity_generation();
        let (ready, listening) = std::sync::mpsc::channel();
        let (done, finished) = std::sync::mpsc::channel();
        let waiting = control.clone();
        let thread = std::thread::spawn(move || {
            ready.send(()).unwrap();
            waiting.wait_for_activity_or_stop(observed);
            done.send(()).unwrap();
        });
        listening.recv().unwrap();
        control
            .request_stop(RunControlRequestId::new("stop-waiter").unwrap())
            .unwrap();
        finished.recv_timeout(Duration::from_secs(1)).unwrap();
        thread.join().unwrap();
    }
}
