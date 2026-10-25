//! The same fresh Host/authority and exact Source, through two representations.
use conduit_core as c;
use conduit_environment_make::NativeSoftwareSourceRecipe;
use conduit_environment_runtime::{
    appliance::software::ReadySoftwareBacks,
    construction::{compile, ApplianceSource},
};
use conduit_planner::{ConnectionEndpoints, ConnectionQueueLimits, PlanningOptions};
use std::collections::BTreeMap;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
mod plan {
    pub const PREFIX: &str = "fixture/environment-indexed";
}
#[path = "../../runtime/tests/appliance_support/mod.rs"]
mod appliance_support;
#[allow(
    dead_code,
    reason = "shared inert provider also covers timed execution outcomes"
)]
#[path = "../../runtime/tests/support/pulse/provider.rs"]
mod pulse;
fn limits(source: &ApplianceSource) -> BTreeMap<ConnectionEndpoints, ConnectionQueueLimits> {
    source.expanded.connections.iter().map(|cord| (
        (cord.source_gear_id.clone(), cord.source_port_id.clone(), cord.sink_gear_id.clone(), cord.sink_port_id.clone()),
        ConnectionQueueLimits { item_capacity: 1, byte_capacity: if source.expanded.gears.iter().any(|g| g.gear_id == cord.sink_gear_id && g.kind_id.as_str() == conduit_environment_runtime::publication::PublicationChannel::Reading.kind_id()) {
            conduit_environment_runtime::publication::PublicationChannel::Reading.maximum_bytes() as u32
        } else { 1024 } },
    )).collect()
}
#[test]
fn indexed_appliance_preserves_the_complete_plan_and_refusals_without_loading_the_graph() {
    let source = compile(1, 0).unwrap();
    let recipe = NativeSoftwareSourceRecipe::prepare(&source).unwrap();
    // SAFETY: this exact image was just checked/encoded by trusted Make.
    let image = unsafe {
        conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(
            recipe.planning_image(),
        )
    }
    .unwrap();
    let (ready, effects) = pulse::ready(0);
    let id = pulse::identity();
    let mut software = ReadySoftwareBacks::prepare(&source, id.host_id, id.boot_id).unwrap();
    let host = appliance_support::advertisement(&source, &mut software, &ready);
    let bases = ["conduit.base/local@1".into()];
    let connections = limits(&source);
    let placements = conduit_planner::default_expanded_placements(
        &source.expanded,
        core::slice::from_ref(&host),
    )
    .unwrap();
    let connection_bases = BTreeMap::new();
    let line_candidates = BTreeMap::new();
    for scenario in 0..5 {
        let mut host = host.clone();
        let mut grants = vec![pulse::grant()];
        let mut placements = placements.clone();
        match scenario {
            1 => grants.clear(),
            2 => grants[0].boot_id = "stale/boot".into(),
            3 => host.capabilities.retain(|offer| {
                offer.kind_id.as_str()
                    != conduit_environment_runtime::publication::PublicationChannel::Reading
                        .kind_id()
            }),
            4 => {
                placements.by_gear.values_mut().next().unwrap().host_id = "unknown/host".into();
            }
            _ => {}
        }
        if scenario != 4 {
            let expected = conduit_planner::default_expanded_placements(
                &source.expanded,
                core::slice::from_ref(&host),
            );
            let (selected, memory) = allocation_probe::observe(|| {
                conduit_planner::indexed_source::default_placements(
                    &image,
                    core::slice::from_ref(&host),
                )
            });
            assert_eq!(selected, expected, "selection scenario {scenario}");
            if scenario == 0 {
                assert_eq!(selected.as_ref().unwrap(), &placements);
                assert!(memory.peak_bytes <= 128 * 1024, "{memory:?}");
                eprintln!("indexed_selection_peak={}", memory.peak_bytes);
            }
        }
        let options = PlanningOptions {
            connection_bases: &connection_bases,
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: 1024,
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        };
        let (borrowed, borrowed_memory) = allocation_probe::observe(|| {
            conduit_planner::plan_expanded_canonical_with_connection_limits(
                &source.expanded,
                core::slice::from_ref(&host),
                &placements,
                &bases,
                options,
                &connections,
            )
        });
        let (indexed, indexed_memory) = allocation_probe::observe(|| {
            conduit_planner::indexed_source::plan_with_connection_limits(
                &image,
                core::slice::from_ref(&host),
                &placements,
                &bases,
                options,
                &connections,
            )
        });
        assert_eq!(indexed, borrowed, "scenario {scenario}");
        let (compact, compact_memory) = allocation_probe::observe(|| {
            conduit_planner::indexed_source::plan_compact_with_connection_limits(
                &image,
                core::slice::from_ref(&host),
                &placements,
                &bases,
                options,
                &connections,
            )
        });
        match (&borrowed, compact) {
            (Ok(expected), Ok(compact)) => {
                assert_eq!(compact.plan_id(), &expected.plan_id);
                assert_eq!(compact.materialize().unwrap(), *expected);
                // Fingerprint verification must not replace lowering's exact
                // mandatory sign-storage admission, even for a resealed Plan.
                let mut unfit = expected.clone();
                assert!(unfit.fragments[0].states.is_empty());
                unfit.fragments[0].sign_storage_budget.byte_capacity += 1;
                let mut unfit = c::seal_plan_with_realization_backs_and_completion(
                    c::PlotIdentity {
                        source_document_id: unfit.source_document_id,
                        checked_plot_id: unfit.checked_plot_id,
                        expanded_plot_id: unfit.expanded_plot_id,
                    },
                    unfit.completion_policy,
                    unfit.realization_backs,
                    unfit.fragments,
                );
                assert!(c::verify_plan(&unfit));
                let fragment_id = unfit.fragments[0].fragment_id.clone();
                let refusal = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
                    &unfit,
                    &fragment_id,
                )
                .unwrap_err();
                assert_eq!(
                    refusal,
                    conduit_plan_lowering::lowering::LoweringError::SignBudgetInvalid
                );
                let configurations = unfit
                    .fragments
                    .iter_mut()
                    .flat_map(|f| &mut f.placements)
                    .map(|g| {
                        (
                            g.placement_id.clone(),
                            core::mem::take(&mut g.configuration),
                        )
                    })
                    .collect::<BTreeMap<_, _>>();
                let verified = c::verify_plan_metadata_with_configurations(unfit, |g| {
                    Ok::<_, ()>(configurations[&g.placement_id].clone())
                })
                .unwrap();
                assert_eq!(
                    conduit_plan_lowering::lowering::lower_verified_plan_metadata_fragment(
                        &verified,
                        &fragment_id,
                    )
                    .unwrap_err(),
                    refusal
                );

                for (fi, fragment) in expected.fragments.iter().enumerate() {
                    assert_eq!(compact.fragment_id(fi), Some(&fragment.fragment_id));
                    for (pi, gear) in fragment.placements.iter().enumerate() {
                        compact
                            .with_placement(fi, pi, |complete| assert_eq!(complete, gear))
                            .unwrap();
                    }
                }
                assert!(compact.with_placement(usize::MAX, 0, |_| ()).is_err());
                assert!(
                    compact_memory.peak_bytes <= 192 * 1024,
                    "{compact_memory:?}"
                );
                assert!(
                    compact_memory.peak_bytes < indexed_memory.peak_bytes,
                    "compact={compact_memory:?} indexed={indexed_memory:?}"
                );
                eprintln!(
                    "compact_indexed_planning_peak={}",
                    compact_memory.peak_bytes
                );
            }
            (Err(expected), Err(actual)) => assert_eq!(&actual, expected),
            _ => panic!("compact result differs in scenario {scenario}"),
        }

        if scenario == 0 {
            assert!(c::verify_plan(indexed.as_ref().unwrap()));
            // Include a decoded graph's backing in the comparison window. The
            // indexed path borrows immutable image bytes (flash on the MCU).
            let (owned, owned_memory) = allocation_probe::observe(|| {
                conduit_planner::plan_expanded_canonical_consuming_with_connection_limits(
                    source.expanded.clone(),
                    core::slice::from_ref(&host),
                    &placements,
                    &bases,
                    options,
                    &connections,
                )
            });
            assert_eq!(owned, indexed);
            assert!(indexed_memory.peak_bytes < owned_memory.peak_bytes);
            assert!(
                indexed_memory.peak_bytes <= 224 * 1024,
                "{indexed_memory:?}"
            );
            eprintln!("owned_graph_and_planning_peak={}", owned_memory.peak_bytes);
            eprintln!(
                "indexed_planning_peak={} borrowed_planning_peak={}",
                indexed_memory.peak_bytes, borrowed_memory.peak_bytes
            );
        } else {
            assert!(indexed.is_err());
        }
    }
    assert_eq!(effects.get(), 0);
}

