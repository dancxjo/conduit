//! Test-only finite sink proving recurrence values traversed an ordinary Cord.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ConfigurationValue,
    ExecutionProfileId, ImplementationId, ImplementationOffer, KindIdentity, PlannedGear,
    PortDescriptor, PortDirection, PortTemporal,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};

const KIND: &str = "conduit-test/recurrence-sink";
const IMPLEMENTATION: &str = "conduit-test/recurrence-sink@1";

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct TestRecurrenceSinkOperation {
    expected: u32,
    received: u32,
}

impl<const PORTS: usize> StepBack<PORTS> for TestRecurrenceSinkOperation {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if io.input(PortId(0)).is_some() {
            let Some(canonical) = input_bytes.input(PortId(0)) else {
                return StepOutcome::Fail(Failure {
                    code: FailureCode::InvalidInput,
                    detail: 230,
                });
            };
            if canonical.is_empty()
                || canonical.len() > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
                || self.received != 0
            {
                return StepOutcome::Fail(Failure {
                    code: FailureCode::InvalidInput,
                    detail: 230,
                });
            }
            io.consume(PortId(0)).expect("present recurrence fixture");
            self.received = self.expected;
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && self.received == self.expected {
            io.consume_closed(PortId(0))
                .expect("observed recurrence fixture closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}

impl TestRecurrenceSinkOperation {}

pub(crate) fn offer() -> CapabilityOffer {
    let value_kind = conduit_semantic_catalog::recurrence_result_type()
        .profile()
        .unwrap()
        .value_kind()
        .clone();
    CapabilityOffer {
        startup_parameters: vec![conduit_core::FrontStartupParameter {
            name: "expected".into(),
            value_type: conduit_core::kind_id("value/count"),
            has_default: false,
        }],
        shorthand: None,
        capability_id: CapabilityId::from(KIND),
        kind_id: conduit_core::kind_id(KIND),
        kind_contract_revision: KindIdentity::from("conduit-test/recurrence-sink@1"),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("conduit-test/recurrence-sink@1"),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-std-host/test-recurrence-sink@1"),
        },
        inputs: vec![PortDescriptor {
            port_id: port_id("occurrences"),
            value_kind,
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
        }],
        outputs: vec![],
        host_calls: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: conduit_semantic_catalog::RECURRENCE_MAXIMUM_RESULTS,
            max_queue_bytes: (conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
                * usize::from(conduit_semantic_catalog::RECURRENCE_MAXIMUM_RESULTS))
                as u32,
        },
    }
}

fn expected(placement: &PlannedGear) -> Result<u32, String> {
    let [entry] = placement.configuration.as_slice() else {
        return Err("recurrence sink requires one expected count".into());
    };
    match (entry.key.as_str(), &entry.value) {
        ("expected", ConfigurationValue::U64(value)) => (*value)
            .try_into()
            .ok()
            .filter(|value| {
                *value <= u32::from(conduit_semantic_catalog::RECURRENCE_MAXIMUM_RESULTS)
            })
            .ok_or_else(|| "recurrence sink expected count exceeds profile".into()),
        _ => Err("recurrence sink expected count is malformed".into()),
    }
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    expected(placement)?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 16,
        maximum_value_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    Ok(InstalledOperation::TestRecurrenceSink(
        TestRecurrenceSinkOperation {
            expected: expected(placement)?,
            received: 0,
        },
    ))
}
