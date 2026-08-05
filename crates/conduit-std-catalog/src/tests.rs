use alloc::collections::BTreeMap;
use alloc::vec;

use super::{
    contract_revision, execution_profile, find_contract, standard_contracts,
    standard_host_advertisement, standard_host_operation_requirements,
    standard_profile_catalog, standard_registry, standard_resource_offers,
    standard_resource_requirements, FILTER_KIND, FORMAT_KIND, GENERIC_VALUE_KIND, LATEST_KIND,
    MAP_KIND, PULSE_KIND, SHOW_KIND, TEE_KIND, TICK_KIND,
};
use conduit_core::{
    kind_id, ArtifactId, CapabilityId, CapabilityOffer, ConnectionProvider, HostAdvertisement,
    HostCommand, HostEvent, HostId, HostProfileId, ImplementationId, ObservationKind,
    OfferGeneration, PlatformEffect, PROTOCOL_VERSION,
};
use conduit_form::parse;
use conduit_planner::{plan, PlacementChoice, PlacementChoices};
use conduit_runtime::HostRuntime;

#[test]
fn standard_catalog_contains_the_m4_socket_set() {
    let contracts = standard_contracts();
    let kind_ids = contracts
        .iter()
        .map(|contract| contract.kind_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        kind_ids,
        vec![
            PULSE_KIND,
            SHOW_KIND,
            MAP_KIND,
            FILTER_KIND,
            TEE_KIND,
            FORMAT_KIND,
            TICK_KIND,
            LATEST_KIND
        ]
    );
    for contract in &contracts {
        assert!(!contract.plain_name.is_empty());
        assert!(!contract.summary.is_empty());
        assert!(!contract.example.is_empty());
        assert!(contract.limits.max_active_instances > 0);
        assert!(contract.limits.max_queue_items > 0);
        assert!(contract.limits.max_queue_bytes > 0);
    }
    let map = find_contract(&kind_id(MAP_KIND)).expect("map contract exists");
    assert!(map
        .inputs
        .iter()
        .chain(map.outputs.iter())
        .all(|port| port.value_kind == kind_id(GENERIC_VALUE_KIND)));
}

#[test]
fn contracts_convert_to_form_catalog_without_runtime_kind_changes() {
    let catalog = standard_profile_catalog();
    let form = parse(
        "form 0\n\nstd_catalog {\n pulse: flow/pulse\n show: presentation/show\n pulse > show\n}\n",
        &catalog,
    )
    .expect("existing pulse/show form parses through standard catalog");
    assert_eq!(form.operations.len(), 2);
    assert_eq!(form.connections.len(), 1);

    let flow_form = parse(
        "form 0\n\nstd_flow {\n clock: time/tick\n source: flow/map\n filtered: flow/filter\n split: flow/tee\n latest: state/latest\n formatted: text/format\n clock.tick -> source.in\n source > filtered\n filtered > split\n split.left -> latest.in\n split.right -> formatted.in\n}\n",
        &catalog,
    )
    .expect("new standard flow form parses");
    assert_eq!(flow_form.operations.len(), 6);
    assert_eq!(flow_form.connections.len(), 5);
}

#[test]
fn conformance_fixture_plans_standard_contracts_without_ui() {
    let catalog = standard_profile_catalog();
    let form = parse(
        "form 0\n\nstd_conformance {\n clock: time/tick\n source: flow/map\n filter: flow/filter\n split: flow/tee\n latest: state/latest\n format: text/format\n clock.tick -> source.in\n source > filter\n filter > split\n split.left -> latest.in\n split.right -> format.in\n}\n",
        &catalog,
    )
    .expect("standard conformance form parses");
    let host = conformance_host_advertisement();
    let placements = PlacementChoices {
        by_operation: BTreeMap::from([
            ("source", "flow-map"),
            ("filter", "flow-filter"),
            ("split", "flow-tee"),
            ("latest", "state-latest"),
            ("format", "text-format"),
            ("clock", "time-tick"),
        ])
        .into_iter()
        .map(|(operation, capability)| {
            (
                conduit_core::OperationId::from(operation),
                PlacementChoice {
                    host_id: host.host_id.clone(),
                    capability_id: CapabilityId::from(capability),
                },
            )
        })
        .collect(),
    };
    let plan = plan(
        &form,
        core::slice::from_ref(&host),
        &placements,
        &[ConnectionProvider::Local],
    )
    .expect("standard conformance form plans");
    assert_eq!(plan.fragments.len(), 1);
    let fragment = &plan.fragments[0];
    assert_eq!(fragment.placements.len(), 6);
    assert_eq!(fragment.connections.len(), 5);
    assert!(fragment
        .placements
        .iter()
        .all(|placement| placement.implementation_id.as_str().starts_with("std/")));
}

