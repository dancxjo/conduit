use super::*;

fn fixture() -> (std::path::PathBuf, DurableHostRuntime) {
    let (root, owner) = crate::durable_host::owner::service_workset_fixture();
    let mut runtime = DurableHostRuntime::new(
        "std/x86_64/computer".into(),
        "fixture".into(),
        conduit_std_host::StdHost::new(),
    );
    runtime.host = HostSource::Body {
        owner: Box::new(owner),
        root: root.clone(),
        running: None,
    };
    (root, runtime)
}

#[test]
fn clean_deadline_starts_one_additional_play_on_the_same_boot() {
    let (root, mut runtime) = fixture();
    let original_boot = runtime.host.advertisement().boot_id.clone();
    runtime
        .start_owned_body_selected(100, None, None, 1)
        .unwrap();

    let wait = Instant::now() + Duration::from_secs(10);
    // Do not acknowledge Started until the actual service timer has fired.
    std::thread::sleep(Duration::from_millis(200));
    while runtime.body_renewal.is_some() {
        runtime.progress_owned_body().unwrap();
        assert!(Instant::now() < wait, "first Play must retain its deadline");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(runtime.host.body_is_running(), "one renewal must start");
    assert_eq!(runtime.host.advertisement().boot_id, original_boot);

    while runtime.host.body_is_running() {
        runtime.progress_owned_body().unwrap();
        assert!(Instant::now() < wait, "second Play must finish");
        std::thread::sleep(Duration::from_millis(5));
    }
    runtime.progress_owned_body().unwrap();
    assert!(
        !runtime.host.body_is_running(),
        "no third Play was admitted"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn explicit_lull_disarms_renewal_even_when_deadline_already_requested_stop() {
    let (root, mut runtime) = fixture();
    runtime
        .start_owned_body_selected(100, None, None, 1)
        .unwrap();
    std::thread::sleep(Duration::from_millis(200));
    // A competing deadline may already own the single stop slot. Lull still
    // withdraws the separate authority to start a successor Play.
    let _ = runtime.request_owned_body_lull();
    assert!(runtime.body_renewal.is_none());

    let wait = Instant::now() + Duration::from_secs(10);
    while runtime.host.body_is_running() {
        runtime.progress_owned_body().unwrap();
        assert!(Instant::now() < wait, "owner must settle after Lull");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(runtime.body_renewal.is_none());
    runtime.progress_owned_body().unwrap();
    assert!(!runtime.host.body_is_running());
    std::fs::remove_dir_all(root).unwrap();
}
