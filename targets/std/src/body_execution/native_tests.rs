//! Cooperative provider proof: ordinary typed Fore, real ledger, no device IO.
use super::native::*;
use super::*;
use crate::PreparedNativeWork;
use conduit_core::*;
use std::collections::BTreeMap;

struct Provider {
    calls: usize,
    storage: Option<(usize, usize, usize)>,
    stop: Option<RunControl>,
    oversized: bool,
}
impl BodyNativeProvider for Provider {
    fn execute(
        &mut self,
        host: StdHost,
        mut work: PreparedNativeWork,
    ) -> (StdHost, PreparedNativeWork, Result<Vec<u8>, String>) {
        assert!(!host.native_resources_idle());
        host.begin_native_work(&mut work).unwrap();
        assert!(host.begin_native_work(&mut work).is_err());
        assert_eq!(
            work.selected().fragment.plan_id,
            work.selected().active.plan_id
        );
        assert_eq!(
            work.selected().placement_id,
            &work.selected().fragment.placements[0].placement_id
        );
        let storage = (
            work.input().as_ptr() as usize,
            work.selected().fragment.placements.as_ptr() as usize,
            work.selected().lowered.nodes.as_ptr() as usize,
        );
        if let Some(previous) = self.storage {
            assert_eq!(
                previous, storage,
                "native turn allocated replacement descriptor/input storage"
            );
        }
        self.storage = Some(storage);
        self.calls += 1;
        if let Some(control) = &self.stop {
            control
                .request_stop(crate::RunControlRequestId::new("native-stop").unwrap())
                .unwrap();
        }
        let result = if self.oversized {
            vec![0; work.maximum_output_bytes() as usize + 1]
        } else {
            vec![1]
        };
        (host, work, Ok(result))
    }
}
struct Timer;
impl TimerAdapter for Timer {
    fn wait(&mut self, _: std::time::Duration) {}
}
#[derive(Default)]
struct Capture(Vec<ExternalForeDelivery>);
impl BodyForeOutputAdapter for Capture {
    fn deliver(&mut self, value: &ExternalForeDelivery) -> Result<(), String> {
        self.0.push(value.clone());
        Ok(())
    }
}
fn fixture() -> (StdHost, BodyPlan, Wake, Vec<ExternalForeInput>) {
    let mut host = StdHost::new();
    let mut kind = conduit_semantic_catalog::button_indicator_state_semantic_contract();
    kind.kind_id = "test/native-indicator".into();
    kind.kind_contract_revision = "test/native-indicator@1".into();
    kind.inputs[0].temporal = PortTemporal::Flow { closes: true };
    let offer = BackOfferBuilder::new(
        kind.clone(),
        Back {
            capability_id: "test/native-mapper".into(),
            execution_profile_id: "test/native-mapper".into(),
            implementation_id: "test/native-mapper@1".into(),
            artifact_id: "test/native-mapper@1".into(),
            host_calls: vec![HostCallRequirement {
                contract_id: "test/native-map@1".into(),
                target_kind: None,
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES,
                maximum_output_bytes: 1,
            }],
            resource_requirements: vec![resource_requirement("test/native-resource", 1)],
            authority_requirements: vec![],
        },
    )
    .build();
    let base = BaseProviderEntry {
        base_id: "base/test-native".into(),
        provider_instance_id: "base/test-native/instance".into(),
        provider_generation: 1,
        implementation_id: "test/native@1".into(),
        mechanism_family: "test/native".into(),
        enforcement_class: BaseEnforcementClass::Cooperative,
        lifecycle: BaseLifecycle::Ready,
        capabilities: vec![offer],
        resources: vec![resource_offer("test/native", "test/native-resource", 1)],
    };
    // This fixture owns the independently initialized finite Provider above.
    unsafe { host.install_retained_native_base(base, vec![kind.clone()]) }.unwrap();
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    startup
        .insert(conduit_plot::KindSignature {
            kind: kind.kind_id.as_str().to_owned(),
            startup_parameters: Vec::new(),
        })
        .unwrap();
    profiles.insert_kind(kind.clone()).unwrap();
    startup
        .insert_value_kind_alias("Transition", kind.inputs[0].value_kind.clone())
        .unwrap();
    startup
        .insert_value_kind_alias("Indicator", kind.outputs[0].value_kind.clone())
        .unwrap();
    let source = "plot native (\n >> begin: Transition...|\n result: $Indicator >>\n) {\n operation: test/native-indicator()\n begin >> operation.transition\n operation.state >> result\n}\n";
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &startup)
            .unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "native", &profiles).unwrap();
    let placements = conduit_planner::PlacementChoices {
        by_gear: expanded
            .expanded
            .gears
            .iter()
            .map(|gear| {
                (
                    gear.gear_id.clone(),
                    conduit_planner::PlacementChoice {
                        host_id: host.advertisement.host_id.clone(),
                        capability_id: "test/native-mapper".into(),
                    },
                )
            })
            .collect(),
    };
    let mut bounds = BTreeMap::new();
    for (direction, bindings) in [
        (PortDirection::Input, &expanded.input_bindings),
        (PortDirection::Output, &expanded.output_bindings),
    ] {
        for binding in bindings {
            bounds.insert(
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 2,
                    byte_capacity: conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES,
                },
            );
        }
    }
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &expanded,
        &[host.advertisement.clone()],
        &placements,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &bounds,
    )
    .unwrap();
    let body = conduit_body::Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        "test/born".into(),
    )
    .unwrap();
    let wake = body.wake(1, "test/wake".into()).unwrap().1;
    let body_plan = BodyPlan::seal(
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
    let mut encoder =
        conduit_semantic_catalog::PreparedButtonTransitionEncoder::new("fixture").unwrap();
    let inputs = vec![ExternalForeInput {
        front_port_id: "begin".into(),
        track: ConnectionTrack::Payload,
        bytes: encoder.encode(true, 0).unwrap().to_vec(),
    }];
    (host, body_plan, wake, inputs)
}
#[test]
fn native_effect_returns_original_reservation_before_typed_fore_delivery() {
    let (host, plan, wake, inputs) = fixture();
    let control = RunControl::default();
    let mut capture = Capture::default();
    let mut provider = Provider {
        calls: 0,
        storage: None,
        stop: None,
        oversized: false,
    };
    let (host, report) = host.run_body_plan_owned_native_to(
        BodyRunRequest {
            wake: &wake,
            plan: &plan,
            control: &control,
            keyboard: None,
        },
        &inputs,
        &mut capture,
        &mut Vec::new(),
        &mut Timer,
        &mut provider,
        |_, _| Ok(()),
    );
    let report = report.unwrap();
    assert_eq!(
        report.terminal,
        TerminalDisposition::Completed,
        "{:?}",
        report.failure
    );
    assert!(host.native_resources_idle());
    assert_eq!(provider.calls, 1);
    assert_eq!(capture.0.len(), 1);
    assert_eq!(&*capture.0[0].bytes, &[1]);
}
#[test]
fn cancelled_native_return_discards_late_output_and_releases_lease() {
    let (host, plan, wake, inputs) = fixture();
    let control = RunControl::default();
    let mut capture = Capture::default();
    let mut provider = Provider {
        calls: 0,
        storage: None,
        stop: Some(control.clone()),
        oversized: false,
    };
    let (host, report) = host.run_body_plan_owned_native_to(
        BodyRunRequest {
            wake: &wake,
            plan: &plan,
            control: &control,
            keyboard: None,
        },
        &inputs,
        &mut capture,
        &mut Vec::new(),
        &mut Timer,
        &mut provider,
        |_, _| Ok(()),
    );
    let report = report.unwrap();
    assert!(
        matches!(report.terminal, TerminalDisposition::Cancelled { .. }),
        "{:?}",
        report.failure
    );
    assert!(host.native_resources_idle());
    assert!(capture.0.is_empty());
    assert_eq!(provider.calls, 1);
}
#[test]
fn oversized_native_output_is_refused_after_original_lease_returns() {
    let (host, plan, wake, inputs) = fixture();
    let control = RunControl::default();
    let mut capture = Capture::default();
    let mut provider = Provider {
        calls: 0,
        storage: None,
        stop: None,
        oversized: true,
    };
    let (host, report) = host.run_body_plan_owned_native_to(
        BodyRunRequest {
            wake: &wake,
            plan: &plan,
            control: &control,
            keyboard: None,
        },
        &inputs,
        &mut capture,
        &mut Vec::new(),
        &mut Timer,
        &mut provider,
        |_, _| Ok(()),
    );
    let report = report.unwrap();
    assert!(matches!(
        report.terminal,
        TerminalDisposition::Failed { .. }
    ));
    assert!(host.native_resources_idle());
    assert!(capture.0.is_empty());
}

