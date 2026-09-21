//! Pure browser realization of the deterministic Little Seismograph inputs.

use super::factory::{validate_placement, BrowserInstallation};
use super::{BrowserOperation, MAXIMUM_BROWSER_VALUE_BYTES};
use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, ImplementationId, PlannedGear, StructuredInfoType, StructuredInfoValue,
};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    HostedValueStore, PortId, ValueRef, ValueStorage,
};

const IMPLEMENTATION: &str = "browser/kernel-deterministic-seismograph-inputs@1";

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

fn offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_little_seismograph_fixture::little_seismograph_fixture_semantic_contract(),
        Back {
            capability_id: CapabilityId::from(IMPLEMENTATION),
            execution_profile_id: ExecutionProfileId::from(IMPLEMENTATION),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(IMPLEMENTATION),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .narrow_capacity(CapabilityLimits {
        max_active_instances: 1,
        max_queue_items: 3,
        max_queue_bytes: MAXIMUM_BROWSER_VALUE_BYTES as u32 * 3,
    })
    .expect("browser Little Seismograph capacity narrows its semantic contract")
    .build()
}

fn prepare(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &offer())?;
    let (profile, samples, threshold) =
        conduit_little_seismograph_fixture::deterministic_little_seismograph_inputs();
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
    Ok(BrowserOperation::installed_step(SourceOperation {
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

impl<const PORTS: usize> StepOperation<PORTS> for SourceOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        let Some((port, value)) = self.emissions.get(self.next).copied() else {
            return StepOutcome::Complete;
        };
        if !io.output_ready(port) {
            return StepOutcome::Await;
        }
        io.send(port, value)
            .expect("ready Little Seismograph fixture output");
        self.next += 1;
        StepOutcome::Progress
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn browser_seismograph_fixture_preserves_semantics_and_narrows_capacity() {
        let offer = super::offer();
        let semantic =
            conduit_little_seismograph_fixture::little_seismograph_fixture_semantic_contract();
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
