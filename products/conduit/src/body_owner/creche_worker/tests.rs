//! Real Body lifecycle/kernel tests with refusal-only software Mask custody.
//! No image manufacture, native rendering or physical deployment is asserted.
use super::*;
use conduit::creche_encounter::{CrecheEncounterController, CrecheMaskShowCustody};
use conduit_body::ResidentPlot;
use conduit_face::{Face, FaceInteraction, MaskShow};
use conduit_std_host::hosted_creche::CrecheLimits;
use std::sync::atomic::{AtomicUsize, Ordering};
#[path = "../../../../../make/body/tests/common/deployment.rs"]
mod draft_fixture;

struct Custody(Arc<AtomicUsize>, bool);
impl CrecheMaskShowCustody for Custody {
    fn validate_acknowledged(&mut self, _: &Face, _: &MaskShow) -> Result<(), String> {
        Err("no fixture presentation".into())
    }
    fn validate_returned_interaction(
        &mut self,
        _: &Face,
        _: &MaskShow,
        _: &FaceInteraction,
    ) -> Result<(), String> {
        Err("no fixture interaction".into())
    }
    fn retire_and_drain(&mut self) -> Result<(), String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        if self.1 {
            Err("fixture retirement failed".into())
        } else {
            Ok(())
        }
    }
}
struct OtherFore;
impl BodyForeOutputAdapter for OtherFore {
    fn deliver(&mut self, _: &ExternalForeDelivery) -> Result<(), String> {
        Err("fixture expected no other Fore".into())
    }
}
fn fixture(
    fail_retirement: bool,
) -> (
    Owner,
    std::path::PathBuf,
    CrecheEncounterAdapter,
    CrecheEncounterController,
    Arc<AtomicUsize>,
) {
    fixture_with_source(
        include_str!("../../../../../plots/creche/main.conduit"),
        fail_retirement,
    )
}
pub(in crate::durable_host::owner::controller) fn fixture_with_source(
    source: &str,
    fail_retirement: bool,
) -> (
    Owner,
    std::path::PathBuf,
    CrecheEncounterAdapter,
    CrecheEncounterController,
    Arc<AtomicUsize>,
) {
    let draft = draft_fixture::encounter("Child Body");
    let profiles = draft
        .construction()
        .hosts()
        .iter()
        .map(|host| host.configuration.clone())
        .collect();
    let retired = Arc::new(AtomicUsize::new(0));
    let (adapter, controller) = CrecheEncounterAdapter::new(
        draft,
        profiles,
        "fixture/owner-creche".into(),
        "fixture/owner-creche-slot".into(),
        CrecheLimits {
            maximum_evidence_bytes: 4096,
            maximum_polls: 1000,
            minimum_poll_interval_ms: 1,
        },
        32,
        Box::new(Custody(retired.clone(), fail_retirement)),
    )
    .unwrap();
    let checked = crate::plot_source::parse(source)
        .unwrap()
        .expand_entry_for_authoring()
        .unwrap();
    let resident = ResidentPlot::new(
        checked.expanded.source_document_id.clone(),
        checked.expanded.checked_plot_id.clone(),
    );
    let mut owner = Owner::open(StdHost::new(), resident, None, "Existing owner fixture").unwrap();
    owner
        .attach_creche_encounter(adapter.resource_offer())
        .unwrap();
    owner.plan(&checked).unwrap();
    let root = std::env::temp_dir().join(crate::durable_host::fresh_identity(
        "creche-owner-worker",
        "fixture",
    ));
    std::fs::create_dir_all(&root).unwrap();
    let installation = crate::durable_host::Installation {
        schema: crate::durable_host::INSTALL_SCHEMA.into(),
        host_id: owner.host.advertisement().host_id.as_str().into(),
        release_source_identity: "source/fixture".into(),
        release_bundle_sha256: crate::durable_host::digest(b"fixture"),
        product_executable: "fixture-unused".into(),
        body_state: None,
        joined_body_state: None,
        selected_speech: None,
        selected_model: None,
        selected_todo_checkpoint: None,
    };
    crate::durable_host::write_json_atomic(&root.join("installation.json"), &installation).unwrap();
    (owner, root, adapter, controller, retired)
}
fn finish(
    worker: &mut CrecheWorker,
    owner: &mut Owner,
    root: &Path,
    fail_binding: bool,
) -> (usize, Vec<String>) {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut bound = 0;
    let mut errors = Vec::new();
    loop {
        assert!(Instant::now() < deadline, "worker must actually finish");
        match worker.progress(owner, root, |context, play| {
            bound += 1;
            assert_eq!(context.play(), play);
            if fail_binding {
                Err("fixture native binding failed".into())
            } else {
                Ok(())
            }
        }) {
            Ok(true) => return (bound, errors),
            Ok(false) => {}
            Err(error) => errors.push(error),
        }
        thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn native_binding_failure_after_retained_start_gets_actual_terminal_report_and_same_host() {
    let (mut owner, root, adapter, controller, retired) = fixture(false);
    let before = owner.session.evidence().body_id.clone();
    let advertisement = owner.host.advertisement().clone();
    let mut worker = owner
        .start_creche_run(&root, 1000, adapter, Box::new(OtherFore))
        .unwrap();
    let (bound, errors) = finish(&mut worker, &mut owner, &root, true);
    assert_eq!(bound, 1);
    assert_eq!(errors, ["fixture native binding failed"]);
    assert_eq!(owner.session.evidence().body_id, before);
    assert!(owner.session.realization().is_none());
    assert_eq!(owner.host.advertisement(), &advertisement);
    assert!(!owner.host.is_playing());
    assert!(owner.last_execution.as_ref().unwrap()["play"].is_object());
    assert!(retired.load(Ordering::SeqCst) > 0);
    assert!(controller.face().unwrap().is_none());
    let completion = worker.take_completion().unwrap();
    assert!(completion.adapter.limits().maximum_polls > 0);
    if let Some(delivery) = completion.delivery {
        use conduit_body_make::deployment_runtime::{CrecheDisposition, CrecheOutcome};
        assert!(matches!(
            CrecheOutcome::decode(&delivery.bytes).unwrap().disposition,
            CrecheDisposition::Cancelled | CrecheDisposition::Refused
        ));
    }
    drop(completion.other_fore);
    assert!(worker.take_completion().is_none());
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn delayed_owner_ack_remains_owned_after_deadline_then_records_real_terminal() {
    let (mut owner, root, adapter, _, _) = fixture(false);
    let mut worker = owner
        .start_creche_run(&root, 100, adapter, Box::new(OtherFore))
        .unwrap();
    thread::sleep(Duration::from_millis(250));
    assert!(!worker.thread.as_ref().unwrap().is_finished());
    assert!(owner.session.realization().unwrap().play.is_none());
    let (bound, errors) = finish(&mut worker, &mut owner, &root, false);
    assert_eq!(bound, 1);
    assert!(errors.is_empty());
    assert!(owner.session.realization().is_none());
    assert!(worker.take_completion().is_some());
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn panic_is_latched_without_inventing_host_restoration() {
    let (mut owner, root, _, _, _) = fixture(false);
    let host = owner.host.take_for_play().unwrap();
    let (_, started) = mpsc::sync_channel(1);
    let thread = thread::spawn(move || {
        drop(host);
        panic!("fixture worker panic");
    });
    while !thread.is_finished() {
        thread::yield_now();
    }
    let mut worker = CrecheWorker {
        control: RunControl::default(),
        started,
        acknowledge: None,
        thread: Some(thread),
        completion: None,
        retained: false,
        terminal_error: None,
    };
    let first = worker
        .progress(&mut owner, &root, |_, _| unreachable!())
        .unwrap_err();
    assert_eq!(
        worker
            .progress(&mut owner, &root, |_, _| unreachable!())
            .unwrap_err(),
        first
    );
    assert!(owner.host.is_playing());
    assert!(worker.take_completion().is_none());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn retirement_failure_stays_in_actual_report_without_claiming_cleanup() {
    let (mut owner, root, adapter, _, retired) = fixture(true);
    let mut worker = owner
        .start_creche_run(&root, 1000, adapter, Box::new(OtherFore))
        .unwrap();
    let (bound, errors) = finish(&mut worker, &mut owner, &root, true);
    assert_eq!(bound, 1);
    assert_eq!(errors, ["fixture native binding failed"]);
    assert!(retired.load(Ordering::SeqCst) > 0);
    assert!(owner.last_execution.as_ref().unwrap()["cleanup_failure"]
        .as_str()
        .unwrap()
        .contains("fixture retirement failed"));
    assert!(!owner.host.is_playing());
    assert!(worker.take_completion().is_some());
    std::fs::remove_dir_all(root).unwrap();
}

struct CountingFore(Arc<AtomicUsize>);
impl BodyForeOutputAdapter for CountingFore {
    fn deliver(&mut self, _: &ExternalForeDelivery) -> Result<(), String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
#[test]
fn exact_selected_outcome_endpoint_preserves_unrelated_fore_custody() {
    use conduit_body_make::deployment_runtime::{
        CrecheDisposition, CrecheOutcome, CRECHE_OUTCOME_KIND,
    };
    let (owner, root, adapter, _, _) = fixture(false);
    let proposed = owner.session.realization().unwrap();
    let prepared = Arc::new(OnceLock::new());
    let mut adapter = ObservedAdapter::new(adapter, prepared.clone());
    let prospective = BodyPlayIdentity::bind(&proposed.plan, 0);
    let mut host = conduit_std_host::hosted_creche::CrecheHost::prepare(
        &mut adapter,
        owner.host.advertisement(),
        &proposed.plan,
        &prospective,
    )
    .unwrap();
    host.retire().unwrap();
    drop(host);
    let context = prepared.get().unwrap();
    let counted = Arc::new(AtomicUsize::new(0));
    let mut output = OneOutcome {
        prepared: prepared.clone(),
        delivery: None,
        other_fore: Box::new(CountingFore(counted.clone())),
    };
    let fixture_outcome = CrecheOutcome {
        request: context.request,
        disposition: CrecheDisposition::Refused,
        evidence_digest: [17; 32],
    };
    let unrelated = ExternalForeDelivery::new(
        conduit_core::port_id("other-plot-output"),
        conduit_core::ConnectionTrack::Payload,
        conduit_core::kind_id(CRECHE_OUTCOME_KIND),
        0,
        fixture_outcome.encode().to_vec(),
    );
    output.deliver(&unrelated).unwrap();
    assert_eq!(counted.load(Ordering::SeqCst), 1);
    assert!(output.delivery.is_none());
    let selected = ExternalForeDelivery::new(
        context.output.clone(),
        conduit_core::ConnectionTrack::Payload,
        conduit_core::kind_id(CRECHE_OUTCOME_KIND),
        1,
        fixture_outcome.encode().to_vec(),
    );
    output.deliver(&selected).unwrap();
    assert_eq!(output.delivery.as_ref(), Some(&selected));
    assert!(output.deliver(&selected).is_err());
    assert_eq!(counted.load(Ordering::SeqCst), 1);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn pre_worker_refusal_returns_adapter_and_other_fore_custody() {
    let (mut owner, root, adapter, _, retired) = fixture(false);
    let resource = adapter.resource_offer();
    let before = owner.truth();
    let refusal = match owner.start_creche_run(&root, 0, adapter, Box::new(OtherFore)) {
        Ok(_) => panic!("invalid duration must refuse before worker ownership"),
        Err(refusal) => refusal,
    };
    assert!(refusal.reason.contains("duration"));
    assert_eq!(refusal.adapter.resource_offer(), resource);
    assert_eq!(owner.truth(), before);
    assert_eq!(retired.load(Ordering::SeqCst), 0);
    let mut adapter = refusal.adapter;
    adapter.cancel_and_drain().unwrap();
    assert_eq!(retired.load(Ordering::SeqCst), 1);
    drop(refusal.other_fore);
    std::fs::remove_dir_all(root).unwrap();
}
