use super::super::image_test_fixture::{bounds, fixture, SOURCE as PLAN_SOURCE};
use super::*;
use crate::kernel_multivalue::{plan_local, profile_catalog};
use crate::kernel_preparation::KernelResourceReservation;
use conduit_core::{HostAdvertisement, Plan};

#[derive(Clone, Copy, Debug)]
enum Entrance {
    CompletePlans,
    Partitions,
}

impl Entrance {
    fn reserve(
        self,
        ledger: &mut KernelResourceLedger,
        host: &HostAdvertisement,
        plans: &[&Plan],
    ) -> Result<Vec<KernelResourceReservation>, String> {
        match self {
            Self::CompletePlans => ledger.prepare_and_reserve_plans(
                host,
                &plans.iter().map(|plan| (*plan, false)).collect::<Vec<_>>(),
            ),
            Self::Partitions => ledger.prepare_and_reserve_partitions(
                host,
                &plans
                    .iter()
                    .map(|plan| (&plan.fragments[0], false))
                    .collect::<Vec<_>>(),
            ),
        }
    }
}

const ENTRANCES: [Entrance; 2] = [Entrance::CompletePlans, Entrance::Partitions];

fn second_plan(host: &HostAdvertisement) -> Plan {
    let source = PLAN_SOURCE.replace("plot kernel-multivalue {", "plot image-budget-second {");
    let plot = conduit_plot::parse(&source, &profile_catalog()).unwrap();
    plan_local(&plot, host).unwrap()
}

fn snapshot(ledger: &KernelResourceLedger) -> String {
    // Include the entire retained ledger: identities, pool/topology usage,
    // instance charges, selected bounds, and retirement-held reservations.
    format!("{ledger:?}")
}

#[test]
fn both_entrances_refuse_one_short_aggregate_before_charging_either_plan() {
    let (host, first) = fixture();
    let second = second_plan(&host);
    assert_ne!(first.plan_id, second.plan_id);
    let mut limits = bounds();
    limits.active_instances =
        (first.fragments[0].placements.len() + second.fragments[0].placements.len() - 1)
            .try_into()
            .unwrap();
    // Each independently sealed input fits; only their complete sum refuses.
    for plan in [&first, &second] {
        let mut probe = KernelResourceLedger::new(&host).unwrap();
        probe.bind_image_bounds(&limits).unwrap();
        let refusal = probe
            .preflight_image_workset([&plan.fragments[0]])
            .unwrap_err();
        assert!(refusal.contains("complete ordinary managed-memory accounting"));
        assert!(probe.is_idle());
    }
    for entrance in ENTRANCES {
        let mut ledger = KernelResourceLedger::new(&host).unwrap();
        ledger.bind_image_bounds(&limits).unwrap();
        let before = snapshot(&ledger);
        let error = entrance
            .reserve(&mut ledger, &host, &[&first, &second])
            .unwrap_err();
        assert!(
            error.contains("aggregate active_instances"),
            "{entrance:?}: {error}"
        );
        assert_eq!(snapshot(&ledger), before, "{entrance:?}");
        assert!(ledger.is_idle());
    }
}

#[test]
fn both_entrances_refuse_scalar_fit_without_complete_managed_memory_accounting() {
    let (host, plan) = fixture();
    for entrance in ENTRANCES {
        let mut ledger = KernelResourceLedger::new(&host).unwrap();
        ledger.bind_image_bounds(&bounds()).unwrap();
        let before = snapshot(&ledger);
        let error = entrance.reserve(&mut ledger, &host, &[&plan]).unwrap_err();
        assert!(
            error.contains("complete ordinary managed-memory accounting"),
            "{entrance:?}: {error}"
        );
        assert_eq!(snapshot(&ledger), before, "{entrance:?}");
        assert!(ledger.is_idle());
    }
}

#[test]
fn both_entrances_refuse_stale_host_and_offer_generation_without_mutation() {
    let (host, plan) = fixture();
    let second = second_plan(&host);
    assert_ne!(plan.plan_id, second.plan_id);
    let mut stale_host = second.clone();
    stale_host.fragments[0].host_id = "another-image-host".into();
    let mut stale_generation = second;
    stale_generation.fragments[0].offer_generation = OfferGeneration(2);
    for entrance in ENTRANCES {
        for stale in [&stale_host, &stale_generation] {
            let mut ledger = KernelResourceLedger::new(&host).unwrap();
            ledger.bind_image_bounds(&bounds()).unwrap();
            let before = snapshot(&ledger);
            let error = entrance
                .reserve(&mut ledger, &host, &[&plan, stale])
                .unwrap_err();
            assert!(
                error.contains("stale Host/Boot/offer identity"),
                "{entrance:?}: {error}"
            );
            assert_eq!(snapshot(&ledger), before, "{entrance:?}");
            assert!(ledger.is_idle());
        }
    }
}

#[test]
fn selected_bounds_cannot_be_rebound_or_replaced_even_while_idle() {
    let (host, _) = fixture();
    let mut ledger = KernelResourceLedger::new(&host).unwrap();
    ledger.bind_image_bounds(&bounds()).unwrap();
    let before = snapshot(&ledger);
    let mut enlarged = bounds();
    enlarged.active_instances += 1;
    assert!(ledger.bind_image_bounds(&enlarged).is_err());
    assert_eq!(snapshot(&ledger), before);
    assert!(ledger.replacement_for(&host).is_err());
    assert_eq!(snapshot(&ledger), before);
    assert!(ledger.is_idle());
}

#[test]
fn active_reference_reservations_prevent_binding_and_remain_releasable() {
    let (host, plan) = fixture();
    for entrance in ENTRANCES {
        let mut ledger = KernelResourceLedger::new(&host).unwrap();
        let reservations = entrance.reserve(&mut ledger, &host, &[&plan]).unwrap();
        assert!(!ledger.is_idle());
        let before = snapshot(&ledger);
        assert!(ledger.bind_image_bounds(&bounds()).is_err());
        assert_eq!(snapshot(&ledger), before, "{entrance:?}");
        for reservation in reservations {
            ledger.release(reservation).unwrap();
        }
        assert!(ledger.is_idle());
        ledger.bind_image_bounds(&bounds()).unwrap();
        assert!(ledger.is_idle());
    }
}

#[test]
fn scalar_fit_does_not_invent_complete_memory_accounting() {
    let (host, plan) = fixture();
    let mut ledger = KernelResourceLedger::new(&host).unwrap();
    ledger.bind_image_bounds(&bounds()).unwrap();
    let pools = ledger.pools.clone();
    let error = ledger
        .prepare_and_reserve_plans(&host, &[(&plan, false)])
        .unwrap_err();
    assert!(
        error.contains("complete ordinary managed-memory accounting"),
        "{error}"
    );
    assert_eq!(ledger.pools, pools);
    assert!(ledger.is_idle());
}
