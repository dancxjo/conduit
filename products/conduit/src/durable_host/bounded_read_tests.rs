use super::*;
use std::io::Write;

struct Directory(std::path::PathBuf);
impl Directory {
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn fixture() -> (Directory, std::path::PathBuf) {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path =
        std::env::temp_dir().join(format!("conduit-bounded-read-{}-{id}", std::process::id()));
    fs::create_dir(&path).unwrap();
    let directory = Directory(path);
    let path = directory.path().join("state.json");
    fs::write(&path, b"old").unwrap();
    (directory, path)
}

#[test]
fn accepts_complete_files_at_and_below_the_limit() {
    let (_directory, path) = fixture();
    assert_eq!(read(&path, 3).unwrap(), b"old");
    assert_eq!(read(&path, 4).unwrap(), b"old");
    assert_eq!(read(&path, u64::MAX).unwrap(), b"old");
}

#[test]
fn rejects_empty_overlimit_and_zero_limit() {
    let (_directory, path) = fixture();
    assert!(read(&path, 2).unwrap_err().contains("finite file bound"));
    assert!(read(&path, 0).unwrap_err().contains("finite file bound"));
    fs::write(&path, b"").unwrap();
    assert!(read(&path, 3).unwrap_err().contains("finite file bound"));
}

#[test]
fn growth_after_inspection_is_refused_without_silent_truncation() {
    let (_directory, path) = fixture();
    let inspected = fs::metadata(&path).unwrap();
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(&[b'x'; 4096])
        .unwrap();
    assert!(read_inspected(&path, 3, &inspected)
        .unwrap_err()
        .contains("finite file bound"));
}

#[test]
fn replacement_after_inspection_is_checked_against_actual_extent() {
    let (_directory, path) = fixture();
    let inspected = fs::metadata(&path).unwrap();
    let replacement = path.with_extension("replacement");
    fs::write(&replacement, b"replacement").unwrap();
    fs::rename(&replacement, &path).unwrap();
    assert!(read_inspected(&path, 3, &inspected)
        .unwrap_err()
        .contains("finite file bound"));
    fs::write(&path, b"").unwrap();
    assert!(read_inspected(&path, 3, &inspected)
        .unwrap_err()
        .contains("finite file bound"));
    fs::write(&path, b"new").unwrap();
    assert_eq!(read_inspected(&path, 3, &inspected).unwrap(), b"new");
}

#[test]
fn preserves_inspect_read_and_nonfile_refusals() {
    let (directory, path) = fixture();
    assert!(read(directory.path(), 4096)
        .unwrap_err()
        .contains("finite file bound"));
    let inspected = fs::metadata(&path).unwrap();
    fs::remove_file(&path).unwrap();
    assert!(read_inspected(&path, 3, &inspected)
        .unwrap_err()
        .starts_with("read "));
    assert!(read(&path, 3).unwrap_err().starts_with("inspect "));
}

#[test]
fn actual_read_stops_after_one_overlimit_sentinel_byte() {
    let mut reader = std::io::Cursor::new([b'x'; 4096]);
    assert_eq!(read_limited(&mut reader, 3).unwrap(), b"xxxx");
    assert_eq!(reader.position(), 4);
}
