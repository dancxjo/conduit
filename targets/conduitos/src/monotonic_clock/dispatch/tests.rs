//! A named machine-contract fixture executes the actual native factory and kernel.
use super::*;
use alloc::{sync::Arc, vec};
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
mod possession;

struct Provider {
    now: Arc<AtomicU64>,
    polls: Arc<AtomicU64>,
    revoked: Arc<AtomicBool>,
}
impl MonotonicDeadlineProvider for Provider {
    fn poll_until(
        &mut self,
        deadline: u64,
    ) -> Result<Option<u64>, super::super::codec::ClockDisposition> {
        assert!(!self.revoked.load(Ordering::SeqCst));
        self.polls.fetch_add(1, Ordering::SeqCst);
        let now = self.now.load(Ordering::SeqCst);
        Ok((now >= deadline).then_some(now))
    }
    fn revoke(&mut self) {
        self.revoked.store(true, Ordering::SeqCst);
    }
}
struct Execution {
    kernel: KernelCompositeHost,
    clock: PreparedClockDispatch<Provider>,
    now: Arc<AtomicU64>,
    polls: Arc<AtomicU64>,
    revoked: Arc<AtomicBool>,
    result_kind: KindId,
}
impl Execution {
    fn prepare() -> Self {
        let contract = MonotonicClockContract::prepare().unwrap();
        let identity = ClockNativeIdentity {
            host_id: "fixture/clock-host".into(),
            boot_id: "fixture/clock-boot".into(),
            base_id: "fixture/clock-base".into(),
            provider_instance_id: "fixture/clock-provider".into(),
            provider_generation: 1,
            resource_pool_id: "fixture/clock-resource".into(),
            resource_generation_id: ResourceGenerationId("fixture/clock-generation".into()),
            envelope_id: "fixture/clock-envelope".into(),
            artifact_id: "fixture/native-clock".into(),
        };
        let now = Arc::new(AtomicU64::new(0));
        let polls = Arc::new(AtomicU64::new(0));
        let revoked = Arc::new(AtomicBool::new(false));
        // SAFETY: the scripted fixture owns no hardware and only reads this test's atomics.
        let ready = unsafe {
            ReadyClockBase::new(
                identity.clone(),
                Provider {
                    now: now.clone(),
                    polls: polls.clone(),
                    revoked: revoked.clone(),
                },
            )
        }
        .unwrap();
        let mut host = HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: identity.host_id.clone(),
            boot_id: identity.boot_id.clone(),
            offer_generation: OfferGeneration(1),
            profile: "fixture/clock".into(),
            bases: vec![],
            resources: vec![],
            capabilities: vec![],
            planner_capabilities: vec![],
        };
        ready.append_to_advertisement(&mut host, &contract).unwrap();
        let offer = host.capabilities[0].clone();
        let mut scope = BaseCapabilityScope {
            host_id: identity.host_id.clone(),
            boot_id: identity.boot_id.clone(),
            base_instance_id: identity.provider_instance_id.clone(),
            base_provider_generation: 1,
            plan_id: "fixture/clock-plan".into(),
            active_play_id: "fixture/clock-play".into(),
            authority_grant_id: "fixture/clock-grant".into(),
            authority_contract_id: CLOCK_AUTHORITY.into(),
            capability_id: offer.capability_id.clone(),
            implementation_id: CLOCK_IMPLEMENTATION.into(),
            operation_contract_id: CLOCK_CALL.into(),
            subject_kind: contract.kind().kind_id.clone(),
            resource_pool_id: identity.resource_pool_id.clone(),
            resource_generation_id: identity.resource_generation_id.clone(),
            envelope_id: identity.envelope_id.clone(),
            maximum_parameter_bytes: CLOCK_MAXIMUM_BYTES,
            maximum_result_bytes: CLOCK_MAXIMUM_BYTES,
            maximum_work_units: super::super::owner::MAXIMUM_POLL_STEPS,
            maximum_in_flight: 1,
            maximum_operations: 2,
        };
        let (mut fragment, _, _, placement) = crate::machine_membrane::selection_fixture::selected(
            &scope,
            contract.kind(),
            CLOCK_CALL,
            CLOCK_MAXIMUM_BYTES,
            "clock",
            "clock",
        );
        let gear = &mut fragment.placements[0];
        gear.execution_profile_id = CLOCK_EXECUTION_PROFILE.into();
        gear.artifact_id = identity.artifact_id;
        let base = gear.base.as_mut().unwrap();
        base.base_id = identity.base_id;
        base.implementation_id = "conduitos.base/monotonic-clock@1".into();
        base.mechanism_family = CLOCK_RESOURCE_CLASS.into();
        gear.resources[0].class_id = CLOCK_RESOURCE_CLASS.into();
        for port in contract
            .kind()
            .inputs
            .iter()
            .chain(&contract.kind().outputs)
        {
            fragment.fore_ports.push(PlannedForePort {
                front_port_id: port.port_id.clone(),
                direction: port.direction,
                placement_id: placement.clone(),
                gear_port_id: port.port_id.clone(),
                value_kind: port.value_kind.clone(),
                value_contract: None,
                abnormal_kind: None,
                track: ConnectionTrack::Payload,
                temporal: port.temporal,
                pressure_policy: DeliveryPressurePolicy::default(),
                item_capacity: 1,
                byte_capacity: CLOCK_MAXIMUM_BYTES,
            });
        }
        let plan = seal_plan(
            PlotIdentity {
                source_document_id: fragment.source_document_id.clone(),
                checked_plot_id: fragment.checked_plot_id.clone(),
                expanded_plot_id: fragment.expanded_plot_id.clone(),
            },
            vec![fragment],
        );
        scope.plan_id = plan.plan_id.clone();
        scope.active_play_id =
            bind_active_play(&plan.plan_id, &scope.host_id, &scope.boot_id, 0).active_play_id;
        let (table, handle, claim) = possession::possession(scope);
        let clock = PreparedClockDispatch::prepare(&plan, ready, table, handle, claim).unwrap();
        let mut external = offer;
        external.host_calls.clear();
        external.resource_requirements.clear();
        external.authority_requirements.clear();
        let definition = crate::protocol_kernel_fixture::definition(plan, external);
        let mut registry = KernelOperationRegistry::new();
        registry
            .install(ClockOperationFactory::prepare_contract().unwrap())
            .unwrap();
        let mut kernel = KernelCompositeHost::prepare(definition, &registry).unwrap();
        kernel.start().unwrap();
        Self {
            kernel,
            clock,
            now,
            polls,
            revoked,
            result_kind: contract
                .result_type()
                .profile()
                .unwrap()
                .value_kind()
                .clone(),
        }
    }
    fn step(&mut self) -> KernelCompositeStatus {
        let status = self.kernel.step().unwrap();
        if self.clock.has_pending() {
            self.clock.poll(&mut self.kernel).unwrap();
        } else if let Some(request) = self.kernel.next_host_request() {
            self.clock.dispatch(&mut self.kernel, &request).unwrap();
        }
        status
    }
    fn request(&mut self) {
        let contract = MonotonicClockContract::prepare().unwrap();
        let StructuredInfoTypeShape::Record { fields, .. } = contract.request_type().shape() else {
            panic!("request");
        };
        let value = StructuredInfoValue::record(
            contract.request_type().clone(),
            vec![
                StructuredFieldValue::new(
                    "deadline",
                    StructuredInfoValue::leaf(
                        fields[0].value_type().clone(),
                        10_u64.to_le_bytes().to_vec(),
                    )
                    .unwrap(),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        self.kernel
            .admit_input(
                &port_id("request"),
                0,
                &ValuePayload {
                    value_kind: contract
                        .request_type()
                        .profile()
                        .unwrap()
                        .value_kind()
                        .clone(),
                    encoded: value.canonical_bytes().unwrap(),
                },
            )
            .unwrap();
        self.kernel.close_input(&port_id("request")).unwrap();
    }
}
#[test]
fn native_kernel_retains_deadline_call_then_drains_the_observed_result_under_pressure() {
    let mut run = Execution::prepare();
    run.request();
    let mut output = ValuePayload {
        value_kind: run.result_kind.clone(),
        encoded: Vec::with_capacity(CLOCK_MAXIMUM_BYTES as usize),
    };
    for _ in 0..20 {
        run.step();
    }
    assert!(run.clock.has_pending());
    assert!(run.polls.load(Ordering::SeqCst) <= 20);
    assert_eq!(
        run.kernel
            .output_into(&port_id("result"), &mut output)
            .unwrap(),
        None
    );
    run.now.store(12, Ordering::SeqCst);
    for _ in 0..20 {
        run.step();
    }
    assert_eq!(
        run.kernel
            .output_into(&port_id("result"), &mut output)
            .unwrap(),
        Some(0)
    );
    let value = StructuredInfoValue::from_canonical_bytes(&output.encoded).unwrap();
    assert!(
        matches!(value.shape(), StructuredInfoValueShape::Variant { tag, payload }
        if tag == "completed" && matches!(payload.shape(), StructuredInfoValueShape::Leaf(bytes) if bytes == 12_u64.to_le_bytes()))
    );
    run.kernel.complete_output(&port_id("result"), 0).unwrap();
    let mut status = KernelCompositeStatus::Active;
    for _ in 0..20 {
        status = run.step();
        if status == KernelCompositeStatus::Complete {
            break;
        }
    }
    assert_eq!(status, KernelCompositeStatus::Complete);
}
#[test]
fn cancellation_quiesces_pending_native_clock_before_late_time_can_complete() {
    let mut run = Execution::prepare();
    run.request();
    for _ in 0..20 {
        run.step();
    }
    assert!(run.clock.has_pending());
    let before = run.polls.load(Ordering::SeqCst);
    run.clock.revoke(&run.kernel).unwrap();
    run.kernel.cancel().unwrap();
    run.now.store(12, Ordering::SeqCst);
    run.clock.poll(&mut run.kernel).unwrap();
    assert!(run.revoked.load(Ordering::SeqCst));
    assert_eq!(run.polls.load(Ordering::SeqCst), before);
    assert!(run.kernel.next_host_request().is_none());
}
