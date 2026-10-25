//! Media input custody and finite reads, without manufacturing membership.
use super::{read_exact_bounded, read_regular};
use std::{fs, io::Read, path::PathBuf};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "conduit-spore-read-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn a_growing_stream_stops_at_one_sentinel_byte() {
    let mut reader = std::io::repeat(1).take(1000);
    assert!(read_exact_bounded(&mut reader, 4, 8).is_err());
    assert_eq!(reader.limit(), 991);
    assert!(read_exact_bounded(&mut &b"abc"[..], 4, 8).is_err());
    assert!(read_exact_bounded(&mut &b"abc"[..], 3, u64::MAX).is_err());
}

#[test]
fn regular_files_obey_the_actual_open_descriptor_bound() {
    let directory = Directory::new();
    let path = directory.0.join("image");
    fs::write(&path, b"12345678").unwrap();
    assert_eq!(read_regular(&path, 8).unwrap(), b"12345678");
    assert!(read_regular(&path, 7).is_err());
    assert!(read_regular(&directory.0, 8).is_err());
}

#[test]
fn symbolic_links_and_fifos_refuse_without_waiting_for_a_writer() {
    use std::os::unix::fs::symlink;
    let directory = Directory::new();
    let target = directory.0.join("target");
    fs::write(&target, b"image").unwrap();
    let link = directory.0.join("link");
    symlink(&target, &link).unwrap();
    assert!(read_regular(&link, 8).is_err());
    let fifo = directory.0.join("fifo");
    let name = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
    // The fixture owns this fresh path; no writer is created.
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    assert!(read_regular(&fifo, 8).is_err());
}
