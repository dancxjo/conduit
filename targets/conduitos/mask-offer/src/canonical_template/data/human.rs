//! Canonical native Mask data; regenerated from ordinary planning.
use super::*;
pub(super) fn construct() -> PlannedGear {
    PlannedGear {
        placement_id: "dd81120a27a414bbc5ec5c1c69656d03103a54966c504d8acae66c3084decd23".into(),
        gear_id: "native-graphical/human".into(),
        kind_id: "face/interaction".into(),
        kind_contract_revision: "conduit.face/interaction@2".into(),
        source_span: Some(SourceSpan {
            start: 347,
            end: 379,
            line: 10,
            column: 12,
            end_line: 10,
            end_column: 44,
        }),
        execution_profile_id: "conduitos/bounded-interaction@1".into(),
        configuration: Vec::from([ConfigurationEntry {
            key: "label".into(),
            value: ConfigurationValue::Text("User actions".into()),
        }]),
        host_id: "canonical/native-template".into(),
        boot_id: "canonical/native-template/boot".into(),
        offer_generation: OfferGeneration(1),
        capability_id: "renderer-conduitos-input".into(),
        implementation_id: "face/conduitos-human-input@1".into(),
        artifact_id: "conduitos/native-compositor-input@1".into(),
        base: Some(BaseProviderBinding {
            base_id: "canonical/native-surface".into(),
            provider_instance_id: "canonical/native-surface/1".into(),
            provider_generation: 1,
            implementation_id: "conduitos/framebuffer@1".into(),
            mechanism_family: "conduitos.base/framebuffer@1".into(),
            enforcement_class: BaseEnforcementClass::ConduitOsKernelEnforced,
        }),
        realization_characteristics: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 8,
            max_queue_bytes: 65536,
        },
        inputs: Vec::from([
            PortDescriptor {
                port_id: "face".into(),
                value_kind: "face/face@1".into(),
                direction: PortDirection::Input,
                temporal: PortTemporal::Value,
                abnormal_kind: None,
            },
            PortDescriptor {
                port_id: "show".into(),
                value_kind: "face/show@1".into(),
                direction: PortDirection::Input,
                temporal: PortTemporal::Value,
                abnormal_kind: None,
            },
        ]),
        outputs: Vec::from([PortDescriptor {
            port_id: "interaction".into(),
            value_kind: "face/interaction@2".into(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        }]),
        semantic_contract: KindSemanticContract {
            configuration: Vec::from([KindConfigurationField {
                key: "label".into(),
                default_value: ConfigurationValue::Text("".into()),
                rule: KindConfigurationRule::DataLabel { maximum: 128 },
            }]),
            laws: Vec::new(),
        },
        host_calls: Vec::from([HostCallRequirement {
            contract_id: "conduit.host/face-interaction@1".into(),
            target_kind: Some("face/interaction@2".into()),
            maximum_in_flight: 1,
            maximum_input_bytes: 524288,
            maximum_output_bytes: 8192,
        }]),
        resources: Vec::from([ResourceBinding {
            pool_id: "canonical/input".into(),
            class_id: "conduit.resource/conduitos-human-input@1".into(),
            units: 1,
            protected: None,
            compute: None,
            content: None,
        }]),
        authority: Vec::new(),
        pool_references: Vec::new(),
        realization_properties: Vec::new(),
        terminal_transductions: Vec::new(),
    }
}
