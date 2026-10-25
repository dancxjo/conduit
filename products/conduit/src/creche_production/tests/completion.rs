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
    let expected_digest: [u8; 32] = Sha256::digest(&evidence).into();
    match controller.complete_images(products) {
        Ok(()) => {}
        Err((error, _)) => panic!("complete images: {error}"),
    }
    assert!(controller.take_products().unwrap().is_none());
    std::thread::sleep(owner.poll_wait());
    let CrechePoll::Outcome(outcome) = owner.poll().unwrap() else {
        panic!("prepared outcome")
    };
    assert_eq!(outcome.request, owner.request());
    assert_eq!(outcome.disposition, CrecheDisposition::Prepared);
    assert_eq!(outcome.evidence_digest, expected_digest);
    let transferred = controller.take_products().unwrap().unwrap();
    assert_eq!(transferred.evidence(4096).unwrap(), evidence);
    assert_eq!(transferred.artifacts().len(), 2);
    assert!(controller.take_products().unwrap().is_none());
    owner.retire().unwrap();
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
