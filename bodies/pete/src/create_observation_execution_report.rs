use crate::{
    CreateObservationEncodeRefusal, CreateObservationFailure, PreparedCreateObservationExecution,
};
use conduit_core::{BootId, HostId, OfferGeneration};

use crate::create_observation_play::MAXIMUM_VALUE_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateObservationExecutionFailure {
    Session(CreateObservationFailure),
    Encoding(CreateObservationEncodeRefusal),
    MissingCurrentValue,
    KernelRefused,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateObservationTerminal {
    Completed,
    CancelledBeforeDispatch,
    CancelledAfterDispatch,
    Failed(CreateObservationExecutionFailure),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateObservationDispatchFailure {
    pub terminal: CreateObservationTerminal,
    pub observation_generation: Option<u32>,
    pub observed_at_tick: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateObservationExecutionReport {
    pub terminal: CreateObservationTerminal,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub serial_base_id: String,
    pub robot_identity: String,
    pub observation_generation: Option<u32>,
    pub observed_at_tick: Option<u64>,
    pub odometry_frame_generation: Option<u32>,
    pub odometry_sample_generation: Option<u32>,
    pub canonical_value: [u8; MAXIMUM_VALUE_BYTES],
    pub canonical_value_len: u8,
    pub kernel_decisions: u32,
    pub kernel_signs: u16,
}

pub(super) fn report(
    execution: &PreparedCreateObservationExecution,
    terminal: CreateObservationTerminal,
    generation: Option<u32>,
    observed_at_tick: Option<u64>,
    canonical: &[u8],
) -> CreateObservationExecutionReport {
    let mut canonical_value = [0_u8; MAXIMUM_VALUE_BYTES];
    canonical_value[..canonical.len()].copy_from_slice(canonical);
    CreateObservationExecutionReport {
        terminal,
        host_id: execution.host_id.clone(),
        boot_id: execution.boot_id.clone(),
        offer_generation: execution.offer_generation,
        serial_base_id: execution.serial_base_id.clone(),
        robot_identity: execution.robot_identity.clone(),
        observation_generation: generation,
        observed_at_tick,
        odometry_frame_generation: execution.odometry_frame_generation,
        odometry_sample_generation: execution.odometry_sample_generation,
        canonical_value,
        canonical_value_len: canonical.len() as u8,
        kernel_decisions: execution.kernel_decisions(),
        kernel_signs: execution.kernel_signs(),
    }
}
