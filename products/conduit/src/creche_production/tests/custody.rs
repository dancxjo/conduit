//! Owner-held path changes must not turn earlier readback into current success.
use super::*;
use std::io::Write;

fn produce_all(job: &mut CrecheImageProduction, files: &Files) {
    for host in ["first", "second"] {
        let input = files.input(job, host, false);
        job.produce_host(host, input, files.output(&format!("{host}.iso")))
            .unwrap();
    }
}

#[test]
fn changed_after_finish_refuses_completion_without_losing_birth_or_files() {
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
    let handoff = controller.take_reviewed().unwrap().unwrap();
    let mut job = CrecheImageProduction::prepare(
        handoff,
        41,
        "fixture/custody-birth".into(),
        1024 * 1024,
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let birth = job.biography().clone();
    let files = Files::new();
    produce_all(&mut job, &files);
    let products = match job.finish() {
        Ok(products) => products,
        Err((error, _)) => panic!("valid initial batch: {error:?}"),
    };
    let mut changed = OpenOptions::new()
        .write(true)
        .open(files.root.join("first.iso"))
        .unwrap();
    changed.seek(SeekFrom::Start(100)).unwrap();
    changed.write_all(b"changed after finish").unwrap();
    changed.sync_all().unwrap();
    let returned = match controller.complete_images(products) {
        Err((reason, products)) => {
            assert!(reason.contains("custody"));
            products
        }
        Ok(()) => panic!("earlier opaque success cannot admit changed current bytes"),
    };
    assert_eq!(returned.biography(), &birth);
    assert_eq!(returned.artifacts().len(), 2);
    assert!(files.root.join("first.iso").exists() && files.root.join("second.iso").exists());
    assert!(controller.take_products().unwrap().is_none());
    std::thread::sleep(owner.poll_wait());
    assert_eq!(owner.poll().unwrap(), CrechePoll::Pending);
    owner.retire().unwrap();
}

#[test]
fn changed_product_can_be_withdrawn_and_replaced_under_original_total_budget() {
    let mut job = job("release", Arc::new(AtomicBool::new(false)));
    let birth = job.biography().clone();
    let files = Files::new();
    produce_all(&mut job, &files);
    // No spare capacity: replacement must reclaim the withdrawn reservation.
    job.maximum_total_bytes = job.produced_bytes;
    OpenOptions::new()
        .write(true)
        .open(files.root.join("first.iso"))
        .unwrap()
        .set_len(1)
        .unwrap();
    let mut job = match job.finish() {
        Err((
            CrecheProductionRefusal::Artifact(PreparedBirthArtifactRefusal::InvalidManifest),
            job,
        )) => *job,
        _ => panic!("changed current bytes must retain original job"),
    };
    let withdrawn = job.withdraw_product("first").unwrap();
    assert_eq!(withdrawn.receipt().host_entry, "first");
    assert_eq!(withdrawn.file.metadata().unwrap().len(), 1);
    assert_eq!(job.biography(), &birth);
    assert!(!job.is_complete());
    assert_eq!(job.artifacts().len(), 1);
    assert!(matches!(
        job.withdraw_product("first"),
        Err(CrecheProductionRefusal::UnknownOrCompletedHost)
    ));
    let input = files.input(&job, "first", false);
    job.produce_host("first", input, files.output("first-replacement.iso"))
        .unwrap();
    let complete = match job.finish() {
        Ok(products) => products,
        Err((error, _)) => panic!("same-birth replacement batch: {error:?}"),
    };
    assert_eq!(complete.biography(), &birth);
    assert_eq!(complete.artifacts().len(), 2);
    assert_eq!(fs::metadata(files.root.join("first.iso")).unwrap().len(), 1);
    for artifact in complete.artifacts() {
        let mut file = artifact.file.try_clone().unwrap();
        file.seek(SeekFrom::Start(SLOT as u64)).unwrap();
        let mut frame = vec![0; PREPARED_BIRTH_MEDIA_BYTES];
        file.read_exact(&mut frame).unwrap();
        assert_eq!(
            PreparedBirthMedia::decode(&frame).unwrap().evidence(),
            &birth
        );
        assert!(!artifact.receipt().boot_observed && !artifact.receipt().join_observed);
    }
}
