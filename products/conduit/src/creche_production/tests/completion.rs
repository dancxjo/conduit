//! Installed owner lifecycle with real fixture-byte products, no boot claim.
use super::*;
use conduit_body_make::deployment_runtime::CrecheDisposition;
fn review_live(
    owner: &mut CrecheHost<'_>,
    controller: &CrecheEncounterController,
    custody: &Arc<Mutex<FixtureReturnedInput>>,
) -> CrecheReviewedHandoff {
    for (action, value) in [
        ("creche.deploy.next", None),
        ("creche.deploy.scope", Some(b"all".as_slice())),
        ("creche.deploy.next", None),
        ("creche.deploy.source", Some(b"release".as_slice())),
        ("creche.deploy.review", None),
    ] {
        act(owner, controller, custody, action, value);
    }
    controller.take_reviewed().unwrap().unwrap()
}
fn products(handoff: CrecheReviewedHandoff, files: &Files) -> CrecheProducedImages {
    let mut job = CrecheImageProduction::prepare(
        handoff,
        1,
        "fixture/installed-production-birth".into(),
        1024 * 1024,
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    for host in ["first", "second"] {
        let input = files.input(&job, host, false);
        job.produce_host(host, input, files.output(&format!("{host}.iso")))
            .unwrap();
    }
    match job.finish() {
        Ok(products) => products,
        Err((error, _)) => panic!("fixture batch failed: {error:?}"),
    }
}
#[test]
fn installed_owner_publishes_exact_prepared_digest_before_single_product_transfer() {
    let (mut adapter, controller, custody) = adapter();
    let (host, plan, play) = plan(&adapter);
    let mut owner = CrecheHost::prepare(&mut adapter, &host, &plan, &play).unwrap();
    owner.confirm_started(&host, &play).unwrap();
    owner.begin(owner.request()).unwrap();
    let handoff = review_live(&mut owner, &controller, &custody);
    let files = Files::new();
    let products = products(handoff, &files);
    let evidence = products.evidence(4096).unwrap();
    match controller.complete_images(products) {
        Ok(()) => {}
        Err((error, _)) => panic!("complete images: {error}"),
    }
    assert!(controller.take_products().unwrap().is_none());
    std::thread::sleep(owner.poll_wait());
    assert_eq!(
        owner.poll().unwrap(),
        CrechePoll::Pending,
        "Images remains active until Done"
    );
    assert!(
        controller.start_finish_images().is_err(),
        "no owner-invented Done"
    );
    image_action(&mut owner, &controller, &custody, "creche.images.done.");
    start_finish(&controller);
    let outcome = await_outcome(&mut owner);
    assert_eq!(outcome.request, owner.request());
    assert_eq!(outcome.disposition, CrecheDisposition::Prepared);
    let transferred = controller.take_products().unwrap().unwrap();
    assert_eq!(transferred.evidence(4096).unwrap(), evidence);
    assert_eq!(transferred.artifacts().len(), 2);
    assert!(controller.take_products().unwrap().is_none());
    owner.retire().unwrap();
    drop(owner);
    let completion = adapter.retained_evidence(&outcome).unwrap();
    assert_eq!(
        outcome.evidence_digest,
        <[u8; 32]>::from(Sha256::digest(completion))
    );
    let completion: serde_json::Value = serde_json::from_slice(completion).unwrap();
    assert_eq!(
        completion["prepared"],
        serde_json::from_slice::<serde_json::Value>(&evidence).unwrap()
    );
    assert_eq!(completion["saved"], serde_json::json!([]));
}
#[test]
fn altered_review_request_cannot_transfer_otherwise_verified_products() {
    let (mut adapter, controller, custody) = adapter();
    let (host, plan, play) = plan(&adapter);
    let mut owner = CrecheHost::prepare(&mut adapter, &host, &plan, &play).unwrap();
    owner.confirm_started(&host, &play).unwrap();
    owner.begin(owner.request()).unwrap();
    let mut handoff = review_live(&mut owner, &controller, &custody);
    handoff.request.0[0] ^= 1;
    let files = Files::new();
    let products = products(handoff, &files);
    let birth = products.biography().clone();
    let evidence = products.evidence(4096).unwrap();
    let returned = match controller.complete_images(products) {
        Err((_, products)) => products,
        Ok(()) => panic!("altered request must refuse custody transfer"),
    };
    assert_eq!(returned.biography(), &birth);
    assert_eq!(returned.artifacts().len(), 2);
    assert_eq!(returned.evidence(4096).unwrap(), evidence);
    assert!(controller.take_products().unwrap().is_none());
    std::thread::sleep(owner.poll_wait());
    assert_eq!(owner.poll().unwrap(), CrechePoll::Pending);
    owner.retire().unwrap();
}

#[test]
fn worker_completion_refusal_classifies_and_retains_exact_products_after_retirement() {
    let (mut adapter, controller, custody) = adapter();
    let (host, plan, play) = plan(&adapter);
    let mut owner = CrecheHost::prepare(&mut adapter, &host, &plan, &play).unwrap();
    owner.confirm_started(&host, &play).unwrap();
    owner.begin(owner.request()).unwrap();
    for (action, value) in [
        ("creche.deploy.next", None),
        ("creche.deploy.scope", Some(b"all".as_slice())),
        ("creche.deploy.next", None),
        ("creche.deploy.source", Some(b"release".as_slice())),
        ("creche.deploy.review", None),
    ] {
        act(&mut owner, &controller, &custody, action, value);
    }
    let files = Arc::new(Files::new());
    let retained_files = files.clone();
    let (produced, observed) = std::sync::mpsc::sync_channel(1);
    controller
        .start_make_worker(move |mut handoff, _| {
            handoff.request.0[0] ^= 1;
            let products = products(handoff, &retained_files);
            produced
                .send((
                    products.biography().clone(),
                    products.evidence(4096).unwrap(),
                ))
                .unwrap();
            Ok(products)
        })
        .unwrap();
    let (birth, evidence) = observed
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        std::thread::sleep(owner.poll_wait());
        if let CrechePoll::Outcome(outcome) = owner.poll().unwrap() {
            assert_eq!(outcome.disposition, CrecheDisposition::Refused);
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "worker rejection must terminate the encounter"
        );
    }
    owner.retire().unwrap();
    let retained = controller.take_rejected_products().unwrap().unwrap();
    assert!(!retained.reason().is_empty() && retained.reason().len() <= 256);
    assert_eq!(retained.products().biography(), &birth);
    assert_eq!(retained.products().evidence(4096).unwrap(), evidence);
    assert_eq!(retained.products().artifacts().len(), 2);
    assert!(controller.take_rejected_products().unwrap().is_none());
    assert!(files.root.join("first.iso").is_file());
}

