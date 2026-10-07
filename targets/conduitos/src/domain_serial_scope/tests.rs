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
    let prepared = crate::ordinary_plan::prepare(&ids, &fixed, "build").unwrap();
    let region = &prepared.plan.fragments[0].execution_regions[0].region_id;
    let binding = RegionBinding::admit(
        &prepared.plan,
        &prepared.active_play,
        region,
        ProtectionDomainId(1),
    )
    .unwrap();
    (prepared.plan, binding, fixed)
}

#[test]
fn ordinary_source_selects_an_exact_ready_provider_and_finite_gate_scope() {
    let (plan, binding, fixed) = fixture();
    let admitted = SerialScope::admit(&plan, &binding, &fixed).unwrap();
    assert_eq!(admitted.scope.host, fixed.host_id);
    assert_eq!(admitted.scope.boot, fixed.boot_id);
    assert_eq!(admitted.scope.maximum_operations, 1);
    assert_eq!(admitted.scope.maximum_in_flight, 1);
    assert_eq!(admitted.scope.maximum_parameter_bytes, 256);
    assert_eq!(admitted.current(&binding, Some(1)), Ok(admitted.scope));
    for generation in [None, Some(0), Some(2)] {
        assert_eq!(
            admitted.current(&binding, generation),
            Err(DomainRefusal::WrongBinding)
        );
    }
    let mut other_play = binding;
    other_play.active.active_play_id = "00".repeat(32).into();
    assert_eq!(
        admitted.current(&other_play, Some(1)),
        Err(DomainRefusal::WrongBinding)
    );
}

#[test]
fn exact_selected_provider_and_offer_replacements_refuse_before_issuance() {
    for change in 0..8 {
        let (plan, binding, mut fixed) = fixture();
        let base = fixed
            .bases
            .iter_mut()
            .find(|base| base.kind == BaseKind::Serial)
            .unwrap();
        match change {
            0 => base.provider_generation += 1,
            1 => base.provider_instance_id[0] ^= 1,
            2 => base.id[0] ^= 1,
            3 => base.lifecycle = BaseLifecycle::Lost,
            4 => base.lifecycle = BaseLifecycle::Revoked,
            5 => fixed.boot_id[0] ^= 1,
            6 => fixed.generation += 1,
            7 => fixed.capabilities[4].implementation = "other",
            _ => unreachable!(),
        }
        assert!(
            SerialScope::admit(&plan, &binding, &fixed).is_err(),
            "change {change}"
        );
    }
}