#[test]
fn hosted_standard_profile_runs_bounded_flow_form_without_ui() {
    let observations = run_hosted_standard_form(
        "form 0\n\nstd_exec {\n clock: time/tick\n map: flow/map\n filter: flow/filter\n split: flow/tee\n latest: state/latest\n format: text/format\n show_latest: presentation/show\n show_text: presentation/show\n clock.count = 1\n clock.period-ms = 0\n clock.tick -> map.in\n map > filter\n filter > split\n split.left -> latest.in\n split.right -> format.in\n latest > show_latest\n format.text -> show_text.signal\n}\n",
        [
            ("clock", "time-tick"),
            ("map", "flow-map"),
            ("filter", "flow-filter"),
            ("split", "flow-tee"),
            ("latest", "state-latest"),
            ("format", "text-format"),
            ("show_latest", "presentation-show"),
            ("show_text", "presentation-show"),
        ],
    );
    assert_completed_plan(&observations);
    assert!(
        observations
            .iter()
            .filter(|observation| matches!(
                observation.kind,
                ObservationKind::ValueAccepted { .. }
            ))
            .count()
            >= 5,
        "latest and format should receive values through tee branches"
    );
}

#[test]
fn hosted_standard_profile_runs_pulse_show_form_without_ui() {
    let observations = run_hosted_standard_form(
        "form 0\n\nstd_pulse_show {\n pulse: flow/pulse\n show: presentation/show\n pulse.count = 1\n pulse.period-ms = 0\n pulse.signal -> show.signal\n}\n",
        [("pulse", "flow-pulse"), ("show", "presentation-show")],
    );
    assert_completed_plan(&observations);
    assert_presented_value(&observations);
}

#[test]
fn hosted_standard_profile_runs_tick_format_show_form_without_ui() {
    let observations = run_hosted_standard_form(
        "form 0\n\nstd_tick_format {\n clock: time/tick\n format: text/format\n show: presentation/show\n clock.count = 1\n clock.period-ms = 0\n clock.tick -> format.in\n format.text -> show.signal\n}\n",
        [
            ("clock", "time-tick"),
            ("format", "text-format"),
            ("show", "presentation-show"),
        ],
    );
    assert_completed_plan(&observations);
    assert!(observations.iter().any(|observation| {
        matches!(
            &observation.kind,
            ObservationKind::ValuePresented { value }
                if value.encoded.as_slice() == b"value:0"
        )
    }));
}

#[test]
fn platform_manifestation_truth_is_explicit() {
    let contracts = standard_contracts();
    let show = contracts
        .iter()
        .find(|contract| contract.kind_id.as_str() == SHOW_KIND)
        .expect("show contract exists");
    assert!(show.browser_manifestation_honest);
    assert!(show.pico_manifestation_honest);
    for contract in contracts
        .iter()
        .filter(|contract| contract.kind_id.as_str() != SHOW_KIND)
    {
        if contract.kind_id.as_str() != PULSE_KIND {
            assert!(!contract.browser_manifestation_honest);
            assert!(!contract.pico_manifestation_honest);
        }
    }
}

fn conformance_host_advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("std-catalog-host"),
        boot_id: conduit_core::BootId::from("std-catalog-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("conduit.std/conformance"),
        resources: standard_resource_offers(16),
        capabilities: vec![
            offer("flow-pulse", PULSE_KIND, "std/pulse-v1"),
            offer("presentation-show", SHOW_KIND, "std/show-v1"),
            offer("flow-map", MAP_KIND, "std/map-v1"),
            offer("flow-filter", FILTER_KIND, "std/filter-v1"),
            offer("flow-tee", TEE_KIND, "std/tee-v1"),
            offer("text-format", FORMAT_KIND, "std/text-format-v1"),
            offer("time-tick", TICK_KIND, "std/time-tick-v1"),
            offer("state-latest", LATEST_KIND, "std/latest-v1"),
        ],
    }
}

