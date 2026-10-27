use super::*;
use conduit_pulse_capture::{installation::*, owner::*};
include!("../../../../../targets/esp32/proof/tm1637/src/native_root_state.rs");
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_local_mask.rs"]
mod local_mask;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_local_renewal.rs"]
mod local_renewal;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_local_sampler_run.rs"]
mod local_sampler_run;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_local_sampler_plan.rs"]
mod local_sampler_plan;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_root_policy.rs"]
mod policy;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_offer.rs"]
mod offers;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_body_root.rs"]
mod body;
mod mask {
    pub(super) mod material {
        include!(concat!(env!("OUT_DIR"), "/mask-source.rs"));
    }
}
#[path = "memory_check.rs"]
mod memory_check;
mod setup {
    pub struct PendingWorkspace;
}

fn initial_evidence(host: &c::HostId, boot: &c::BootId) -> conduit_body::BodyBiographyEvidence {
    use conduit_body::*;
    let sampler = native_software::source_identity();
    let mask = mask::material::role().unwrap().plot_identity;
    let workset = BodyWorkset::from_plots(
        [sampler, mask]
            .into_iter()
            .map(|plot| ResidentPlot::new(plot.source_document_id, plot.checked_plot_id)),
    )
    .unwrap();
    let sign = |at| c::bind_sign(host, boot, None, at).sign_id;
    let body = Body::born_with_plots(workset, 1, sign(1)).unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut evidence = BodyBiographyEvidence::born(
        body.clone(),
        membership.clone(),
        "CPU hardware-adapter Root".into(),
    )
    .unwrap();
    let part = PartId::bind(&body.body_id, host.as_str(), 1).unwrap();
    let proof = MembershipProofId::bind("CPU initial local birth").unwrap();
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            sign(2),
        )
        .unwrap();
    let present = membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: host.clone(),
                boot_id: boot.clone(),
                offer_generation: c::OfferGeneration(1),
                proof_id: proof,
                sequence: 1,
            },
            sign(3),
        )
        .unwrap();
    evidence
        .append_membership_events(membership, &[(admitted, 2), (present, 3)])
        .unwrap();
    evidence
}
fn bootstrap(
    executor: &manual_clock::Executor,
) -> (
    NativeRoot,
    native_evidence_region::BodyRegion,
    std::rc::Rc<std::cell::Cell<usize>>,
) {
    use conduit_monotonic_wait::*;
    let observed = boot::ObservedBoot {
        host: "esp32/cpu-renewal",
        boot: "boot/00000000000000000000000000000001",
    };
    let (host, boot) = observed.ids();
    let host: c::HostId = host.into();
    let boot: c::BootId = boot.into();
    let media = native_evidence_region::test_empty();
    native_restart::initialize(media.clone(), initial_evidence(&host, &boot)).unwrap();
    let mut admission = native_admission::NativeAdmission::new([7; 32]).unwrap();
    let restored = native_restart::take_body().unwrap();
    let mut body = Box::new(
        conduit_body::SealedBodyLifecycleSession::open_admitted(restored.evidence, &host, &boot)
            .unwrap(),
    );
    native_restart::checkpoint(&mut body, &admission.verifying_key()).unwrap();
    native_observed_archive::initialize_epochs(
        None,
        core::array::from_fn(|_| native_evidence_region::Region::erased(0x20000)),
    )
    .unwrap();
    admission.activate_after_checkpoint().unwrap();
    let effects = std::rc::Rc::new(std::cell::Cell::new(0));
    let pulse_identity = PulseNativeIdentity {
        host_id: host.clone(),
        boot_id: boot.clone(),
        base_id: "base/esp32/gpio27".into(),
        provider_instance_id: "instance/esp32/gpio27".into(),
        provider_generation: 1,
        resource_pool_id: "pool/esp32/gpio27".into(),
        resource_generation_id: c::ResourceGenerationId("generation/gpio27/1".into()),
        envelope_id: "envelope/esp32/gpio27".into(),
        artifact_id: "native/esp32-gpio-pulse-root@1".into(),
        implementation_id: "esp32/gpio27-pulse@1".into(),
        base_implementation_id: "esp32/gpio27-pulse-base@1".into(),
    };
    // SAFETY: this explicitly initialized inert CPU provider returns only the
    // bounded complete transcript above. No GPIO/native hardware is claimed.
    let pulse = unsafe {
        ReadyPulseBase::new(
            pulse_identity,
            PulseAttachment {
                generation: 1,
                resource_bytes: 64,
                limits: LIMITS,
            },
            Esp32PulseCapture {
                effects: effects.clone(),
                lifetime: core::marker::PhantomData,
            },
        )
    }
    .unwrap();
    let wait_identity = WaitNativeIdentity {
        host_id: host.clone(),
        boot_id: boot.clone(),
        base_id: "base/esp32/monotonic-wait".into(),
        provider_instance_id: "instance/esp32/monotonic-wait".into(),
        provider_generation: 1,
        resource_pool_id: "pool/esp32/monotonic-wait".into(),
        resource_generation_id: c::ResourceGenerationId("generation/monotonic-wait/1".into()),
        envelope_id: "envelope/esp32/monotonic-wait".into(),
        artifact_id: "native/esp32-monotonic-wait@1".into(),
        implementation_id: "esp32/monotonic-wait@1".into(),
        base_implementation_id: "esp32/monotonic-wait-base@1".into(),
    };
    let mut provider = native_wait::EspWait;
    let bytes = {
        let future = provider.wait(60_000);
        core::mem::size_of_val(&future) as u64
    };
    // SAFETY: the registered manual driver is monotonic and the provider has
    // one bounded cancellable Timer future, no detached effect or allocation.
    let wait = unsafe {
        ReadyMonotonicWaitBase::new(
            wait_identity,
            WaitAttachment {
                generation: 1,
                resource_bytes: bytes,
                maximum_duration_ms: 60_000,
            },
            provider,
        )
    }
    .unwrap();
    let channel = Box::leak(Box::new(
        conduit_environment_http::network_setup::NetworkSetupChannel::new(2),
    ));
    let (credential_writer, _unavailable_reader) = channel.split();
    let root = NativeRoot {
        body: Some(body),
        retired_native: None,
        worker: None,
        face_reader: None,
        worker_spawner: Some(executor.spawner()),
        execution: None,
        native_bindings: None,
        owner_control: native_owner_control::OwnerControl::new(None).unwrap(),
        display_reservation: None,
        owner_reservation: None,
        assignment_seal: None,
        appliance_kernel: None,
        admission,
        admission_freshness: 0,
        network_preparation: None,
        network_source: native_software::source_identity(),
        credentials: None,
        issuer: native_authority::RootIssuer::fixture(host.clone(), boot.clone()),
        play: native_play::NativePlay,
        offer_generation: 1,
        software: None,
        assignment: None,
        credential_writer,
        network: native_network::NativeNetwork,
        pulse: Some(pulse),
        display: native_display::NativeDisplay::prepare(gpio::CpuPin, gpio::CpuPin, &observed)
            .unwrap(),
        local_mask_scope: None,
        local_mask_worker: None,
        pending_network_workspace: None,
        local_renewal: None,
        local_renewal_archive_refusal: None,
        local_history_custody: None,
        wait: Some(wait),
        host,
        boot,
    };
    (root, media, effects)
}

