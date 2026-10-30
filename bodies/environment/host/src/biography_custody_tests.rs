use super::*;
use conduit_body as b;
use conduit_core as c;
use std::{
    os::unix::fs::PermissionsExt,
    sync::atomic::{AtomicUsize, Ordering},
};
fn pending() -> BodyLifecycleSession {
    let workset = b::BodyWorkset::from_plots([b::ResidentPlot::new(
        "source/test".into(),
        "checked/test".into(),
    )])
    .unwrap();
    let body = b::Body::born_with_plots(workset, 1, "sign/birth".into()).unwrap();
    let membership = b::BodyMembership::new(body.body_id.clone()).unwrap();
    let evidence =
        b::BodyBiographyEvidence::born(body, membership, "Custody Fixture".into()).unwrap();
    let host = c::HostAdvertisement {
        declared_name: None,
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "host/custody".into(),
        boot_id: "boot/0".into(),
        offer_generation: c::OfferGeneration(1),
        profile: "fixture/custody".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let mut body =
        conduit_environment_runtime::body_image::open_installed_image(evidence, &host).unwrap();
    for i in 1..=b::MAX_MEMBERSHIP_EVENTS + 2 {
        body = b::BodyLifecycleSession::resume_here(
            body.evidence().clone(),
            &host.host_id,
            &format!("boot/{i}").into(),
        )
        .unwrap();
        if !body.pending_archives().is_empty() {
            return body;
        }
    }
    panic!("fixture failed to cross the actual membership compaction boundary")
}
fn directory() -> std::path::PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "conduit-biography-custody-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}
#[test]
fn exact_fsynced_archive_acknowledges_and_existing_identical_copy_is_reused() {
    let mut body = pending();
    let id = body.evidence().body_id.clone();
    let expected = body.pending_archives()[0].clone();
    let dir = directory();
    retain(&mut body, &dir).unwrap();
    assert!(body.pending_archives().is_empty());
    assert_eq!(body.evidence().body_id, id);
    let files = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(files.len(), 1);
    let stored: BodyBiographyArchiveSegment =
        serde_json::from_slice(&std::fs::read(&files[0]).unwrap()).unwrap();
    stored.validate().unwrap();
    assert_eq!(stored, expected);
    assert_eq!(
        std::fs::metadata(&files[0]).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let mut again = pending();
    retain(&mut again, &dir).unwrap();
    assert!(again.pending_archives().is_empty());
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn failed_or_conflicting_custody_never_acknowledges_or_overwrites() {
    let mut body = pending();
    let dir = directory();
    std::fs::write(&dir, b"blocked parent").unwrap();
    assert!(retain(&mut body, &dir).is_err());
    assert!(!body.pending_archives().is_empty());
    std::fs::remove_file(&dir).unwrap();
    std::fs::create_dir(&dir).unwrap();
    let name: String = body.pending_archives()[0]
        .digest
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let path = dir.join(format!("{name}.json"));
    std::fs::write(&path, b"corrupt").unwrap();
    assert!(retain(&mut body, &dir).is_err());
    assert!(!body.pending_archives().is_empty());
    assert_eq!(std::fs::read(path).unwrap(), b"corrupt");
    std::fs::remove_dir_all(dir).unwrap();
}
