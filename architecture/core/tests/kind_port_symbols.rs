//! Actual public borrowed validator with a refused legacy tree-node allocation.
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core::*;
const IDS: [&str; 16] = [
    "p0", "p1", "p2", "p3", "p4", "p5", "p6", "p7", "p8", "p9", "p10", "p11", "p12", "p13", "p14",
    "p15",
];
static LIMITS: CapabilityLimits = CapabilityLimits {
    max_active_instances: 1,
    max_queue_items: 1,
    max_queue_bytes: 1,
};
fn ports() -> [PortDescriptorRef<'static>; 16] {
    core::array::from_fn(|index| PortDescriptorRef {
        port_id: IDS[index],
        value_kind: "u64",
        direction: if index < 8 {
            PortDirection::Input
        } else {
            PortDirection::Output
        },
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    })
}
fn kind<'a>(ports: &'a [PortDescriptorRef<'a>]) -> KindDefinitionRef<'a> {
    let split = ports.len() / 2;
    KindDefinitionRef {
        startup_parameters: &[],
        shorthand: None,
        kind_id: "kind/ports",
        kind_contract_revision: "revision/1",
        inputs: PortDescriptorRefs::Borrowed(&ports[..split]),
        outputs: PortDescriptorRefs::Borrowed(&ports[split..]),
        configuration: &[],
        semantic_laws: &[],
        limits: &LIMITS,
    }
}
#[test]
fn sixteen_ports_validate_with_zero_allocations_when_legacy_node_is_refused() {
    let ports = ports();
    for count in [0, 1, 2, 16] {
        let description = kind(&ports[..count]);
        let (result, memory) = allocation_probe::observe(|| {
            let _refusal = allocation_probe::refuse_next_allocation_size(192);
            description.validate()
        });
        assert_eq!(result, Ok(()));
        assert_eq!(memory.allocations + memory.reallocations, 0);
    }
}
#[test]
fn cross_direction_duplicate_refuses_without_requesting_a_tree_node() {
    let mut ports = ports();
    ports[8].port_id = ports[0].port_id;
    let (result, memory) = allocation_probe::observe(|| {
        let _refusal = allocation_probe::refuse_next_allocation_size(192);
        kind(&ports).validate()
    });
    assert_eq!(result, Err(KindValidationError::DuplicatePortSymbol));
    assert_eq!(memory.allocations + memory.reallocations, 0);
}
