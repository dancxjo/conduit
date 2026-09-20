//! Pure browser realization of the canonical deterministic Garden observations.

use super::factory::{validate_placement, BrowserInstallation};
use super::{BrowserOperation, MAXIMUM_BROWSER_VALUE_BYTES};
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, CapabilityOfferBuilder,
    CapabilityRealization, ExecutionProfileId, ImplementationId, PlannedGear,
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
    CapabilityOfferBuilder::new(
        conduit_semantic_catalog::garden_fixture_semantic_contract(),
        CapabilityRealization {
            capability_id: CapabilityId::from(IMPLEMENTATION),
            execution_profile_id: ExecutionProfileId::from(IMPLEMENTATION),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(IMPLEMENTATION),
            host_operations: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .narrow_capacity(CapabilityLimits {
        max_active_instances: 1,
        max_queue_items: 3,
        max_queue_bytes: MAXIMUM_BROWSER_VALUE_BYTES as u32 * 3,
    })
    .expect("browser Garden fixture capacity narrows its semantic contract")
    .build()
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

#[cfg(test)]
mod tests {
    #[test]
    fn browser_garden_fixture_preserves_semantics_and_narrows_capacity() {
        let offer = super::offer();
        let semantic = conduit_semantic_catalog::garden_fixture_semantic_contract();
        assert_eq!(offer.startup_parameters, semantic.startup_parameters);
        assert_eq!(offer.kind_id, semantic.kind_id);
        assert_eq!(
            offer.kind_contract_revision,
            semantic.kind_contract_revision
        );
        assert_eq!(offer.inputs, semantic.inputs);
        assert_eq!(offer.outputs, semantic.outputs);
        assert_eq!(offer.limits.max_active_instances, 1);
        assert_eq!(offer.limits.max_queue_items, 3);
        assert!(offer.limits.max_active_instances < semantic.limits.max_active_instances);
        assert!(offer.limits.max_queue_bytes < semantic.limits.max_queue_bytes);
    }
}
