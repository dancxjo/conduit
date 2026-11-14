//! Real checked Source/Plan to the existing kernel's bounded topology tables.
use super::*;
use conduit_kernel::{assigned::*, scheduler::CordSpec, NodeId};
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod probe;
#[global_allocator]
static ALLOCATOR: probe::Allocator = probe::Allocator;

#[test]
fn checked_fore_plan_uses_only_its_active_prefix_without_enlarging_the_exact_entrance() {
    let plan = plan(64);
    let generated = generate(&plan);
    let fragment = &plan.fragments[0];
    let maxima = c::AssignedPlanMaxima {
        encoded_bytes: 16384,
        runtime_state_bytes: 32768,
        counts: [32; c::ASSIGNED_PLAN_COUNT_KINDS],
    };
    let bytes = generated.encode(maxima).unwrap();
    let computations = b::assigned_computation_requirements(generated.kernel()).unwrap();
    let profile = c::AssignedComputationProfile {
        maximum_text_bytes: u32::from(u16::MAX),
        maximum_total_text_bytes: u32::MAX,
        requirements: &computations,
    };
    let required = c::AssignedPlanRequirements {
        host: c::AssignedIdentity::from_text(fragment.host_id.as_str()),
        boot: c::AssignedIdentity::from_text(fragment.boot_id.as_str()),
        host_calls: &[],
        resources: &[],
        remote_bindings: &[],
    };
    let identity =
        c::decode_assigned_plan_with_fore(&bytes, maxima, required, profile, generated.fore())
            .unwrap();
    let pins = generated.layout_requirements(maxima).unwrap();
    let layout = ResidentLayout {
        records: &pins,
        counts: identity.counts,
    };
    let ((), memory) = probe::observe(|| {
        let exact = AssignedKernelTables::<1, 2, 1, 1, 1, 0>::prepare_with_fore(
            &bytes,
            maxima,
            required,
            profile,
            &layout,
            generated.fore(),
        )
        .unwrap();
        let bounded = BoundedAssignedKernelTables::<4, 8, 1, 4, 8, 0>::prepare_with_fore(
            &bytes,
            maxima,
            required,
            profile,
            &layout,
            generated.fore(),
        )
        .unwrap();
        assert_eq!(bounded.identity(), exact.identity);
        assert_eq!((bounded.active_nodes(), bounded.active_cords()), (1, 2));
        assert_eq!(&bounded.nodes[..1], &exact.nodes);
        assert_eq!(&bounded.cords[..2], &exact.cords);
        assert!(bounded.nodes[1..].iter().all(|n| n.maximum_step_fuel == 0));
        assert!(bounded.cords[2..]
            .iter()
            .all(|s| *s == CordSpec::inactive()));
        assert_eq!(bounded.startup_order(), &[NodeId(0)]);
        let mut visited = 0;
        let backs = bounded
            .prepare_backs(|node| {
                visited += 1;
                assert_eq!(node, NodeId(0));
                Ok::<_, ()>(node)
            })
            .unwrap();
        assert_eq!(visited, 1);
        assert_eq!(backs, [Some(NodeId(0)), None, None, None]);
        assert_eq!(
            AssignedKernelTables::<4, 8, 1, 4, 8, 0>::prepare_with_fore(
                &bytes,
                maxima,
                required,
                profile,
                &layout,
                generated.fore(),
            )
            .err(),
            Some(AssignedKernelRefusal::Layout)
        );
        assert!(
            BoundedAssignedKernelTables::<0, 8, 1, 4, 8, 0>::prepare_with_fore(
                &bytes,
                maxima,
                required,
                profile,
                &layout,
                generated.fore(),
            )
            .is_err()
        );
        assert!(
            BoundedAssignedKernelTables::<4, 1, 1, 4, 8, 0>::prepare_with_fore(
                &bytes,
                maxima,
                required,
                profile,
                &layout,
                generated.fore(),
            )
            .is_err()
        );
        assert!(
            BoundedAssignedKernelTables::<4, 8, 1, 0, 8, 0>::prepare_with_fore(
                &bytes,
                maxima,
                required,
                profile,
                &layout,
                generated.fore(),
            )
            .is_err()
        );
        assert!(
            BoundedAssignedKernelTables::<4, 8, 1, 4, 0, 0>::prepare_with_fore(
                &bytes,
                maxima,
                required,
                profile,
                &layout,
                generated.fore(),
            )
            .is_err()
        );
        let incomplete = ResidentLayout {
            records: &pins[..pins.len() - 1],
            counts: identity.counts,
        };
        assert_eq!(
            BoundedAssignedKernelTables::<4, 8, 1, 4, 8, 0>::prepare_with_fore(
                &bytes,
                maxima,
                required,
                profile,
                &incomplete,
                generated.fore(),
            )
            .err(),
            Some(AssignedKernelRefusal::Layout)
        );
        assert!(
            BoundedAssignedKernelTables::<4, 8, 1, 4, 8, 0>::prepare_with_fore(
                &bytes,
                maxima,
                required,
                profile,
                &layout,
                &[],
            )
            .is_err()
        );
    });
    assert_eq!(memory.allocations, 0);
    assert_eq!(memory.reallocations, 0);
}
