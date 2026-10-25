//! Source authority and cooperative cancellation for the complete Linux operation.
use super::process::Execution;
use conduit_host_make::{BuilderBounds, BuilderRefusal, BuilderSource, SealedBuilderSource};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    os::unix::fs::OpenOptionsExt,
    path::{Component, Path, PathBuf},
    process::Command,
};
pub(super) fn acquire(
    selected: &BuilderSource,
    workspace: &Path,
    bounds: &BuilderBounds,
    local: &[PathBuf],
    origins: &[String],
    execution: &mut Execution<'_>,
) -> Result<SealedBuilderSource, BuilderRefusal> {
    execution.check_progress()?;
    let (root, origin, expected) = match selected {
        BuilderSource::CurrentCheckout { root } | BuilderSource::LocalCheckout { root } => {
            let root = root
                .canonicalize()
                .map_err(|_| BuilderRefusal::MissingSource)?;
            if !local.iter().any(|allowed| {
                allowed
                    .canonicalize()
                    .is_ok_and(|allowed| root.starts_with(allowed))
            }) {
                return Err(BuilderRefusal::UnauthorizedSource);
            }
            (root, None, None)
        }
        BuilderSource::RemoteGit {
            https_url,
            revision,
        } => {
            if !exact_revision(revision)
                || !origins.iter().any(|origin| {
                    https_url.starts_with("https://")
                        && (https_url == origin
                            || https_url
                                .strip_prefix(origin)
                                .is_some_and(|suffix| suffix.starts_with('/')))
                })
            {
                return Err(BuilderRefusal::UnauthorizedNetwork);
            }
            let root = workspace.join("source");
            if root.exists() {
                return Err(BuilderRefusal::WorkspaceUnavailable);
            }
            git(workspace, &["init", "--quiet", "source"], execution)?;
            git(&root, &["remote", "add", "origin", https_url], execution)?;
            git(
                &root,
                &["fetch", "--quiet", "--depth", "1", "origin", revision],
                execution,
            )?;
            git(
                &root,
                &["checkout", "--quiet", "--detach", "FETCH_HEAD"],
                execution,
            )?;
            (root, Some(https_url.clone()), Some(revision))
        }
    };
    let revision = String::from_utf8(git(&root, &["rev-parse", "HEAD"], execution)?)
        .map_err(|_| BuilderRefusal::MalformedArtifact)?
        .trim()
        .to_owned();
    if !exact_revision(&revision) {
        return Err(BuilderRefusal::MalformedArtifact);
    }
    if expected.is_some_and(|expected| expected != &revision) {
        return Err(BuilderRefusal::StaleRevision);
    }
    let dirty = !git(
        &root,
        &["status", "--porcelain=v1", "--untracked-files=all"],
        execution,
    )?
    .is_empty();
    let names = git(
        &root,
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ],
        execution,
    )?;
    let (files, bytes, content_sha256) =
        seal_files(&root, &names, bounds, &mut || execution.check_progress())?;
    Ok(SealedBuilderSource {
        source_identity: format!("git:{revision}+{content_sha256}"),
        revision,
        content_sha256,
        root,
        files,
        bytes,
        dirty,
        remote_origin: origin,
    })
}
fn exact_revision(revision: &str) -> bool {
    matches!(revision.len(), 40 | 64) && revision.bytes().all(|b| b.is_ascii_hexdigit())
}
fn git(
    root: &Path,
    args: &[&str],
    execution: &mut Execution<'_>,
) -> Result<Vec<u8>, BuilderRefusal> {
    let mut command = Command::new("git");
    command
        .current_dir(root)
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin");
    Ok(execution.run(&mut command)?.stdout)
}
fn seal_files(
    root: &Path,
    names: &[u8],
    bounds: &BuilderBounds,
    check: &mut impl FnMut() -> Result<(), BuilderRefusal>,
) -> Result<(u32, u64, String), BuilderRefusal> {
    let mut files = 0u32;
    let mut bytes = 0u64;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    for raw in names.split(|b| *b == 0).filter(|name| !name.is_empty()) {
        check()?;
        let relative =
            Path::new(std::str::from_utf8(raw).map_err(|_| BuilderRefusal::MalformedArtifact)?);
        let mut path = root.to_owned();
        let mut count = 0;
        for component in relative.components() {
            let Component::Normal(component) = component else {
                return Err(BuilderRefusal::MalformedArtifact);
            };
            path.push(component);
            count += 1;
            if fs::symlink_metadata(&path)
                .map_err(|_| BuilderRefusal::MissingSource)?
                .file_type()
                .is_symlink()
            {
                return Err(BuilderRefusal::MalformedArtifact);
            }
        }
        if count == 0 {
            return Err(BuilderRefusal::MalformedArtifact);
        }
        let metadata = fs::symlink_metadata(&path).map_err(|_| BuilderRefusal::MissingSource)?;
        if !metadata.is_file() {
            return Err(BuilderRefusal::MalformedArtifact);
        }
        files = files
            .checked_add(1)
            .ok_or(BuilderRefusal::SourceBoundExhausted)?;
        bytes = bytes
            .checked_add(metadata.len())
            .ok_or(BuilderRefusal::SourceBoundExhausted)?;
        if files > bounds.maximum_source_files || bytes > bounds.maximum_source_bytes {
            return Err(BuilderRefusal::SourceBoundExhausted);
        }
        // Nonblocking rejects a raced FIFO without hanging; no-follow rejects raced leaf links.
        let mut file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&path)
            .map_err(|_| BuilderRefusal::MissingSource)?;
        let opened = file.metadata().map_err(|_| BuilderRefusal::MissingSource)?;
        if !opened.is_file() || opened.len() != metadata.len() {
            return Err(BuilderRefusal::MalformedArtifact);
        }
        hash.update((raw.len() as u64).to_le_bytes());
        hash.update(raw);
        hash.update(metadata.len().to_le_bytes());
        let mut read = 0u64;
        loop {
            check()?;
            let n = file
                .read(&mut buffer)
                .map_err(|_| BuilderRefusal::MissingSource)?;
            if n == 0 {
                break;
            }
            read = read
                .checked_add(n as u64)
                .ok_or(BuilderRefusal::SourceBoundExhausted)?;
            if read > metadata.len() {
                return Err(BuilderRefusal::SourceBoundExhausted);
            }
            hash.update(&buffer[..n]);
        }
        if read != metadata.len() {
            return Err(BuilderRefusal::MalformedArtifact);
        }
    }
    Ok((files, bytes, format!("sha256:{:x}", hash.finalize())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::unix::fs::symlink,
        sync::atomic::{AtomicBool, AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Root(PathBuf);
    impl Root {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "conduit-controlled-source-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Root {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn bounds() -> BuilderBounds {
        BuilderBounds {
            maximum_source_files: 10,
            maximum_source_bytes: 1024 * 1024,
            maximum_workspace_bytes: 2 * 1024 * 1024,
            maximum_output_bytes: 1024,
            maximum_processes: 16,
            maximum_seconds: 5,
            maximum_evidence_bytes: 4096,
        }
    }
    #[test]
    fn streamed_seal_preserves_exact_identity_and_checks_cancellation_between_chunks() {
        let root = Root::new();
        let content = vec![42; 192 * 1024];
        fs::write(root.0.join("file"), &content).unwrap();
        let mut expected = Sha256::new();
        expected.update(4u64.to_le_bytes());
        expected.update(b"file");
        expected.update((content.len() as u64).to_le_bytes());
        expected.update(&content);
        let (_, bytes, digest) = seal_files(&root.0, b"file\0", &bounds(), &mut || Ok(())).unwrap();
        assert_eq!(bytes, content.len() as u64);
        assert_eq!(digest, format!("sha256:{:x}", expected.finalize()));
        let mut checks = 0;
        assert_eq!(
            seal_files(&root.0, b"file\0", &bounds(), &mut || {
                checks += 1;
                if checks == 4 {
                    Err(BuilderRefusal::Interrupted)
                } else {
                    Ok(())
                }
            }),
            Err(BuilderRefusal::Interrupted)
        );
        assert_eq!(checks, 4);
        let mut small = bounds();
        small.maximum_source_bytes = 1;
        assert_eq!(
            seal_files(&root.0, b"file\0", &small, &mut || Ok(())),
            Err(BuilderRefusal::SourceBoundExhausted)
        );
    }
    #[test]
    fn source_paths_refuse_symlink_parents_special_files_and_traversal() {
        let root = Root::new();
        fs::create_dir(root.0.join("real")).unwrap();
        fs::write(root.0.join("real/file"), b"x").unwrap();
        symlink("real", root.0.join("linked")).unwrap();
        symlink("real/file", root.0.join("leaf")).unwrap();
        let fifo = root.0.join("fifo");
        let encoded = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
        // SAFETY: encoded is a valid terminated pathname in this private fixture.
        assert_eq!(unsafe { libc::mkfifo(encoded.as_ptr(), 0o600) }, 0);
        for name in [
            b"linked/file\0".as_slice(),
            b"leaf\0",
            b"real\0",
            b"../escape\0",
            b"fifo\0",
        ] {
            assert_eq!(
                seal_files(&root.0, name, &bounds(), &mut || Ok(())),
                Err(BuilderRefusal::MalformedArtifact)
            );
        }
    }
    #[test]
    fn precancelled_acquisition_has_no_process_or_source_effects() {
        let root = Root::new();
        let cancelled = AtomicBool::new(true);
        let limits = bounds();
        let mut execution = Execution::new(&root.0, None, &limits, &cancelled);
        let source = BuilderSource::RemoteGit {
            https_url: "https://example.invalid/repo".into(),
            revision: "a".repeat(40),
        };
        assert_eq!(
            acquire(
                &source,
                &root.0,
                &limits,
                &[],
                &["https://example.invalid".into()],
                &mut execution
            ),
            Err(BuilderRefusal::Interrupted)
        );
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
        cancelled.store(false, Ordering::Release);
        let denied = BuilderSource::LocalCheckout {
            root: root.0.clone(),
        };
        assert_eq!(
            acquire(&denied, &root.0, &limits, &[], &[], &mut execution),
            Err(BuilderRefusal::UnauthorizedSource)
        );
    }
}
