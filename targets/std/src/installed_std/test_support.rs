use super::contract::{self, TICK_ENCODED_LEN, TICK_VALUE_KIND};
use super::test_audio_source;
use super::test_gate;
use super::test_input_semantics;
use super::test_logic;
use super::test_midi_source;
use super::test_scalar_flow;
use super::test_text_source;
use super::tick_operations::{TEST_OBSERVER_IMPLEMENTATION, TEST_OBSERVER_KIND};
use conduit_core::{
    kind_id, present_host_operation_requirement, ArtifactId, CapabilityId, CapabilityLimits,
    CapabilityOffer, ExecutionProfileId, KindContractRevision, PortDescriptor, PortDirection,
};
use conduit_form::KindDefinition;

const TEST_OBSERVER_REVISION: &str = "conduit-test/tick-observer@1";
const TEST_OBSERVER_PROFILE: &str = "conduit-test/tick-observer-kernel@1";
const TEST_OBSERVER_ARTIFACT: &str = "conduit-std-host/test-tick-observer@1";

pub(crate) fn test_observer_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from("test-tick-observer"),
        kind_id: kind_id(TEST_OBSERVER_KIND),
        kind_contract_revision: KindContractRevision::from(TEST_OBSERVER_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(TEST_OBSERVER_PROFILE),
            implementation_id: conduit_core::ImplementationId::from(TEST_OBSERVER_IMPLEMENTATION),
            artifact_id: ArtifactId::from(TEST_OBSERVER_ARTIFACT),
        },
        inputs: vec![PortDescriptor {
            port_id: conduit_core::port_id("in"),
            value_kind: kind_id(TICK_VALUE_KIND),
            direction: PortDirection::Input,
            temporal: conduit_core::PortTemporal::Flow { closes: true },
        }],
        outputs: Vec::new(),
        host_operations: vec![present_host_operation_requirement(
            kind_id("conduit-test/tick-observation"),
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
            kind_id: kind_id(TEST_OBSERVER_KIND),
            kind_contract_revision: KindContractRevision::from(TEST_OBSERVER_REVISION),
            inputs: test_observer_offer().inputs,
            outputs: Vec::new(),
            configuration: Vec::new(),
        })
        .expect("test observer kind is distinct from typed tick");
    test_text_source::install_catalog(&mut catalog);
    test_audio_source::install_catalog(&mut catalog);
    test_midi_source::install_catalog(&mut catalog);
    test_scalar_flow::install_catalog(&mut catalog);
    test_gate::install_catalog(&mut catalog);
    test_input_semantics::install_catalog(&mut catalog);
    test_logic::install_catalog(&mut catalog);
    super::test_timing_sink::install_catalog(&mut catalog);
    super::test_json_codec::install_catalog(&mut catalog);
    let mut startup = conduit_form::StartupCatalog::new();
    conduit_web::install_json_catalogs(&mut startup, &mut catalog)
        .expect("JSON catalogs are exact and unique");
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut catalog)
        .expect("text catalogs are exact and unique");
    conduit_semantic_catalog::install_timing_catalogs(&mut startup, &mut catalog)
        .expect("timing catalogs are exact and unique");
    conduit_semantic_catalog::install_bool_presentation_catalog(&mut catalog)
        .expect("Boolean presentation catalog is exact and unique");
    conduit_semantic_catalog::install_logic_catalogs(&mut startup, &mut catalog)
        .expect("logic catalogs are exact and unique");
    conduit_semantic_catalog::install_math_catalogs(&mut startup, &mut catalog)
        .expect("math catalogs are exact and unique");
    conduit_semantic_catalog::install_quantity_mapping_catalog(&mut startup, &mut catalog)
        .expect("quantity mapping catalog is exact and unique");
    conduit_semantic_catalog::install_layout_catalogs(&mut startup, &mut catalog)
        .expect("layout catalogs are exact and unique");
    conduit_semantic_catalog::install_input_semantic_catalogs(&mut startup, &mut catalog)
        .expect("input semantic catalogs are exact and unique");
    conduit_semantic_catalog::install_presentation_composition_catalogs(&mut startup, &mut catalog)
        .expect("presentation composition catalogs are exact and unique");
    conduit_semantic_catalog::install_graphics_catalogs(&mut startup, &mut catalog)
        .expect("graphics catalogs are exact and unique");
    conduit_semantic_catalog::install_graphics_presentation_catalog(&mut startup, &mut catalog)
        .expect("graphics presentation catalog is exact and unique");
    conduit_presentation::install_bitmap_presentation_catalog(&mut startup, &mut catalog)
        .expect("graphics presentation catalog is exact and unique");
    conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut catalog)
        .expect("sound catalogs are exact and unique");
    catalog
        .insert(KindDefinition {
            kind_id: kind_id("conduit-test/presentation-sink"),
            kind_contract_revision: KindContractRevision::from("conduit-test/presentation-sink@1"),
            inputs: test_presentation_sink_offer().inputs,
            outputs: Vec::new(),
            configuration: Vec::new(),
        })
        .expect("presentation sink is unique");
    catalog
        .insert(KindDefinition {
            kind_id: kind_id("conduit-test/graphics-sink"),
            kind_contract_revision: KindContractRevision::from("conduit-test/graphics-sink@1"),
            inputs: test_graphics_sink_offer().inputs,
            outputs: Vec::new(),
            configuration: Vec::new(),
        })
        .expect("graphics sink is unique");
    catalog
        .insert(KindDefinition {
            kind_id: kind_id("conduit-test/layout-sink"),
            kind_contract_revision: KindContractRevision::from("conduit-test/layout-sink@1"),
            inputs: vec![PortDescriptor {
                port_id: conduit_core::port_id("in"),
                value_kind: kind_id(conduit_presentation::LAYOUT_FRAME_KIND),
                direction: PortDirection::Input,
                temporal: conduit_core::PortTemporal::Value,
            }],
            outputs: Vec::new(),
            configuration: Vec::new(),
        })
        .expect("layout sink is unique");
    conduit_semantic_catalog::install_robotics_catalogs(&mut startup, &mut catalog)
        .expect("robotics catalogs are exact and unique");
    catalog
}