#[test]
fn source_resident_owner_survives_selection_planning_and_lowering_in_one_memory_window() {
    use conduit_environment_runtime::appliance::assigned::PreparedResidentSoftware;
    let source = compile(1, 0).unwrap();
    let recipe = NativeSoftwareSourceRecipe::prepare(&source).unwrap();
    // Trusted Make material stands in for linked flash, outside dynamic RAM.
    let computations = recipe.resident_computations().collect::<Vec<_>>();
    let offer_material = recipe.resident_offers();
    // SAFETY: independently checked and encoded by this same Make recipe.
    let image = unsafe {
        conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(
            recipe.planning_image(),
        )
    }
    .unwrap();
    let (ready, effects) = pulse::ready(0);
    enum KeptPlan {
        Rich(c::Plan),
        Compact(c::VerifiedPlanMetadata),
    }
    let mut reference_lowering = None;
    let mut rich_peak = 0;
    for compact_mode in [false, true] {
        let ((software, host, placements, plan, lowered), memory) =
            allocation_probe::observe(|| {
                let connections = limits(&source);
                let values =
                    conduit_environment_runtime::appliance::prepare_value_storage().unwrap();
                let software = Box::new(
                    PreparedResidentSoftware::prepare_source(&recipe.spec(), &computations, values)
                        .unwrap(),
                );
                let mut host = appliance_support::native_advertisement(&ready);
                {
                    let offers = software
                        .source_offers(&recipe.spec(), &offer_material)
                        .unwrap();
                    for bytes in offers.iter() {
                        host.capabilities
                            .push(serde_json::from_slice(bytes).unwrap());
                    }
                }
                let placements = conduit_planner::indexed_source::default_placements(
                    &image,
                    core::slice::from_ref(&host),
                )
                .unwrap();
                let connection_bases = BTreeMap::new();
                let line_candidates = BTreeMap::new();
                let grants = [pulse::grant()];
                let options = PlanningOptions {
                    connection_bases: &connection_bases,
                    line_candidates: &line_candidates,
                    connection_item_capacity: 1,
                    connection_byte_capacity: 1024,
                    authority_grants: &grants,
                    protected_resource_grants: &[],
                    line_offers: &[],
                };
                let bases = ["conduit.base/local@1".into()];
                let (plan, lowered) = if compact_mode {
                    let compact =
                        conduit_planner::indexed_source::plan_compact_with_connection_limits(
                            &image,
                            core::slice::from_ref(&host),
                            &placements,
                            &bases,
                            options,
                            &connections,
                        )
                        .unwrap();
                    let fragment = compact.fragment_id(0).unwrap().clone();
                    let verified = compact.into_verified_metadata().unwrap();
                    let lowered =
                        conduit_plan_lowering::lowering::lower_verified_plan_metadata_fragment(
                            &verified, &fragment,
                        )
                        .unwrap();
                    (KeptPlan::Compact(verified), lowered)
                } else {
                    let plan = conduit_planner::indexed_source::plan_with_connection_limits(
                        &image,
                        core::slice::from_ref(&host),
                        &placements,
                        &bases,
                        options,
                        &connections,
                    )
                    .unwrap();
                    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
                        &plan,
                        &plan.fragments[0].fragment_id,
                    )
                    .unwrap()
                    .0;
                    (KeptPlan::Rich(plan), lowered)
                };
                (software, host, placements, plan, lowered)
            });
        match &plan {
            KeptPlan::Rich(plan) => assert!(c::verify_plan(plan)),
            KeptPlan::Compact(plan) => assert_eq!(plan.plan_id(), &lowered.identity.plan_id),
        }
        assert_eq!(placements.by_gear.len(), source.expanded.gears.len());
        assert_eq!(
            software
                .source_offers(&recipe.spec(), &offer_material)
                .unwrap()
                .iter()
                .len(),
            host.capabilities.len() - 2
        );
        assert_eq!(effects.get(), 0);
        eprintln!(
            "resident_owner_offers_selection_plan_lowering_compact={compact_mode} peak={}",
            memory.peak_bytes
        );
        assert!(memory.peak_bytes <= 400 * 1024, "{memory:?}");
        if compact_mode {
            assert_eq!(Some(&lowered), reference_lowering.as_ref());
            assert!(memory.peak_bytes < rich_peak, "{memory:?}");
        } else {
            reference_lowering = Some(lowered.clone());
            rich_peak = memory.peak_bytes;
        }

        // This excludes Root/biography, display, radio/SDK and target stack. It is a
        // single-window CPU ledger entry, never complete Xtensa memory admission.
        drop((software, host, placements, plan, lowered));
    }
}
