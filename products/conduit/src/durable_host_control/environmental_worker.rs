//! One bounded native request on the actual installed Host and admitted lease.
use super::*;
use crate::durable_host::selected_environmental_sources::AttachedEquipment;
use std::thread::JoinHandle;
pub(super) struct Worker {
    thread: JoinHandle<(
        StdHost,
        AttachedEquipment,
        conduit_std_host::PreparedNativeWork,
        Result<Vec<u8>, String>,
    )>,
}
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    struct Wake(std::thread::Thread);
    impl std::task::Wake for Wake {
        fn wake(self: std::sync::Arc<Self>) {
            self.0.unpark()
        }
        fn wake_by_ref(self: &std::sync::Arc<Self>) {
            self.0.unpark()
        }
    }
    let waker = std::task::Waker::from(std::sync::Arc::new(Wake(std::thread::current())));
    let mut context = std::task::Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(result) => return result,
            std::task::Poll::Pending => std::thread::park(),
        }
    }
}
impl HostSource {
    fn take_native_worker_host(&mut self) -> Result<StdHost, String> {
        match self {
            Self::Bare(owner) => owner.take_for_play(),
            Self::Body { owner, .. } => owner.host.take_for_play(),
            Self::Transitioning => Err("Host ownership is transitioning".into()),
        }
    }
    fn restore_native_worker_host(&mut self, host: StdHost) -> Result<(), String> {
        match self {
            Self::Bare(owner) => owner.restore_after_play(host),
            Self::Body { owner, .. } => owner.host.restore_after_play(host),
            Self::Transitioning => Err("Host ownership is transitioning".into()),
        }
    }
}
impl DurableHostRuntime {
    pub(super) fn environmental_worker_active(&self) -> bool {
        self.environmental_worker.is_some()
    }
    pub(super) fn start_pending_environmental_source(&mut self) -> Result<(), String> {
        let Some(request) = self.pending_native_source else {
            return Ok(());
        };
        if self.environmental_worker.is_some() {
            return Ok(());
        }
        // Validate every fallible preparation before either affine owner moves.
        let admitted = self
            .remote_fragment
            .as_ref()
            .ok_or("native admitted fragment absent")?;
        self.host.admitted_native_request(admitted, request)?;
        if self.selected_environmental_equipment.is_none() {
            return Err("selected Source equipment absent".into());
        }
        self.selected_environmental_equipment
            .as_ref()
            .expect("validated retained equipment")
            .validate_owner(&self.host)?;
        let work = self.host.prepare_native_work(
            self.remote_fragment
                .as_mut()
                .expect("validated admitted fragment"),
            request,
        )?;
        let host = self.host.take_native_worker_host()?;
        let equipment = self
            .selected_environmental_equipment
            .take()
            .expect("validated retained equipment");
        // pending_native_source and the ordinary admitted/session remain resident.
        // The ordinary thread is finite by the selected provider's admitted IO
        // deadlines. No display polling or additional resource reservation occurs.
        self.environmental_worker = Some(Worker {
            thread: std::thread::spawn(move || {
                let AttachedEquipment {
                    native,
                    instruments,
                    authority_grants,
                } = equipment;
                let (host, native, work, result) =
                    block_on(native.complete_native_work(host, work));
                let equipment = AttachedEquipment {
                    native,
                    instruments,
                    authority_grants,
                };
                (
                    host,
                    equipment,
                    work,
                    result
                        .map_err(|reason| format!("native Source completion refused: {reason:?}")),
                )
            }),
        });
        Ok(())
    }
    pub(super) fn progress_environmental_worker(&mut self) -> Result<(), String> {
        if !self
            .environmental_worker
            .as_ref()
            .is_some_and(|worker| worker.thread.is_finished())
        {
            return Ok(());
        }
        let worker = self
            .environmental_worker
            .take()
            .expect("finished native Source worker");
        let (host, equipment, work, result) = worker
            .thread
            .join()
            .map_err(|_| "native Source worker failed before returning actual Host")?;
        let admitted = self
            .remote_fragment
            .as_mut()
            .ok_or("native work lost its retained ordinary admitted kernel")?;
        let returned = host
            .return_native_work(admitted, work)
            .map_err(|refusal| refusal.reason)?;
        // Actual IO is retired and the ORIGINAL lease is reunited before any
        // kernel completion/output can be observed by another control request.
        self.host.restore_native_worker_host(host)?;
        self.selected_environmental_equipment = Some(equipment);
        returned.complete(admitted, result.as_ref().ok().map(Vec::as_slice))?;
        self.pending_native_source = None;
        Ok(())
    }
}

#[cfg(test)]
#[path = "environmental_worker/tests.rs"]
mod tests;
