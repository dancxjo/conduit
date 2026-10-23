//! Retained software owners are real; only the pulse/timer and Boot are CPU fixtures.
use super::*;
use conduit_environment_runtime::appliance::software::SoftwareRefusal as E;
#[test]
fn wrong_current_advertisement_and_duplicate_projection_leave_it_unchanged() {
    let source = compile(1, 0).unwrap();
    let (pulse, _) = pulse::ready(0);
    let mut software = software(&source);
    let host = appliance_support::advertisement(&source, &mut software, &pulse);
    for change in 0..3 {
        let mut invalid = host.clone();
        match change {
            0 => invalid.host_id = "foreign".into(),
            1 => invalid.boot_id = "stale".into(),
            _ => {}
        }
        let before = invalid.clone();
        assert_eq!(
            software.append_to_advertisement(&mut invalid),
            Err(E::Advertisement)
        );
        assert_eq!(invalid, before);
    }
}
#[test]
fn forged_selected_facts_refuse_before_transfer_and_selected_objects_cannot_replay() {
    let source = compile(1, 0).unwrap();
    let (pulse, _) = pulse::ready(0);
    let mut software = software(&source);
    let host = appliance_support::advertisement(&source, &mut software, &pulse);
    let plan = source
        .plan_local(&host, &["conduit.base/local@1".into()], &[pulse::grant()])
        .unwrap();
    let target = plan.fragments[0]
        .placements
        .iter()
        .position(|g| g.kind_id.as_str() == Channel::Reading.kind_id())
        .unwrap();
    let placement = plan.fragments[0].placements[target].placement_id.clone();
    for change in 0..8 {
        let mut fragments = plan.fragments.clone();
        let g = &mut fragments[0].placements[target];
        match change {
            0 => g.artifact_id = "foreign/artifact".into(),
            1 => g.implementation_id = "foreign/implementation".into(),
            2 => g.capability_id = "foreign/capability".into(),
            3 => g.configuration.push(c::ConfigurationEntry {
                key: "extra".into(),
                value: c::ConfigurationValue::U64(1),
            }),
            4 => g.execution_profile_id = "foreign/profile".into(),
            5 => g.limits.max_queue_bytes += 1,
            6 => g.boot_id = "stale".into(),
            _ => g.offer_generation = c::OfferGeneration(2),
        }
        let forged = c::seal_plan_with_realization_backs_and_completion(
            c::PlotIdentity {
                source_document_id: plan.source_document_id.clone(),
                checked_plot_id: plan.checked_plot_id.clone(),
                expanded_plot_id: plan.expanded_plot_id.clone(),
            },
            plan.completion_policy,
            plan.realization_backs.clone(),
            fragments,
        );
        assert!(matches!(
            software.take_selected(&forged.fragments[0], &placement),
            Err(E::Selection)
        ));
    }
    let actual = software
        .take_selected(&plan.fragments[0], &placement)
        .unwrap();
    assert!(matches!(actual.driver, Driver::Publication(_)));
    assert!(matches!(
        software.take_selected(&plan.fragments[0], &placement),
        Err(E::AlreadySelected)
    ));
    let mut fresh = host;
    fresh.capabilities.clear();
    software.append_to_advertisement(&mut fresh).unwrap();
    assert!(!fresh
        .capabilities
        .iter()
        .any(|offer| offer.kind_id.as_str() == Channel::Reading.kind_id()));
}
#[test]
fn unchecked_configuration_and_contract_changes_cannot_initialize_an_available_back() {
    use conduit_environment_runtime::appliance::software::ReadySoftwareBacks;
    let mut source = compile(1, 0).unwrap();
    let expression = source
        .expanded
        .gears
        .iter()
        .position(|g| g.configuration.iter().any(|c| c.key == "program"))
        .unwrap();
    source.expanded.gears[expression]
        .configuration
        .iter_mut()
        .find(|c| c.key == "program")
        .unwrap()
        .value = c::ConfigurationValue::Text("bad".into());
    assert!(matches!(
        ReadySoftwareBacks::prepare(
            &source,
            pulse::identity().host_id,
            pulse::identity().boot_id
        ),
        Err(E::Initialization)
    ));
    let mut source = compile(1, 0).unwrap();
    source
        .expanded
        .gears
        .iter_mut()
        .find(|g| g.kind_id.as_str() == Channel::Reading.kind_id())
        .unwrap()
        .inputs
        .clear();
    assert!(matches!(
        ReadySoftwareBacks::prepare(
            &source,
            pulse::identity().host_id,
            pulse::identity().boot_id
        ),
        Err(E::Contract)
    ));
}
