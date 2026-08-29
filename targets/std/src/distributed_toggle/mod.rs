//! Std source half of the distributed toggle proof.
//!
//! Split by stable responsibility:
//! - `plan`: planning, advertisement resolution, two-fragment plan creation.
//! - `operation`: `ToggleSourceOperation` kernel state machine with mutation tests.
//! - `source`: `DistributedToggleSource` struct, preparation, and host-op adapter.
//! - `line`: WebSocket session and line transport for the source.

mod line;
mod operation;
mod plan;
mod source;

pub use line::bind_listener;
pub use plan::{exact_distributed_toggle_plan, DistributedTogglePlan};
pub use source::DistributedToggleSource;

/// A bounded physical adapter entrance backed by the ordinary planned toggle
/// scheduler. Device code receives absolute manifestations; it never owns the
/// switch state.
pub struct PhysicalLightSwitchKernel {
    source: DistributedToggleSource,
}

impl PhysicalLightSwitchKernel {
    pub fn prepare() -> Result<Self, String> {
        Ok(Self {
            source: DistributedToggleSource::prepare_form(include_str!(
                "../../../../proof/fixtures/forms/physical-light-switch-runtime.conduit"
            ))?,
        })
    }

    pub fn initial(&mut self) -> Result<(u64, bool), String> {
        self.source
            .next_manifestation(false)?
            .ok_or_else(|| "planned light switch omitted its initial value".to_owned())
    }

    pub fn press(&mut self) -> Result<(u64, bool), String> {
        self.source
            .next_manifestation(true)?
            .ok_or_else(|| "planned light switch completed before its admitted press".to_owned())
    }
}

#[cfg(test)]
mod physical_light_switch_tests {
    use super::PhysicalLightSwitchKernel;

    #[test]
    fn planned_kernel_emits_off_on_off() {
        let mut kernel = PhysicalLightSwitchKernel::prepare().expect("prepare");
        assert_eq!(kernel.initial().expect("initial"), (0, false));
        assert_eq!(kernel.press().expect("first press"), (1, true));
        assert_eq!(kernel.press().expect("second press"), (2, false));
    }
}
