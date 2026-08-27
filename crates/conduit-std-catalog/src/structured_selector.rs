//! Portable typed contract for one exact checked structured selector.

use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    port_id, CapabilityLimits, FaceStartupParameter, KindContractRevision, KindId, PortDescriptor,
    PortDirection, PortId, PortTemporal, StructuredSelector, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub const STRUCTURED_SELECTOR_REVISION: &str = "structured-info/selector-operation@1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredSelectorContract {
    pub startup_parameters: Vec<FaceStartupParameter>,
    pub shorthand: Option<(PortId, PortId)>,
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub limits: CapabilityLimits,
}

pub fn structured_selector_contract(
    selector: &StructuredSelector,
    temporal: PortTemporal,
) -> StructuredSelectorContract {
    let kind_id = selector
        .kind_id(temporal)
        .expect("checked selector has finite semantic identity");
    let input_kind = selector
        .input_type()
        .profile()
        .expect("checked selector input has finite profile")
        .value_kind()
        .clone();
    let output_kind = selector
        .output_type()
        .profile()
        .expect("checked selector output has finite profile")
        .value_kind()
        .clone();
    StructuredSelectorContract {
        startup_parameters: vec![FaceStartupParameter {
            name: "selector".into(),
            value_type: "Text".into(),
            has_default: false,
        }],
        shorthand: Some((port_id("input"), port_id("output"))),
        kind_id,
        kind_contract_revision: KindContractRevision::from(STRUCTURED_SELECTOR_REVISION),
        inputs: vec![PortDescriptor {
            port_id: port_id("input"),
            value_kind: input_kind,
            direction: PortDirection::Input,
            temporal,
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("output"),
            value_kind: output_kind,
            direction: PortDirection::Output,
            temporal,
        }],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 4,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32,
        },
    }
}
