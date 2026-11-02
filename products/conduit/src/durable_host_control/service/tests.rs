use super::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[test]
fn idle_foreground_progress_keeps_authenticated_control_responsive() {
    let root = std::env::temp_dir().join(format!(
        "conduit-foreground-{}-{}",
        std::process::id(),
        crate::durable_host::fresh_identity("foreground", "test").replace('/', "-")
    ));
    fs::create_dir_all(&root).unwrap();
    ensure_secret(&root).unwrap();
    let done = Arc::new(AtomicBool::new(false));
    let client_done = done.clone();
    let client_root = root.clone();
    let client = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !client_root.join("control.sock").exists() {
            assert!(std::time::Instant::now() < deadline, "service did not bind");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let result = current(&client_root);
        client_done.store(true, Ordering::Release);
        result
    });
    let thread = std::thread::current().id();
    let mut polls = 0;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    let mut poll = |runtime: &mut DurableHostRuntime| {
        assert_eq!(std::thread::current().id(), thread);
        assert!(!runtime.host.body_is_running());
        polls += 1;
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
    assert!(client.join().unwrap().is_ok());
    fs::remove_dir_all(root).unwrap();
}
