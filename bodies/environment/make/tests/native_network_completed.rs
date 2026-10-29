//! Actual native execution/retirement with checked Source and inert CPU providers.
//! No radio, target heap/stack, admitted Body or physical deadline proof.
#![allow(unexpected_cfgs)]
extern crate alloc;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_box_backing.rs"]
mod native_box_backing;
#[allow(
    dead_code,
    reason = "actual recovery module exposes other Root eligibility entrances"
)]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_network_recovery.rs"]
mod native_network_recovery;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_network_retirement_backing.rs"]
mod native_network_retirement_backing;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[allow(
    dead_code,
    unused_imports,
    reason = "shared native policy has other sampler/display entrances"
)]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_authority.rs"]
mod native_authority;
#[allow(
    dead_code,
    reason = "other native provider types only satisfy shared policy signatures"
)]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_retirement.rs"]
mod native_retirement;
#[allow(
    dead_code,
    reason = "includes verified and indexed preparation alongside tested rich entrance"
)]
#[path = "../src/network_planning.rs"]
mod network_planning;
#[path = "native_network_completed/support.rs"]
mod support;
mod native_credentials {
    pub type RootCredentialSource = crate::support::Provider;
    pub fn committed_revision() -> u64 {
        1
    }
}
mod native_network {
    pub type Provider = crate::support::Provider;
}
mod services {
    pub struct Mailbox;
    pub static MAILBOX: Mailbox = Mailbox;
    impl Mailbox {
        pub fn observed_state(
            &self,
        ) -> Option<conduit_environment_http::service_control::ServiceState> {
            Some(conduit_environment_http::service_control::ServiceState::StationReady)
        }
    }
}
#[allow(
    dead_code,
    reason = "production facade includes inactive alternate preparation entrances"
)]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_play.rs"]
mod native_play;
struct CpuClock;
embassy_time_driver::time_driver_impl!(static CLOCK: CpuClock = CpuClock);
impl embassy_time_driver::Driver for CpuClock {
    fn now(&self) -> u64 {
        0
    }
    fn schedule_wake(&self, _: u64, _: &core::task::Waker) {
        panic!("this immediate CPU profile admits no timer wait")
    }
}