fn complete<F: core::future::Future>(executor: &manual_clock::Executor, future: F) -> F::Output {
    let mut cx = core::task::Context::from_waker(std::task::Waker::noop());
    let mut future = std::pin::pin!(future);
    for _ in 0..100_000 {
        executor.poll_ready();
        if let core::task::Poll::Ready(result) = future.as_mut().poll(&mut cx) {
            return result;
        }
        executor.poll_ready();
        if let core::task::Poll::Ready(result) = future.as_mut().poll(&mut cx) {
            return result;
        }
        assert!(
            executor.advance_next_timer(),
            "Root async preparation stalled without a timer"
        );
    }
    panic!("Root async preparation exceeded bounded CPU polls");
}

pub(super) fn prove_two_handoffs() {
    use conduit_kernel::ValueStorage;
    let executor = manual_clock::Executor::new();
    let (mut root, media, effects) = bootstrap(&executor);
    complete(&executor, root.start_local_appliance()).unwrap();
    let original_body = root.body.as_ref().unwrap().evidence().body_id.clone();
    let original_boot = root.boot.clone();
    let mut old_lease = root.face_reader.as_ref().unwrap().lease();
    let mut old_active = root.issuer.native_issued_active().unwrap().clone();
    let mut last_generation = 1;
    let mut fault_proved = false;
    let mut fixed_address = None;
    let mut fixed_generation = 0;
    let mut store_generation = 0;
    let mut reclaimed = 0;
    let mut progress = (0, 0);
    for _ in 0..10_000_000 {
        executor.poll_ready();
        let sampler = root.worker.as_ref().map(|w| w.status());
        let mask = root.local_mask_worker.as_ref().map(|w| w.status());
        if let Some(mask) = mask {
            assert!(
                mask.failure.is_none(),
                "Mask failure {:?} at child {}",
                mask.failure,
                mask.completed
            );
            let next = (root.offer_generation, mask.completed / 2000);
            if next != progress {
                std::println!(
                    "ROOT_CPU_PROGRESS generation={} mask_children={} reads={}",
                    root.offer_generation,
                    mask.completed,
                    effects.get()
                );
                progress = next;
            }
        }
        if let Some(sampler) = sampler {
            assert!(
                sampler.failure.is_none(),
                "sampler failure {:?}",
                sampler.failure
            );
        }
        if root.offer_generation == 3
            && root.local_renewal.is_none()
            && sampler.is_some_and(|s| !s.task_active && s.completed == [960, 960])
            && mask
                .is_some_and(|s| !s.task_active && s.completed == native_display::MAXIMUM_CHILDREN)
        {
            break;
        }
        let phase = root.local_renewal.as_ref().map(|s| s.phase_name());
        if phase == Some("checkpoint-lull") && !fault_proved {
            let state = root.local_renewal.as_ref().unwrap();
            let execution = state.execution.as_ref().unwrap();
            let kernel = execution.material.kernel() as *const _;
            let signs = execution
                .material
                .kernel()
                .signs()
                .events()
                .collect::<Vec<_>>();
            let generations = execution.providers.provider_generations();
            let face_address = &**state.face.as_ref().unwrap() as *const _;
            media.media.lock().unwrap().2 = Some(1);
            let result = complete(&executor, root.advance_local_renewal());
            assert!(result.is_err(), "archive write fault was not observed");
            let state = root.local_renewal.as_ref().unwrap();
            assert_eq!(state.phase_name(), "checkpoint-lull");
            let execution = state.execution.as_ref().unwrap();
            assert_eq!(execution.material.kernel() as *const _, kernel);
            assert_eq!(
                execution
                    .material
                    .kernel()
                    .signs()
                    .events()
                    .collect::<Vec<_>>(),
                signs
            );
            assert_eq!(execution.providers.provider_generations(), generations);
            assert_eq!(&**state.face.as_ref().unwrap() as *const _, face_address);
            assert!(!root.body.as_ref().unwrap().pending_archives().is_empty());
            assert!(root.pulse.is_none() && root.wait.is_none());
            assert!(state.receipt.as_ref().unwrap().matches_active(&old_active));
            media.media.lock().unwrap().2 = None;
            fault_proved = true;
            executor.advance_millis(1000);
        } else {
            let result = complete(&executor, root.advance_local_renewal());
            assert!(
                result.is_ok(),
                "Root phase {:?} refused: {:?}",
                phase,
                result
            );
        }
        if let Some(state) = root.local_renewal.as_mut() {
            if state.phase_name() == "prepare-software" {
                let values = state.mask_values.as_deref_mut().unwrap();
                let address = values as *mut _ as usize;
                if let Some(old) = fixed_address {
                    assert_eq!(old, address);
                } else {
                    fixed_address = Some(address);
                }
                let reference = values.store(&[11]).unwrap();
                assert!(reference.generation > fixed_generation);
                fixed_generation = reference.generation;
                values.release(reference).unwrap();
                assert!(values.get(reference).is_err());
                let values = state.values.as_mut().unwrap();
                let reference = values.store(&[12]).unwrap();
                assert!(reference.generation > store_generation);
                store_generation = reference.generation;
                values.release(reference).unwrap();
                assert!(values.get(reference).is_err());
                reclaimed += 1;
                assert_eq!(
                    root.pulse
                        .as_ref()
                        .unwrap()
                        .native_identity()
                        .provider_generation,
                    root.offer_generation
                );
            }
            executor.advance_millis(50);
        } else if root.offer_generation != last_generation {
            assert_eq!(root.offer_generation, last_generation + 1);
            assert!(old_lease
                .inspect(embassy_time::Instant::now().as_millis())
                .is_err());
            let new_active = root.issuer.native_issued_active().unwrap();
            assert_ne!(new_active.active_play_id, old_active.active_play_id);
            assert!(root
                .body
                .as_ref()
                .unwrap()
                .realization()
                .unwrap()
                .play
                .is_some());
            assert_eq!(
                root.body.as_ref().unwrap().evidence().body_id,
                original_body
            );
            assert_eq!(root.boot, original_boot);
            assert!(root.body.as_ref().unwrap().pending_archives().is_empty());
            old_active = new_active.clone();
            old_lease = root.face_reader.as_ref().unwrap().lease();
            last_generation = root.offer_generation;
        } else {
            assert!(
                executor.advance_next_timer(),
                "workers stalled without pending timer"
            );
        }
    }
    assert_eq!(root.offer_generation, 3);
    assert_eq!(last_generation, 3);
    assert_eq!(reclaimed, 2);
    assert!(fault_proved);
    assert_eq!(effects.get(), 3 * 960);
    let sampler = root.worker.as_ref().unwrap().status();
    let mask = root.local_mask_worker.as_ref().unwrap().status();
    assert!(!sampler.task_active && sampler.completed == [960, 960]);
    assert!(!mask.task_active && mask.completed == native_display::MAXIMUM_CHILDREN);
    assert!(mask.failure.is_none() && mask.cleanup_refusal.is_none());
    assert!(root.local_history_custody.is_none());
    std::println!("ROOT_CPU_TWO_HANDOFFS reads={} MaskChildren={} fixed_generation={} store_generation={} checkpoint_generation={:?}",effects.get(),3*native_display::MAXIMUM_CHILDREN,fixed_generation,store_generation,native_restart::generation());
}

