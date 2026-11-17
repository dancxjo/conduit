//! CPU qualification of original three-fragment Source setup and execution.
//! Platform effects require original authenticated Start; no peer frames exist.
#![allow(dead_code, unused_imports)]
extern crate alloc;
mod accounting;
mod authority;
mod cpu_time;
mod effects;
mod execution;
mod factory;
mod full_residence;
mod payload;
#[path = "../../../../rp2040/firmware/host/src/phase_trace.rs"]
mod phase_trace;
mod shortage;
mod config {
    include!(concat!(env!("OUT_DIR"), "/host.rs"));
    pub use EXECUTION as EXECUTION_DELEGATION;
    pub use PREPARATION as PREPARATION_DELEGATION;
    pub use WORK_POOL as RESIDENT_WORK_POOL;
}
mod runtime {
    static TRACE: std::sync::Mutex<crate::phase_trace::Trace> =
        std::sync::Mutex::new(crate::phase_trace::Trace::new());
    pub struct MonotonicTimer {
        pub slot: usize,
    }
    impl conduit_monotonic_wait::MonotonicWaitProvider for MonotonicTimer {
        async fn wait(
            &mut self,
            milliseconds: u64,
        ) -> Result<(), conduit_monotonic_wait::WaitProviderFailure> {
            crate::effects::require();
            crate::effects::WAITS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            crate::cpu_time::Timer::after_millis(milliseconds).await;
            Ok(())
        }
    }

    pub fn remaining() -> usize {
        crate::config::HEAP_BYTES.saturating_sub(allocator_used())
    }
    pub fn allocator_used() -> usize {
        crate::accounting::used()
    }
    pub fn allocator_peak() -> Option<usize> {
        Some(crate::accounting::peak())
    }
    pub fn untouched_stack_mark_bytes() -> Option<usize> {
        None
    }
    pub fn now_ms() -> u64 {
        crate::cpu_time::now_ms()
    }
    pub fn stack_checkpoint(phase: u32) {
        TRACE.lock().unwrap().record(
            phase as usize,
            crate::accounting::used(),
            crate::accounting::peak(),
            u32::MAX as usize,
            None,
        );
    }
    pub fn retain_trace_arena(bytes: usize) {
        assert!(TRACE.lock().unwrap().retain_arena(bytes));
    }
    pub fn encode_memory_trace(output: &mut [u8]) -> Option<usize> {
        TRACE.lock().unwrap().encode_into(output)
    }
    pub fn observe_receipt(_: Option<[u8; 32]>) {
        panic!("absent owner cannot return a receipt")
    }
}
mod equipment;
mod renderer;
mod transport {
    pub struct Link {
        failure_after_ms: u64,
        deadline: Option<u64>,
    }
    impl Link {
        pub fn new() -> Self {
            Self {
                failure_after_ms: if std::env::var_os("CPU_FAIL_FORE_READ").is_some() {
                    conduit_mcu_host_runtime::transport::READ_TIMEOUT_MILLIS as u64
                } else {
                    0
                },
                deadline: None,
            }
        }
    }

    impl Link {
        pub async fn connected(&mut self) {}
        pub fn reset(&mut self) {}
        pub async fn receive<'a>(&mut self, _: &'a mut [u8]) -> Result<&'a [u8], ()> {
            crate::effects::require();
            crate::effects::RECEIVES.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if self.failure_after_ms != 0 {
                let now = crate::cpu_time::now_ms();
                let deadline = *self.deadline.get_or_insert(now + self.failure_after_ms);
                if now >= deadline {
                    crate::effects::RECEIVE_FAILURES
                        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    return Err(());
                }
            }
            core::future::pending().await
        }
        pub async fn send(&mut self, _: &[u8]) -> Result<(), ()> {
            crate::effects::require();
            crate::effects::TRANSPORT_SENDS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
    }
    impl conduit_mcu_host_runtime::transport::MessageTransport for Link {
        type Error = ();
        async fn receive<'a>(&mut self, out: &'a mut [u8]) -> Result<&'a [u8], ()> {
            self.receive(out).await
        }
        async fn send(&mut self, bytes: &[u8]) -> Result<(), ()> {
            self.send(bytes).await
        }
    }
    impl conduit_mcu_host_runtime::transport::DocumentTransport for Link {
        async fn receive_document(
            &mut self,
            _: &mut conduit_wire::BorrowedBoundedDocumentAssembly<'_>,
        ) -> Result<(), ()> {
            core::future::pending().await
        }
    }
}

mod native_images {
    include!(concat!(env!("OUT_DIR"), "/native_images.rs"));
}
mod workset {
    pub mod generated {
        include!(concat!(env!("OUT_DIR"), "/workset.rs"));
    }
    pub use generated::*;
    pub mod native_images {
        pub use crate::native_images::*;
    }
    pub mod renderer {
        pub use crate::config::*;
    }
    pub use conduit_mcu_host_runtime::native_values::prepare as values;
}
#[path = "../../../../esp32/firmware/host/src/resident.rs"]
mod resident;
mod play {
    include!(concat!(env!("OUT_DIR"), "/source_play.rs"));
}
mod play_service {
    include!(concat!(env!("OUT_DIR"), "/source_play_service.rs"));
}
// The exact production Source execution, with CPU platform time only.
mod play_run {
    use crate::cpu_time as embassy_time;
    include!(concat!(env!("OUT_DIR"), "/source_play_run.rs"));
}

fn main() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(probe)
        .unwrap()
        .join()
        .unwrap();
}
fn probe() {
    accounting::begin();
    authority::verify_public_admission();
}