#[test]
fn absent_retained_definition_refuses_before_provider_or_play() {
    let (mut host, plan, wake, inputs) = fixture();
    host.retained_native_calls.clear();
    let sequence = host.next_kernel_play_sequence;
    let control = RunControl::default();
    let mut capture = Capture::default();
    let mut provider = Provider {
        calls: 0,
        storage: None,
        stop: None,
        oversized: false,
    };
    let (host, report) = host.run_body_plan_owned_native_to(
        BodyRunRequest {
            wake: &wake,
            plan: &plan,
            control: &control,
            keyboard: None,
        },
        &inputs,
        &mut capture,
        &mut Vec::new(),
        &mut Timer,
        &mut provider,
        |_, _| panic!("unsupported native work started"),
    );
    assert!(report.is_err());
    assert_eq!(provider.calls, 0);
    assert_eq!(host.next_kernel_play_sequence, sequence);
    assert!(host.native_resources_idle());
}

#[test]
fn start_refusal_returns_host_and_original_partition_reservation() {
    let (host, plan, wake, inputs) = fixture();
    let identity = host.advertisement.clone();
    let control = RunControl::default();
    let mut capture = Capture::default();
    let mut provider = Provider {
        calls: 0,
        storage: None,
        stop: None,
        oversized: false,
    };
    let (host, report) = host.run_body_plan_owned_native_to(
        BodyRunRequest {
            wake: &wake,
            plan: &plan,
            control: &control,
            keyboard: None,
        },
        &inputs,
        &mut capture,
        &mut Vec::new(),
        &mut Timer,
        &mut provider,
        |_, _| Err("fixture lifecycle refusal".into()),
    );
    assert!(report.unwrap_err().contains("fixture lifecycle refusal"));
    assert_eq!(host.advertisement, identity);
    assert_eq!(provider.calls, 0);
    assert!(host.native_resources_idle());
}

