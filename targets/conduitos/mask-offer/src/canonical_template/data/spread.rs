//! Canonical native Mask data; regenerated from ordinary planning.
use super::*;
pub(super) fn construct() -> PlannedGear {
    PlannedGear {
        placement_id: "bb80d3911353a3d6fad132b7bc21ab4e98a8b7f8414c26278b751fd82e4d4445".into(),
        gear_id: "native-graphical/spread".into(),
        kind_id: "face/tee".into(),
        kind_contract_revision: "conduit.face/tee@1".into(),
        source_span: Some(SourceSpan {
            start: 202,
            end: 225,
            line: 7,
            column: 13,
            end_line: 7,
            end_column: 36,
        }),
        execution_profile_id: "conduitos/bounded-mask@1".into(),
        configuration: Vec::from([ConfigurationEntry {
            key: "label".into(),
            value: ConfigurationValue::Text("Shared Face".into()),
        }]),
        host_id: "canonical/native-template".into(),
        boot_id: "canonical/native-template/boot".into(),
        offer_generation: OfferGeneration(1),
        capability_id: "renderer-conduitos-tee".into(),
        implementation_id: "conduit.face/tee-kernel@1".into(),
        artifact_id: "conduitos/face-tee@1".into(),
        realization_characteristics: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 524288,
        },
        inputs: Vec::from([PortDescriptor {
            port_id: "source".into(),
            value_kind: "face/face@1".into(),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }]),
        outputs: Vec::from([PortDescriptor {
            port_id: "face".into(),
            value_kind: "face/face@1".into(),
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
            laws: Vec::new(),
        },
        host_calls: Vec::new(),
        resources: Vec::new(),
        authority: Vec::new(),
        pool_references: Vec::new(),
        base: None,
        realization_properties: Vec::new(),
        terminal_transductions: Vec::new(),
    }
}
