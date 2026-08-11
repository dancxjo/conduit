//! Portable contract for one exact host/boot-scoped monotonic deadline slot.

use crate::{
    resource_requirement, HostOperationContractId, HostOperationRequirement, ResourceRequirement,
};

pub const MONOTONIC_MILLISECOND_TIMER_RESOURCE_CLASS: &str =
    "conduit.resource/monotonic-millisecond-timer-slot@1";
pub const MONOTONIC_TIMER_HOST_OPERATION_CONTRACT: &str =
    "conduit.host/monotonic-millisecond-timer@1";
pub const MONOTONIC_TIMER_INPUT_BYTES: u32 = core::mem::size_of::<u64>() as u32;

/// One relative millisecond duration. The selected host/boot-scoped Base
/// measures it from the exact arm event on its monotonic clock.
pub const fn encode_monotonic_duration(duration_ms: u64) -> [u8; 8] {
    duration_ms.to_le_bytes()
}

pub fn decode_monotonic_duration(encoded: &[u8]) -> Result<u64, MonotonicTimerInputError> {
    let bytes: [u8; 8] = encoded
        .try_into()
        .map_err(|_| MonotonicTimerInputError::WrongLength)?;
    Ok(u64::from_le_bytes(bytes))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonotonicTimerInputError {
    WrongLength,
}

pub fn monotonic_timer_host_operation_requirement() -> HostOperationRequirement {
    HostOperationRequirement {
        contract_id: HostOperationContractId::from(MONOTONIC_TIMER_HOST_OPERATION_CONTRACT),
        target_kind: None,
        maximum_in_flight: 1,
        maximum_input_bytes: MONOTONIC_TIMER_INPUT_BYTES,
        maximum_output_bytes: 0,
    }
}

pub fn monotonic_timer_resource_requirement() -> ResourceRequirement {
    resource_requirement(MONOTONIC_MILLISECOND_TIMER_RESOURCE_CLASS, 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_is_exact_bounded_and_round_trips_every_boundary() {
        let operation = monotonic_timer_host_operation_requirement();
        assert_eq!(
            operation.contract_id.as_str(),
            MONOTONIC_TIMER_HOST_OPERATION_CONTRACT
        );
        assert_eq!(operation.maximum_in_flight, 1);
        assert_eq!(operation.maximum_input_bytes, 8);
        assert_eq!(operation.maximum_output_bytes, 0);
        assert!(operation.target_kind.is_none());

        let resource = monotonic_timer_resource_requirement();
        assert_eq!(
            resource.class_id.as_str(),
            MONOTONIC_MILLISECOND_TIMER_RESOURCE_CLASS
        );
        assert_eq!(resource.units, 1);

        for duration in [0, 1, u64::MAX] {
            assert_eq!(
                decode_monotonic_duration(&encode_monotonic_duration(duration)),
                Ok(duration)
            );
        }
        assert_eq!(
            decode_monotonic_duration(&[0; 7]),
            Err(MonotonicTimerInputError::WrongLength)
        );
    }
}