pub(crate) fn test_layout_sink_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from("test-layout-sink"),
        kind_id: kind_id("conduit-test/layout-sink"),
        kind_contract_revision: KindContractRevision::from("conduit-test/layout-sink@1"),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("conduit-test/layout-sink-kernel@1"),
            implementation_id: conduit_core::ImplementationId::from(
                "conduit-test/layout-sink-implementation@1",
            ),
            artifact_id: ArtifactId::from("conduit-std-host/test-layout-sink@1"),
        },
        inputs: vec![PortDescriptor {
            port_id: conduit_core::port_id("in"),
            value_kind: kind_id(conduit_presentation::LAYOUT_FRAME_KIND),
            direction: PortDirection::Input,
            temporal: conduit_core::PortTemporal::Value,
        }],
        outputs: Vec::new(),
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_presentation::MAX_LAYOUT_FRAME_BYTES as u32,
        },
    }
}

pub(crate) fn test_presentation_sink_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from("test-presentation-sink"),
        kind_id: kind_id("conduit-test/presentation-sink"),
        kind_contract_revision: KindContractRevision::from("conduit-test/presentation-sink@1"),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(
                "conduit-test/presentation-sink-kernel@1",
            ),
            implementation_id: conduit_core::ImplementationId::from(
                "conduit-test/presentation-sink-implementation@1",
            ),
            artifact_id: ArtifactId::from("conduit-std-host/test-presentation-sink@1"),
        },
        inputs: vec![PortDescriptor {
            port_id: conduit_core::port_id("in"),
            value_kind: kind_id(conduit_presentation::PRESENTATION_COMPOSITION_KIND),
            direction: PortDirection::Input,
            temporal: conduit_core::PortTemporal::Value,
        }],
        outputs: Vec::new(),
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_presentation::MAX_PRESENTATION_COMPOSITION_BYTES as u32,
        },
    }
}

pub(crate) fn test_graphics_sink_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from("test-graphics-sink"),
        kind_id: kind_id("conduit-test/graphics-sink"),
        kind_contract_revision: KindContractRevision::from("conduit-test/graphics-sink@1"),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("conduit-test/graphics-sink-kernel@1"),
            implementation_id: conduit_core::ImplementationId::from(
                "conduit-test/graphics-sink-implementation@1",
            ),
            artifact_id: ArtifactId::from("conduit-std-host/test-graphics-sink@1"),
        },
        inputs: vec![PortDescriptor {
            port_id: conduit_core::port_id("in"),
            value_kind: kind_id(conduit_presentation::GRAPHICS_SCENE_KIND),
            direction: PortDirection::Input,
            temporal: conduit_core::PortTemporal::Value,
        }],
        outputs: Vec::new(),
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_presentation::MAX_PRESENTATION_COMPOSITION_BYTES as u32,
        },
    }
}
