//! Actual retained native Source -> selected intrinsic -> original Body kernel.
use super::*;
use crate::body_execution::*;
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
struct Source(Arc<AtomicUsize>);
impl crate::body_execution::native::BodyNativeProvider for Source {
    fn execute(
        &mut self,
        host: StdHost,
        mut work: crate::PreparedNativeWork,
    ) -> (StdHost, crate::PreparedNativeWork, Result<Vec<u8>, String>) {
        if let Err(why) = host.begin_native_work(&mut work) {
            return (host, work, Err(why));
        }
        self.0.fetch_add(1, Ordering::SeqCst);
        let output = vec![1 - work.input()[0]];
        (host, work, Ok(output))
    }
}
struct Timer;
impl crate::TimerAdapter for Timer {
    fn wait(&mut self, _: std::time::Duration) {}
}
#[derive(Default)]
struct Capture(Vec<Vec<u8>>);
impl BodyForeOutputAdapter for Capture {
    fn deliver(&mut self, output: &crate::ExternalForeDelivery) -> Result<(), String> {
        self.0.push(output.bytes.to_vec());
        Ok(())
    }
}
fn install_source(host: &mut StdHost, temporal: c::PortTemporal) -> (c::Kind, c::AuthorityGrant) {
    let port = |id: &str, direction| c::PortDescriptor {
        port_id: id.into(),
        value_kind: c::kind_id(c::BOOL_INFO_ID),
        direction,
        temporal,
        abnormal_kind: None,
    };
    let kind = c::Kind {
        startup_parameters: Vec::new(),
        shorthand: Some(("input".into(), "output".into())),
        kind_id: "test/selected-bool-source".into(),
        kind_contract_revision: "test/selected-bool-source@1".into(),
        inputs: vec![port("input", c::PortDirection::Input)],
        outputs: vec![port("output", c::PortDirection::Output)],
        configuration: Vec::new(),
        semantic_laws: Vec::new(),
        limits: c::CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 1,
        },
    };
    let offer = c::BackOfferBuilder::new(
        kind.clone(),
        c::Back {
            capability_id: "test/selected-bool-source".into(),
            execution_profile_id: "test/selected-bool-source@1".into(),
            implementation_id: "test/selected-bool-source@1".into(),
            artifact_id: "test/selected-bool-source@1".into(),
            host_calls: vec![c::HostCallRequirement {
                contract_id: "test/selected-bool-source.read@1".into(),
                target_kind: Some(kind.kind_id.clone()),
                maximum_in_flight: 1,
                maximum_input_bytes: 1,
                maximum_output_bytes: 1,
            }],
            resource_requirements: vec![c::resource_requirement(
                "test/selected-bool-source-slot",
                1,
            )],
            authority_requirements: vec![c::AuthorityRequirement {
                contract_id: "test/selected-bool-source.authority@1".into(),
                host_call_contract_id: "test/selected-bool-source.read@1".into(),
                subject_kind: kind.kind_id.clone(),
            }],
        },
    )
    .build();
    let grant = c::AuthorityGrant {
        grant_id: "test/selected-bool-source-grant".into(),
        contract_id: offer.authority_requirements[0].contract_id.clone(),
        host_call_contract_id: offer.host_calls[0].contract_id.clone(),
        subject_kind: kind.kind_id.clone(),
        host_id: host.advertisement.host_id.clone(),
        boot_id: host.advertisement.boot_id.clone(),
        capability_id: offer.capability_id.clone(),
    };
    let base = c::BaseProviderEntry {
        base_id: "test/selected-bool-source-base".into(),
        provider_instance_id: "test/selected-bool-source-owner".into(),
        provider_generation: 1,
        implementation_id: "test/selected-bool-source-base@1".into(),
        mechanism_family: "test/bounded-cpu-source".into(),
        enforcement_class: c::BaseEnforcementClass::Cooperative,
        lifecycle: c::BaseLifecycle::Ready,
        capabilities: vec![offer],
        resources: vec![c::resource_offer(
            "test/selected-bool-source-pool",
            "test/selected-bool-source-slot",
            1,
        )],
    };
    // SAFETY: the fixture independently retains the matching finite CPU Source
    // provider and grant; installation is not derived from an incoming Plan.
    unsafe { host.install_retained_native_base(base, vec![kind.clone()]) }
        .map_err(|(_, why)| why)
        .unwrap();
    (kind, grant)
}
fn execute_selected_program(temporal: c::PortTemporal) {
    let mut host = host();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut provider_owner = Source(calls.clone());
    let (kind, grant) = install_source(&mut host, temporal);
    let source_payload = |host: &StdHost| {
        host.base_registry
            .entries()
            .iter()
            .find(|entry| entry.base_id.as_str() == "test/selected-bool-source-base")
            .unwrap()
            .capabilities[0]
            .inputs
            .as_ptr()
    };
    let provider = source_payload(&host);
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    startup
        .insert(conduit_plot::KindSignature {
            kind: kind.kind_id.as_str().into(),
            startup_parameters: Vec::new(),
        })
        .unwrap();
    startup
        .insert_value_kind_alias("Bool", c::kind_id(c::BOOL_INFO_ID))
        .unwrap();
    profiles.insert_kind(kind).unwrap();
    let suffix = if temporal == c::PortTemporal::Value {
        ""
    } else {
        "...|"
    };
    let source = format!("plot selected (\n >> begin: Bool{suffix}\n result: Bool{suffix} >>\n) {{\n source: test/selected-bool-source()\n begin >> source.input\n source.output >> (.) >> result\n}}\n");
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&source),
        &startup,
    )
    .unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "selected", &profiles).unwrap();
    let expression = expanded
        .expanded
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() != "test/selected-bool-source")
        .unwrap();
    let c::ConfigurationValue::Text(encoded) = &expression.configuration[0].value else {
        panic!("source-bound canonical program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let selected = match temporal {
        c::PortTemporal::Value => conduit_std_offers::pure_expression_local_offer(&program),
        _ => conduit_std_offers::pure_expression_local_flow_offer(&program),
    }
    .unwrap();
    let requested = [(program, temporal)];
    host.select_expression_programs(&requested, 1_048_576)
        .unwrap();
    assert_eq!(source_payload(&host), provider);
    assert!(selected.host_calls.is_empty());
    let choices = conduit_planner::PlacementChoices {
        by_gear: expanded
            .expanded
            .gears
            .iter()
            .map(|gear| {
                (
                    gear.gear_id.clone(),
                    conduit_planner::PlacementChoice {
                        host_id: host.advertisement.host_id.clone(),
                        capability_id: if gear.kind_id.as_str() == "test/selected-bool-source" {
                            "test/selected-bool-source".into()
                        } else {
                            selected.capability_id.clone()
                        },
                    },
                )
            })
            .collect(),
    };
    let mut bounds = BTreeMap::new();
    for (direction, bindings) in [
        (c::PortDirection::Input, &expanded.input_bindings),
        (c::PortDirection::Output, &expanded.output_bindings),
    ] {
        for binding in bindings {
            bounds.insert(
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: 1,
                },
            );
        }
    }
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &expanded,
        std::slice::from_ref(&host.advertisement),
        &choices,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 1,
            authority_grants: std::slice::from_ref(&grant),
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &bounds,
    )
    .unwrap();
    let local = plan.fragments[0]
        .placements
        .iter()
        .find(|gear| gear.capability_id == selected.capability_id)
        .unwrap();
    assert!(is_local_expression_base(local.base.as_ref().unwrap()));
    // The local factory recognizes only the selected software ownership profile.
    use conduit_composite::KernelOperationFactory;
    let mut wrong = local.clone();
    wrong.base.as_mut().unwrap().provider_instance_id = "other/owner".into();
    assert!(crate::flow_activation::LocalExpressionFactory
        .budget(&wrong)
        .is_err());
    let body = conduit_body::Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        "test/expression-born".into(),
    )
    .unwrap();
    let wake = body.wake(1, "test/expression-wake".into()).unwrap().1;
    let sealed = conduit_body::BodyPlan::seal(
        &wake,
        vec![conduit_body::BodyPlotPlan {
            plot: conduit_body::ResidentPlot::new(
                plan.source_document_id.clone(),
                plan.checked_plot_id.clone(),
            ),
            plan,
        }],
    )
    .unwrap();
    let mut capture = Capture::default();
    let (mut host, report) = host.run_body_plan_owned_native_to(
        BodyRunRequest {
            wake: &wake,
            plan: &sealed,
            control: &crate::RunControl::default(),
            keyboard: None,
        },
        &[crate::ExternalForeInput {
            front_port_id: "begin".into(),
            track: c::ConnectionTrack::Payload,
            bytes: vec![0],
        }],
        &mut capture,
        &mut Vec::new(),
        &mut Timer,
        &mut provider_owner,
        |_, _| Ok(()),
    );
    let report = report.unwrap();
    assert_eq!(
        report.terminal,
        c::TerminalDisposition::Completed,
        "{:?}",
        report.failure
    );
    assert!(report.failure.is_none());
    assert!(report.cleanup_failure.is_none());
    assert_eq!(capture.0, vec![vec![1]]);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(source_payload(&host), provider);
    assert!(host.native_resources_idle());
    // A new selection invalidates this old sealed Plan before another Source
    // call; retain the actual Host, owner and original resource accounting.
    host.select_expression_programs(&requested, 1_048_576)
        .unwrap();
    let generation = host.advertisement.offer_generation;
    assert_ne!(
        sealed.plots[0].plan.fragments[0].offer_generation,
        generation
    );
    let (host, stale) = host.run_body_plan_owned_native_to(
        BodyRunRequest {
            wake: &wake,
            plan: &sealed,
            control: &crate::RunControl::default(),
            keyboard: None,
        },
        &[crate::ExternalForeInput {
            front_port_id: "begin".into(),
            track: c::ConnectionTrack::Payload,
            bytes: vec![0],
        }],
        &mut capture,
        &mut Vec::new(),
        &mut Timer,
        &mut provider_owner,
        |_, _| Ok(()),
    );
    assert!(stale.is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(source_payload(&host), provider);
    assert!(host.native_resources_idle());
}

#[test]
fn selected_value_program_executes_with_retained_source_and_sealed_body_play() {
    execute_selected_program(c::PortTemporal::Value);
}
#[test]
fn selected_closing_flow_program_executes_with_retained_source_and_sealed_body_play() {
    execute_selected_program(c::PortTemporal::Flow { closes: true });
}
