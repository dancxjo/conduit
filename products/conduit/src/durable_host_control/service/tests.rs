use super::*;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

#[test]
fn idle_foreground_progress_keeps_authenticated_control_responsive() {
    let root = std::env::temp_dir().join(format!(
        "c-fg-{}-{:.16}",
        std::process::id(),
        crate::durable_host::fresh_identity("foreground", "test").replace('/', "-")
    ));
    fs::create_dir_all(&root).unwrap();
    ensure_secret(&root).unwrap();
    let done = Arc::new(AtomicBool::new(false));
    let client_done = done.clone();
    let idle_polls = Arc::new(AtomicUsize::new(0));
    let client_polls = idle_polls.clone();
    let client_root = root.clone();
    let client = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !client_root.join("control.sock").exists() {
            assert!(std::time::Instant::now() < deadline, "service did not bind");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        while client_polls.load(Ordering::Acquire) < 3 && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let observed_idle_progress = client_polls.load(Ordering::Acquire) >= 3;
        // Also wake a mistakenly blocking accept on failure, so the fixture
        // reports a regression instead of leaving the test server stranded.
        let result = current(&client_root);
        client_done.store(true, Ordering::Release);
        (result, observed_idle_progress)
    });
    let thread = std::thread::current().id();
    let mut polls = 0;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    let mut poll = |runtime: &mut DurableHostRuntime| {
        assert_eq!(std::thread::current().id(), thread);
        assert!(!runtime.host.body_is_running());
        polls += 1;
        idle_polls.fetch_add(1, Ordering::Release);
        if done.load(Ordering::Acquire) || std::time::Instant::now() > deadline {
            return Err("fixture stopped".into());
        }
        Ok(())
    };
    let runtime = DurableHostRuntime::new(
        "test/foreground".into(),
        "sha256:fixture".into(),
        StdHost::new(),
    );
    assert_eq!(
        serve_with_foreground(&root, runtime, &mut poll).unwrap_err(),
        "fixture stopped"
    );
    assert!(polls >= 2);
    let (response, observed_idle_progress) = client.join().unwrap();
    assert!(response.is_ok());
    assert!(
        observed_idle_progress,
        "idle output polling blocked on control accept"
    );
    fs::remove_dir_all(root).unwrap();
}
