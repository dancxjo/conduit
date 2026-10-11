//! Ordinary Planner configuration seals a bounded policy without relaxing admission.
mod common;
#[allow(dead_code)]
#[path = "common/local_model_pool.rs"]
mod fixture;
use conduit_core::*;
use conduit_planner::*;
use std::collections::BTreeMap;

fn hosts() -> Vec<HostAdvertisement> {
    let plot = fixture::expanded();
    let providers = common::local_model::dual_local_model_providers();
    vec![
        fixture::consumer_host(&plot.gears[0].checked_front()),
        providers[0].advertisement.clone(),
        providers[1].advertisement.clone(),
    ]
}
fn plan_with(
    hosts: &[HostAdvertisement],
    requirements: &BTreeMap<SharedPoolId, SharedPoolPlanningRequirement>,
    policies: &[SharedPoolPolicySelection<'_>],
) -> Result<Plan, SharedPoolPolicyPlanningError> {
    let plot = fixture::expanded();
    let placements = default_expanded_placements(&plot, hosts).unwrap();
    plan_expanded_canonical_with_shared_pool_policies(
        &plot,
        hosts,
        &placements,
        &["conduit.base/local@1".into()],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 1,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        requirements,
        policies,
    )
}
#[test]
fn policy_choice_is_normal_sealed_planning_with_the_same_admitted_envelope() {
    let hosts = hosts();
    let original = fixture::build_plan(&hosts);
    let unchanged = plan_with(&hosts, &fixture::requirement(), &[]).unwrap();
    assert_eq!(original, unchanged);
    let completion = fixture::build_plan_with_policy(
        &hosts,
        SharedPoolSelectionPolicy::MinimumPredictedCompletionThenCapacity,
    );
    assert!(verify_plan(&completion));
    assert_ne!(original.plan_id, completion.plan_id);
    assert_eq!(original.source_document_id, completion.source_document_id);
    assert_eq!(original.checked_plot_id, completion.checked_plot_id);
    assert_eq!(original.expanded_plot_id, completion.expanded_plot_id);
    assert_eq!(original.fragments.len(), completion.fragments.len());
    for (before, after) in original.fragments.iter().zip(&completion.fragments) {
        assert_eq!(before.placements, after.placements);
        assert_eq!(before.connections, after.connections);
        for (before, after) in before.shared_pools.iter().zip(&after.shared_pools) {
            let mut expected = before.clone();
            expected.selection_policy =
                SharedPoolSelectionPolicy::MinimumPredictedCompletionThenCapacity;
            assert_eq!(&expected, after);
        }
    }
}
#[test]
fn policy_inputs_refuse_duplicates_unknown_pools_and_unbounded_extent() {
    let hosts = hosts();
    let id = fixture::expanded().shared_pools[0].pool_id.clone();
    let selected = SharedPoolPolicySelection {
        pool_id: &id,
        policy: SharedPoolSelectionPolicy::MinimumPredictedCompletionThenCapacity,
    };
    assert!(matches!(
        plan_with(&hosts, &fixture::requirement(), &[selected; 33]),
        Err(SharedPoolPolicyPlanningError::InputLimit)
    ));
    assert!(matches!(
        plan_with(&hosts, &fixture::requirement(), &[selected; 2]),
        Err(SharedPoolPolicyPlanningError::DuplicatePool)
    ));
    let foreign: SharedPoolId = "foreign/pool".into();
    assert!(matches!(
        plan_with(
            &hosts,
            &fixture::requirement(),
            &[SharedPoolPolicySelection {
                pool_id: &foreign,
                ..selected
            }]
        ),
        Err(SharedPoolPolicyPlanningError::PoolOutsidePlot)
    ));
    let empty: SharedPoolId = "".into();
    assert!(matches!(
        plan_with(
            &hosts,
            &fixture::requirement(),
            &[SharedPoolPolicySelection {
                pool_id: &empty,
                ..selected
            }]
        ),
        Err(SharedPoolPolicyPlanningError::InvalidIdentity)
    ));
}
#[test]
fn choosing_completion_cannot_bypass_authority_resources_or_member_bounds() {
    let hosts = hosts();
    let id = fixture::expanded().shared_pools[0].pool_id.clone();
    let selected = [SharedPoolPolicySelection {
        pool_id: &id,
        policy: SharedPoolSelectionPolicy::MinimumPredictedCompletionThenCapacity,
    }];
    let mut requirements = fixture::requirement();
    requirements
        .get_mut(&id)
        .unwrap()
        .admission_authority
        .boot_id = "foreign/boot".into();
    assert!(matches!(
        plan_with(&hosts, &requirements, &selected),
        Err(SharedPoolPolicyPlanningError::Planning(
            PlannerError::InvalidSharedPool(_)
        ))
    ));
    let mut requirements = fixture::requirement();
    requirements
        .get_mut(&id)
        .unwrap()
        .member_limits
        .queue_item_capacity = 0;
    assert!(matches!(
        plan_with(&hosts, &requirements, &selected),
        Err(SharedPoolPolicyPlanningError::Planning(
            PlannerError::InvalidSharedPool(_)
        ))
    ));
    let mut missing_resources = hosts;
    for host in &mut missing_resources[1..] {
        host.resources.clear();
    }
    assert!(matches!(
        plan_with(&missing_resources, &fixture::requirement(), &selected),
        Err(SharedPoolPolicyPlanningError::Planning(
            PlannerError::InvalidSharedPool(_)
        ))
    ));
}

#[test]
fn completion_envelopes_accept_32_and_refuse_33_realizations() {
    for maximum in [32, 33] {
        let plot = fixture::expanded_with_maximum(maximum);
        let provider = common::local_model::dual_local_model_providers()[0]
            .advertisement
            .clone();
        let mut hosts = vec![fixture::consumer_host(&plot.gears[0].checked_front())];
        for index in 0..maximum {
            let mut host = provider.clone();
            host.host_id = format!("host/scoped-{index:02}").into();
            host.boot_id = format!("boot/scoped-{index:02}/1").into();
            hosts.push(host);
        }
        let placements = default_expanded_placements(&plot, &hosts).unwrap();
        let connection_bases = BTreeMap::new();
        let line_candidates = BTreeMap::new();
        let options = PlanningOptions {
            connection_bases: &connection_bases,
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: 1,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        };
        let requirements = fixture::requirement();
        let reference = plan_expanded_canonical_with_shared_pools(
            &plot,
            &hosts,
            &placements,
            &["conduit.base/local@1".into()],
            options,
            &requirements,
        )
        .unwrap();
        assert_eq!(
            reference.fragments[0].shared_pools[0]
                .realization_envelope
                .len(),
            usize::from(maximum)
        );
        let result = plan_expanded_canonical_with_shared_pool_policies(
            &plot,
            &hosts,
            &placements,
            &["conduit.base/local@1".into()],
            options,
            &requirements,
            &[SharedPoolPolicySelection {
                pool_id: &plot.shared_pools[0].pool_id,
                policy: SharedPoolSelectionPolicy::MinimumPredictedCompletionThenCapacity,
            }],
        );
        if maximum == 32 {
            assert!(verify_plan(&result.unwrap()));
        } else {
            assert!(matches!(
                result,
                Err(SharedPoolPolicyPlanningError::CompletionEnvelopeLimit)
            ));
        }
    }
}
