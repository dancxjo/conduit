//! Checked primitive integer narrowing. Rounding and domain admission belong to Source.
use crate::{
    fixed_numeric_catalog::fixed_numeric_contracts,
    fixed_numeric_preparation::verify_fixed_placement,
};
use alloc::{format, string::String, vec};
use conduit_core::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
pub const INTEGER_NARROWING_IMPLEMENTATION: &str = "conduit.numeric/checked-u64-u16@1";
pub const INTEGER_NARROWING_FLOW_IMPLEMENTATION: &str =
    "conduit.numeric/closing-flow-checked-u64-u16@1";
pub fn checked_integer_narrowing_contract(flow: bool) -> Result<Kind, String> {
    if flow {
        crate::fixed_numeric_temporal::closing_numeric_contract(
            "numeric/u64-to-u16",
            INTEGER_NARROWING_FLOW_IMPLEMENTATION,
        )
    } else {
        fixed_numeric_contracts()?
            .into_iter()
            .find(|kind| kind.kind_id.as_str() == "numeric/u64-to-u16")
            .ok_or("absent checked narrowing contract".into())
    }
}
pub fn checked_integer_narrowing_offer(flow: bool) -> Result<CapabilityOffer, String> {
    let implementation = if flow {
        INTEGER_NARROWING_FLOW_IMPLEMENTATION
    } else {
        INTEGER_NARROWING_IMPLEMENTATION
    };
    let kind = checked_integer_narrowing_contract(flow)?;
    let identity = String::from(kind.kind_id.as_str());
    Ok(BackOfferBuilder::new(
        kind,
        Back {
            capability_id: CapabilityId::from(format!("{implementation}/{identity}")),
            execution_profile_id: ExecutionProfileId::from(implementation),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(implementation),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build())
}
pub fn install_checked_integer_flow_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    let kind = checked_integer_narrowing_contract(true)?;
    startup.insert(conduit_plot::KindSignature {
        kind: kind.kind_id.as_str().into(),
        startup_parameters: vec![],
    })?;
    startup.insert_fore(kind.kind_id.as_str(), kind.checked_front())?;
    profile
        .insert_kind(kind)
        .map_err(|error| format!("{error:?}"))?;
    Ok(())
}
/// One fixed two-byte output, with no heap/resource/schema work during Step.
pub struct CheckedU64ToU16Back {
    output: [u8; 2],
    flow: bool,
    staged: bool,
    finished: bool,
    cancelled: bool,
    committed_frames: u64,
}
impl CheckedU64ToU16Back {
    pub fn prepare_planned<const PORTS: usize>(
        gear: &PlannedGear,
        fuel: u16,
        flow: bool,
    ) -> Result<Self, String> {
        if PORTS < 1 || fuel < 2 {
            return Err("checked integer step bound".into());
        }
        verify_fixed_placement(gear, &checked_integer_narrowing_offer(flow)?)
            .map_err(|error| format!("{error:?}"))?;
        Ok(Self {
            output: [0; 2],
            flow,
            staged: false,
            finished: false,
            cancelled: false,
            committed_frames: 0,
        })
    }
    pub fn committed_frames(&self) -> u64 {
        self.committed_frames
    }
}
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
impl<const PORTS: usize> StepBack<PORTS> for CheckedU64ToU16Back {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.staged = false;
        if self.cancelled {
            return StepOutcome::Fail(Failure {
                code: FailureCode::Cancelled,
                detail: 2870,
            });
        }
        if self.finished {
            return StepOutcome::Complete;
        }
        if io.input_closed(PortId(0)) {
            return if self.flow {
                StepOutcome::Complete
            } else {
                fail(2871)
            };
        }
        if self.committed_frames == u64::MAX {
            return fail(2872);
        }
        if io.input(PortId(0)).is_none() || !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let Some(bytes) = inputs.input(PortId(0)) else {
            return fail(2873);
        };
        let Ok(raw) = <[u8; 8]>::try_from(bytes) else {
            return fail(2874);
        };
        let Ok(value) = u16::try_from(u64::from_le_bytes(raw)) else {
            return fail(2875);
        };
        self.output = value.to_le_bytes();
        if io.consume(PortId(0)).is_err() || io.send_prepared(PortId(0), 2).is_err() {
            return fail(2876);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.staged).then_some(&self.output)
    }
    fn step_committed(&mut self) {
        if self.staged {
            self.staged = false;
            self.committed_frames += 1;
            self.finished = !self.flow;
        }
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.staged = false;
    }
}
