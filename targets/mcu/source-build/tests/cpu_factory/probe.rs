//! CPU construction probe of unchanged generated three-fragment factory modules.
//! Native provider methods panic: no IO or execution is permitted by this probe.
#![allow(dead_code, unused_imports)]
extern crate alloc;
mod accounting;
mod authority;
mod factory;
mod payload;
mod config {
    include!(concat!(env!("OUT_DIR"), "/host.rs"));
    pub use EXECUTION as EXECUTION_DELEGATION;
    pub use PREPARATION as PREPARATION_DELEGATION;
    pub use WORK_POOL as RESIDENT_WORK_POOL;
}
mod runtime {
    pub struct MonotonicTimer {
        pub slot: usize,
    }
    impl conduit_monotonic_wait::MonotonicWaitProvider for MonotonicTimer {
        async fn wait(
            &mut self,
            _: u64,
        ) -> Result<(), conduit_monotonic_wait::WaitProviderFailure> {
            panic!("CPU preparation must not wait")
        }
    }
    pub fn remaining() -> usize {
        crate::config::HEAP_BYTES.saturating_sub(allocator_used())
    }
    pub fn allocator_used() -> usize {
        crate::accounting::used()
    }
    pub fn now_ms() -> u64 {
        17
    }
}
mod equipment;
mod renderer;
mod transport {
    pub struct Link;
    impl conduit_mcu_host_runtime::transport::MessageTransport for Link {
        type Error = ();
        async fn receive<'a>(&mut self, _: &'a mut [u8]) -> Result<&'a [u8], ()> {
            panic!("CPU construction probe must not receive")
        }
        async fn send(&mut self, _: &[u8]) -> Result<(), ()> {
            panic!("CPU construction probe must not send")
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
// Execution is deliberately unavailable; this probe invokes constructors only.
// The unchanged factory is included above, rather than copied or weakened.
mod play_run {
    use conduit_mcu_host_runtime::{fore_bridge::ObserverPort, root::ForeIoActor};
    pub enum Carriage<'a, 'b, 'p> {
        Local {
            link: &'a mut crate::transport::Link,
            control: Option<()>,
        },
        Line {
            actor: ForeIoActor<'a, 'b, 'p, crate::transport::Link>,
            observer: ObserverPort<'a, 'b, 'p>,
        },
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn run<A, B, C, D, E, F, G, H, I, J, K, L, M, N, O>(
        _: A,
        _: B,
        _: C,
        _: D,
        _: E,
        _: F,
        _: G,
        _: H,
        _: I,
        _: J,
        _: K,
        _: L,
        _: M,
        _: N,
        _: O,
    ) -> ! {
        panic!("CPU construction probe cannot execute")
    }
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
