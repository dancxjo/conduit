use super::*;
use crate::{
    identity::BootIdentities,
    offer::{BaseLifecycle, CpuFeatures},
    protection_domain::ProtectionDomainId,
};

fn fixture() -> (Plan, RegionBinding, HostOffer<'static>) {
    let ids = BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    };
    let fixed = HostOffer::new(
        &ids,
        "build",
        CpuFeatures {
            sse2: true,
            rdrand: true,
            invariant_tsc: true,
        },
        256 * 1024,
    );
    let prepared = crate::tour_timer_plan::prepare(&ids, &fixed, "build").unwrap();
    let binding = RegionBinding::admit(
        &prepared.plan,
        &prepared.active_play,
        &prepared.plan.fragments[0].execution_regions[0].region_id,
        ProtectionDomainId(4),
    )
    .unwrap();
    (prepared.plan, binding, fixed)
}

#[test]
fn tour_timer_scope_seals_both_physical_providers_in_the_selected_plan() {
    let (plan, binding, mut fixed) = fixture();
    let scope = TimerScope::admit(&plan, &binding, &fixed).unwrap();
    let selected = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_time::TIME_EVERY_KIND)
        .unwrap()
        .base
        .as_ref()
        .unwrap();
    assert_eq!(
        selected.implementation_id.as_str(),
        crate::ordinary_base::TIMER_PROVIDER_IMPLEMENTATION
    );
    assert_ne!(
        selected.provider_instance_id.as_str(),
        crate::identity::hex(&scope.timer.provider_instance_id)
    );
    assert_eq!(scope.scope.maximum_parameter_bytes, 8);
    assert_eq!(scope.scope.maximum_operations, 2);
    assert_eq!(scope.scope.maximum_in_flight, 1);
    assert_eq!(scope.scope.maximum_work_units, 1);
    assert_eq!(scope.scope.operation, TIMER_WAIT_OPERATION);
    assert_eq!(scope.current(&binding, Some(1), Some(1)), Ok(scope.scope));
    for epochs in [
        (None, Some(1)),
        (Some(1), None),
        (Some(2), Some(1)),
        (Some(1), Some(2)),
    ] {
        assert_eq!(
            scope.current(&binding, epochs.0, epochs.1),
            Err(DomainRefusal::WrongBinding)
        );
    }
    fixed
        .bases
        .iter_mut()
        .find(|base| base.kind == BaseKind::Clock)
        .unwrap()
        .provider_generation += 1;
    let ids = BootIdentities {
        host: fixed.host_id,
        boot: fixed.boot_id,
    };
    let replacement = crate::tour_timer_plan::prepare(&ids, &fixed, "build").unwrap();
    assert_ne!(replacement.plan.plan_id, plan.plan_id);
    assert!(TimerScope::admit(&plan, &binding, &fixed).is_err());
}

#[test]
fn tour_timer_scope_refuses_independent_timer_clock_offer_and_artifact_replacements() {
    for kind in [BaseKind::Timer, BaseKind::Clock] {
        for change in 0..5 {
            let (plan, binding, mut fixed) = fixture();
            let base = fixed
                .bases
                .iter_mut()
                .find(|base| base.kind == kind)
                .unwrap();
            match change {
                0 => base.provider_generation += 1,
                1 => base.provider_instance_id[0] ^= 1,
                2 => base.id[0] ^= 1,
                3 => base.lifecycle = BaseLifecycle::Lost,
                4 => base.lifecycle = BaseLifecycle::Revoked,
                _ => unreachable!(),
            }
            assert!(
                TimerScope::admit(&plan, &binding, &fixed).is_err(),
                "{kind:?} change {change}"
            );
        }
    }
    for change in 0..4 {
        let (plan, binding, mut fixed) = fixture();
        match change {
            0 => fixed.generation += 1,
            1 => fixed.boot_id[0] ^= 1,
            2 => fixed.capabilities[12].artifact_build = "replacement-build",
            3 => fixed.capabilities[12].contract_revision = "replacement-contract",
            _ => unreachable!(),
        }
        assert!(
            TimerScope::admit(&plan, &binding, &fixed).is_err(),
            "offer change {change}"
        );
    }
}
