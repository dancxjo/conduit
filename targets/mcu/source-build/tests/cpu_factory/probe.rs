//! CPU construction probe of unchanged generated three-fragment factory modules.
//! Native provider methods panic: no IO or execution is permitted by this probe.
#![allow(dead_code, unused_imports)]
extern crate alloc;
mod authority;
mod config {
    include!(concat!(env!("OUT_DIR"), "/host.rs"));
    pub use EXECUTION as EXECUTION_DELEGATION;
    pub use PREPARATION as PREPARATION_DELEGATION;
    pub use WORK_POOL as RESIDENT_WORK_POOL;
}
mod runtime {
    pub struct MonotonicTimer;
    impl conduit_monotonic_wait::MonotonicWaitProvider for MonotonicTimer {
        async fn wait(
            &mut self,
            _: u64,
        ) -> Result<(), conduit_monotonic_wait::WaitProviderFailure> {
            panic!("CPU preparation must not wait")
        }
    }
    pub fn remaining() -> usize {
        panic!("whole-factory allocator funding is not installed in this constructor-only probe")
    }
    pub fn now_ms() -> u64 {
        17
    }
}
mod equipment {
    pub const WAIT_SLOTS: usize = crate::workset::sampling::CLOCKS.len();
    pub use crate::runtime::MonotonicTimer;
    pub struct Capture;
    impl conduit_pulse_capture::owner::PulseReadProvider for Capture {
        async fn read(
            &mut self,
            _: conduit_pulse_capture::CheckedPulseRead,
        ) -> Result<conduit_pulse_capture::PulseReadCapture, conduit_pulse_capture::PulseReadRefusal>
        {
            panic!("CPU preparation must not capture")
        }
    }
    pub struct Wire;
    impl conduit_tm1637_wire::OpenDrain for Wire {
        type Error = ();
        fn clock(&mut self, _: bool) -> Result<(), ()> {
            panic!("CPU preparation must not drive GPIO")
        }
        fn data(&mut self, _: bool) -> Result<(), ()> {
            panic!("CPU preparation must not drive GPIO")
        }
        fn read_data(&mut self) -> Result<bool, ()> {
            panic!("CPU preparation must not read GPIO")
        }
        fn half_period(&mut self) -> Result<(), ()> {
            panic!("CPU preparation must not delay")
        }
    }
    pub struct RecordingStorage;
    impl conduit_resource_snapshot::nor::NorStorage for RecordingStorage {
        fn capacity(&self) -> u32 {
            0
        }
        fn program_unit(&self) -> u32 {
            256
        }
        fn read(
            &mut self,
            _: u32,
            _: &mut [u8],
        ) -> Result<(), conduit_resource_snapshot::nor::NorRefusal> {
            panic!("CPU construction probe must not read media")
        }
        fn program(
            &mut self,
            _: u32,
            _: &[u8],
        ) -> Result<(), conduit_resource_snapshot::nor::NorRefusal> {
            panic!("CPU construction probe must not program media")
        }
    }
    pub type RecordingReady =
        conduit_resource_snapshot::native_service::ReadyRecordService<RecordingStorage>;
    pub struct Equipment {
        pub activity: conduit_mcu_host_runtime::native_runner::NativeActivity,
        pub recording:
            conduit_mcu_host_runtime::recording_service_equipment::Equipment<RecordingStorage>,
        pub renderer:
            conduit_mcu_host_runtime::execution_mask::ReadyRenderer<crate::renderer::Clock>,
        pub pulse: Option<conduit_pulse_capture::installation::ReadyPulseBase<Capture>>,
        pub waits:
            [Option<conduit_monotonic_wait::ReadyMonotonicWaitBase<MonotonicTimer>>; WAIT_SLOTS],
        pub display: Option<conduit_tm1637_wire::installation::ReadyWriteBase<Wire>>,
    }
}
mod renderer {
    pub struct Clock;
    impl conduit_mcu_host_runtime::execution_mask::RendererClock for Clock {
        fn now_ms(&self) -> u64 {
            17
        }
        fn retained_backing_bytes(&self) -> Option<usize> {
            Some(0)
        }
    }
}
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
    authority::verify_public_admission();
    assert_eq!(
        workset::SOURCE_SHA256,
        [
            236, 27, 4, 16, 168, 202, 35, 94, 11, 57, 70, 37, 238, 222, 112, 192, 212, 103, 203,
            25, 51, 192, 35, 196, 248, 17, 185, 109, 185, 229, 246, 200
        ]
    );
    assert_eq!(workset::ENTRY_COUNT, 3);
    let residents = resident::inventory().expect("actual a422 resident constructors");
    let clocks: &[(u16, u64, u64); equipment::WAIT_SLOTS] =
        workset::sampling::CLOCKS.try_into().unwrap();
    let (values, parameters) = workset::values(
        workset::sampling_layout::CORD_VALUE_SLOT_CAPACITIES,
        workset::sampling_layout::STEP_VALUE_SLOT_CAPACITIES,
        workset::sampling_layout::HOST_CALL_VALUE_SLOT_CAPACITIES,
        clocks,
        config::HEAP_BYTES,
    )
    .expect("original sampling value constructor");
    let (output_values, _) = workset::values(
        workset::output_layout::CORD_VALUE_SLOT_CAPACITIES,
        workset::output_layout::STEP_VALUE_SLOT_CAPACITIES,
        workset::output_layout::HOST_CALL_VALUE_SLOT_CAPACITIES,
        &[],
        config::HEAP_BYTES,
    )
    .expect("original output value constructor");
    println!("CPU_CONSTRUCTOR_GATE residents={} source_entries={} kernels={:?} original_heap={} original_assignment={} original_runtime={} whole_factory_called=false admission=false start=false target_fit_proved=false", resident::ENTRY_COUNT,workset::ENTRY_COUNT,play::CONDUIT_EXECUTION_KERNEL_BYTES,config::HEAP_BYTES,config::MAX_ASSIGNMENT_BYTES,config::MAX_RUNTIME_STATE_BYTES);
    drop((residents, values, parameters, output_values));
}
