//! Canonical native Mask data; regenerated from ordinary planning.
use super::*;
pub(super) fn construct() -> PlannedGear {
    PlannedGear {
        placement_id: "5ffb809dfaeca106917d7288bb0f329a4820fa7b5d3ae60eaf668cf245a50f7b".into(),
        gear_id: "native-graphical/render".into(),
        kind_id: "face/resource-renderer".into(),
        kind_contract_revision: "conduit.face/resource-renderer@1".into(),
        source_span: Some(SourceSpan {
            start: 296,
            end: 335,
            line: 9,
            column: 13,
            end_line: 9,
            end_column: 52,
        }),
        execution_profile_id: "conduitos/bounded-mask@1".into(),
        configuration: Vec::from([ConfigurationEntry {
            key: "label".into(),
            value: ConfigurationValue::Text("Resource view".into()),
        }]),
        host_id: "canonical/native-template".into(),
        boot_id: "canonical/native-template/boot".into(),
        offer_generation: OfferGeneration(1),
        capability_id: "renderer-conduitos".into(),
        implementation_id: "face/renderer-conduitos-native@1".into(),
        artifact_id: "conduitos/native-compositor@1".into(),
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
            max_queue_items: 1,
            max_queue_bytes: 524288,
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
                port_id: "show-resource".into(),
                value_kind: "face/show-resource@1".into(),
                direction: PortDirection::Input,
                temporal: PortTemporal::Value,
                abnormal_kind: None,
            },
        ]),
        outputs: Vec::from([PortDescriptor {
            port_id: "show".into(),
            value_kind: "face/show@1".into(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }]),
        semantic_contract: KindSemanticContract {
            configuration: Vec::from([KindConfigurationField {
                key: "label".into(),
                default_value: ConfigurationValue::Text("".into()),
                rule: KindConfigurationRule::DataLabel { maximum: 128 },
            }]),
            laws: Vec::from([KindSemanticLaw::ResourcePorts(Vec::from([
                ResourcePortContract {
                    port_id: "show-resource".into(),
                    class_id: "face.resource/show@1".into(),
                    ownership: ResourcePortOwnership::Move,
                    lifecycle: ResourcePortLifecycle::Play,
                    mobility: ResourcePortMobility::HostLocal,
                },
            ]))]),
        },
        host_calls: Vec::from([HostCallRequirement {
            contract_id: "conduit.host/present@1".into(),
            target_kind: Some("face/base/conduitos-surface@1".into()),
            maximum_in_flight: 1,
            maximum_input_bytes: 49152,
            maximum_output_bytes: 49152,
        }]),
        resources: Vec::new(),
        authority: Vec::from([AuthorityBinding {
            grant_id: "conduitos/native-mask/present".into(),
            contract_id: "conduit.authority/present@1".into(),
            host_call_contract_id: "conduit.host/present@1".into(),
            subject_kind: "face/base/conduitos-surface@1".into(),
            host_id: "canonical/native-template".into(),
            boot_id: "canonical/native-template/boot".into(),
            capability_id: "renderer-conduitos".into(),
        }]),
        pool_references: Vec::new(),
        realization_properties: Vec::new(),
        terminal_transductions: Vec::new(),
    }
}
