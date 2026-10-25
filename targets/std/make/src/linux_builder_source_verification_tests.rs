use super::*;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct SourceFixture {
    root: PathBuf,
    names: Vec<u8>,
}
impl SourceFixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "conduit-source-pass-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let mut names = Vec::new();
        for i in 0..513 {
            let name = format!("source-{i:04}");
            fs::write(root.join(&name), b"plot").unwrap();
            names.extend_from_slice(name.as_bytes());
            names.push(0);
        }
        Self { root, names }
    }
    fn bounds(&self) -> BuilderBounds {
        BuilderBounds {
            maximum_source_files: 1000,
            maximum_source_bytes: 4096,
            maximum_workspace_bytes: 8192,
            maximum_output_bytes: 4096,
            maximum_processes: 4,
            maximum_seconds: 60,
            maximum_evidence_bytes: 1024,
        }
    }
}
impl Drop for SourceFixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
#[test]
fn read_only_verification_checks_whole_workspace_once() {
    let fixture = SourceFixture::new();
    let bounds = fixture.bounds();
    let cancelled = AtomicBool::new(false);
    let execution = Execution::new(&fixture.root, None, &bounds, &cancelled);
    assert_eq!(
        hash_source_files(&fixture.root, &fixture.names, &bounds, &execution)
            .unwrap()
            .len(),
        64
    );
    assert_eq!(execution.whole_workspace_checks(), 1);
}
#[test]
fn read_only_verification_still_counts_unlisted_workspace_bytes() {
    let fixture = SourceFixture::new();
    let bounds = fixture.bounds();
    let cancelled = AtomicBool::new(false);
    fs::write(
        fixture.root.join("unlisted-workspace-output"),
        vec![0; 8192],
    )
    .unwrap();
    let execution = Execution::new(&fixture.root, None, &bounds, &cancelled);
    assert!(matches!(
        hash_source_files(&fixture.root, &fixture.names, &bounds, &execution),
        Err(BuilderRefusal::BoundExhausted)
    ));
    assert_eq!(execution.whole_workspace_checks(), 1);
}
#[test]
fn read_only_verification_checks_reserve_before_final_traversal() {
    let fixture = SourceFixture::new();
    let bounds = fixture.bounds();
    let cancelled = AtomicBool::new(false);
    let execution =
        Execution::new(&fixture.root, None, &bounds, &cancelled).minimum_free_bytes(u64::MAX);
    assert!(matches!(
        hash_source_files(&fixture.root, &fixture.names, &bounds, &execution),
        Err(BuilderRefusal::BoundExhausted)
    ));
    assert_eq!(execution.whole_workspace_checks(), 0);
}

#[test]
fn read_only_verification_keeps_per_file_source_byte_bounds() {
    let fixture = SourceFixture::new();
    let mut bounds = fixture.bounds();
    bounds.maximum_source_bytes = 1000;
    let cancelled = AtomicBool::new(false);
    let execution = Execution::new(&fixture.root, None, &bounds, &cancelled);
    assert!(matches!(
        hash_source_files(&fixture.root, &fixture.names, &bounds, &execution),
        Err(BuilderRefusal::SourceBoundExhausted)
    ));
    assert_eq!(execution.whole_workspace_checks(), 0);
}