// These are actual fixture files + installed owner with software Mask custody;
// no SDL/physical presentation or boot/join evidence is asserted.
fn image_action(
    owner: &mut CrecheHost<'_>,
    controller: &CrecheEncounterController,
    custody: &Arc<Mutex<FixtureReturnedInput>>,
    prefix: &str,
) {
    let face = controller.face().unwrap().unwrap();
    let action = face
        .actions
        .iter()
        .find(|action| action.identity.starts_with(prefix))
        .unwrap()
        .identity
        .clone();
    act(owner, controller, custody, &action, None);
}
fn start_finish(controller: &CrecheEncounterController) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match controller.start_finish_images() {
            Ok(()) => return,
            Err(error) => {
                assert!(controller.finish_images_pending().unwrap(), "{error}");
                assert!(std::time::Instant::now() < deadline, "{error}");
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
    }
}
fn await_outcome(
    owner: &mut CrecheHost<'_>,
) -> conduit_body_make::deployment_runtime::CrecheOutcome {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        std::thread::sleep(owner.poll_wait());
        if let CrechePoll::Outcome(outcome) = owner.poll().unwrap() {
            return outcome;
        }
        assert!(std::time::Instant::now() < deadline);
    }
}
fn await_images_idle(owner: &mut CrecheHost<'_>, controller: &CrecheEncounterController) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        std::thread::sleep(owner.poll_wait());
        assert_eq!(owner.poll().unwrap(), CrechePoll::Pending);
        let face = controller.face().unwrap().unwrap();
        if face
            .actions
            .iter()
            .all(|a| a.availability == conduit_face::FaceActionAvailability::Available)
        {
            return;
        }
        assert!(std::time::Instant::now() < deadline);
    }
}
#[test]
fn canonical_save_failure_retry_and_done_preserve_one_birth_and_exact_saved_bytes() {
    let (mut adapter, controller, custody) = adapter();
    let (host, plan, play) = plan(&adapter);
    let mut owner = CrecheHost::prepare(&mut adapter, &host, &plan, &play).unwrap();
    owner.confirm_started(&host, &play).unwrap();
    owner.begin(owner.request()).unwrap();
    let handoff = review_live(&mut owner, &controller, &custody);
    let files = Files::new();
    let products = products(handoff, &files);
    let birth = products.biography().clone();
    let expected = products.artifacts()[0].receipt().clone();
    let (_, returned) = controller
        .start_save(files.output("no-action.iso"), 1024 * 1024)
        .unwrap_err();
    returned.set_len(7).unwrap();
    assert_eq!(
        returned.metadata().unwrap().len(),
        7,
        "refused grant remains usable"
    );
    controller
        .complete_images(products)
        .unwrap_or_else(|(error, _)| panic!("{error}"));
    let revision = controller.face().unwrap().unwrap().revision;
    image_action(&mut owner, &controller, &custody, "creche.images.save.");
    assert_eq!(
        controller.pending_save().unwrap().unwrap().receipt(),
        &expected
    );
    assert!(controller
        .face()
        .unwrap()
        .unwrap()
        .actions
        .iter()
        .all(|a| a.availability != conduit_face::FaceActionAvailability::Available));
    controller
        .start_save(files.output("too-small.iso"), 1)
        .unwrap();
    await_images_idle(&mut owner, &controller);
    assert!(controller.saved_receipts().unwrap().is_empty());
    assert_eq!(
        std::fs::metadata(files.root.join("too-small.iso"))
            .unwrap()
            .len(),
        0
    );
    assert!(controller.face().unwrap().unwrap().revision > revision);
    image_action(&mut owner, &controller, &custody, "creche.images.save.");
    // Finished handle reaping is outside poll; a worker can still be exiting
    // after publishing its in-memory completion, so wait for that bounded seam.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut output = files.output("saved.iso");
    loop {
        match controller.start_save(output, 1024 * 1024) {
            Ok(()) => break,
            Err((error, returned)) => {
                output = returned;
                assert!(controller.pending_save().unwrap().is_some(), "{error}");
                assert!(std::time::Instant::now() < deadline, "{error}");
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
    }
    await_images_idle(&mut owner, &controller);
    let receipts = controller.saved_receipts().unwrap();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].artifact_sha256(), expected.artifact_sha256);
    assert_eq!(receipts[0].artifact_bytes(), expected.artifact_bytes);
    let bytes = std::fs::read(files.root.join("saved.iso")).unwrap();
    assert_eq!(
        format!("sha256:{:x}", Sha256::digest(bytes)),
        expected.artifact_sha256
    );
    assert!(controller.take_products().unwrap().is_none());
    image_action(&mut owner, &controller, &custody, "creche.images.done.");
    assert!(controller.finish_images_pending().unwrap());
    start_finish(&controller);
    let outcome = await_outcome(&mut owner);
    assert_eq!(outcome.disposition, CrecheDisposition::Prepared);
    let retained = controller.take_products().unwrap().unwrap();
    assert_eq!(retained.biography(), &birth);
    assert_eq!(retained.artifacts()[0].receipt(), &expected);
    owner.retire().unwrap();
    drop(owner);
    let evidence = adapter.retained_evidence(&outcome).unwrap();
    assert_eq!(
        outcome.evidence_digest,
        <[u8; 32]>::from(Sha256::digest(evidence))
    );
    let evidence: serde_json::Value = serde_json::from_slice(evidence).unwrap();
    assert_eq!(evidence["saved"], serde_json::to_value(&receipts).unwrap());
    assert_eq!(evidence["boot_observed"], false);
    assert_eq!(evidence["join_observed"], false);
}
#[test]
fn retiring_images_retains_exact_batch_without_inventing_prepared_terminal() {
    let (mut adapter, controller, custody) = adapter();
    let (host, plan, play) = plan(&adapter);
    let mut owner = CrecheHost::prepare(&mut adapter, &host, &plan, &play).unwrap();
    owner.confirm_started(&host, &play).unwrap();
    owner.begin(owner.request()).unwrap();
    let handoff = review_live(&mut owner, &controller, &custody);
    let files = Files::new();
    let products = products(handoff, &files);
    let birth = products.biography().clone();
    controller
        .complete_images(products)
        .unwrap_or_else(|(error, _)| panic!("{error}"));
    assert!(controller.take_retired_products().is_err());
    custody.lock().unwrap().drain_failures = 1;
    assert!(owner.retire().is_err());
    assert!(
        controller.take_retired_products().is_err(),
        "inactive and worker-drained is insufficient while Mask drain is pending"
    );
    owner.retire().unwrap();
    assert!(controller.take_products().unwrap().is_none());
    let retained = controller.take_retired_products().unwrap().unwrap();
    assert_eq!(retained.biography(), &birth);
    assert_eq!(retained.artifacts().len(), 2);
}
