//! Storage accounting only: fixture Line references establish no transport or
//! authority. The original normal lowering/verification remains authoritative.
use conduit_core::*;
use conduit_plan_lowering::{
    fragment_set::{lowered_local_fragment_set_backing_bytes, LoweredFragmentSet},
    lowering::{
        fragment_lowering_layout, lower_plan_fragment_from_plan, LoweredPlanFragment, LoweringError,
    },
};
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "../../core/tests/common/sealed_state.rs"]
mod common;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

#[path = "../../core/tests/common/selected_fore.rs"]
mod selected_fore;
use selected_fore::line;
fn plan() -> Plan {
    selected_fore::plan(common::fragment())
}
fn line_bytes(line: &AdmittedLine, capacity: bool) -> usize {
    let b = &line.binding;
    let strings = [
        &line.line_id.0,
        &b.binding_id.0,
        &b.source.host_id.0,
        &b.source.boot_id.0,
        &b.source.endpoint_id.0,
        &b.sink.host_id.0,
        &b.sink.boot_id.0,
        &b.sink.endpoint_id.0,
        &b.base.0,
        &b.base_instance_id.0,
    ];
    let mut bytes: usize = strings
        .into_iter()
        .map(|s| if capacity { s.capacity() } else { s.len() })
        .sum();
    if let LinkCredentialReference::Opaque(id) = &b.credential {
        bytes += if capacity {
            id.0.capacity()
        } else {
            id.0.len()
        };
    }
    if let LinkAuthorityReference::Grant(id) = &b.authority {
        bytes += if capacity {
            id.0.capacity()
        } else {
            id.0.len()
        };
    }
    bytes
}

#[test]
fn selected_fore_line_adds_exactly_one_owned_clone_without_layout_allocation() {
    let base = plan();
    let base_layout = fragment_lowering_layout(&base.fragments[0]).unwrap();
    for credential in [false, true] {
        for grant in [false, true] {
            let mut fragment = base.fragments[0].clone();
            let line = line(&fragment, credential, grant);
            let expected = line_bytes(&line, false);
            assert!(line_bytes(&line, true) > expected); // source spare capacity is not cloned
            fragment.fore_ports[1].selected_line = Some(line.clone());
            let (layout, observed) =
                allocation_probe::observe(|| fragment_lowering_layout(&fragment));
            assert_eq!((observed.allocations, observed.reallocations), (0, 0));
            let layout = layout.unwrap();
            assert_eq!(
                layout.retained_owned_bytes - base_layout.retained_owned_bytes,
                expected
            );
            assert_eq!(layout.refusal_owned_bytes, base_layout.refusal_owned_bytes);
            let (copied, observed) = allocation_probe::observe(|| line.clone());
            assert_eq!(copied, line);
            assert_eq!(line_bytes(&copied, true), expected);
            assert_eq!(observed.requested_bytes, expected);
            assert_eq!(
                observed.allocations,
                10 + usize::from(credential) + usize::from(grant)
            );
            assert_eq!(observed.reallocations, 0);
        }
    }
}

#[test]
fn normal_materializer_preserves_line_and_accounted_retained_capacities() {
    let mut fragment = plan().fragments.remove(0);
    let selected = line(&fragment, true, true);
    fragment.fore_ports[1].selected_line = Some(selected.clone());
    let plan = common::seal(fragment);
    assert!(verify_plan(&plan));
    assert!(local_plan_verification_backing_bytes(&plan).is_some());
    let layout = fragment_lowering_layout(&plan.fragments[0]).unwrap();
    let (mut lowered, activations) =
        lower_plan_fragment_from_plan(&plan, &plan.fragments[0].fragment_id).unwrap();
    drop(activations);
    assert_eq!(
        lowered.fore_ports[1].selected_line.as_ref(),
        Some(&selected)
    );
    assert!(lowered.remote_endpoints.is_empty());
    // Existing inspector deliberately excludes selected Lines. Move ONLY that
    // owner out for an independent census of its exact String capacities, then
    // restore it; this is not a funded-local-fragment-set acceptance claim.
    let selected = lowered.fore_ports[1].selected_line.take().unwrap();
    let mut set = LoweredFragmentSet {
        state_slots: 0,
        nodes: 1,
        cords: 2,
        queue_slots: 2,
        value_bytes: 2,
        sign_items: lowered.sign_items,
        sign_bytes: lowered.sign_bytes,
        partitions: vec![lowered],
    };
    let (base_bytes, observed) =
        allocation_probe::observe(|| lowered_local_fragment_set_backing_bytes(&set));
    assert_eq!((observed.allocations, observed.reallocations), (0, 0));
    let retained = base_bytes.unwrap()
        - set.partitions.capacity() * std::mem::size_of::<LoweredPlanFragment>()
        + line_bytes(&selected, true);
    assert_eq!(retained, layout.retained_owned_bytes);
    set.partitions[0].fore_ports[1].selected_line = Some(selected);
    assert!(lowered_local_fragment_set_backing_bytes(&set).is_none());
}

#[test]
fn state_and_constrained_fore_families_still_refuse_without_allocation() {
    let mut fragment = plan().fragments.remove(0);
    fragment.fore_ports[1].selected_line = Some(line(&fragment, true, true));
    fragment.states = common::retained_fragment().states;
    let (result, observed) = allocation_probe::observe(|| fragment_lowering_layout(&fragment));
    assert_eq!(result, Err(LoweringError::UnsupportedOwnedLayout));
    assert_eq!((observed.allocations, observed.reallocations), (0, 0));
    fragment.states.clear();
    fragment.fore_ports[1].value_contract = Some(CheckedValueContract {
        value_kind: kind_id("fixture/byte@1"),
        maximum_bytes: 1,
        constraints: vec![ValueConstraint::ByteLength {
            minimum: 0,
            maximum: 1,
        }],
    });
    let (result, observed) = allocation_probe::observe(|| fragment_lowering_layout(&fragment));
    assert_eq!(result, Err(LoweringError::UnsupportedOwnedLayout));
    assert_eq!((observed.allocations, observed.reallocations), (0, 0));
}
