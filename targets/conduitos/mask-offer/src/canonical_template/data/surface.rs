//! Canonical native Mask data; regenerated from ordinary planning.
use super::*;
pub(super) fn construct() -> PlannedGear {
    PlannedGear {
        placement_id: "71b907f7982840d85d6e9087a88cd7f18a950619a0aa4eae4b8aad9f8da39a2b".into(),
        gear_id: "native-graphical/surface".into(),
        kind_id: "face/show-resource-source".into(),
        kind_contract_revision: "conduit.face/show-resource-source@1".into(),
        source_span: Some(SourceSpan {
            start: 239,
            end: 283,
            line: 8,
            column: 14,
            end_line: 8,
            end_column: 58,
        }),
        execution_profile_id: "conduitos/bounded-mask@1".into(),
        configuration: Vec::from([ConfigurationEntry {
            key: "label".into(),
            value: ConfigurationValue::Text("Output resource".into()),
        }]),
        host_id: "canonical/native-template".into(),
        boot_id: "canonical/native-template/boot".into(),
        offer_generation: OfferGeneration(1),
        capability_id: "renderer-conduitos-resource-source".into(),
        implementation_id: "face/conduitos-surface-source@1".into(),
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
        inputs: Vec::new(),
        outputs: Vec::from([PortDescriptor {
            port_id: "show-resource".into(),
            value_kind: "face/show-resource@1".into(),
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
        host_calls: Vec::new(),
        resources: Vec::from([ResourceBinding {
            pool_id: "canonical/surface".into(),
            class_id: "face.resource/show@1".into(),
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
