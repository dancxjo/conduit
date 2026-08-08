use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "patchbay-native-copy-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn provider(
    choices: impl IntoIterator<Item = Result<Option<PathBuf>, String>>,
) -> NativeFileProvider {
    NativeFileProvider {
        backend: DialogBackend::Scripted(choices.into_iter().collect()),
    }
}

fn wait(task: &mut NativeFileTask) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if task.poll().unwrap() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("copy did not publish its receipt within five seconds");
}

#[test]
fn omitted_provider_is_valid_and_advertises_no_file_capability() {
    let mut task = NativeFileTask::new(None);
    let lines = task.lines().join("\n");
    assert!(lines.contains("usable=false capability-advertised=false"));
    assert!(task.choose_source().unwrap_err().contains("unavailable"));
    assert!(task.plan().unwrap_err().contains("not been chosen"));
}

#[test]
fn choices_become_opaque_grants_and_copy_runs_through_the_ordinary_kernel() {
    let directory = TestDirectory::new();
    let source = directory.path("private-source.txt");
    let destination = directory.path("private-destination.txt");
    std::fs::write(&source, b"native provider copy").unwrap();
    let mut task = NativeFileTask::new(Some(provider([
        Ok(Some(source.clone())),
        Ok(Some(destination.clone())),
    ])));
    task.choose_source().unwrap();
    task.choose_destination(DestinationPolicy::Create).unwrap();
    task.plan().unwrap();
    let prepared = task.prepared.as_ref().unwrap();
    let semantic = format!("{:?}{:?}", prepared.form, prepared.plan);
    assert!(!semantic.contains(source.to_string_lossy().as_ref()));
    assert!(!semantic.contains(destination.to_string_lossy().as_ref()));
    task.run().unwrap();
    wait(&mut task);
    assert_eq!(
        std::fs::read(&destination).unwrap(),
        b"native provider copy"
    );
    let lines = task.lines().join("\n");
    assert!(lines.contains("capability-advertised=true"));
    assert!(lines.contains("FILE-PLAN checked="));
    assert!(lines.contains("FILE-RUN request=patchbay/file-copy/0 plan="));
    assert!(lines.contains("FILE-RECEIPT request=patchbay/file-copy/0 play="));
    assert!(lines.contains("source=patchbay-native/file/source"));
    assert!(lines.contains("destination=patchbay-native/file/destination"));
    assert!(lines.contains("result=Success"));
    assert!(!lines.contains(source.to_string_lossy().as_ref()));
    assert!(!lines.contains(destination.to_string_lossy().as_ref()));
}

#[test]
fn cancelled_dialog_and_destination_conflict_cannot_manufacture_success() {
    let directory = TestDirectory::new();
    let source = directory.path("source.txt");
    let destination = directory.path("destination.txt");
    std::fs::write(&source, b"new").unwrap();
    std::fs::write(&destination, b"old").unwrap();
    let mut cancelled = NativeFileTask::new(Some(provider([Ok(None)])));
    assert_eq!(
        cancelled.choose_source().unwrap(),
        ChoiceDisposition::Cancelled
    );
    assert!(cancelled
        .lines()
        .join("\n")
        .contains("role=source disposition=Cancelled"));
    assert!(cancelled.plan().unwrap_err().contains("not been chosen"));

    let mut conflict = NativeFileTask::new(Some(provider([
        Ok(Some(source)),
        Ok(Some(destination.clone())),
    ])));
    conflict.choose_source().unwrap();
    conflict
        .choose_destination(DestinationPolicy::Create)
        .unwrap();
    conflict.plan().unwrap();
    conflict.run().unwrap();
    wait(&mut conflict);
    assert!(conflict.lines().join("\n").contains("DestinationExists"));
    assert_eq!(std::fs::read(destination).unwrap(), b"old");
}
