//! Browser realization of the shared finite named-pattern template store.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserBack;
use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ConfigurationValue,
    ExecutionProfileId, HostCallContractId, HostCallRequirement, ImplementationId, PlannedGear,
    ResourceRequirement, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, HostedValueStore, PortId, ValueRef, ValueStorage,
};

pub(crate) const HOST_CALL: &str = "conduit.host/browser-named-pattern-storage@1";
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
    let contract = conduit_semantic_catalog::named_pattern_template_storage_semantic_contract();
    let target_kind = contract.kind_id.clone();
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(IMPLEMENTATION),
            execution_profile_id: ExecutionProfileId::from(
                "browser/named-pattern-storage-kernel-hosted@1",
            ),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-browser-runtime/named-pattern-storage@1"),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(HOST_CALL),
                target_kind: Some(target_kind),
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
        },
    )
    .build()
}

fn initializer_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_semantic_catalog::named_pattern_template_initializer_semantic_contract(),
        Back {
            capability_id: CapabilityId::from(INITIALIZER_IMPLEMENTATION),
            execution_profile_id: ExecutionProfileId::from(
                "browser/named-pattern-template-initializer@1",
            ),
            implementation_id: ImplementationId::from(INITIALIZER_IMPLEMENTATION),
            artifact_id: ArtifactId::from(
                "conduit-browser-runtime/named-pattern-template-initializer@1",
            ),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
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

fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserBack, String> {
    let maximum = validate(placement)?;
    Ok(BrowserBack::installed_step(
        conduit_semantic_catalog::TemplateStorageBack::new(
            maximum,
            MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        ),
    ))
}

fn prepare_initializer(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
) -> Result<BrowserBack, String> {
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
    Ok(BrowserBack::installed_step(TemplateInitializerBack {
        command: stored,
        emitted: false,
    }))
}

struct TemplateInitializerBack {
    command: ValueRef,
    emitted: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for TemplateInitializerBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.emitted {
            return StepOutcome::Complete;
        }
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        io.send(PortId(0), self.command)
            .expect("ready template initializer output");
        self.emitted = true;
        StepOutcome::Progress
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
    fn offer_preserves_the_portable_front_and_one_finite_slot() {
        let contract = conduit_semantic_catalog::named_pattern_template_storage_definition();
        let offer = offer();
        assert_eq!(offer.inputs, contract.inputs);
        assert_eq!(offer.outputs, contract.outputs);
        assert_eq!(offer.resource_requirements.len(), 1);
        assert_eq!(offer.resource_requirements[0].units, 1);
    }
}
