use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "conduit-workset-capture-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("source.conduit"), b"plot main() {}\n").unwrap();
        Self(path)
    }
    fn receipt(&self, changed: bool) -> PathBuf {
        let bytes = fs::read(self.0.join("source.conduit")).unwrap();
        let input = self.0.join("input.json");
        let selections = if changed {
            serde_json::json!([{"kind":"merge","value":{"type_name":"Frame","maximum_bytes":512}}])
        } else {
            serde_json::json!([])
        };
        fs::write(&input,serde_json::to_vec(&serde_json::json!({"schema":1,"source_sha256":format!("{:x}",Sha256::digest(bytes)),"specializations":selections})).unwrap()).unwrap();
        input
    }
    fn capture(&self, receipt: Option<&Path>, output: &str) -> WorksetSourceInput {
        WorksetSourceInput::capture(
            &self.0.join("source.conduit"),
            &["main".into()],
            receipt,
            "fixture/git",
            &self.0.join(output),
            false,
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn captured_selection_bytes_are_pinned_and_change_the_image_source_identity() {
    let f = Fixture::new();
    let path = f.receipt(false);
    let selected = f.capture(Some(&path), "selected");
    let plain = f.capture(None, "plain");
    assert_ne!(selected.identity, plain.identity);
    f.receipt(true);
    selected.verify().unwrap(); // The captured copy does not follow later input changes.
    let changed = f.capture(Some(&path), "changed");
    assert_ne!(selected.identity, changed.identity);
    assert_eq!(selected.sha256, changed.sha256);
}
#[test]
fn mutation_of_the_captured_receipt_refuses_before_configuring_a_child() {
    let f = Fixture::new();
    let path = f.receipt(false);
    let selected = f.capture(Some(&path), "selected");
    fs::write(f.0.join("selected/workset-specializations.json"), b"{}").unwrap();
    let mut command = Command::new("unused");
    assert!(selected.configure(&mut command).is_err());
    assert_eq!(command.get_envs().count(), 0);
}
#[test]
fn omitted_selections_cannot_enter_from_the_parent_environment() {
    let f = Fixture::new();
    let plain = f.capture(None, "plain");
    let mut command = Command::new("unused");
    command
        .env("CONDUIT_WORKSET_SPECIALIZATIONS", "stale")
        .env("CONDUIT_WORKSET_SPECIALIZATIONS_SHA256", "stale");
    plain.configure(&mut command).unwrap();
    for name in [
        "CONDUIT_WORKSET_SPECIALIZATIONS",
        "CONDUIT_WORKSET_SPECIALIZATIONS_SHA256",
    ] {
        assert_eq!(
            command.get_envs().find(|(key, _)| *key == name).unwrap().1,
            None
        );
    }
}
#[test]
fn mismatched_source_and_oversized_selection_inputs_refuse_capture() {
    let f = Fixture::new();
    let path = f.receipt(false);
    fs::write(
        &path,
        b"{\"schema\":1,\"source_sha256\":\"wrong\",\"specializations\":[]}",
    )
    .unwrap();
    let capture = || {
        WorksetSourceInput::capture(
            &f.0.join("source.conduit"),
            &["main".into()],
            Some(&path),
            "fixture/git",
            &f.0.join("out"),
            false,
        )
    };
    assert!(capture().is_err());
    fs::write(&path, vec![b' '; 64 * 1024 + 1]).unwrap();
    assert!(capture().is_err());
}

#[test]
fn no_selected_workset_clears_all_inherited_compilation_inputs() {
    let names = [
        "CONDUIT_WORKSET_SOURCE",
        "CONDUIT_WORKSET_SOURCE_SHA256",
        "CONDUIT_WORKSET_ENTRIES",
        "CONDUIT_WORKSET_SPECIALIZATIONS",
        "CONDUIT_WORKSET_SPECIALIZATIONS_SHA256",
    ];
    let mut command = Command::new("unused");
    for name in names {
        command.env(name, "stale");
    }
    clear_inputs(&mut command);
    for name in names {
        assert_eq!(
            command.get_envs().find(|(key, _)| *key == name).unwrap().1,
            None
        );
    }
}
