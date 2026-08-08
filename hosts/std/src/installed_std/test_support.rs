use super::contract::{self, TICK_ENCODED_LEN, TICK_VALUE_KIND};
use super::operation::{TEST_OBSERVER_IMPLEMENTATION, TEST_OBSERVER_KIND};
use super::test_text_source;
use conduit_core::{
    kind_id, present_host_operation_requirement, ArtifactId, CapabilityId, CapabilityLimits,
    CapabilityOffer, ExecutionProfileId, KindContractRevision, PortDescriptor, PortDirection,
};
use conduit_form::KindDefinition;

const TEST_OBSERVER_REVISION: &str = "conduit.test/tick-observer@1";
const TEST_OBSERVER_PROFILE: &str = "conduit.test/tick-observer-kernel@1";
const TEST_OBSERVER_ARTIFACT: &str = "conduit-std-host/test-tick-observer@1";

pub(crate) fn test_observer_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from("test-tick-observer"),
        kind_id: kind_id(TEST_OBSERVER_KIND),
        kind_contract_revision: KindContractRevision::from(TEST_OBSERVER_REVISION),
        execution_profile_id: ExecutionProfileId::from(TEST_OBSERVER_PROFILE),
        implementation_id: conduit_core::ImplementationId::from(TEST_OBSERVER_IMPLEMENTATION),
        artifact_id: ArtifactId::from(TEST_OBSERVER_ARTIFACT),
        inputs: vec![PortDescriptor {
            port_id: conduit_core::port_id("in"),
            value_kind: kind_id(TICK_VALUE_KIND),
            direction: PortDirection::Input,
        }],
        outputs: Vec::new(),
        host_operations: vec![present_host_operation_requirement(
            kind_id("conduit.test/tick-observation"),
            TICK_ENCODED_LEN,
        )],
        resource_requirements: vec![conduit_core::resource_requirement(
            conduit_core::PRESENTATION_RESOURCE_CLASS,
            1,
        )],
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 4,
            max_queue_bytes: 64,
        },
    }
}

pub(crate) fn test_catalog() -> conduit_form::ProfileCatalog {
    let mut catalog = contract::test_tick_catalog();
    catalog
        .insert(KindDefinition {
            kind_id: kind_id(contract::TEXT_PRESENTATION_KIND),
            kind_contract_revision: KindContractRevision::from(
                contract::TEXT_PRESENTATION_CONTRACT_REVISION,
            ),
            inputs: conduit_std_catalog::text_presentation_inputs(),
            outputs: Vec::new(),
            configuration: vec![conduit_form::ConfigurationField {
                key: "maximum-values".to_string(),
                default_value: conduit_core::ConfigurationValue::U64(contract::MAX_TEXT_VALUES),
                validation: conduit_form::ConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: contract::MAX_TEXT_VALUES,
                },
            }],
        })
        .expect("text presentation kind is distinct from typed tick");
    catalog
        .insert(KindDefinition {
            kind_id: kind_id(TEST_OBSERVER_KIND),
            kind_contract_revision: KindContractRevision::from(TEST_OBSERVER_REVISION),
            inputs: test_observer_offer().inputs,
            outputs: Vec::new(),
            configuration: Vec::new(),
        })
        .expect("test observer kind is distinct from typed tick");
    test_text_source::install_catalog(&mut catalog);
    catalog
}
