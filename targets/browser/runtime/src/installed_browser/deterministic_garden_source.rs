//! Pure browser realization of the canonical deterministic Garden observations.

use super::factory::{validate_placement, BrowserInstallation};
use super::{BrowserOperation, MAXIMUM_BROWSER_VALUE_BYTES};
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    ImplementationId, ImplementationOffer, PlannedGear,
};
use conduit_kernel::{
    Failure, FailureCode, HostedValueStore, Operation, OperationAction, OperationInput, PortId,
    ValueRef, ValueStorage,
};

const IMPLEMENTATION: &str = "browser/kernel-deterministic-garden-observations@1";

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

fn offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::garden_fixture_definition();
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
    let (state, clock, contact) = conduit_semantic_catalog::deterministic_garden_observations();
    let canonical = [
        conduit_semantic_catalog::garden_state_value(state),
        conduit_semantic_catalog::garden_clock_observation_value(clock),
        conduit_semantic_catalog::garden_contact_observation_value(contact),
    ];
    let stored = canonical
        .into_iter()
        .map(|value| {
            value
                .and_then(|value| value.canonical_bytes())
                .map_err(|error| format!("encode deterministic Garden observation: {error:?}"))
                .and_then(|value| values.store(&value).map_err(|error| format!("{error:?}")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let values: [ValueRef; 3] = stored
        .try_into()
        .map_err(|_| "deterministic Garden source value count".to_string())?;
    Ok(BrowserOperation::installed(SourceOperation {
        values,
        next: 0,
    }))
}

struct SourceOperation {
    values: [ValueRef; 3],
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
        let Some(value) = self.values.get(self.next).copied() else {
            return OperationAction::Complete;
        };
        let port = PortId(u16::try_from(self.next).expect("three Garden source ports"));
        self.next += 1;
        OperationAction::Emit { port, value }
    }
}
