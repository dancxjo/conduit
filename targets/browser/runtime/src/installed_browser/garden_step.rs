//! Browser production realization of the deterministic minimal Garden reducer.

use super::factory::{validate_placement, BrowserInstallation};
use super::{BrowserOperation, MAXIMUM_BROWSER_VALUE_BYTES};
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    HostOperationRequirement, ImplementationId, ImplementationOffer, KindContractRevision,
    PlannedGear,
};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    HostedValueStore, Operation, OperationAction, OperationInput, PortId, RequestId,
};

pub(crate) const OPERATIONS: [&str; 6] = [
    "conduit.host/browser-garden-step-0-prior@1",
    "conduit.host/browser-garden-step-1-clock@1",
    "conduit.host/browser-garden-observation-0-clock@1",
    "conduit.host/browser-garden-observation-1-contact@1",
    "conduit.host/browser-garden-enriched-step-0-prior@1",
    "conduit.host/browser-garden-enriched-step-1-observation@1",
];
const MINIMAL_IMPLEMENTATION: &str = "browser/kernel-garden-step@1";
const OBSERVATION_IMPLEMENTATION: &str = "browser/kernel-garden-observation@1";
const ENRICHED_IMPLEMENTATION: &str = "browser/kernel-garden-enriched-step@1";

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: MINIMAL_IMPLEMENTATION,
    offer: minimal_offer,
    prepare,
    perform: None,
};

pub(super) static OBSERVATION_INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: OBSERVATION_IMPLEMENTATION,
    offer: observation_offer,
    prepare,
    perform: None,
};

pub(super) static ENRICHED_INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: ENRICHED_IMPLEMENTATION,
    offer: enriched_offer,
    prepare,
    perform: None,
};

pub(crate) struct PreparedGardenStep {
    mode: GardenMode,
    first: Option<GardenFirst>,
}

#[derive(Clone, Copy)]
enum GardenMode {
    Minimal,
    Observation,
    Enriched,
}

enum GardenFirst {
    State(conduit_semantic_catalog::GardenState),
    Clock(conduit_semantic_catalog::GardenClockObservation),
}

impl PreparedGardenStep {
    pub(crate) fn for_placement(placement: &PlannedGear) -> Result<Option<Self>, String> {
        let (mode, offered) = match placement.implementation_id.as_str() {
            MINIMAL_IMPLEMENTATION => (GardenMode::Minimal, minimal_offer()),
            OBSERVATION_IMPLEMENTATION => (GardenMode::Observation, observation_offer()),
            ENRICHED_IMPLEMENTATION => (GardenMode::Enriched, enriched_offer()),
            _ => return Ok(None),
        };
        validate_placement(placement, &offered)?;
        Ok(Some(Self { mode, first: None }))
    }

    pub(crate) fn execute(
        &mut self,
        contract: &str,
        input: &[u8],
    ) -> Result<Option<Vec<u8>>, Failure> {
        let operation_offset = match self.mode {
            GardenMode::Minimal => 0,
            GardenMode::Observation => 2,
            GardenMode::Enriched => 4,
        };
        match contract {
            value if value == OPERATIONS[operation_offset] => {
                if self.first.is_some() {
                    return Err(failure(1));
                }
                self.first = Some(self.decode_first(input)?);
                Ok(None)
            }
            value if value == OPERATIONS[operation_offset + 1] => {
                let first = self.first.take().ok_or(failure(3))?;
                let bytes = self.finish(first, input)?;
                if bytes.len() > MAXIMUM_BROWSER_VALUE_BYTES {
                    return Err(Failure {
                        code: FailureCode::StorageExhausted,
                        detail: 11,
                    });
                }
                Ok(Some(bytes))
            }
            _ => Err(failure(12)),
        }
    }

    fn decode_first(&self, input: &[u8]) -> Result<GardenFirst, Failure> {
        match self.mode {
            GardenMode::Minimal | GardenMode::Enriched => {
                conduit_semantic_catalog::decode_garden_state(input)
                    .map(GardenFirst::State)
                    .map_err(|_| failure(2))
            }
            GardenMode::Observation => {
                conduit_semantic_catalog::decode_garden_clock_observation(input)
                    .map(GardenFirst::Clock)
                    .map_err(|_| failure(2))
            }
        }
    }

