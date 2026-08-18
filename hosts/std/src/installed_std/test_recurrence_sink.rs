//! Test-only finite sink proving recurrence values traversed an ordinary Cord.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ConfigurationValue,
    ExecutionProfileId, ImplementationId, ImplementationOffer, KindContractRevision, PlannedGear,
    PortDescriptor, PortDirection, PortTemporal,
};
use conduit_kernel::{Failure, FailureCode, OperationAction, OperationInput, PortId};

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

impl TestRecurrenceSinkOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume_value(&mut self, port: PortId, canonical: &[u8]) -> OperationAction {
        let valid = !canonical.is_empty()
            && canonical.len() <= conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES;
        if port != PortId(0) || !valid || self.received != 0 {
            return OperationAction::Fail(Failure {
                code: FailureCode::InvalidInput,
                detail: 230,
            });
        }
        self.received = self.expected;
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Closed { port: PortId(0) } if self.received == self.expected => {
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(231),
        }
    }
}

pub(crate) fn offer() -> CapabilityOffer {
    let value_kind = conduit_std_catalog::recurrence_result_type()
        .profile()
        .unwrap()
        .value_kind()
        .clone();
    CapabilityOffer {
        startup_parameters: vec![conduit_core::FaceStartupParameter {
            name: "expected".into(),
            value_type: "Count".into(),
            has_default: false,
        }],
        shorthand: None,
        capability_id: CapabilityId::from(KIND),
        kind_id: conduit_core::kind_id(KIND),
        kind_contract_revision: KindContractRevision::from("conduit-test/recurrence-sink@1"),
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
        host_operations: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: conduit_std_catalog::RECURRENCE_MAXIMUM_RESULTS,
            max_queue_bytes: (conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
                * usize::from(conduit_std_catalog::RECURRENCE_MAXIMUM_RESULTS))
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
            .filter(|value| *value <= u32::from(conduit_std_catalog::RECURRENCE_MAXIMUM_RESULTS))
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