fn offer(capability: &str, kind: &str, implementation: &str) -> CapabilityOffer {
    let kind_id = kind_id(kind);
    let contract = find_contract(&kind_id).expect("standard contract exists");
    CapabilityOffer {
        capability_id: CapabilityId::from(capability),
        kind_id: kind_id.clone(),
        kind_contract_revision: contract_revision(&kind_id),
        execution_profile_id: execution_profile(&kind_id),
        implementation_id: ImplementationId::from(implementation),
        artifact_id: ArtifactId::from(alloc::format!("conduit-std-catalog/{kind}").as_str()),
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: standard_host_operation_requirements(
            &kind_id,
            contract.limits.max_queue_bytes,
        ),
        resource_requirements: standard_resource_requirements(&kind_id),
        authority_requirements: vec![],
        limits: conduit_core::CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 4,
            max_queue_bytes: 64,
        },
    }
}

fn placements_for<const N: usize>(
    host: &HostAdvertisement,
    mappings: [(&str, &str); N],
) -> PlacementChoices {
    PlacementChoices {
        by_operation: mappings
            .into_iter()
            .map(|(operation, capability)| {
                (
                    conduit_core::OperationId::from(operation),
                    PlacementChoice {
                        host_id: host.host_id.clone(),
                        capability_id: CapabilityId::from(capability),
                    },
                )
            })
            .collect(),
    }
}

fn run_hosted_standard_form<const N: usize>(
    form_source: &str,
    mappings: [(&str, &str); N],
) -> Vec<conduit_core::Observation> {
    let catalog = standard_profile_catalog();
    let form = parse(form_source, &catalog).expect("executable standard form parses");
    let host = standard_host_advertisement(
        HostId::from("std-catalog-host"),
        conduit_core::BootId::from("std-catalog-boot"),
        OfferGeneration(1),
    );
    let placements = placements_for(&host, mappings);
    let plan = plan(
        &form,
        core::slice::from_ref(&host),
        &placements,
        &[ConnectionProvider::Local],
    )
    .expect("hosted standard form plans");
    let fragment = plan.fragments.first().expect("fragment exists").clone();
    let mut runtime = HostRuntime::new(
        host,
        standard_registry("std").expect("standard registry installs"),
        128,
    );
    let prepared = runtime.handle(HostCommand::Prepare(fragment.clone()));
    assert!(
        prepared.events.iter().any(|event| {
            matches!(
                event,
                HostEvent::Prepared { plan_id } if plan_id == &fragment.plan_id
            )
        }),
        "prepare events: {:?}",
        prepared.events
    );
    drive_runtime(&mut runtime, fragment.plan_id);
    inspect(&mut runtime)
}

fn assert_completed_plan(observations: &[conduit_core::Observation]) {
    assert!(
        observations.iter().any(|observation| {
            matches!(
                observation.kind,
                ObservationKind::PlanTerminal {
                    disposition: conduit_core::TerminalDisposition::Completed
                }
            )
        }),
        "observations: {:?}",
        observations
    );
}

fn assert_presented_value(observations: &[conduit_core::Observation]) {
    assert!(observations
        .iter()
        .any(|observation| matches!(observation.kind, ObservationKind::ValuePresented { .. })));
}

fn drive_runtime(runtime: &mut HostRuntime, plan_id: conduit_core::PlanId) {
    let mut pending = runtime.handle(HostCommand::Activate(plan_id)).effects;
    while let Some(effect) = pending.pop() {
        let output = match effect {
            PlatformEffect::Wait {
                plan_id,
                placement_id,
                ..
            } => runtime.handle(HostCommand::CompleteWait {
                plan_id,
                placement_id,
            }),
            PlatformEffect::PresentValue {
                plan_id,
                active_play_id,
                presentation_id,
                placement_id,
                value,
                ..
            } => runtime.handle(HostCommand::CompletePresentation {
                plan_id,
                active_play_id,
                presentation_id,
                placement_id,
                value,
                success: true,
                message: None,
            }),
            PlatformEffect::TransmitConnection { .. } => {
                panic!("standard catalog conformance uses only local connections")
            }
        };
        pending.extend(output.effects);
    }
}

fn inspect(runtime: &mut HostRuntime) -> Vec<conduit_core::Observation> {
    runtime
        .handle(HostCommand::Inspect)
        .events
        .into_iter()
        .find_map(|event| match event {
            HostEvent::Observations { items } => Some(items),
            _ => None,
        })
        .expect("inspect returns observations")
}