    fn finish(&self, first: GardenFirst, second: &[u8]) -> Result<Vec<u8>, Failure> {
        let value = match (self.mode, first) {
            (GardenMode::Minimal, GardenFirst::State(prior)) => {
                let clock = conduit_semantic_catalog::decode_garden_clock_observation(second)
                    .map_err(|_| failure(4))?;
                let next = conduit_semantic_catalog::evolve_garden_minimal(prior, clock)
                    .map_err(|error| failure(evolution_detail(error)))?;
                conduit_semantic_catalog::garden_state_value(next)
            }
            (GardenMode::Observation, GardenFirst::Clock(clock)) => {
                let contact = conduit_semantic_catalog::decode_garden_contact_observation(second)
                    .map_err(|_| failure(4))?;
                let observation =
                    conduit_semantic_catalog::combine_garden_observations(clock, contact)
                        .map_err(|error| failure(evolution_detail(error)))?;
                conduit_semantic_catalog::garden_enriched_observation_value(observation)
            }
            (GardenMode::Enriched, GardenFirst::State(prior)) => {
                let observation =
                    conduit_semantic_catalog::decode_garden_enriched_observation(second)
                        .map_err(|_| failure(4))?;
                let next = conduit_semantic_catalog::evolve_garden_enriched_observation(
                    prior,
                    observation,
                )
                .map_err(|error| failure(evolution_detail(error)))?;
                conduit_semantic_catalog::garden_state_value(next)
            }
            _ => return Err(failure(2)),
        };
        value
            .and_then(|value| value.canonical_bytes())
            .map_err(|_| failure(10))
    }
}

fn minimal_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::garden_minimal_step_definition();
    offer_for(contract, MINIMAL_IMPLEMENTATION, &OPERATIONS[0..2])
}

fn observation_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::garden_observation_combine_definition();
    offer_for(contract, OBSERVATION_IMPLEMENTATION, &OPERATIONS[2..4])
}

fn enriched_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::garden_enriched_reducer_definition();
    offer_for(contract, ENRICHED_IMPLEMENTATION, &OPERATIONS[4..6])
}

fn offer_for(
    contract: conduit_form::KindDefinition,
    implementation: &str,
    operations: &[&str],
) -> CapabilityOffer {
    let kind = contract.kind_id.clone();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(implementation),
        kind_id: kind.clone(),
        kind_contract_revision: KindContractRevision::from(
            conduit_semantic_catalog::GARDEN_CONTRACT_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(implementation),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(implementation),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: operations
            .iter()
            .enumerate()
            .map(|(index, contract_id)| HostOperationRequirement {
                contract_id: (*contract_id).into(),
                target_kind: Some(kind.clone()),
                maximum_in_flight: 1,
                maximum_input_bytes: MAXIMUM_BROWSER_VALUE_BYTES as u32,
                maximum_output_bytes: if index == 0 {
                    0
                } else {
                    MAXIMUM_BROWSER_VALUE_BYTES as u32
                },
            })
            .collect(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 2,
            max_queue_bytes: MAXIMUM_BROWSER_VALUE_BYTES as u32,
        },
    }
}

fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserOperation, String> {
    PreparedGardenStep::for_placement(placement)?
        .ok_or_else(|| "Garden operation selected another implementation".to_string())?;
    Ok(BrowserOperation::installed(GardenStepOperation::new()))
}

struct GardenStepOperation {
    prior_ready: bool,
    pending: Option<(RequestId, HostOperationId)>,
    emitted: bool,
}

impl GardenStepOperation {
    const fn new() -> Self {
        Self {
            prior_ready: false,
            pending: None,
            emitted: false,
        }
    }
}

