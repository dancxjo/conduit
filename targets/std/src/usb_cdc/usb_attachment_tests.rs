//! Synthetic sysfs fixtures qualify only bounded attribute lookup, not hardware.
use super::*;
use std::path::PathBuf;
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "conduit-usb-attachment-{}-{id}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("idVendor"), b"2e8a\n").unwrap();
        std::fs::write(root.join("idProduct"), b"000a\n").unwrap();
        std::fs::write(root.join("serial"), b"fixture-serial\n").unwrap();
        Self(root)
    }
    fn child(&self) -> PathBuf {
        let path = self.0.join("interface/tty");
        std::fs::create_dir_all(&path).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn fixture_lookup_requires_exact_independent_pin_and_bounded_serial() {
    let fixture = Fixture::new();
    let device = fixture.child();
    verify_attachment(&device, "fixture-serial").unwrap();
    for expected in ["foreign-serial", "", "fixture-serial\n"] {
        assert!(verify_attachment(&device, expected).is_err());
    }
    let maximum = "s".repeat(128);
    std::fs::write(fixture.0.join("serial"), format!("{maximum}\n")).unwrap();
    verify_attachment(&device, &maximum).unwrap();
    std::fs::write(fixture.0.join("serial"), "s".repeat(130)).unwrap();
    assert!(verify_attachment(&device, &maximum).is_err());
    std::fs::remove_file(fixture.0.join("serial")).unwrap();
    assert!(verify_attachment(&device, "fixture-serial").is_err());
}
#[test]
fn fixture_lookup_refuses_missing_usb_parent_and_excessive_ancestry() {
    let fixture = Fixture::new();
    std::fs::remove_file(fixture.0.join("idVendor")).unwrap();
    assert!(verify_attachment(&fixture.child(), "fixture-serial").is_err());
    std::fs::write(fixture.0.join("idVendor"), b"2e8a\n").unwrap();
    let mut path = fixture.0.clone();
    for _ in 0..40 {
        path.push("child");
    }
    std::fs::create_dir_all(&path).unwrap();
    assert!(verify_attachment(&path, "fixture-serial").is_err());
}