#[test]
fn changed_host_boot_refuses_before_native_effect() {
    let (mut host, plan, wake, inputs) = fixture();
    host.advertisement.boot_id = "test/replaced-boot".into();
    let control = RunControl::default();
    let mut capture = Capture::default();
    let mut provider = Provider {
        calls: 0,
        storage: None,
        stop: None,
        oversized: false,
    };
    let (host, report) = host.run_body_plan_owned_native_to(
        BodyRunRequest {
            wake: &wake,
            plan: &plan,
            control: &control,
            keyboard: None,
        },
        &inputs,
        &mut capture,
        &mut Vec::new(),
        &mut Timer,
        &mut provider,
        |_, _| panic!("stale Boot started"),
    );
    assert!(report.is_err());
    assert_eq!(provider.calls, 0);
    assert!(host.native_resources_idle());
    assert!(capture.0.is_empty());
}

#[test]
fn repeated_native_turns_reuse_prepared_metadata_and_input_storage() {
    let (host, plan, wake, mut inputs) = fixture();
    inputs.push(inputs[0].clone());
    let control = RunControl::default();
    let mut capture = Capture::default();
    let mut provider = Provider {
        calls: 0,
        storage: None,
        stop: None,
        oversized: false,
    };
    let (host, report) = host.run_body_plan_owned_native_fore_to(
        BodyRunRequest {
            wake: &wake,
            plan: &plan,
            control: &control,
            keyboard: None,
        },
        &inputs,
        true,
        &mut capture,
        &mut Vec::new(),
        &mut Timer,
        &mut provider,
        |_, _| Ok(()),
    );
    let report = report.unwrap();
    assert_eq!(
        report.terminal,
        TerminalDisposition::Completed,
        "{:?}",
        report.failure
    );
    assert!(host.native_resources_idle());
    assert!(report.cleanup_failure.is_none());
    assert_eq!(provider.calls, 2);
    assert_eq!(capture.0.len(), 2);
}

#[test]
fn classic_runner_refuses_native_work_before_start_or_any_output() {
    let (mut host, plan, wake, inputs) = fixture();
    let sequence = host.next_kernel_play_sequence;
    let control = RunControl::default();
    let mut output = Vec::new();
    let mut capture = Capture::default();
    let error = host
        .run_body_plan_with_fore_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &inputs,
            false,
            &mut capture,
            &mut output,
            &mut Timer,
            |_, _| panic!("classic runner started without native provider"),
        )
        .unwrap_err();
    assert!(error.contains("unsupported"), "{error}");
    assert!(output.is_empty());
    assert!(capture.0.is_empty());
    assert_eq!(host.next_kernel_play_sequence, sequence);
    assert!(host.native_resources_idle());
}

#[test]
fn unprepared_native_route_refuses_execution_support_before_admission() {
    let (host, plan, _, inputs) = fixture();
    let play = BodyPlayIdentity::bind(&plan, host.next_kernel_play_sequence);
    let mut kernel = BodyKernel::prepare_with_live(
        &plan.plots,
        false,
        &play.active_play_id,
        &inputs,
        false,
        true,
        None,
        None,
        #[cfg(target_os = "linux")]
        None,
        &host.retained_native_calls,
    )
    .unwrap();
    assert!(kernel
        .require_supported_execution()
        .unwrap_err()
        .contains("native work has no prepared"));
    assert!(host.native_resources_idle());
    kernel
        .prepare_native_slots(host.advertisement(), play.play_sequence)
        .unwrap();
    kernel.require_supported_execution().unwrap();
    // Preparing descriptive slots still grants no resource possession.
    assert!(host.native_resources_idle());
}