impl Operation for GardenStepOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.prior_ready && self.pending.is_none() => {
                self.request(value, RequestId(0), HostOperationId(0), 20)
            }
            OperationInput::Value {
                port: PortId(1),
                value,
            } if self.prior_ready && self.pending.is_none() && !self.emitted => {
                self.request(value, RequestId(1), HostOperationId(1), 21)
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending.map(|pending| pending.0) == Some(request) =>
            {
                let Some((_, operation)) = self.pending.take() else {
                    return OperationAction::Fail(failure(22));
                };
                match (
                    operation,
                    outcome.disposition,
                    outcome.output,
                    outcome.failure,
                ) {
                    (HostOperationId(0), HostOperationDisposition::Completed, None, None) => {
                        self.prior_ready = true;
                        OperationAction::Await
                    }
                    (
                        HostOperationId(1),
                        HostOperationDisposition::Completed,
                        Some(output),
                        None,
                    ) => {
                        self.emitted = true;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (_, HostOperationDisposition::Failed, None, Some(reason)) => {
                        OperationAction::Fail(reason)
                    }
                    _ => OperationAction::Fail(failure(22)),
                }
            }
            OperationInput::Closed { port: PortId(0) } if self.prior_ready => {
                OperationAction::Await
            }
            OperationInput::Closed { port: PortId(1) } if self.emitted => OperationAction::Complete,
            _ => OperationAction::Fail(failure(23)),
        }
    }

    fn advance(&mut self) -> OperationAction {
        if self.emitted {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

impl GardenStepOperation {
    fn request(
        &mut self,
        value: conduit_kernel::ValueRef,
        request: RequestId,
        operation: HostOperationId,
        detail: u16,
    ) -> OperationAction {
        let Ok(input) = BoundedValueRef::new(value, MAXIMUM_BROWSER_VALUE_BYTES as u32) else {
            return OperationAction::Fail(failure(detail));
        };
        self.pending = Some((request, operation));
        OperationAction::RequestHostOperation {
            request,
            operation,
            input,
        }
    }
}

fn evolution_detail(error: conduit_semantic_catalog::GardenEvolutionRefusal) -> u16 {
    use conduit_semantic_catalog::GardenEvolutionRefusal::*;
    match error {
        MalformedState => 5,
        MalformedClockObservation => 6,
        MalformedContactObservation => 7,
        StepCapacityExceeded => 8,
        ArithmeticOverflow => 9,
        MalformedEnrichedObservation => 10,
    }
}

fn failure(detail: u16) -> Failure {
    Failure {
        code: FailureCode::InvalidInput,
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{OfferGeneration, Scalar};

    fn placement() -> PlannedGear {
        let offered = minimal_offer();
        PlannedGear {
            placement_id: "garden-step-placement".into(),
            gear_id: "garden-step".into(),
            kind_id: offered.kind_id,
            kind_contract_revision: offered.kind_contract_revision,
            execution_profile_id: offered.implementation.execution_profile_id,
            configuration: Vec::new(),
            host_id: "browser/garden".into(),
            boot_id: "browser-boot/garden".into(),
            offer_generation: OfferGeneration(1),
            capability_id: offered.capability_id,
            implementation_id: offered.implementation.implementation_id,
            artifact_id: offered.implementation.artifact_id,
            realization_characteristics: Vec::new(),
            limits: offered.limits,
            inputs: offered.inputs,
            outputs: offered.outputs,
            host_operations: offered.host_operations,
            resources: Vec::new(),
            authority: Vec::new(),
            pool_references: Vec::new(),
        }
    }

    #[test]
    fn exact_state_and_clock_execute_the_shared_reducer() {
        let mut prepared = PreparedGardenStep::for_placement(&placement())
            .unwrap()
            .unwrap();
        let prior = conduit_semantic_catalog::GardenState {
            vitality: Scalar::from_raw_microunits(400_000),
            activity: Scalar::ZERO,
            step: 0,
        };
        let clock = conduit_semantic_catalog::GardenClockObservation {
            phase: Scalar::from_raw_microunits(800_000),
        };
        let prior = conduit_semantic_catalog::garden_state_value(prior)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        let clock = conduit_semantic_catalog::garden_clock_observation_value(clock)
            .unwrap()
            .canonical_bytes()
            .unwrap();

        assert_eq!(prepared.execute(OPERATIONS[0], &prior), Ok(None));
        let output = prepared.execute(OPERATIONS[1], &clock).unwrap().unwrap();
        let next = conduit_semantic_catalog::decode_garden_state(&output).unwrap();
        assert_eq!(next.vitality.raw_microunits(), 500_000);
        assert_eq!(next.activity.raw_microunits(), 400_000);
        assert_eq!(next.step, 1);
    }

    #[test]
    fn wrong_order_and_wrong_typed_input_refuse() {
        let mut prepared = PreparedGardenStep::for_placement(&placement())
            .unwrap()
            .unwrap();
        let clock = conduit_semantic_catalog::garden_clock_observation_value(
            conduit_semantic_catalog::GardenClockObservation {
                phase: Scalar::from_raw_microunits(800_000),
            },
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();

        assert_eq!(prepared.execute(OPERATIONS[1], &clock), Err(failure(3)));
        assert_eq!(prepared.execute(OPERATIONS[0], &clock), Err(failure(2)));
    }
}
