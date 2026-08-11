use super::contract::{MAX_TEXT_BYTES, TEXT_PRESENTATION_VALUE_KIND};
use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ConfigurationValue, ExecutionProfileId, ImplementationId, KindContractRevision, PlannedGear,
    PortDescriptor, PortDirection,
};
use conduit_form::{ConfigurationField, ConfigurationRule, KindDefinition, ProfileCatalog};
use conduit_kernel::{OperationAction, PortId, ValueRef, ValueStorage};

pub(super) const TEST_TEXT_SOURCE_KIND: &str = "conduit.test/text-source";
const TEST_TEXT_SOURCE_REVISION: &str = "conduit.test/text-source@1";
const TEST_TEXT_SOURCE_PROFILE: &str = "conduit.test/text-source-kernel@1";
pub(super) const TEST_TEXT_SOURCE_IMPLEMENTATION: &str = "conduit.test/text-source-kernel@1";
const TEST_TEXT_SOURCE_ARTIFACT: &str = "conduit-std-host/test-text-source@1";

pub(super) static TEST_TEXT_SOURCE_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: TEST_TEXT_SOURCE_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct TestTextSourceOperation {
    pub(super) values: Vec<ValueRef>,
    pub(super) next: usize,
}

impl TestTextSourceOperation {
    pub(super) fn emit_or_complete(&self) -> OperationAction {
        self.values
            .get(self.next)
            .copied()
            .map_or(OperationAction::Complete, |value| OperationAction::Emit {
                port: PortId(0),
                value,
            })
    }
}

pub(super) fn offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![conduit_core::FaceStartupParameter {
            name: "invalid".into(),
            value_type: "Boolean".into(),
            has_default: true,
        }],
        shorthand: None,
        capability_id: CapabilityId::from("test-text-source"),
        kind_id: kind_id(TEST_TEXT_SOURCE_KIND),
        kind_contract_revision: KindContractRevision::from(TEST_TEXT_SOURCE_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(TEST_TEXT_SOURCE_PROFILE),
            implementation_id: ImplementationId::from(TEST_TEXT_SOURCE_IMPLEMENTATION),
            artifact_id: ArtifactId::from(TEST_TEXT_SOURCE_ARTIFACT),
        },
        inputs: Vec::new(),
        outputs: outputs(),
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 4,
            max_queue_bytes: MAX_TEXT_BYTES,
        },
    }
}

pub(super) fn install_catalog(catalog: &mut ProfileCatalog) {
    catalog
        .insert(KindDefinition {
            kind_id: kind_id(TEST_TEXT_SOURCE_KIND),
            kind_contract_revision: KindContractRevision::from(TEST_TEXT_SOURCE_REVISION),
            inputs: Vec::new(),
            outputs: outputs(),
            configuration: vec![ConfigurationField {
                key: "invalid".into(),
                default_value: ConfigurationValue::Bool(false),
                validation: ConfigurationRule::Any,
            }],
        })
        .expect("test text source kind is unique");
}

fn outputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id("text"),
        value_kind: kind_id(TEXT_PRESENTATION_VALUE_KIND),
        direction: PortDirection::Output,
        temporal: conduit_core::PortTemporal::Value,
    }]
}

fn invalid(placement: &PlannedGear) -> Result<bool, String> {
    placement
        .configuration
        .iter()
        .find(|entry| entry.key == "invalid")
        .and_then(|entry| match entry.value {
            ConfigurationValue::Bool(value) => Some(value),
            ConfigurationValue::I64(_) => None,
            ConfigurationValue::U64(_) => None,
            ConfigurationValue::Text(_) => None,
        })
        .ok_or_else(|| "test text source requires boolean invalid configuration".to_string())
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    if placement.kind_id.as_str() != TEST_TEXT_SOURCE_KIND
        || placement.kind_contract_revision.as_str() != TEST_TEXT_SOURCE_REVISION
        || placement.execution_profile_id.as_str() != TEST_TEXT_SOURCE_PROFILE
        || placement.implementation_id.as_str() != TEST_TEXT_SOURCE_IMPLEMENTATION
        || placement.artifact_id.as_str() != TEST_TEXT_SOURCE_ARTIFACT
        || !placement.inputs.is_empty()
        || placement.outputs != outputs()
    {
        return Err("planned test text source identity does not match its fixture".to_string());
    }
    invalid(placement).map(|_| ())
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    let (value_items, value_bytes) = if invalid(placement)? { (1, 1) } else { (1, 5) };
    Ok(OperationBudget {
        value_items,
        value_bytes,
        host_requests: 0,
        sign_items: 32,
        maximum_value_bytes: MAX_TEXT_BYTES,
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    let payloads: &[&[u8]] = if invalid(placement)? {
        &[&[0xff]]
    } else {
        &[b"Hello"]
    };
    let values = payloads
        .iter()
        .map(|payload| {
            values
                .store(payload)
                .map_err(|error| format!("store test text: {error:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(InstalledOperation::TestTextSource(
        TestTextSourceOperation { values, next: 0 },
    ))
}
