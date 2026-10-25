//! Actual installed std Body runner + generic kernel + checked native CPU Mask.
//! Manufacture uses explicit ISO fixture files; output is a CPU diagnostic,
//! never SDL/DRM scanout, real source compilation, destination boot or join.
use super::*;
use crate::creche_encounter::{CrecheEncounterController, CrecheMakeRefusal};
use crate::creche_production::{tests::Files, CrecheImageProduction};
use conduit_body_make::deployment_runtime::{CrecheDisposition, CrecheOutcome, CrecheRequest};
use conduit_std_host::{
    body_execution::{BodyForeOutputAdapter, BodyRunRequest},
    hosted_creche::{
        CrecheExecutionContext, CrechePoll, HostedCrecheAdapter, PreparedCrecheRequest,
    },
    ExternalForeDelivery, RunControl, StdHost, StdHostComposition, StdHostConfig, TimerAdapter,
};
use std::{
    collections::BTreeMap,
    sync::{mpsc, Mutex},
    time::Instant,
};

struct ObserveAdapter {
    inner: CrecheEncounterAdapter,
    prepared: mpsc::Sender<CrecheExecutionContext>,
    begun: Arc<Mutex<Vec<CrecheRequest>>>,
}
impl HostedCrecheAdapter for ObserveAdapter {
    fn resource_offer(&self) -> conduit_core::ResourceOffer {
        self.inner.resource_offer()
    }
    fn limits(&self) -> CrecheLimits {
        self.inner.limits()
    }
    fn prepare(
        &mut self,
        context: &CrecheExecutionContext,
    ) -> Result<PreparedCrecheRequest, String> {
        let request = self.inner.prepare(context)?;
        self.prepared
            .send(context.clone())
            .map_err(|_| "diagnostic UI absent")?;
        Ok(request)
    }
    fn begin(&mut self, request: CrecheRequest) -> Result<CrechePoll, String> {
        self.begun.lock().unwrap().push(request);
        self.inner.begin(request)
    }
    fn poll(&mut self) -> Result<CrechePoll, String> {
        self.inner.poll()
    }
    fn retained_evidence(&self, outcome: &CrecheOutcome) -> Result<&[u8], String> {
        self.inner.retained_evidence(outcome)
    }
    fn cancel_and_drain(&mut self) -> Result<(), String> {
        self.inner.cancel_and_drain()
    }
}
struct Timer;
impl TimerAdapter for Timer {
    fn wait(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}
struct Outputs(Vec<ExternalForeDelivery>);
impl BodyForeOutputAdapter for Outputs {
    fn deliver(&mut self, output: &ExternalForeDelivery) -> Result<(), String> {
        if !self.0.is_empty() {
            return Err("diagnostic expected one typed outcome".into());
        }
        self.0.push(output.clone());
        Ok(())
    }
}
fn draft() -> conduit_body_make::DeploymentDraft {
    let mut draft = crate::creche_draft_fixture::encounter("Root Garden");
    let mut description = draft.construction().description().clone();
    for host in &mut description.hosts {
        host.configuration = "fixture/native.host.conduit".into();
        host.spore.output = conduit_host_make::SporeOutputKind::DiskImage;
    }
    let config = conduit_host_make::parse_host_configuration_conduit(include_str!(
        "../../../../../targets/conduitos/profiles/conduitos-native.host.conduit"
    ))
    .unwrap();
    let checked = conduit_body_make::check_body_description(
        description,
        &BTreeMap::from([("fixture/native.host.conduit".into(), config)]),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .unwrap();
    draft
        .replace_construction(draft.revision(), checked, "first")
        .unwrap();
    draft
        .select_hosts(
            draft.revision(),
            "first",
            conduit_body_make::DeploymentScope::AllHosts,
        )
        .unwrap();
    draft
        .select_image_source(
            draft.revision(),
            conduit_body_make::DeploymentImageSource::ReviewedRelease,
        )
        .unwrap();
    draft
}
fn wait_ui(
    native: &mut LocalCrecheNativeOwner<CpuOutput>,
    controller: &CrecheEncounterController,
    mut ready: impl FnMut() -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !ready() {
        assert!(Instant::now() < deadline, "bounded installed UI wait");
        native.service_retirement().unwrap();
        if !native.display.revoked {
            native.present_current(controller).unwrap_or_else(|error| {
                panic!(
                    "native waiting publication failed: {error}; current Face {:?}",
                    controller
                        .face()
                        .unwrap()
                        .map(|face| (face.revision, face.subjects[0].identity.clone()))
                )
            });
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn activate(
    native: &mut LocalCrecheNativeOwner<CpuOutput>,
    controller: &CrecheEncounterController,
    prefix: &str,
) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    let face = loop {
        native
            .present_current(controller)
            .unwrap_or_else(|error| panic!("native action {prefix} publication failed: {error}"));
        let face = controller.face().unwrap().unwrap();
        if native.shared.same_face(&face).unwrap() && native.mask.show().is_some() {
            break face;
        }
        assert!(
            Instant::now() < deadline,
            "current native Show before input"
        );
        native.service_retirement().unwrap();
        std::thread::sleep(Duration::from_millis(1));
    };
    let identity = face
        .actions
        .iter()
        .find(|action| action.identity.starts_with(prefix))
        .unwrap_or_else(|| panic!("native action {prefix} absent"))
        .identity
        .clone();
    for _ in 0..128 {
        if native
            .mask
            .scene()
            .unwrap()
            .focused()
            .is_some_and(|control| face.actions[control.action].identity == identity)
        {
            assert!(native
                .key(
                    controller,
                    KeyEvent::new(40, KeyTransition::Pressed, KeyModifiers::NONE).unwrap()
                )
                .unwrap());
            return face.identity.as_str().into();
        }
        assert!(!native
            .key(
                controller,
                KeyEvent::new(43, KeyTransition::Pressed, KeyModifiers::NONE).unwrap()
            )
            .unwrap());
    }
    panic!("bounded native focus navigation did not reach {prefix}")
}

#[test]
fn installed_root_executes_shipped_plot_through_native_review_images_save_and_done() {
    let resource = conduit_core::resource_offer(
        "diagnostic/installed-creche-slot",
        conduit_body_make::deployment_runtime_catalog::CRECHE_RESOURCE,
        1,
    );
    let mut host = StdHost::new_with_creche_encounter(
        StdHostConfig {
            host_id: "host/installed-native".into(),
            boot_id: "boot/installed-native".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        StdHostComposition::minimal(),
        resource,
    )
    .unwrap();
    let actual_host = host.advertisement().clone();
    let basis = conduit_observatory::FramebufferBasis {
        base_id: "diagnostic/cpu-installed-output".into(),
        width: 640,
        height: 480,
        pitch_bytes: 2560,
        bits_per_pixel: 32,
    };
    let provider = conduitos::product_bases::NativeProductBases::observe_framebuffer(&basis)
        .unwrap()
        .framebuffer_provider([29; 32])
        .unwrap();
    let mask = NativeFaceMask::prepare(
        actual_host.host_id.clone(),
        actual_host.boot_id.clone(),
        actual_host.offer_generation,
        "diagnostic",
        provider.entry.base_id.clone(),
        "diagnostic/installed-creche",
        &provider,
    )
    .unwrap();
    let (mut native, custody) = LocalCrecheNativeOwner::new(
        mask,
        CpuOutput {
            writes: 0,
            commits: 0,
            revoked: false,
        },
        LocalCrecheNativeLimits {
            maximum_presentations: 128,
            maximum_input_events: 4096,
            retirement_timeout: Duration::from_secs(2),
        },
    )
    .unwrap();
    let draft = draft();
    let profiles = draft
        .construction()
        .hosts()
        .iter()
        .map(|host| host.configuration.clone())
        .collect();
    let (adapter, controller) = CrecheEncounterAdapter::new(
        draft,
        profiles,
        "diagnostic/installed-creche".into(),
        "diagnostic/installed-creche-slot".into(),
        CrecheLimits {
            maximum_evidence_bytes: 64 * 1024,
            maximum_polls: 100_000,
            minimum_poll_interval_ms: 1,
        },
        128,
        custody,
    )
    .unwrap();
    let (planned_host, wake, plan, prospective) =
        crate::creche_encounter::tests::plan_and_wake_on_host(&adapter, actual_host.clone());
    assert_eq!(
        planned_host, actual_host,
        "planning must preserve the actual installed Host advertisement"
    );
    let (context_tx, context_rx) = mpsc::channel();
    let (started_tx, started_rx) = mpsc::channel();
    let begun = Arc::new(Mutex::new(Vec::with_capacity(1)));
    let mut adapter = ObserveAdapter {
        inner: adapter,
        prepared: context_tx,
        begun: begun.clone(),
    };
    let control = RunControl::default();
    let worker_control = control.clone();
    let worker = std::thread::spawn(move || {
        let mut outputs = Outputs(Vec::with_capacity(1));
        let report = host
            .run_body_plan_with_creche_to_with_start(
                BodyRunRequest {
                    wake: &wake,
                    plan: &plan,
                    control: &worker_control,
                    keyboard: None,
                },
                &mut adapter,
                &mut outputs,
                &mut Vec::with_capacity(64 * 1024),
                &mut Timer,
                |play, _| {
                    started_tx
                        .send(play.clone())
                        .map_err(|_| "diagnostic UI absent".into())
                },
            )
            .unwrap();
        (report, outputs)
    });
    let context = context_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let issued = started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(issued, prospective);
    native.bind_execution(&context, &issued).unwrap();
    wait_ui(&mut native, &controller, || {
        controller.face().unwrap().is_some()
    });
    let mut submitted = 0;
    for action in [
        "creche.deploy.next",
        "creche.deploy.next",
        "creche.deploy.review",
    ] {
        let prior = activate(&mut native, &controller, action);
        submitted += 1;
        wait_ui(&mut native, &controller, || {
            if action == "creche.deploy.review" {
                controller.reviewed().unwrap()
            } else {
                controller
                    .face()
                    .unwrap()
                    .is_some_and(|face| face.identity.as_str() != prior)
            }
        });
    }
    assert!(controller.reviewed().unwrap());
    controller
        .start_make_worker(|handoff, cancelled| {
            let mut job = CrecheImageProduction::prepare(
                handoff,
                41,
                "diagnostic/installed-birth".into(),
                1024 * 1024,
                cancelled,
            )
            .map_err(|_| CrecheMakeRefusal::BuildFailed)?;
            let files = Files::new();
            for host in ["first", "second"] {
                let input = files.input(&job, host, false);
                job.produce_host(host, input, files.output(&format!("{host}.iso")))
                    .map_err(|_| CrecheMakeRefusal::ArtifactRejected)?;
            }
            job.finish()
                .map_err(|_| CrecheMakeRefusal::ArtifactRejected)
        })
        .unwrap();
    wait_ui(&mut native, &controller, || {
        controller.face().unwrap().is_some_and(|face| {
            face.actions
                .iter()
                .any(|action| action.identity.starts_with("creche.images.save."))
        })
    });
    let outputs = Files::new();
    activate(&mut native, &controller, "creche.images.save.");
    submitted += 1;
    wait_ui(&mut native, &controller, || {
        controller.pending_save().unwrap().is_some()
    });
    let save = controller.pending_save().unwrap().unwrap();
    let destination = outputs.output("saved.iso");
    // Retry only while the preceding make handle is still finishing. No UI
    // action is repeated; the single pending request owns this exact grant.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if controller
            .start_save(
                destination.try_clone().unwrap(),
                save.receipt().artifact_bytes,
            )
            .is_ok()
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "preceding make worker must drain"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    wait_ui(&mut native, &controller, || {
        !controller.saved_receipts().unwrap().is_empty()
    });
    assert_eq!(
        controller.saved_receipts().unwrap()[0].artifact_sha256(),
        save.receipt().artifact_sha256
    );
    assert_eq!(
        destination.metadata().unwrap().len(),
        save.receipt().artifact_bytes
    );
    {
        use sha2::{Digest, Sha256};
        use std::io::{Read, Seek, SeekFrom};
        let mut saved = destination.try_clone().unwrap();
        saved.seek(SeekFrom::Start(0)).unwrap();
        let mut digest = Sha256::new();
        let mut buffer = [0; 64 * 1024];
        let mut total = 0_u64;
        loop {
            let n = saved.read(&mut buffer).unwrap();
            if n == 0 {
                break;
            }
            total += n as u64;
            assert!(total <= save.receipt().artifact_bytes);
            digest.update(&buffer[..n]);
        }
        assert_eq!(total, save.receipt().artifact_bytes);
        assert_eq!(
            format!("sha256:{:x}", digest.finalize()),
            save.receipt().artifact_sha256
        );
    }
    activate(&mut native, &controller, "creche.images.done.");
    submitted += 1;
    wait_ui(&mut native, &controller, || {
        controller.finish_images_pending().unwrap()
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if controller.start_finish_images().is_ok() {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    wait_ui(&mut native, &controller, || worker.is_finished());
    let (report, outputs) = worker.join().unwrap();
    assert_eq!(
        report.terminal,
        conduit_core::TerminalDisposition::Completed
    );
    assert!(report.failure.is_none() && report.cleanup_failure.is_none());
    assert_eq!(report.requests.len(), 1);
    assert_eq!(outputs.0.len(), 1);
    let outcome = CrecheOutcome::decode(&outputs.0[0].bytes).unwrap();
    assert_eq!(outcome.disposition, CrecheDisposition::Prepared);
    assert_eq!(begun.lock().unwrap().as_slice(), &[outcome.request]);
    assert_eq!(report.fore_deliveries, outputs.0);
    let products = controller.take_products().unwrap().unwrap();
    assert_eq!(products.artifacts().len(), 2);
    products.biography().validate().unwrap();
    assert_eq!(products.biography().friendly_name, "Root Garden");
    for artifact in products.artifacts() {
        assert_eq!(
            artifact.receipt().body_id,
            products.biography().body_id.as_str()
        );
        assert!(!artifact.receipt().boot_observed && !artifact.receipt().join_observed);
    }
    assert_eq!(submitted, 5);
    assert!(native.inputs >= submitted && native.display.writes > 0 && native.display.commits > 0);
    assert!(native.display.revoked && native.mask.show().is_none());
}
