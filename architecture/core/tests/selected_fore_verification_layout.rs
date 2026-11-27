//! Selected Fore Line verifier scratch only: fixture bindings do not establish
//! a live Line, authenticated peer or possession of the referenced authority.
use conduit_core::*;
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "common/sealed_state.rs"]
mod common;
#[path = "common/selected_fore.rs"]
mod selected_fore;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn plan(opaque_bytes: usize) -> Plan {
    let mut fragment = selected_fore::plan(common::fragment()).fragments.remove(0);
    let mut line = selected_fore::line(&fragment, true, true);
    line.binding.credential =
        LinkCredentialReference::Opaque(CredentialReferenceId::from("c".repeat(opaque_bytes)));
    line.binding.authority =
        LinkAuthorityReference::Grant(AuthorityGrantId::from("g".repeat(opaque_bytes)));
    fragment.fore_ports[1].selected_line = Some(line);
    common::seal(fragment)
}

#[test]
fn selected_fore_line_descriptor_allocates_nothing_and_ignores_borrowed_line_length() {
    let short = plan(16);
    let long = plan(64 * 1024);
    let (short_bound, observation) =
        allocation_probe::observe(|| local_plan_verification_backing_bytes(&short));
    assert_eq!((observation.allocations, observation.reallocations), (0, 0));
    let (long_bound, observation) =
        allocation_probe::observe(|| local_plan_verification_backing_bytes(&long));
    assert_eq!((observation.allocations, observation.reallocations), (0, 0));
    assert_eq!(short_bound.unwrap(), long_bound.unwrap());
}

#[test]
fn original_verifier_peak_fits_bound_with_long_opaque_credential_and_grant() {
    for length in [16, 4096, 64 * 1024] {
        let plan = plan(length);
        let admitted = local_plan_verification_backing_bytes(&plan).unwrap();
        let (accepted, observation) = allocation_probe::observe(|| verify_plan(&plan));
        assert!(accepted);
        assert!(
            observation.peak_bytes <= admitted,
            "length={length}, {observation:?}, admitted={admitted}"
        );
        assert_eq!(observation.reallocations, 0);
    }
}

#[test]
fn retained_state_and_constrained_fore_refusals_remain_allocation_free() {
    let mut plan = plan(4096);
    plan.fragments[0].states = common::fragment().states;
    let (bound, observed) =
        allocation_probe::observe(|| local_plan_verification_backing_bytes(&plan));
    assert_eq!(bound, None);
    assert_eq!((observed.allocations, observed.reallocations), (0, 0));
    plan.fragments[0].states.clear();
    plan.fragments[0].fore_ports[1].value_contract = Some(CheckedValueContract {
        value_kind: kind_id("fixture/byte@1"),
        maximum_bytes: 1,
        constraints: vec![ValueConstraint::ByteLength {
            minimum: 0,
            maximum: 1,
        }],
    });
    let (bound, observed) =
        allocation_probe::observe(|| local_plan_verification_backing_bytes(&plan));
    assert_eq!(bound, None);
    assert_eq!((observed.allocations, observed.reallocations), (0, 0));
}
