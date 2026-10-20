//! Dependency reuse changes preparation ownership, never Plan/Play acceptance.
use super::*;
use conduit_plot::{
    CanonicalBackCatalog, ExpandedAuthoringPlot, PreparationCacheLimits, SourcePreparationCache,
};
use std::sync::Arc;

fn source(limit: &str) -> String {
    format!("plot project (\n >> value: Quantity <= 1800B\n result: NearQuantity <= 16384B >>\n) {{\n boundary: math/near(limit={limit})\n value >> boundary.value\n boundary.result >> result\n}}\n")
}
fn plan_from(host: &mut StdHost, authored: &ExpandedAuthoringPlot) -> Result<Plan, String> {
    host.prepare_numerical_realizations(&authored.expanded)?;
    let hosts = [host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(&authored.expanded, &hosts)
        .map_err(|e| format!("{e:?}"))?;
    let boundaries = [
        (PortDirection::Input, "value"),
        (PortDirection::Output, "result"),
    ]
    .into_iter()
    .map(|(direction, port)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: port.into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: NUMERICAL_NEAR_MAX_BYTES,
            },
        )
    })
    .collect();
    conduit_planner::plan_expanded_authoring_with_options(
        authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from(LOCAL_BASE_IMPLEMENTATION_ID)],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: NUMERICAL_NEAR_MAX_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .map_err(|e| format!("{e:?}"))
}
fn run(
    advertisement: &conduit_core::HostAdvertisement,
    plan: &Plan,
    bytes: Vec<u8>,
) -> (
    Vec<Vec<u8>>,
    Result<crate::StdKernelExecutionReport, String>,
) {
    let mut host = StdHost::from_advertisement(advertisement.clone()).unwrap();
    let mut outputs = Outputs(Vec::with_capacity(1));
    let result = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "value".into(),
                track: ConnectionTrack::Payload,
                bytes,
            }],
            &mut outputs,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .map(|report| report.kernel.unwrap());
    (outputs.0, result)
}

#[test]
fn dependency_cache_preserves_std_plans_kernel_results_and_refusals() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_numerical_catalogs(&mut startup, &mut profile).unwrap();
    let startup = Arc::new(startup);
    let profile = Arc::new(profile);
    let backs = Arc::new(CanonicalBackCatalog::new());
    let mut cache = SourcePreparationCache::new(PreparationCacheLimits::default()).unwrap();
    let mut previous = None;
    for case in 0..6 {
        let source = source(if case >= 4 { "0.0002m" } else { "0.0001m" });
        let mut current_startup = (*startup).clone();
        let mut current_profile = (*profile).clone();
        if case > 0 {
            let name = format!("unused/cache-corpus-{case}");
            current_startup
                .insert(conduit_plot::KindSignature {
                    kind: name.clone(),
                    startup_parameters: vec![],
                })
                .unwrap();
            current_profile
                .insert_kind(conduit_core::Kind {
                    kind_id: name.into(),
                    kind_contract_revision: "unused@1".into(),
                    startup_parameters: vec![],
                    shorthand: None,
                    configuration: vec![],
                    inputs: vec![],
                    outputs: vec![],
                    semantic_laws: vec![],
                    limits: conduit_core::CapabilityLimits {
                        max_active_instances: 1,
                        max_queue_items: 1,
                        max_queue_bytes: 1,
                    },
                })
                .unwrap();
        }
        let current_startup = Arc::new(current_startup);
        let current_profile = Arc::new(current_profile);
        let checked =
            check_syntax_document(&parse_syntax_document(&source), &current_startup).unwrap();
        let cold = conduit_plot::expand_canonical_plot_for_authoring_with_backs(
            &checked,
            "project",
            &current_profile,
            &backs,
        )
        .unwrap();
        let token = cache.check(&source, &current_startup).unwrap();
        let incremental = cache
            .expand(&token, "project", &current_profile, &backs)
            .unwrap();
        assert_eq!(**token.checked(), checked);
        assert_eq!(*incremental, cold);
        if case == 1 {
            let (prior_checked, prior_expanded) = previous.as_ref().unwrap();
            assert!(Arc::ptr_eq(prior_checked, token.checked()));
            assert!(Arc::ptr_eq(prior_expanded, &incremental));
        }
        previous = Some((token.checked().clone(), incremental.clone()));
        let mut host = StdHost::new();
        let cold_plan = plan_from(&mut host, &cold).unwrap();
        let cached_plan = plan_from(&mut host, &incremental).unwrap();
        assert_eq!(cold_plan, cached_plan);
        assert!(verify_plan(&cached_plan));
        let mut advertisement = host.advertisement().clone();
        if case == 2 {
            advertisement.boot_id = "cache-corpus/fresh-boot".into();
            let mut fresh = StdHost::from_advertisement(advertisement.clone()).unwrap();
            assert_eq!(
                plan_from(&mut fresh, &cold).unwrap(),
                plan_from(&mut fresh, &incremental).unwrap()
            );
        }
        let input = if case == 3 { vec![0xff, 0x00] } else { third() };
        let expected = run(host.advertisement(), &cold_plan, input.clone());
        let actual = run(host.advertisement(), &cached_plan, input);
        assert_eq!(
            actual, expected,
            "exact output, complete kernel evidence or refusal, case {case}"
        );
        if case == 3 {
            assert!(actual.1.is_err());
        } else {
            assert!(actual.1.is_ok());
        }
        let mut absent = host.advertisement().clone();
        absent.capabilities.clear();
        let cold_refusal =
            conduit_planner::default_expanded_placements(&cold.expanded, &[absent.clone()]);
        let cached_refusal =
            conduit_planner::default_expanded_placements(&incremental.expanded, &[absent]);
        assert!(cold_refusal.is_err());
        assert_eq!(cold_refusal, cached_refusal);
    }
    assert_eq!(cache.stats().checks, 2);
    assert_eq!(cache.stats().expansions, 2);
    assert_eq!(cache.stats().check_hits, 4);
    assert_eq!(cache.stats().expansion_hits, 4);
}
