//! One bounded native request on the actual installed Host and admitted lease.
use super::*;
use crate::durable_host::selected_environmental_sources::AttachedEquipment;
use std::thread::JoinHandle;
pub(super) struct Worker {
    thread: JoinHandle<(
        StdHost,
        AttachedEquipment,
        AdmittedRemoteFragment,
        Result<bool, String>,
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
        let host = self.host.take_native_worker_host()?;
        let equipment = self
            .selected_environmental_equipment
            .take()
            .expect("validated retained equipment");
        let admitted = self
            .remote_fragment
            .take()
            .expect("validated admitted fragment");
        self.pending_native_source = None;
        // The ordinary thread is finite by the selected provider's admitted IO
        // deadlines. No display polling or additional resource reservation occurs.
        self.environmental_worker = Some(Worker {
            thread: std::thread::spawn(move || {
                let AttachedEquipment {
                    native,
                    instruments,
                    authority_grants,
                } = equipment;
                let (host, native, admitted, result) =
                    block_on(native.complete_admitted_request(host, admitted, request));
                let equipment = AttachedEquipment {
                    native,
                    instruments,
                    authority_grants,
                };
                (
                    host,
                    equipment,
                    admitted,
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
        let (host, equipment, mut admitted, result) = worker
            .thread
            .join()
            .map_err(|_| "native Source worker failed before returning actual Host")?;
        self.host.restore_native_worker_host(host)?;
        self.selected_environmental_equipment = Some(equipment);
        if result.is_err() {
            admitted.cancel()?;
        }
        self.remote_fragment = Some(admitted);
        Ok(())
    }
}
