//! Portable sink Fore for canonical resident interactions, separate from events.
use alloc::vec;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, Kind, KindIdentity, KindSemanticLaw, KindTerminalBehavior,
    PortDescriptor, PortDirection, PortTemporal,
};

pub const PATCHBAY_INTERACTION_KIND: &str = "patchbay/accept-interaction";
pub const PATCHBAY_INTERACTION_REVISION: &str = "conduit.patchbay/accept-interaction@1";

/// An owner consumes canonical commands from one current Show's closing Flow.
/// Preparing this Fore does not identify an owner or make any action available.
/// The realization must bind current owner evidence and admit complete storage.
pub fn patchbay_interaction_contract() -> Kind {
    Kind {
        kind_id: kind_id(PATCHBAY_INTERACTION_KIND),
        kind_contract_revision: KindIdentity::from(PATCHBAY_INTERACTION_REVISION),
        startup_parameters: vec![],
        shorthand: None,
        inputs: vec![PortDescriptor {
            port_id: port_id("interaction"),
            value_kind: kind_id(conduit_face::FACE_INTERACTION_VALUE_KIND),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        }],
        outputs: vec![],
        configuration: vec![],
        semantic_laws: vec![KindSemanticLaw::Terminal(
            KindTerminalBehavior::CompletesWhenInputsClose,
        )],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_face::MAX_FACE_INTERACTION_BYTES as u32,
        },
    }
}
