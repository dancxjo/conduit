//! Authored finite cadence, admitted queue1 and synchronous copied Fore custody.
use super::native::BodyNativeProvider;
use super::*;
use conduit_core::*;
use std::{collections::BTreeMap, time::Duration};
struct Provider(usize);
impl BodyNativeProvider for Provider {
    fn execute(
        &mut self,
        host: StdHost,
        mut work: crate::PreparedNativeWork,
    ) -> (StdHost, crate::PreparedNativeWork, Result<Vec<u8>, String>) {
        assert!(!host.native_resources_idle());
        assert!(work.input().is_empty());
        host.begin_native_work(&mut work).unwrap();
        self.0 += 1;
        (host, work, Ok(vec![1]))
    }
}
#[derive(Default)]
struct Timer(u64);
impl TimerAdapter for Timer {
    fn wait(&mut self, duration: Duration) {
        self.0 += duration.as_millis() as u64;
    }
    fn monotonic_now_ms(&mut self) -> Option<u64> {
        Some(self.0)
    }
}
struct Capture {
    count: usize,
    first_ptr: Option<usize>,
    refuse_at: Option<usize>,
    control: Option<RunControl>,
}
impl BodyBorrowedForeOutputAdapter for Capture {
    fn deliver(&mut self, value: BodyForeDeliveryRef<'_>) -> Result<(), String> {
        assert_eq!(value.front_port_id.as_str(), "result");
        assert_eq!(value.bytes, &[1]);
        let ptr = value.bytes.as_ptr() as usize;
        if let Some(first) = self.first_ptr {
            assert_eq!(first, ptr, "kernel output storage must be recycled");
        }
        self.first_ptr = Some(ptr);
        if self.refuse_at == Some(self.count) {
            return Err("bounded consumer full".into());
        }
        self.count += 1;
        if let Some(control) = &self.control {
            control
                .request_stop(crate::RunControlRequestId::new("stream-stop").unwrap())
                .unwrap();
        }
        Ok(())
    }
}
fn fixture() -> (StdHost, BodyPlan, Wake) {
    let mut host = StdHost::new();
    let kind = Kind {
        kind_id: "test/native-empty-bool".into(),
        kind_contract_revision: "test/native-empty-bool@1".into(),
        startup_parameters: vec![],
        shorthand: None,
        inputs: vec![PortDescriptor {
            port_id: "request".into(),
            value_kind: EMPTY_INFO_ID.into(),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        }],
        outputs: vec![PortDescriptor {
            port_id: "value".into(),
            value_kind: BOOL_INFO_ID.into(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        }],
        configuration: vec![],
        semantic_laws: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 8,
        },
    };
    let offer = BackOfferBuilder::new(
        kind.clone(),
        Back {
            capability_id: "test/native-stream".into(),
            execution_profile_id: "test/native-stream@1".into(),
            implementation_id: "test/native-stream@1".into(),
            artifact_id: "test/native-stream@1".into(),
            host_calls: vec![HostCallRequirement {
                contract_id: "test/native-read@1".into(),
                target_kind: None,
                maximum_in_flight: 1,
                maximum_input_bytes: 0,
                maximum_output_bytes: 1,
            }],
            resource_requirements: vec![resource_requirement("test/native-resource", 1)],
            authority_requirements: vec![],
        },
    )
    .build();
    unsafe {
        host.install_retained_native_base(
            BaseProviderEntry {
                base_id: "base/test-native-stream".into(),
                provider_instance_id: "base/test-native-stream/instance".into(),
                provider_generation: 1,
                implementation_id: "test/native-stream@1".into(),
                mechanism_family: "test/native-stream".into(),
                lifecycle: BaseLifecycle::Ready,
                enforcement_class: BaseEnforcementClass::Cooperative,
                capabilities: vec![offer],
                resources: vec![resource_offer(
                    "test/native-stream",
                    "test/native-resource",
                    1,
                )],
            },
            vec![kind.clone()],
        )
    }
    .unwrap();
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    conduit_time::install_tick_catalog(&mut startup, &mut profiles).unwrap();
    conduit_time::install_tick_trigger_catalog(&mut startup, &mut profiles).unwrap();
    startup
        .insert(conduit_plot::KindSignature {
            kind: kind.kind_id.as_str().into(),
            startup_parameters: vec![],
        })
        .unwrap();
    profiles.insert_kind(kind).unwrap();
    startup
        .insert_value_kind_alias("Indicator", BOOL_INFO_ID.into())
        .unwrap();
    let source = "plot native (\n result: Indicator...| >>\n) {\n clock: time/tick(3, 10)\n trigger: time/tick-trigger\n source: test/native-empty-bool()\n clock.tick >> trigger.tick\n trigger.begin >> source.request\n source.value >> result\n}\n";
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &startup)
            .unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "native", &profiles).unwrap();
    let ads = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&expanded.expanded, &ads).unwrap();
    let bounds = BTreeMap::from([(
        conduit_planner::ForeBoundaryKey {
            direction: PortDirection::Output,
            front_port_id: "result".into(),
            track: ConnectionTrack::Payload,
        },
        conduit_planner::ConnectionQueueLimits {
            item_capacity: 1,
            byte_capacity: 1,
        },
    )]);
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &expanded,
        &ads,
        &placements,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 8,
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
    let plan = BodyPlan::seal(
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
    (host, plan, wake)
}
#[test]
fn authored_three_sample_cadence_reuses_queue_one_output_without_accumulation() {
    let (host, plan, wake) = fixture();
    let control = RunControl::default();
    let mut capture = Capture {
        count: 0,
        first_ptr: None,
        refuse_at: None,
        control: None,
    };
    let mut provider = Provider(0);
    let (host, report) = host.run_body_plan_owned_native_borrowed_to(
        BodyRunRequest {
            wake: &wake,
            plan: &plan,
            control: &control,
            keyboard: None,
        },
        &[],
        &mut capture,
        &mut Vec::new(),
        &mut Timer::default(),
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
    assert_eq!((capture.count, provider.0), (3, 3));
    assert!(report.fore_deliveries.is_empty());
    assert!(report.fore_pending_output.is_none());
    assert!(host.native_resources_idle());
}
#[test]
fn borrowed_consumer_pressure_preserves_exact_pending_output_and_stage() {
    let (host, plan, wake) = fixture();
    let control = RunControl::default();
    let mut capture = Capture {
        count: 0,
        first_ptr: None,
        refuse_at: Some(1),
        control: None,
    };
    let mut provider = Provider(0);
    let (host, report) = host.run_body_plan_owned_native_borrowed_to(
        BodyRunRequest {
            wake: &wake,
            plan: &plan,
            control: &control,
            keyboard: None,
        },
        &[],
        &mut capture,
        &mut Vec::new(),
        &mut Timer::default(),
        &mut provider,
        |_, _| Ok(()),
    );
    let report = report.unwrap();
    assert!(matches!(
        report.terminal,
        TerminalDisposition::Failed { .. }
    ));
    assert_eq!(capture.count, 1);
    let pending = report.fore_pending_output.unwrap();
    assert_eq!(
        pending.metadata().stage,
        BodyForeDeliveryStage::AdapterOutcomeUnknown
    );
    assert_eq!(pending.delivery.sequence, 1);
    assert_eq!(&*pending.delivery.bytes, &[1]);
    assert!(host.native_resources_idle());
}
#[test]
fn cancellation_after_borrowed_accept_keeps_accepted_data_and_quiesces_lease() {
    let (host, plan, wake) = fixture();
    let control = RunControl::default();
    let mut capture = Capture {
        count: 0,
        first_ptr: None,
        refuse_at: None,
        control: Some(control.clone()),
    };
    let mut provider = Provider(0);
    let (host, report) = host.run_body_plan_owned_native_borrowed_to(
        BodyRunRequest {
            wake: &wake,
            plan: &plan,
            control: &control,
            keyboard: None,
        },
        &[],
        &mut capture,
        &mut Vec::new(),
        &mut Timer::default(),
        &mut provider,
        |_, _| Ok(()),
    );
    let report = report.unwrap();
    assert!(
        matches!(report.terminal, TerminalDisposition::Cancelled { .. }),
        "{:?}",
        report.failure
    );
    assert_eq!(capture.count, 1);
    assert!(host.native_resources_idle());
}
