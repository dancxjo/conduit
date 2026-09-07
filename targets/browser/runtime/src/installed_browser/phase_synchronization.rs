//! Effect-free browser realization of deterministic phase following.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    ImplementationId, ImplementationOffer,
};

const PROFILE: &str = "browser/phase-synchronize-bounded@1";
const IMPLEMENTATION: &str = "browser/kernel-phase-synchronize@1";
const ARTIFACT: &str = "conduit-browser-runtime/phase-synchronize@1";

fn offer() -> CapabilityOffer {
    let contract = conduit_time::phase_synchronize_kind_definition();
    CapabilityOffer {
        capability_id: CapabilityId::from("phase-synchronize"),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        inputs: contract.inputs,
        outputs: contract.outputs,
        startup_parameters: vec![],
        shorthand: None,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        host_operations: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 2,
            max_queue_bytes: (conduit_time::RHYTHM_STATE_ENCODED_LEN
                + conduit_time::PULSE_OBSERVATION_ENCODED_LEN) as u32,
        },
    }
}

fn prepare(
    placement: &conduit_core::PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<super::BrowserOperation, String> {
    super::factory::validate_placement(placement, &offer())?;
    if !placement.configuration.is_empty()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
    {
        return Err("phase synchronization admission differs from installation".into());
    }
    Ok(super::BrowserOperation::installed(
        conduit_time::PhaseSynchronizationOperation::new(),
    ))
}

pub(super) static INSTALLATION: super::factory::BrowserInstallation =
    super::factory::BrowserInstallation {
        implementation_id: IMPLEMENTATION,
        offer,
        prepare,
        perform: None,
    };

#[cfg(test)]
mod tests {
    use conduit_kernel::{Operation, OperationAction, PortId, ValueRef};

    fn input(byte_len: usize) -> ValueRef {
        ValueRef {
            slot: 0,
            generation: 1,
            byte_len: byte_len as u32,
        }
    }

    #[test]
    fn shared_operation_derives_exact_adjusted_state_without_a_host_effect() {
        let mut operation = conduit_time::PhaseSynchronizationOperation::new();
        assert_eq!(operation.start(), OperationAction::Await);
        let local = conduit_time::RhythmState {
            sequence: 7,
            next_pulse_at_ms: 1_000,
            period_ms: 240,
            expected_peer_sequence: 4,
        };
        let peer = conduit_time::PulseObservation {
            sequence: 4,
            period_ms: 280,
        };
        assert_eq!(
            operation.resume_value(
                PortId(0),
                input(conduit_time::RHYTHM_STATE_ENCODED_LEN),
                &conduit_time::encode_rhythm_state(local)
            ),
            OperationAction::Await
        );
        let OperationAction::EmitCanonical { port, value } = operation.resume_value(
            PortId(1),
            input(conduit_time::PULSE_OBSERVATION_ENCODED_LEN),
            &conduit_time::encode_pulse_observation(peer),
        ) else {
            panic!("phase synchronization must derive one canonical state");
        };
        assert_eq!(port, PortId(0));
        let updated = conduit_time::decode_rhythm_state(value.as_slice()).unwrap();
        assert_eq!(updated.expected_peer_sequence, 5);
        assert_eq!(updated.next_pulse_at_ms, 1_060);
        assert_eq!(updated.period_ms, 250);
        assert_eq!(
            operation.last_outcome(),
            Some(conduit_time::SynchronizationOutcome::Adjusted {
                phase_ms: 60,
                period_ms: 10,
            })
        );
    }
}
