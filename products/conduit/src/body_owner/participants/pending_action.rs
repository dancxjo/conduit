//! One asynchronous owner IPC operation; the carrier keeps its lease reader.
//! Only the owner actor may apply the action and report its exact outcome.
//! A Todo acknowledgement still waits for its durable checkpoint receipt.
use super::{In, Out, PROTOCOL};
use conduit_core::LinkBindingId;
use conduit_face::{FaceInteraction, MaskShow, OwnerFaceSnapshotRequest};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
};

// A disconnected carrier must not free an in-flight worker's resource slot.
// The permit follows the operation until it finishes, including unwind/error.
static ACTIVE_WORKERS: AtomicUsize = AtomicUsize::new(0);
const WORKER_STACK_BYTES: usize = 2 * 1024 * 1024;
struct WorkerPermit<'a>(&'a AtomicUsize);
impl<'a> WorkerPermit<'a> {
    fn acquire(active: &'a AtomicUsize) -> Result<Self, String> {
        active
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                count
                    .checked_add(1)
                    .filter(|next| *next <= super::MAX_CONNECTIONS)
            })
            .map(|_| Self(active))
            .map_err(|_| "browser owner operation capacity exhausted".into())
    }
}
impl Drop for WorkerPermit<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

#[derive(Default)]
pub(super) struct PendingAction {
    result: Option<Receiver<Result<(), String>>>,
    deferred: Option<Box<(In, u32)>>,
}

impl PendingAction {
    pub(super) fn is_pending(&self) -> bool {
        self.result.is_some()
    }

    pub(super) fn begin(
        &mut self,
        state_dir: Option<&Path>,
        window_id: Option<&str>,
        binding: LinkBindingId,
        command: (OwnerFaceSnapshotRequest, Box<MaskShow>, FaceInteraction),
    ) -> Result<(), String> {
        let dir = state_dir
            .ok_or("owner interaction requires the installed service actor")?
            .to_path_buf();
        let window = window_id
            .ok_or("browser-route-window-unavailable")?
            .to_owned();
        self.start_operation(move || {
            let (request, show, interaction) = command;
            crate::durable_host_control::submit_browser_face_interaction(
                &dir,
                &window,
                binding,
                request,
                *show,
                interaction,
            )
            .map(|_| ())
        })
    }

    fn start_operation(
        &mut self,
        operation: impl FnOnce() -> Result<(), String> + Send + 'static,
    ) -> Result<(), String> {
        if self.is_pending() {
            return Err("browser owner operation already pending".into());
        }
        let permit = WorkerPermit::acquire(&ACTIVE_WORKERS)?;
        let (send, receive) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("conduit-browser-owner-action".into())
            .stack_size(WORKER_STACK_BYTES)
            .spawn(move || {
                let _permit = permit;
                // Dropping the carrier never retries or cancels an admitted effect.
                // A lost result remains outcome-unknown to the caller.
                let result = operation();
                let _ = send.send(result);
            })
            .map_err(|_| "browser owner worker unavailable".to_string())?;
        self.result = Some(receive);
        Ok(())
    }

    pub(super) fn take_response(&mut self) -> Option<Out> {
        let result = match self.result.as_ref()?.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Err("control-outcome-unknown".into()),
        };
        self.result = None;
        Some(response(result))
    }

    pub(super) fn defer(&mut self, frame: (In, u32)) -> Result<(), String> {
        if frame.1 == 0 || frame.1 as usize > conduit_face::MAX_OWNER_FACE_RESPONSE_BYTES {
            return Err("browser deferred request byte capacity exhausted".into());
        }
        if self.deferred.is_some() {
            return Err("browser deferred request capacity exhausted".into());
        }
        self.deferred = Some(Box::new(frame));
        Ok(())
    }

    pub(super) fn take_deferred(&mut self) -> Option<(In, u32)> {
        if self.is_pending() {
            None
        } else {
            self.deferred.take().map(|frame| *frame)
        }
    }
}

pub(super) fn response(result: Result<(), String>) -> Out {
    let (accepted, code) = match result {
        Ok(_) => (true, String::new()),
        Err(error) => (
            false,
            if error == "browser owner operation capacity exhausted" {
                "owner-action-capacity"
            } else if error == "browser owner worker unavailable" {
                "owner-action-unavailable"
            } else if error == "control-outcome-unknown" {
                "control-outcome-unknown"
            } else if error == "body-play-active"
                || error.contains("retired Play")
                || error == "clock-play-must-lull"
            {
                "clock-play-must-lull"
            } else if error.contains("StaleFace")
                || error.contains("StaleShow")
                || error.contains("stale owner Mask Show")
            {
                "stale-face-or-show"
            } else if error.contains("browser-mask-show-refused")
                || error.contains("browser-mask-route-not-selected")
                || error.contains("browser-route-carrier-mismatch")
            {
                "mask-route-unavailable"
            } else if error.contains("UnavailableAction") {
                "action-unavailable"
            } else if error.contains("RefusedAction") {
                "action-refused"
            } else if error == "owner-face-credential-not-admitted" {
                "credential-not-admitted"
            } else if error == "owner-face-current-part-unavailable" {
                "current-part-unavailable"
            } else {
                "interaction-refused"
            }
            .into(),
        ),
    };
    Out::FaceInteractionResponse {
        protocol: PROTOCOL,
        accepted,
        code,
    }
}

#[cfg(test)]
mod tests;
