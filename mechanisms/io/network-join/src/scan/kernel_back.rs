//! Source and evidence sink for an ordinary checked two-gear scan Plot.
//! Reuse the production control endpoint with a compile-time value bound;
//! no new scheduler, radio dispatch, Line or mutable bound field is introduced.
use crate::control_back::Endpoint;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, ValueRef,
};
use conduit_net::MAXIMUM_SCAN_OUTPUT_BYTES;

type ScanEndpoint = Endpoint<MAXIMUM_SCAN_OUTPUT_BYTES>;
enum Role {
    Source(ScanEndpoint),
    Sign(ScanEndpoint),
}
pub struct NetworkScanBack(Role);
impl NetworkScanBack {
    /// The value is canonically empty; invocation generation/deadlines remain
    /// pinned by the selected provider owner, never an authored input port.
    pub fn source(empty: ValueRef) -> Result<Self, Failure> {
        ScanEndpoint::prepared_source(empty).map(|endpoint| Self(Role::Source(endpoint)))
    }
    pub fn evidence_sink() -> Self {
        Self(Role::Sign(ScanEndpoint::prepared_sink()))
    }
}
impl<const PORTS: usize> StepBack<PORTS> for NetworkScanBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match &mut self.0 {
            Role::Source(endpoint) => endpoint.source(io),
            Role::Sign(endpoint) => endpoint.sink(io),
        }
    }
    fn step_committed(&mut self) {
        match &mut self.0 {
            Role::Source(endpoint) | Role::Sign(endpoint) => endpoint.commit(),
        }
    }
    fn cancel(&mut self) {
        match &mut self.0 {
            Role::Source(endpoint) | Role::Sign(endpoint) => endpoint.cancel(),
        }
    }
}
