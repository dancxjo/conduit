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

#[test]
fn native_scope_uses_aggregate_body_identity_and_refuses_replacement() {
    use crate::{native_workset, protected_region::BodyRegionBinding};
    let (ids, fixed) = native_workset::tests::fixture();
    let wake = native_workset::tests::wake(&native_workset::inventory());
    let prepared = native_workset::prepare(&wake, &ids, &fixed, "build").unwrap();
    let plan = prepared.plan();
    let plot = native_workset::resident(native_workset::NativePlot::KeyboardCanvas).unwrap();
    let partition = plan.plots.iter().find(|p| p.plot == plot).unwrap();
    let fragment = &partition.plan.fragments[0];
    let play = conduit_body::BodyPlayIdentity::bind(plan, 9);
    let binding = BodyRegionBinding::admit(
        plan,
        &play,
        &plot,
        &fragment.host_id,
        &fragment.boot_id,
        &fragment.execution_regions[0].region_id,
        ProtectionDomainId(1),
    )
    .unwrap();
    let scope = SerialScope::admit_body(plan, &binding, &fixed).unwrap();
    assert_eq!(scope.scope.maximum_operations, 8);
    let pending = SerialScope::prepare_body(
        plan,
        &plot,
        &binding.host,
        &binding.boot,
        &binding.region,
        &fixed,
    )
    .unwrap();
    assert_eq!(pending.scope.play, [0; 32]);
    assert!(pending.current_body(&binding, Some(1)).is_err());
    let activated = pending.activate_body(plan, &binding).unwrap();
    assert_eq!(activated.scope, scope.scope);
    assert!(activated.activate_body(plan, &binding).is_err());
    let mut wrong_owner = binding.clone();
    wrong_owner.plot = native_workset::resident(native_workset::NativePlot::MemoryLantern).unwrap();
    assert!(scope.current_body(&wrong_owner, Some(1)).is_err());
    assert!(pending.activate_body(plan, &wrong_owner).is_err());

    assert_eq!(
        scope.scope.plan,
        identity(b"body-plan", &[plan.plan_id.as_str().as_bytes()])
    );
    assert_ne!(
        scope.scope.plan,
        parse_identity(partition.plan.plan_id.as_str()).unwrap()
    );
    assert_eq!(scope.current_body(&binding, Some(1)), Ok(scope.scope));
    assert!(scope.current_body(&binding, Some(2)).is_err());
    let mut other = binding.clone();
    other.active = conduit_body::BodyPlayIdentity::bind(plan, 10);
    assert!(scope.current_body(&other, Some(1)).is_err());
    assert_ne!(
        SerialScope::admit_body(plan, &other, &fixed)
            .unwrap()
            .scope
            .play,
        scope.scope.play
    );
    other.partition_plan = "00".repeat(32).into();
    assert!(SerialScope::admit_body(plan, &other, &fixed).is_err());
}

#[test]
fn current_offer_artifact_must_match_the_sealed_selection() {
    let (plan, binding, mut fixed) = fixture();
    let capability = fixed
        .capabilities
        .iter_mut()
        .find(|capability| capability.implementation == TEXT_PRESENTATION_IMPLEMENTATION)
        .unwrap();
    capability.artifact_build = "replacement-build";
    fixed.validate().unwrap();
    assert!(SerialScope::admit(&plan, &binding, &fixed).is_err());
}

#[test]
fn indicator_and_text_receive_distinct_exact_effect_scopes() {
    let (_, _, fixed) = fixture();
    let ids = BootIdentities {
        host: fixed.host_id,
        boot: fixed.boot_id,
    };
    let prepared = crate::tour_morse_plan::prepare(&ids, &fixed, "build").unwrap();
    let fragment = &prepared.plan.fragments[0];
    let region = &fragment.execution_regions[0].region_id;
    let admit = |presentation| {
        SerialScope::admit_selected(
            &prepared.plan,
            &fragment.host_id,
            &fragment.boot_id,
            region,
            (
                parse_identity(prepared.plan_id.as_str()).unwrap(),
                parse_identity(prepared.active_play.active_play_id.as_str()).unwrap(),
            ),
            &fixed,
            presentation,
        )
        .unwrap()
    };
    let indicator = admit(Presentation::Indicator);
    let text = admit(Presentation::Text);
    assert_eq!(indicator.scope.maximum_operations, 1);
    assert_eq!(
        indicator.scope.maximum_parameter_bytes,
        conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32
    );
    assert_ne!(indicator.scope.implementation, text.scope.implementation);
    assert_ne!(indicator.scope.subject, text.scope.subject);
    assert_ne!(indicator.scope.authority, text.scope.authority);
    assert_eq!(indicator.provider, text.provider);
}

#[test]
fn current_offer_contract_must_match_the_sealed_selection() {
    let (plan, binding, mut fixed) = fixture();
    let capability = fixed
        .capabilities
        .iter_mut()
        .find(|capability| capability.implementation == TEXT_PRESENTATION_IMPLEMENTATION)
        .unwrap();
    capability.contract_revision = "conduit.presentation/text@replacement";
    fixed.validate().unwrap();
    assert!(SerialScope::admit(&plan, &binding, &fixed).is_err());
}
