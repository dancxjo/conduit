//! Actual production Root policy with CPU entropy, Boot and Plan fixtures.
use super::*;
use conduit_monotonic_wait as w;
#[path = "../../../../../architecture/plan-lowering/tests/common/native_possession.rs"]
#[allow(dead_code)] // Only the shared scope fixture is used; Root performs issuance.
mod possession;
#[path = "../../../../../targets/conduitos/src/machine_membrane/selection_fixture.rs"]
mod selection;
#[test]
fn wait_issuance_requires_reserved_play_and_exact_independent_cadence() {
    let mut issuer = native_authority::RootIssuer::observe(&boot::ObservedBoot).unwrap();
    let mut scope = possession::scope(conduit_time::TICK_KIND);
    scope.maximum_parameter_bytes = 8;
    scope.maximum_result_bytes = 0;
    let (mut fragment, _, _, _) = selection::selected(
        &scope,
        &conduit_time::tick_semantic_contract(),
        c::WAIT_HOST_CALL_CONTRACT,
        8,
        "wait",
        "wait",
    );
    let config = conduit_time::TickConfiguration {
        count: 3,
        period_ms: 250,
    };
    fragment.placements[0].configuration = vec![
        c::ConfigurationEntry {
            key: "count".into(),
            value: c::ConfigurationValue::U64(3),
        },
        c::ConfigurationEntry {
            key: "period-ms".into(),
            value: c::ConfigurationValue::U64(250),
        },
    ];
    fragment.placements[0].host_calls[0].maximum_output_bytes = 0;
    let fragment = c::seal_plan(
        c::PlotIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_plot_id: fragment.checked_plot_id.clone(),
            expanded_plot_id: fragment.expanded_plot_id.clone(),
        },
        vec![fragment],
    )
    .fragments
    .remove(0);
    let base = fragment.placements[0].base.as_ref().unwrap();
    let native = w::WaitNativeIdentity {
        host_id: scope.host_id,
        boot_id: scope.boot_id,
        base_id: base.base_id.clone(),
        provider_instance_id: scope.base_instance_id,
        provider_generation: scope.base_provider_generation,
        resource_pool_id: scope.resource_pool_id,
        resource_generation_id: scope.resource_generation_id,
        envelope_id: scope.envelope_id,
        artifact_id: fragment.placements[0].artifact_id.clone(),
        implementation_id: scope.implementation_id,
        base_implementation_id: base.implementation_id.clone(),
    };
    let unreserved =
        c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 1);
    assert!(issuer
        .issue_wait(&native, &fragment, &unreserved, &config)
        .is_err());
    let active = issuer.reserve_play(&fragment).unwrap();
    let mut stale = native.clone();
    stale.boot_id = "stale/boot".into();
    assert!(issuer.grant_wait(&stale).is_err());
    assert!(issuer
        .issue_wait(&stale, &fragment, &active, &config)
        .is_err());
    for altered in [
        conduit_time::TickConfiguration {
            count: 4,
            ..config.clone()
        },
        conduit_time::TickConfiguration {
            period_ms: 251,
            ..config.clone()
        },
    ] {
        assert!(issuer
            .issue_wait(&native, &fragment, &active, &altered)
            .is_err());
    }
    let (mut table, mut handle, claim) = issuer
        .issue_wait(&native, &fragment, &active, &config)
        .unwrap();
    let inspection = table.inspections().next().unwrap();
    assert_eq!(inspection.scope.maximum_operations, 3);
    assert_eq!(inspection.scope.maximum_result_bytes, 0);
    assert_eq!(inspection.scope.maximum_work_units, 1);
    assert_eq!(
        inspection.scope.subject_kind.as_str(),
        conduit_time::TICK_KIND
    );
    for _ in 0..3 {
        let lease = table.authorize(&handle, &claim).unwrap();
        table.complete(&mut handle, lease, 0).unwrap();
    }
    assert!(table.authorize(&handle, &claim).is_err());
    assert!(issuer
        .issue_wait(&native, &fragment, &active, &config)
        .is_err());
}
