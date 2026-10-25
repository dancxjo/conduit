//! One admitted make worker, cancelled and joined before operation retirement.
//! This is cooperative trusted Host execution. The closure must pass the supplied
//! cancellation flag into all production grants, retain one job across retries,
//! and drain effects before returning. Blocked OS reads are not hard-contained.
use super::*;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread::JoinHandle,
};
#[derive(Default)]
pub(super) struct WorkerSlot {
    started: bool,
    drain_failed: bool,
    cancelled: Option<Arc<AtomicBool>>,
    handle: Option<JoinHandle<()>>,
    rejected: Option<CrecheRejectedProducts>,
}
/// Opaque verified products refused by the active completion boundary. File and
/// birth custody remain available to the owning Host, including after retirement.
pub struct CrecheRejectedProducts {
    reason: String,
    products: crate::creche_production::CrecheProducedImages,
}
impl CrecheRejectedProducts {
    pub fn reason(&self) -> &str {
        &self.reason
    }
    pub fn products(&self) -> &crate::creche_production::CrecheProducedImages {
        &self.products
    }
    pub fn into_products(self) -> crate::creche_production::CrecheProducedImages {
        self.products
    }
}
impl CrecheEncounterController {
    /// Nonblocking retrieval; never joins a worker or claims successful preparation.
    pub fn take_rejected_products(&self) -> Result<Option<CrecheRejectedProducts>, String> {
        match self.channel.worker.try_lock() {
            Ok(mut worker) => Ok(worker.rejected.take()),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Poisoned(_)) => Err("Crèche make worker poisoned".into()),
        }
    }
    /// Attach exactly one worker after canonical review. It consumes that review
    /// internally; a separately taken or changed handoff cannot be substituted.
    /// Success requires the private opaque result of actual image production.
    pub fn start_make_worker<F>(&self, work: F) -> Result<(), String>
    where
        F: FnOnce(
                CrecheReviewedHandoff,
                Arc<AtomicBool>,
            )
                -> Result<crate::creche_production::CrecheProducedImages, CrecheMakeRefusal>
            + Send
            + 'static,
    {
        let mut worker = self
            .channel
            .worker
            .lock()
            .map_err(|_| "Crèche make worker poisoned")?;
        if worker.started {
            return Err("Crèche make worker was already attached".into());
        }
        let handoff = {
            let mut state = self.channel.lock()?;
            if !state.active
                || !state.reviewed
                || state.handoff_taken
                || state.make_refusal.is_some()
                || state.products.is_some()
            {
                return Err("Crèche worker requires its single active canonical review".into());
            }
            let handoff = state
                .handoff
                .take()
                .ok_or("Crèche reviewed handoff absent")?;
            state.handoff_taken = true;
            state.handoff_request = Some(handoff.request);
            handoff
        };
        worker.started = true;
        let cancelled = Arc::new(AtomicBool::new(false));
        worker.cancelled = Some(cancelled.clone());
        let controller = self.clone();
        match std::thread::Builder::new()
            .name("creche-make".into())
            .spawn(move || {
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    work(handoff, cancelled)
                })) {
                    Ok(Ok(products)) => {
                        if let Err((mut reason, products)) = controller.complete_images(products) {
                            let mut end = reason.len().min(256);
                            while !reason.is_char_boundary(end) {
                                end -= 1;
                            }
                            reason.truncate(end);
                            // One worker, one slot. Preserve custody even if a previous
                            // panic poisoned the control mutex; never discard this batch.
                            controller
                                .channel
                                .worker
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .rejected = Some(CrecheRejectedProducts {
                                reason,
                                products: *products,
                            });
                            let _ = controller
                                .classify_make_refusal(CrecheMakeRefusal::ArtifactRejected);
                        }
                    }
                    Ok(Err(reason)) => {
                        let _ = controller.classify_make_refusal(reason);
                    }
                    Err(_) => {
                        controller
                            .channel
                            .worker
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .drain_failed = true;
                        let _ = controller.classify_make_refusal(CrecheMakeRefusal::BuildFailed);
                    }
                }
            }) {
            Ok(handle) => {
                worker.handle = Some(handle);
                Ok(())
            }
            Err(error) => {
                // No worker/effects exist, but this consumed review cannot respawn.
                let _ = self.classify_make_refusal(CrecheMakeRefusal::BuildFailed);
                Err(format!("Crèche worker unavailable: {error}"))
            }
        }
    }
}
impl Channel {
    pub(super) fn revoke_and_drain_worker(&self) -> Result<(), String> {
        let revoked = match self.lock() {
            Ok(mut state) => {
                state.active = false;
                state.command = None;
                state.acknowledged = None;
                Ok(())
            }
            Err(error) => Err(error),
        };
        let handle = {
            let mut worker = self
                .worker
                .lock()
                .map_err(|_| "Crèche make worker poisoned")?;
            if let Some(cancelled) = &worker.cancelled {
                cancelled.store(true, Ordering::Release);
            }
            worker.handle.take()
        };
        // No channel, custody or worker lock is held across join. Returning
        // success means this owned thread has actually terminated, never Drop ACK.
        if matches!(handle.map(|handle| handle.join()), Some(Err(_))) {
            self.worker
                .lock()
                .map_err(|_| "Crèche make worker poisoned")?
                .drain_failed = true;
            return Err("Crèche make worker panicked while draining".into());
        }
        if self
            .worker
            .lock()
            .map_err(|_| "Crèche make worker poisoned")?
            .drain_failed
        {
            return Err("Crèche make worker failed to establish effect drain".into());
        }
        revoked
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::creche_encounter::tests::{act, adapter, plan};
    use conduit_std_host::hosted_creche::CrecheHost;
    #[test]
    fn retirement_cancels_and_joins_single_worker_without_holding_channel_lock() {
        let (mut adapter, controller, custody) = adapter();
        let (host, plan, play) = plan(&adapter);
        let mut owner = CrecheHost::prepare(&mut adapter, &host, &plan, &play).unwrap();
        owner.confirm_started(&host, &play).unwrap();
        owner.begin(owner.request()).unwrap();
        assert!(controller
            .start_make_worker(|_, _| Err(CrecheMakeRefusal::BuildFailed))
            .is_err());
        for (action, value) in [
            ("creche.deploy.next", None),
            ("creche.deploy.next", None),
            ("creche.deploy.review", None),
        ] {
            act(&mut owner, &controller, &custody, action, value);
        }
        let exited = Arc::new(AtomicBool::new(false));
        let observed = exited.clone();
        let callback = controller.clone();
        let (ready, started) = std::sync::mpsc::sync_channel(1);
        controller
            .start_make_worker(move |_, cancelled| {
                ready.send(()).unwrap();
                while !cancelled.load(Ordering::Acquire) {
                    std::thread::park_timeout(std::time::Duration::from_millis(1));
                }
                assert!(!callback.channel.lock().unwrap().active);
                observed.store(true, Ordering::Release);
                Err(CrecheMakeRefusal::BuildFailed)
            })
            .unwrap();
        started
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        assert!(controller
            .start_make_worker(|_, _| Err(CrecheMakeRefusal::BuildFailed))
            .is_err());
        owner.retire().unwrap();
        assert!(exited.load(Ordering::Acquire));
        assert!(controller.channel.worker.lock().unwrap().handle.is_none());
        owner.retire().unwrap();
    }
    #[test]
    fn worker_panic_classifies_failure_without_claiming_effect_drain() {
        let (mut adapter, controller, custody) = adapter();
        let (host, plan, play) = plan(&adapter);
        let mut owner = CrecheHost::prepare(&mut adapter, &host, &plan, &play).unwrap();
        owner.confirm_started(&host, &play).unwrap();
        owner.begin(owner.request()).unwrap();
        for action in [
            "creche.deploy.next",
            "creche.deploy.next",
            "creche.deploy.review",
        ] {
            act(&mut owner, &controller, &custody, action, None);
        }
        assert!(custody.lock().unwrap().has_show());
        controller
            .start_make_worker(|_, _| panic!("fixture worker effect drain unknown"))
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            std::thread::sleep(owner.poll_wait());
            if let CrechePoll::Outcome(outcome) = owner.poll().unwrap() {
                assert_eq!(outcome.disposition, CrecheDisposition::Refused);
                break;
            }
            assert!(std::time::Instant::now() < deadline);
        }
        assert!(owner.retire().is_err());
        assert!(
            !custody.lock().unwrap().has_show(),
            "Mask custody still drains when make effects are uncertain"
        );
        assert!(controller.channel.worker.lock().unwrap().handle.is_none());
        assert!(
            owner.retire().is_err(),
            "unknown effect drain remains refused"
        );
    }
}
