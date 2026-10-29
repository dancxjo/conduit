//! Streamed configuration must not bypass ordinary structural verification.
use conduit_core::*;
#[path = "common/sealed_state.rs"]
mod fixture;
fn full_plan() -> Plan {
    let mut fragment = fixture::fragment();
    fragment.placements[0].configuration = vec![
        ConfigurationEntry {
            key: "program".into(),
            value: ConfigurationValue::Text("é\0immutable".into()),
        },
        ConfigurationEntry {
            key: "count".into(),
            value: ConfigurationValue::U64(3),
        },
    ];
    fixture::seal(fragment)
}
fn separate(mut plan: Plan) -> (Plan, Vec<ConfigurationEntry>) {
    let configuration = core::mem::take(&mut plan.fragments[0].placements[0].configuration);
    (plan, configuration)
}
#[test]
fn verified_metadata_binds_complete_identity_without_retaining_configuration() {
    let full = full_plan();
    assert!(verify_plan(&full));
    let (metadata, configuration) = separate(full.clone());
    assert!(!verify_plan(&metadata)); // Metadata is not a complete public Plan.
    let verified =
        verify_plan_metadata_with_configurations(metadata, |_| Ok::<_, ()>(configuration.clone()))
            .unwrap();
    assert_eq!(verified.plan_id(), &full.plan_id);
    assert_eq!(verified.fragment_count(), 1);
    let view = verified.fragment(&full.fragments[0].fragment_id).unwrap();
    assert!(view.lowering_metadata().placements[0]
        .configuration
        .is_empty());
    let placement = &full.fragments[0].placements[0];
    assert!(view.configuration_matches(&placement.placement_id, &placement.configuration));
    assert!(!view.configuration_matches(&"absent".into(), &placement.configuration));
    assert!(!view.configuration_matches(&placement.placement_id, &[]));
    for mutation in 0..4 {
        let mut expected = placement.configuration.clone();
        match mutation {
            0 => expected.reverse(),
            1 => expected[1].value = ConfigurationValue::U64(4),
            2 => expected[1].value = ConfigurationValue::Bool(true),
            _ => expected[1].key = "other".into(),
        }
        assert!(!view.configuration_matches(&placement.placement_id, &expected));
    }

    assert_eq!(view.lowering_metadata().states, full.fragments[0].states);
    assert!(verified.fragment(&"absent".into()).is_none());
}
#[test]
fn payload_omission_provider_failure_and_mixed_representations_refuse() {
    let full = full_plan();
    assert_eq!(
        verify_plan_metadata_with_configurations(full.clone(), |_| Ok::<_, ()>(vec![]))
            .unwrap_err(),
        PlanMetadataRefusal::RetainedConfiguration
    );
    let (metadata, _) = separate(full);
    assert_eq!(
        verify_plan_metadata_with_configurations(metadata.clone(), |_| Ok::<_, ()>(vec![]))
            .unwrap_err(),
        PlanMetadataRefusal::InvalidPlan
    );
    assert_eq!(
        verify_plan_metadata_with_configurations(metadata, |_| Err::<Vec<ConfigurationEntry>, _>(
            "source-unavailable"
        ))
        .unwrap_err(),
        PlanMetadataRefusal::Configuration("source-unavailable")
    );
}
#[test]
fn canonical_resealing_cannot_hide_invalid_state_structure() {
    for mutation in 0..3 {
        let mut fragment = full_plan().fragments.remove(0);
        match mutation {
            0 => fragment.states[0].maximum_value_bytes = 0,
            1 => fragment.states.push(fragment.states[0].clone()),
            _ => fragment.states[0].gear_id = "absent-gear".into(),
        }
        let full = fixture::seal(fragment);
        assert!(!verify_plan(&full));
        let (metadata, configuration) = separate(full);
        assert_eq!(
            verify_plan_metadata_with_configurations(metadata, |_| Ok::<_, ()>(
                configuration.clone()
            ))
            .unwrap_err(),
            PlanMetadataRefusal::InvalidPlan
        );
    }
}

#[test]
fn an_empty_configuration_is_bound_to_its_own_unique_placement() {
    let plan = fixture::seal(fixture::fragment());
    let fragment = plan.fragments[0].fragment_id.clone();
    let placement = plan.fragments[0].placements[0].placement_id.clone();
    let verified = verify_plan_metadata_with_configurations(plan, |_| Ok::<_, ()>(vec![])).unwrap();
    let view = verified.fragment(&fragment).unwrap();
    assert!(view.configuration_matches(&placement, &[]));
    assert!(!view.configuration_matches(&"absent".into(), &[]));
    assert!(!view.configuration_matches(
        &placement,
        &[ConfigurationEntry {
            key: "count".into(),
            value: ConfigurationValue::U64(0),
        }]
    ));
}
