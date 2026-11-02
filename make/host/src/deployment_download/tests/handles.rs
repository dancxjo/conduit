use super::*;
use std::io::{self, Cursor};

#[test]
fn retained_unlinked_file_exports_exact_bytes_and_rewinds() {
    let bytes = b"retained complete seed including private provisioning";
    let (descriptor, artifact) = fixture(bytes);
    let root = root("handle");
    fs::create_dir_all(&root).unwrap();
    let path = root.join("retained");
    fs::write(&path, bytes).unwrap();
    let mut source = File::open(&path).unwrap();
    fs::remove_file(path).unwrap();
    source.seek(SeekFrom::Start(7)).unwrap();
    let destination = root.join("seed.zip");
    let receipt = download_body_bound_artifact_from(
        &descriptor,
        &artifact,
        &mut source,
        &destination,
        || false,
    )
    .unwrap();
    assert_eq!(fs::read(&destination).unwrap(), bytes);
    assert_eq!(source.stream_position().unwrap(), 0);
    assert_eq!(
        receipt.artifact_content_sha256,
        artifact.artifact_content_sha256
    );
    assert_ne!(
        receipt.artifact_content_sha256,
        artifact.image_content_sha256
    );
    assert!(!receipt.boot_observed);
    assert!(!receipt.join_observed);
    assert!(!receipt.membership_admitted);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(destination).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn changed_short_and_extra_streams_leave_no_output() {
    let (descriptor, artifact) = fixture(b"seed");
    let root = root("handle-content");
    fs::create_dir_all(&root).unwrap();
    for (bytes, expected) in [
        (b"Seed".as_slice(), ArtifactDownloadRefusal::ContentMismatch),
        (b"see".as_slice(), ArtifactDownloadRefusal::ExtentMismatch),
        (b"seeds".as_slice(), ArtifactDownloadRefusal::ExtentMismatch),
    ] {
        let destination = root.join("seed.zip");
        let mut source = Cursor::new(bytes);
        assert_eq!(
            download_body_bound_artifact_from(
                &descriptor,
                &artifact,
                &mut source,
                &destination,
                || false,
            ),
            Err(expected)
        );
        assert_eq!(source.position(), 0);
        assert!(!destination.exists());
        assert!(!temporary_path(&destination).unwrap().exists());
    }
    fs::remove_dir_all(root).unwrap();
}

struct Interrupted {
    bytes: Cursor<Vec<u8>>,
    fail: bool,
}
impl Read for Interrupted {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if self.fail {
            return Err(io::Error::other("fixture interrupted"));
        }
        self.fail = true;
        self.bytes.read(&mut output[..1])
    }
}
impl Seek for Interrupted {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.bytes.seek(position)
    }
}

#[test]
fn interruption_and_cancellation_clean_owned_partial_only() {
    let (descriptor, artifact) = fixture(b"seed");
    let root = root("handle-interruption");
    fs::create_dir_all(&root).unwrap();
    let destination = root.join("seed.zip");
    let mut source = Interrupted {
        bytes: Cursor::new(b"seed".to_vec()),
        fail: false,
    };
    assert_eq!(
        download_body_bound_artifact_from(
            &descriptor,
            &artifact,
            &mut source,
            &destination,
            || false,
        ),
        Err(ArtifactDownloadRefusal::SourceUnavailable)
    );
    assert_eq!(source.bytes.position(), 0);
    assert!(!temporary_path(&destination).unwrap().exists());
    let mut calls = 0;
    let mut source = Cursor::new(b"seed");
    assert_eq!(
        download_body_bound_artifact_from(
            &descriptor,
            &artifact,
            &mut source,
            &destination,
            || {
                calls += 1;
                calls == 3
            },
        ),
        Err(ArtifactDownloadRefusal::Cancelled)
    );
    assert_eq!(source.position(), 0);
    assert!(!destination.exists());
    let partial = temporary_path(&destination).unwrap();
    assert!(!partial.exists());
    fs::write(&partial, b"other invocation").unwrap();
    assert_eq!(
        download_body_bound_artifact_from(
            &descriptor,
            &artifact,
            &mut source,
            &destination,
            || false,
        ),
        Err(ArtifactDownloadRefusal::DestinationUnavailable)
    );
    assert_eq!(fs::read(partial).unwrap(), b"other invocation");
    fs::remove_dir_all(root).unwrap();
}

struct DestinationRace {
    bytes: Cursor<Vec<u8>>,
    destination: PathBuf,
}
impl Read for DestinationRace {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if !self.destination.exists() {
            fs::write(&self.destination, b"operator data")?;
        }
        self.bytes.read(output)
    }
}
impl Seek for DestinationRace {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.bytes.seek(position)
    }
}

#[test]
fn destination_created_during_copy_is_not_replaced() {
    let (descriptor, artifact) = fixture(b"seed");
    let root = root("handle-race");
    fs::create_dir_all(&root).unwrap();
    let destination = root.join("seed.zip");
    let mut source = DestinationRace {
        bytes: Cursor::new(b"seed".to_vec()),
        destination: destination.clone(),
    };
    assert_eq!(
        download_body_bound_artifact_from(
            &descriptor,
            &artifact,
            &mut source,
            &destination,
            || false,
        ),
        Err(ArtifactDownloadRefusal::DestinationExists)
    );
    assert_eq!(fs::read(&destination).unwrap(), b"operator data");
    assert!(!temporary_path(&destination).unwrap().exists());
    assert_eq!(source.bytes.position(), 0);
    fs::remove_dir_all(root).unwrap();
}
