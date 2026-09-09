//! Browser realization of the shared finite named-pattern template store.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityOffer, ConfigurationValue, ExecutionProfileId,
    FaceStartupParameter, HostOperationContractId, HostOperationRequirement, ImplementationId,
    ImplementationOffer, PlannedGear, ResourceRequirement, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_kernel::{
    Failure, FailureCode, HostedValueStore, Operation, OperationAction, OperationInput, PortId,
    ValueRef, ValueStorage,
};

pub(crate) const HOST_OPERATION: &str = "conduit.host/browser-named-pattern-storage@1";
pub(crate) const IMPLEMENTATION: &str = "browser/kernel-named-pattern-storage@1";
pub(crate) const RESOURCE_CLASS: &str = "conduit.resource/named-pattern-storage-slot@1";
const INITIALIZER_IMPLEMENTATION: &str = "browser/kernel-named-pattern-template-initializer@1";

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};
pub(super) static INITIALIZER: BrowserInstallation = BrowserInstallation {
    implementation_id: INITIALIZER_IMPLEMENTATION,
    offer: initializer_offer,
    prepare: prepare_initializer,
    perform: None,
};

pub(crate) struct PreparedTemplateStore {
    store: conduit_semantic_catalog::BoundedTemplateStore,
}

impl PreparedTemplateStore {
    pub(crate) fn for_placement(placement: &PlannedGear) -> Result<Option<Self>, String> {
        if placement.implementation_id.as_str() != IMPLEMENTATION {
            return Ok(None);
        }
        validate(placement)?;
        Ok(Some(Self {
            store: conduit_semantic_catalog::BoundedTemplateStore::prepare(),
        }))
    }

    pub(crate) fn execute<'a>(&'a mut self, input: &[u8]) -> Result<&'a [u8], Failure> {
        self.store.execute(input).map_err(failure)
    }
}

fn offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::named_pattern_template_storage_definition();
    CapabilityOffer {
        startup_parameters: vec![FaceStartupParameter {
            name: "maximum-commands".into(),
            value_type: "Count".into(),
            has_default: true,
        }],
        shorthand: None,
        capability_id: CapabilityId::from(IMPLEMENTATION),
        kind_id: contract.kind_id.clone(),
        kind_contract_revision: contract.kind_contract_revision,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(
                "browser/named-pattern-storage-kernel-hosted@1",
            ),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-browser-runtime/named-pattern-storage@1"),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(HOST_OPERATION),
            target_kind: Some(contract.kind_id),
            maximum_in_flight: 1,
            maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        }],
        resource_requirements: vec![ResourceRequirement {
            content: None,
            class_id: conduit_core::ResourceClassId::from(RESOURCE_CLASS),
            units: 1,
            protected_role: None,
            compute: None,
        }],
        authority_requirements: Vec::new(),
        limits: conduit_core::CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: conduit_semantic_catalog::MAXIMUM_TEMPLATE_STORAGE_COMMANDS as u16,
            max_queue_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32
                * (conduit_semantic_catalog::MAXIMUM_TEMPLATE_STORAGE_COMMANDS as u32 + 1),
        },
    }
}

fn initializer_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::named_pattern_template_initializer_definition();
    CapabilityOffer {
        startup_parameters: vec![
            FaceStartupParameter {
                name: "name".into(),
                value_type: "Text".into(),
                has_default: true,
            },
            FaceStartupParameter {
                name: "normalized-values".into(),
                value_type: "Text".into(),
                has_default: true,
            },
        ],
        shorthand: None,
        capability_id: CapabilityId::from(INITIALIZER_IMPLEMENTATION),
        kind_id: contract.kind_id.clone(),
        kind_contract_revision: contract.kind_contract_revision,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(
                "browser/named-pattern-template-initializer@1",
            ),
            implementation_id: ImplementationId::from(INITIALIZER_IMPLEMENTATION),
            artifact_id: ArtifactId::from(
                "conduit-browser-runtime/named-pattern-template-initializer@1",
            ),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: conduit_core::CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 2,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 2) as u32,
        },
    }
}

fn validate(placement: &PlannedGear) -> Result<u64, String> {
    validate_placement(placement, &offer())?;
    if placement.resources.len() != 1
        || placement.resources[0].class_id.as_str() != RESOURCE_CLASS
        || placement.resources[0].units != 1
    {
        return Err("browser template storage lacks its exact admitted slot".into());
    }
    let maximum = placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("maximum-commands", ConfigurationValue::U64(value)) => Some(*value),
            _ => None,
        })
        .ok_or("browser template storage command bound is absent")?;
    if !(1..=conduit_semantic_catalog::MAXIMUM_TEMPLATE_STORAGE_COMMANDS).contains(&maximum) {
        return Err("browser template storage command bound is outside reviewed limits".into());
    }
    Ok(maximum)
}

fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserOperation, String> {
    let maximum = validate(placement)?;
    Ok(BrowserOperation::installed(
        conduit_semantic_catalog::TemplateStorageOperation::new(
            maximum,
            MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        ),
    ))
}

fn prepare_initializer(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &initializer_offer())?;
    let configured = |key| {
        placement
            .configuration
            .iter()
            .find_map(|entry| match (&*entry.key, &entry.value) {
                (found, ConfigurationValue::Text(value)) if found == key => Some(value.as_str()),
                _ => None,
            })
            .ok_or_else(|| format!("template initializer lacks {key}"))
    };
    let name = configured("name")?;
    let normalized = configured("normalized-values")?
        .split(',')
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|_| "invalid normalized pattern")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let pattern = conduit_semantic_catalog::normalized_value(&normalized)
        .map_err(|error| format!("normalized pattern: {error:?}"))?;
    let command = conduit_semantic_catalog::put_and_get_template_command(name, pattern)
        .map_err(|error| format!("template put and get: {error:?}"))?;
    let stored = values
        .store(
            &command
                .canonical_bytes()
                .map_err(|error| format!("template command: {error:?}"))?,
        )
        .map_err(|error| format!("store template command: {error:?}"))?;
    Ok(BrowserOperation::installed(TemplateInitializerOperation {
        command: stored,
        emitted: false,
    }))
}

struct TemplateInitializerOperation {
    command: ValueRef,
    emitted: bool,
}

impl Operation for TemplateInitializerOperation {
    fn start(&mut self) -> OperationAction {
        self.emitted = true;
        OperationAction::Emit {
            port: PortId(0),
            value: self.command,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        let _ = input;
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail: 265,
        })
    }

    fn advance(&mut self) -> OperationAction {
        debug_assert!(self.emitted);
        OperationAction::Complete
    }
}

fn failure(refusal: conduit_semantic_catalog::TemplateStoreRefusal) -> Failure {
    use conduit_semantic_catalog::TemplateStoreRefusal::*;
    let detail = match refusal {
        Malformed => 1,
        DuplicateName => 2,
        Full => 3,
        CorruptRetainedTemplate => 4,
    };
    Failure {
        code: match refusal {
            Full => FailureCode::StorageExhausted,
            _ => FailureCode::InvalidInput,
        },
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_preserves_the_portable_face_and_one_finite_slot() {
        let contract = conduit_semantic_catalog::named_pattern_template_storage_definition();
        let offer = offer();
        assert_eq!(offer.inputs, contract.inputs);
        assert_eq!(offer.outputs, contract.outputs);
        assert_eq!(offer.resource_requirements.len(), 1);
        assert_eq!(offer.resource_requirements[0].units, 1);
    }
}
