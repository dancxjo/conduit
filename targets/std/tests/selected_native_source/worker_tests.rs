//! Actual installed Source and an independent timed Back share one scheduler.
use super::*;
use std::{
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
#[derive(Default)]
struct Signals {
    entered: AtomicBool,
    release: AtomicBool,
    returned: AtomicBool,
    calls: AtomicUsize,
}
struct CpuIo {
    signals: Arc<Signals>,
    stubborn: bool,
}
// SAFETY: CPU fixtures perform no physical IO and allocate nothing while read
// owns the fixed buffers. The stubborn variant deliberately violates only the
// cancellation/time promise to verify that custody is not falsely released.
unsafe impl NativeSourceWorkerProvider for CpuIo {
    fn read(
        &mut self,
        _: &[u8],
        output: &mut [u8],
        context: &NativeReadContext<'_>,
    ) -> Result<usize, WorkerFailure> {
        self.signals.calls.fetch_add(1, Ordering::SeqCst);
        self.signals.entered.store(true, Ordering::Release);
        while !self.signals.release.load(Ordering::Acquire) {
            if !self.stubborn && context.should_stop() {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        // Intentionally return a valid late value after cancellation. The
        // trusted worker and installed commit owner must still suppress it.
        output[..VALUE.len()].copy_from_slice(VALUE);
        self.signals.returned.store(true, Ordering::Release);
        Ok(VALUE.len())
    }
}
fn budget() -> NativeWorkerBudget {
    NativeWorkerBudget {
        requested_stack_bytes: 2 * 1024 * 1024,
        maximum_stack_mapping_bytes: 2 * 1024 * 1024 + 64 * 1024,
        capture_timeout: Duration::from_secs(2),
        cancellation_timeout: Duration::from_millis(20),
    }
}
fn worker_definition(host: &HostAdvertisement) -> NativeSourceDefinition {
    let mut d = definition(host);
    d.base.capabilities[0].implementation.execution_profile_id =
        NATIVE_SOURCE_WORKER_PROFILE.into();
    let bytes = budget()
        .backing_bytes(d.input_bytes, d.output_bytes)
        .unwrap();
    d.base.resources.push(ResourceOffer {
        pool_id: "test/native-worker-backing".into(),
        class_id: RUNTIME_MEMORY_RESOURCE_CLASS.into(),
        capacity_units: bytes,
        compute: None,
        content: None,
    });
    d
}
fn provider(signals: Arc<Signals>, stubborn: bool) -> Box<dyn NativeSourceWorkerProvider> {
    Box::new(CpuIo { signals, stubborn })
}
fn worker_host(signals: Arc<Signals>, clock: bool, stubborn: bool) -> (StdHost, Plan) {
    let mut host = StdHost::new_with_config(StdHostConfig {
        host_id: "test/producing-host".into(),
        boot_id: "test/actual-host-boot".into(),
        offer_generation: OfferGeneration(1),
    });
    let d = worker_definition(host.advertisement());
    let grant = d.grant.clone();
    // SAFETY: Actual fixed CPU native owner and independent definition/grant;
    // the memory offer is configured before receiving any Plan.
    unsafe { host.install_native_source_worker(d, budget(), provider(signals, stubborn)) }
        .unwrap_or_else(|(_, _, reason)| panic!("{reason}"));
    let plan = worker_plan(&host, clock, grant);
    (host, plan)
}
fn worker_plan(host: &StdHost, clock: bool, grant: conduit_core::AuthorityGrant) -> Plan {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    let kind = source_kind();
    startup
        .insert(KindSignature {
            kind: SOURCE_KIND.into(),
            startup_parameters: vec![],
        })
        .unwrap();
    startup
        .insert_fore(SOURCE_KIND, kind.checked_front())
        .unwrap();
    profile.insert_kind(kind).unwrap();
    let clock_front = if clock { " ticks: Tick...| >>\n" } else { "" };
    let clock_work = if clock {
        " clock: time/tick(count = 10, period-ms = 5)\n clock.tick >> ticks\n"
    } else {
        ""
    };
    if clock {
        conduit_time::install_tick_catalog(&mut startup, &mut profile).unwrap();
        startup
            .insert_value_kind_alias("Tick", kind_id(conduit_time::TICK_VALUE_KIND))
            .unwrap();
    }
    let text=format!("plot capture (\n >> begin: Empty...|\n reading: Text...| >>\n{clock_front}) {{\n sensor: {SOURCE_KIND}\n begin >> sensor.request\n sensor.observation >> reading\n{clock_work}}}\n");
    let checked = check_syntax_document(&parse_syntax_document(&text), &startup).unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, "capture", &profile).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements = default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let mut fronts = vec![
        (PortDirection::Input, "begin", 1),
        (PortDirection::Output, "reading", MAXIMUM_BYTES as u32),
    ];
    if clock {
        fronts.push((PortDirection::Output, "ticks", 64));
    }
    let boundaries = fronts
        .into_iter()
        .map(|(direction, name, byte_capacity)| {
            (
                ForeBoundaryKey {
                    direction,
                    front_port_id: name.into(),
                    track: ConnectionTrack::Payload,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity,
                },
            )
        })
        .collect();
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &empty,
            line_candidates: &lines,
            connection_item_capacity: 1,
            connection_byte_capacity: MAXIMUM_BYTES as u32,
            authority_grants: &[grant],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap()
}
struct WorkerCapture {
    signals: Arc<Signals>,
    ticks_while_pending: usize,
    readings: usize,
}
impl ExternalForeOutputAdapter for WorkerCapture {
    fn deliver(&mut self, _: ExternalForeDelivery) -> Result<(), String> {
        panic!("real commit evidence required")
    }
    fn deliver_committed(
        &mut self,
        witness: CommittedForeValue<'_>,
        _: ExternalForeDelivery,
    ) -> Result<(), String> {
        if witness.scope().fore.front_port_id.as_str() == "ticks" {
            if self.signals.entered.load(Ordering::Acquire)
                && !self.signals.returned.load(Ordering::Acquire)
            {
                self.ticks_while_pending += 1;
                if self.ticks_while_pending >= 3 {
                    self.signals.release.store(true, Ordering::Release);
                }
            }
        } else {
            assert_eq!(witness.bytes(), VALUE);
            self.readings += 1;
        }
        Ok(())
    }
}
fn wait_entered(signals: &Signals) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !signals.entered.load(Ordering::Acquire) {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn retire(host: &mut StdHost) -> RetiredNativeSource {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match host.retire_native_source_worker() {
            Ok(v) => return v,
            Err(error) => {
                assert!(Instant::now() < deadline, "{error}");
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }
}
#[test]
fn pending_worker_allows_real_timed_back_progress_and_keeps_backing_across_child_plays() {
    let signals = Arc::new(Signals::default());
    let (mut host, plan) = worker_host(signals.clone(), true, false);
    let before = host.native_source_worker_inspection().unwrap();
    assert!(before.idle_backing_is_reserved);
    assert_eq!(before.reserved_bytes, before.backing.charged_bytes);
    assert!(
        before.backing.stack_bytes + before.backing.guard_bytes
            <= budget().maximum_stack_mapping_bytes as usize
    );
    let mut capture = WorkerCapture {
        signals: signals.clone(),
        ticks_while_pending: 0,
        readings: 0,
    };
    host.run_external_plot_to(
        plan.fragments[0].clone(),
        &[ExternalForeInput {
            front_port_id: "begin".into(),
            track: ConnectionTrack::Payload,
            bytes: vec![],
        }],
        &mut capture,
        &mut Vec::new(),
        &mut ThreadTimer,
    )
    .unwrap();
    assert!(capture.ticks_while_pending >= 3);
    assert_eq!(capture.readings, 1);
    let after = host.native_source_worker_inspection().unwrap();
    assert!(after.idle_backing_is_reserved);
    assert_eq!(before.reserved_bytes, after.reserved_bytes);
    let old_offer = host.advertisement().offer_generation;
    let context = conversation_context();
    host.install_body_conversation_context(&context).unwrap();
    assert_eq!(host.advertisement().offer_generation.0, old_offer.0 + 1);
    assert!(
        host.native_source_worker_inspection()
            .unwrap()
            .idle_backing_is_reserved
    );
    host.install_body_conversation_context(&context).unwrap();
    assert_eq!(host.advertisement().offer_generation.0, old_offer.0 + 1);
    // A new child is planned against the refreshed actual offer. Its Source
    // owner and reserved backing are the same, not reinitialized.
    let grant = worker_definition(host.advertisement()).grant;
    let plan = worker_plan(&host, true, grant);
    host.run_external_plot_to(
        plan.fragments[0].clone(),
        &[ExternalForeInput {
            front_port_id: "begin".into(),
            track: ConnectionTrack::Payload,
            bytes: vec![],
        }],
        &mut capture,
        &mut Vec::new(),
        &mut ThreadTimer,
    )
    .unwrap();
    assert_eq!(signals.calls.load(Ordering::SeqCst), 2);
    assert_eq!(capture.readings, 2);
    assert!(
        host.native_source_worker_inspection()
            .unwrap()
            .idle_backing_is_reserved
    );
    let retired = retire(&mut host);
    assert!(retired.provider.is_some());
    assert!(host.native_source_worker_inspection().is_none());
}
#[test]
fn cancellation_suppresses_late_values_and_stubborn_read_cannot_release_custody() {
    for stubborn in [false, true] {
        let signals = Arc::new(Signals::default());
        let (mut host, plan) = worker_host(signals.clone(), false, stubborn);
        let control = RunControl::default();
        let child_control = control.clone();
        let child_signals = signals.clone();
        let thread = std::thread::spawn(move || {
            let mut capture = WorkerCapture {
                signals: child_signals,
                ticks_while_pending: 0,
                readings: 0,
            };
            let result = host.run_external_plot_controlled_to(
                plan.fragments[0].clone(),
                &[ExternalForeInput {
                    front_port_id: "begin".into(),
                    track: ConnectionTrack::Payload,
                    bytes: vec![],
                }],
                &mut capture,
                &mut Vec::new(),
                &mut ThreadTimer,
                &child_control,
            );
            (host, capture, result)
        });
        wait_entered(&signals);
        control
            .request_stop(RunControlRequestId::new("cancel-native-read").unwrap())
            .unwrap();
        if stubborn {
            std::thread::sleep(Duration::from_millis(60));
            assert!(
                !thread.is_finished(),
                "live IO must retain the Play/session/backing"
            );
        }
        signals.release.store(true, Ordering::Release);
        let (mut host, capture, result) = thread.join().unwrap();
        result.unwrap();
        assert_eq!(capture.readings, 0);
        let retained = host.native_source_worker_inspection().unwrap();
        assert!(retained.idle_backing_is_reserved);
        assert_eq!(retained.cancellation_overdue, stubborn);
        let retired = retire(&mut host);
        assert!(retired.provider.is_some());
    }
}
#[test]
fn insufficient_backing_refuses_before_worker_initialization_and_returns_exact_owner() {
    let signals = Arc::new(Signals::default());
    let mut host = StdHost::new_with_config(StdHostConfig {
        host_id: "test/producing-host".into(),
        boot_id: "test/actual-host-boot".into(),
        offer_generation: OfferGeneration(1),
    });
    let mut d = worker_definition(host.advertisement());
    d.base.resources[1].capacity_units -= 1;
    let before = host.advertisement().clone();
    let owner = provider(signals.clone(), false);
    let address = &*owner as *const dyn NativeSourceWorkerProvider;
    // SAFETY: Same controlled CPU owner; admission is expected to refuse.
    let (_, returned, reason) = unsafe { host.install_native_source_worker(d, budget(), owner) }
        .err()
        .unwrap();
    assert!(std::ptr::addr_eq(address, &*returned));
    assert!(reason.contains("insufficient"));
    assert_eq!(host.advertisement(), &before);
    assert!(host.native_source_worker_inspection().is_none());
    assert_eq!(signals.calls.load(Ordering::SeqCst), 0);
}

fn conversation_context() -> conduit_body::BodyConversationContext {
    use conduit_body::{Body, BodyConversationContext, BodyConversationContextBasis};
    let body = Body::born(
        "source/worker-context".into(),
        "checked/worker-context".into(),
        1,
        "sign/worker-context/born".into(),
    )
    .unwrap();
    let (_, wake) = body.wake(2, "sign/worker-context/wake".into()).unwrap();
    BodyConversationContext {
        schema: "conduit.body/conversation-context-value@2".into(),
        display_name: "Worker Body".into(),
        body_id: wake.body_id.clone(),
        wake_id: wake.wake_id.clone(),
        wake_sequence: 2,
        basis: BodyConversationContextBasis {
            body_id: wake.body_id,
            wake_id: wake.wake_id,
            wake_sequence: 2,
            revision: 1,
        },
        hosts: vec![],
        active_plots: vec![],
        current_plan_id: None,
        active_play_id: None,
        lines: vec![],
        recent_sign_ids: vec![],
    }
}

#[cfg(all(feature = "linux-snapshot", target_os = "linux"))]
#[test]
fn storage_generation_refresh_retains_actual_source_worker_and_reserved_backing() {
    use conduit_std_host::{
        body_storage::{BodyStorageBounds, SelectedBodyStorage},
        body_storage_generation::{BodyStorageGenerationRequest, BodyStorageStateRequest},
    };
    use std::os::unix::fs::PermissionsExt;
    let signals = Arc::new(Signals::default());
    let (mut host, _old_plan) = worker_host(signals.clone(), false, false);
    let initial = host.native_source_worker_inspection().unwrap();
    let old = host.advertisement().clone();
    let root = tempfile::tempdir().unwrap();
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    host.attach_selected_body_storage(
        SelectedBodyStorage::open(
            root.path(),
            "test/source-storage",
            BodyStorageBounds {
                maximum_snapshot_records: 2,
                maximum_record_bytes: 4096,
                generation_slots: 1,
                publication_slots: 1,
                reader_leases: 1,
                state_slots: 1,
                maximum_state_bytes: 4096,
                total_disk_bytes: 131072,
                preparation_bytes: 131072,
            },
            None,
        )
        .unwrap(),
    )
    .unwrap();
    let ty = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let value = StructuredInfoValue::leaf(ty.clone(), InfoBool::FALSE.encode().to_vec()).unwrap();
    host.select_body_storage_generation(BodyStorageGenerationRequest {
        snapshot: None,
        state: Some(BodyStorageStateRequest {
            binding: conduit_std_host::state_value::DurableStateBinding {
                body: "body/source-storage-test".into(),
                state: "state/cell".into(),
                value_kind: kind_id(BOOL_INFO_ID),
                maximum_value_bytes: 4096,
            },
            type_name: "Cell".into(),
            value_type: ty,
            default_value: value,
        }),
    })
    .unwrap();
    let after = host.native_source_worker_inspection().unwrap();
    assert_eq!(after.backing.stack_bytes, initial.backing.stack_bytes);
    assert_eq!(after.backing.guard_bytes, initial.backing.guard_bytes);
    assert_eq!(after.backing.charged_bytes, initial.backing.charged_bytes);
    assert_eq!(after.reserved_bytes, initial.reserved_bytes);
    assert!(after.idle_backing_is_reserved);
    assert_eq!(signals.calls.load(Ordering::SeqCst), 0);
    for resource in &old.resources {
        assert!(host.advertisement().resources.contains(resource));
    }
    for base in &old.bases {
        assert!(host.advertisement().bases.contains(base));
    }
    for capability in &old.capabilities {
        assert!(host.advertisement().capabilities.contains(capability));
    }
    assert!(host.advertisement().offer_generation > old.offer_generation);
    retire(&mut host);
}
