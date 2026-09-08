//! Pure browser realization of the deterministic Little Seismograph inputs.

use super::factory::{validate_placement, BrowserInstallation};
use super::{BrowserOperation, MAXIMUM_BROWSER_VALUE_BYTES};
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    ImplementationId, ImplementationOffer, PlannedGear, StructuredInfoType, StructuredInfoValue,
};
use conduit_kernel::{
    Failure, FailureCode, HostedValueStore, Operation, OperationAction, OperationInput, PortId,
    ValueRef, ValueStorage,
};

const IMPLEMENTATION: &str = "browser/kernel-deterministic-seismograph-inputs@1";

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

fn offer() -> CapabilityOffer {
    let contract = conduit_data::little_seismograph_fixture_definition();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(IMPLEMENTATION),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(IMPLEMENTATION),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(IMPLEMENTATION),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 3,
            max_queue_bytes: MAXIMUM_BROWSER_VALUE_BYTES as u32 * 3,
        },
    }
}

fn prepare(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &offer())?;
    let (profile, samples, threshold) = conduit_data::deterministic_little_seismograph_inputs();
    let mut emissions = Vec::with_capacity(3);
    emissions.push((
        PortId(0),
        store_leaf(
            values,
            conduit_data::measurement_window_profile_type(),
            conduit_data::encode_measurement_window_profile(&profile)
                .map_err(|error| format!("encode deterministic window profile: {error:?}"))?,
        )?,
    ));
    emissions.push((
        PortId(2),
        store_leaf(
            values,
            conduit_data::measurement_hysteresis_profile_type(),
            conduit_data::encode_measurement_hysteresis_profile(threshold)
                .map_err(|error| format!("encode deterministic threshold profile: {error:?}"))?,
        )?,
    ));
    for sample in samples {
        emissions.push((
            PortId(1),
            store_leaf(
                values,
                conduit_data::measurement_sample_type(),
                conduit_data::encode_measurement_sample(&sample)
                    .map_err(|error| format!("encode deterministic sample: {error:?}"))?,
            )?,
        ));
    }
    Ok(BrowserOperation::installed(SourceOperation {
        emissions,
        next: 0,
    }))
}

fn store_leaf(
    values: &mut HostedValueStore,
    value_type: StructuredInfoType,
    payload: Vec<u8>,
) -> Result<ValueRef, String> {
    let canonical = StructuredInfoValue::leaf(value_type, payload)
        .map_err(|error| format!("construct deterministic measurement value: {error:?}"))?
        .canonical_bytes()
        .map_err(|error| format!("encode deterministic measurement value: {error:?}"))?;
    values
        .store(&canonical)
        .map_err(|error| format!("{error:?}"))
}

struct SourceOperation {
    emissions: Vec<(PortId, ValueRef)>,
    next: usize,
}

impl Operation for SourceOperation {
    fn start(&mut self) -> OperationAction {
        self.emit_next()
    }

    fn resume(&mut self, _input: OperationInput) -> OperationAction {
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidInput,
            detail: 1,
        })
    }

    fn advance(&mut self) -> OperationAction {
        self.emit_next()
    }
}

impl SourceOperation {
    fn emit_next(&mut self) -> OperationAction {
        let Some((port, value)) = self.emissions.get(self.next).copied() else {
            return OperationAction::Complete;
        };
        self.next += 1;
        OperationAction::Emit { port, value }
    }
}