pub(super) fn prove_storage_policy() {
    let executor=manual_clock::Executor::new();
    let (mut root,media,_)=bootstrap(&executor);
    let evidence=root.body.as_ref().unwrap().evidence().clone();
    let generation=native_restart::generation().unwrap();
    media.media.lock().unwrap().1=false;
    assert_eq!(root.checkpoint_body(),Err("restart-checkpoint-storage"));
    assert_eq!(root.body.as_ref().unwrap().evidence(),&evidence);
    assert!(!native_restart::publication_ready());
    assert_eq!(native_restart::with_admissions(|_|Ok(())),Err("restart-not-ready"));
    media.media.lock().unwrap().1=true;
    root.checkpoint_body().unwrap();
    assert_eq!(native_restart::generation(),Some(generation+1));
    assert!(native_restart::publication_ready());
    assert!(native_restart::with_admissions(|_|Ok(())).is_ok());
    let mut state=media.media.lock().unwrap();
    let current=native_restart::generation().unwrap();
    let bank=(0..2).find(|bank|u64::from_le_bytes(state.0[bank*32768+8..bank*32768+16].try_into().unwrap())==current).unwrap();
    let at=bank*32768+160+12;
    let original=state.0[at];
    state.0[at]^=1; // Explicit simulated media bit corruption, never a journal write.
    drop(state);
    assert_eq!(root.checkpoint_body(),Err("restart-checkpoint-commit"));
    assert!(!native_restart::publication_ready());
    media.media.lock().unwrap().0[at]=original;
    assert_eq!(root.checkpoint_body(),Err("restart-failed"));
    assert_eq!(root.body.as_ref().unwrap().evidence(),&evidence);
}

pub(super) fn prove_fresh_journal_failure_latches() {
    let host:c::HostId="esp32/cpu-fresh-failure".into();
    let boot:c::BootId="boot/00000000000000000000000000000002".into();
    let media=native_evidence_region::test_empty();
    native_restart::initialize(media.clone(),initial_evidence(&host,&boot)).unwrap();
    media.media.lock().unwrap().1=false;
    assert_eq!(native_restart::select_signer_seed([7;32]),Err("restart-initial-commit"));
    media.media.lock().unwrap().1=true;
    assert_eq!(native_restart::select_signer_seed([7;32]),Err("restart-failed"));
    assert!(!native_restart::publication_ready());
}
