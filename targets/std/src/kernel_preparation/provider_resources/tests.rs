//! Synthetic initialized-provider accounting, not socket or hardware proof.
use super::*;
use crate::{StdHost, StdHostComposition, StdHostConfig};
use conduit_core::{resource_offer, BaseEnforcementClass, BaseProviderEntry};

fn fixture() -> (StdHost, BaseInstanceId, Vec<ResourceBinding>) {
    let mut host = StdHost::new_with_composition(
        StdHostConfig {
            host_id: "fixture/provider".into(),
            boot_id: "fixture/boot".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        StdHostComposition::minimal(),
    );
    let base: BaseInstanceId = "fixture/provider/session".into();
    let entry = BaseProviderEntry {
        base_id: "fixture/listener".into(),
        provider_instance_id: base.clone(),
        provider_generation: 1,
        implementation_id: "fixture/listener@1".into(),
        mechanism_family: "fixture/listener".into(),
        enforcement_class: BaseEnforcementClass::Cooperative,
        lifecycle: BaseLifecycle::Ready,
        capabilities: vec![],
        resources: vec![
            resource_offer("fixture/session", "fixture/line-session", 1),
            resource_offer(
                "fixture/backing",
                conduit_core::RUNTIME_MEMORY_RESOURCE_CLASS,
                128,
            ),
        ],
    };
    let bindings = entry
        .resources
        .iter()
        .map(|offer| ResourceBinding {
            pool_id: offer.pool_id.clone(),
            class_id: offer.class_id.clone(),
            units: offer.capacity_units,
            protected: None,
            compute: None,
            content: None,
        })
        .collect::<Vec<_>>();
    host.base_registry.register(entry).unwrap();
    host.base_registry
        .project_ready_into(&mut host.advertisement)
        .unwrap();
    host.kernel_resources = KernelResourceLedger::new(&host.advertisement).unwrap();
    (host, base, bindings)
}
fn release(host: &mut StdHost, token: ProviderResourceReservation) {
    // SAFETY: This fixture owns no platform effects or workers.
    if let Err((_, error)) = unsafe { host.release_provider_resources(token) } {
        panic!("fixture retirement refused: {error}");
    }
}
#[test]
fn provider_custody_charges_distinct_classes_and_reuses_only_after_retirement() {
    let (mut host, base, bindings) = fixture();
    let token = host
        .reserve_provider_resources(&base, 1, &bindings)
        .unwrap();
    assert_eq!(token.bindings(), bindings);
    assert!(host
        .reserve_provider_resources(&base, 1, &bindings)
        .is_err());
    assert!(!host.kernel_resources.is_idle());
    release(&mut host, token);
    assert!(host.kernel_resources.is_idle());
    let token = host
        .reserve_provider_resources(&base, 1, &bindings)
        .unwrap();
    // Uncertain ownership is conservatively retained even if a caller drops
    // the accounting token; no automatic release publishes spare capacity.
    drop(token);
    assert!(host
        .reserve_provider_resources(&base, 1, &bindings)
        .is_err());
}
#[test]
fn late_shortage_and_wrong_scope_leave_every_counter_unchanged() {
    let (mut host, base, bindings) = fixture();
    let mut wrong = bindings.clone();
    wrong[1].units = 129;
    assert!(host.reserve_provider_resources(&base, 1, &wrong).is_err());
    assert!(host.kernel_resources.is_idle());
    let mut duplicate = bindings.clone();
    duplicate[1] = duplicate[0].clone();
    assert!(host
        .reserve_provider_resources(&base, 1, &duplicate)
        .is_err());
    let mut wrong_class = bindings.clone();
    wrong_class[0].class_id = "runtime/memory".into();
    assert!(host
        .reserve_provider_resources(&base, 1, &wrong_class)
        .is_err());
    assert!(host
        .reserve_provider_resources(&base, 2, &bindings)
        .is_err());
    host.advertisement.boot_id = "fixture/different-boot".into();
    assert!(host
        .reserve_provider_resources(&base, 1, &bindings)
        .is_err());
    assert!(host.kernel_resources.is_idle());
}
#[test]
fn stale_ledger_token_cannot_release_replacement_with_identical_serialized_identity() {
    let (mut old, base, bindings) = fixture();
    let stale = old.reserve_provider_resources(&base, 1, &bindings).unwrap();
    let (mut replacement, replacement_base, replacement_bindings) = fixture();
    assert_eq!(old.advertisement, replacement.advertisement);
    let current = replacement
        .reserve_provider_resources(&replacement_base, 1, &replacement_bindings)
        .unwrap();
    // SAFETY: Both synthetic providers have no actual effects to retire.
    let (stale, _) = unsafe { replacement.release_provider_resources(stale) }
        .err()
        .unwrap();
    assert!(replacement
        .reserve_provider_resources(&base, 1, &bindings)
        .is_err());
    release(&mut old, stale);
    release(&mut replacement, current);
}

#[test]
fn ordinary_plan_staging_preserves_live_provider_charges_on_success_and_refusal() {
    let (mut host, base, bindings) = fixture();
    let offered = crate::kernel_multivalue::advertisement(
        host.advertisement.host_id.clone(),
        host.advertisement.boot_id.clone(),
        host.advertisement.offer_generation,
    );
    host.advertisement.capabilities = offered.capabilities;
    host.advertisement.resources.extend(offered.resources);
    host.advertisement.resources.sort();
    host.kernel_resources = KernelResourceLedger::new(&host.advertisement).unwrap();
    let plot = conduit_plot::parse(
        include_str!("../../../../../proof/fixtures/plots/kernel-multivalue.conduit"),
        &crate::kernel_multivalue::profile_catalog(),
    )
    .unwrap();
    let plan = crate::kernel_multivalue::plan_local(&plot, &host.advertisement).unwrap();
    let token = host
        .reserve_provider_resources(&base, 1, &bindings)
        .unwrap();
    let child = host
        .kernel_resources
        .prepare_and_reserve(&host.advertisement, &plan.fragments[0])
        .unwrap();
    assert!(host
        .reserve_provider_resources(&base, 1, &bindings)
        .is_err());
    assert!(host
        .kernel_resources
        .prepare_and_reserve(&host.advertisement, &plan.fragments[0],)
        .is_err());
    host.kernel_resources.release(child).unwrap();
    assert!(host
        .reserve_provider_resources(&base, 1, &bindings)
        .is_err());
    release(&mut host, token);
    assert!(host.kernel_resources.is_idle());
}
