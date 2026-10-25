//! Cooperative monitored execution of trusted package tooling. Monitoring detects
//! pressure after an observation; it is not a hard OS process/disk quota. Descendants
//! that deliberately escape their process group are outside this execution model.

use conduit_host_make::{BuilderBounds, BuilderRefusal};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    os::unix::{fs::OpenOptionsExt, process::CommandExt},
    path::Path,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

pub(super) struct Captured {
    pub stdout: Vec<u8>,
}
#[derive(Clone)]
pub(super) struct ExecutionEvidence {
    stdout_bytes: u64,
    stderr_bytes: u64,
    hash: Sha256,
    process_sequence: u64,
}

pub(super) struct Execution<'a> {
    workspace: &'a Path,
    output: Option<&'a Path>,
    bounds: &'a BuilderBounds,
    cancelled: &'a AtomicBool,
    started: Instant,
    stdout_bytes: u64,
    stderr_bytes: u64,
    evidence: Sha256,
    process_sequence: u64,
}
impl<'a> Execution<'a> {
    pub fn new(
        workspace: &'a Path,
        output: Option<&'a Path>,
        bounds: &'a BuilderBounds,
        cancelled: &'a AtomicBool,
    ) -> Self {
        Self {
            workspace,
            output,
            bounds,
            cancelled,
            started: Instant::now(),
            stdout_bytes: 0,
            stderr_bytes: 0,
            evidence: Sha256::new(),
            process_sequence: 0,
        }
    }
    pub fn started_at(mut self, started: Instant) -> Self {
        self.started = started;
        self
    }
    pub fn check_progress(&self) -> Result<(), BuilderRefusal> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(BuilderRefusal::Interrupted);
        }
        if self.started.elapsed() >= Duration::from_secs(self.bounds.maximum_seconds) {
            return Err(BuilderRefusal::BoundExhausted);
        }
        Ok(())
    }
    pub fn check_bounds(&self) -> Result<(), BuilderRefusal> {
        self.check_progress()?;
        if directory_bytes(self.workspace, self.bounds.maximum_workspace_bytes)?
            > self.bounds.maximum_workspace_bytes
            || self.output.is_some_and(|path| {
                directory_bytes(path, self.bounds.maximum_output_bytes)
                    .map_or(true, |n| n > self.bounds.maximum_output_bytes)
            })
        {
            return Err(BuilderRefusal::BoundExhausted);
        }
        Ok(())
    }
    pub fn run(&mut self, command: &mut Command) -> Result<Captured, BuilderRefusal> {
        self.check_bounds()?;
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("RUSTUP_AUTO_INSTALL", "0");
        self.process_sequence = self
            .process_sequence
            .checked_add(1)
            .ok_or(BuilderRefusal::BoundExhausted)?;
        // Fixed names within the admitted workspace; no caller-selected sink or overwrite.
        // create_new also refuses a pre-existing symlink. Raw bytes are never env dumps.
        let diagnostic = |stream: &str| {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(self.workspace.join(format!(
                    "execution-{:06}-{stream}.log",
                    self.process_sequence
                )))
                .map_err(|_| BuilderRefusal::WorkspaceUnavailable)
        };
        let stdout_log = diagnostic("stdout")?;
        let stderr_log = diagnostic("stderr")?;
        let mut child = command.spawn().map_err(|_| BuilderRefusal::BuildFailed)?;
        let group = child.id() as i32;
        let remaining = self
            .bounds
            .maximum_evidence_bytes
            .saturating_sub(self.stdout_bytes + self.stderr_bytes);
        let observed = Arc::new(AtomicU64::new(0));
        let stdout = read_pipe(
            child.stdout.take().unwrap(),
            stdout_log,
            observed.clone(),
            remaining,
        );
        let stderr = read_pipe(
            child.stderr.take().unwrap(),
            stderr_log,
            observed.clone(),
            remaining,
        );
        let mut next_disk_check = Instant::now();
        let outcome = loop {
            let refusal = if self.cancelled.load(Ordering::Acquire) {
                Some(BuilderRefusal::Interrupted)
            } else if self.started.elapsed() >= Duration::from_secs(self.bounds.maximum_seconds)
                || observed.load(Ordering::Acquire) > remaining
                || process_group_size(group).unwrap_or(u64::MAX)
                    > u64::from(self.bounds.maximum_processes)
            {
                Some(BuilderRefusal::BoundExhausted)
            } else if Instant::now() >= next_disk_check {
                next_disk_check = Instant::now() + Duration::from_millis(500);
                self.check_bounds().err()
            } else {
                None
            };
            if let Some(error) = refusal {
                break Err(error);
            }
            match child.try_wait() {
                Ok(Some(status)) => {
                    break if status.success() {
                        Ok(())
                    } else {
                        Err(BuilderRefusal::BuildFailed)
                    }
                }
                Ok(None) => thread::sleep(Duration::from_millis(25)),
                Err(_) => break Err(BuilderRefusal::BuildFailed),
            }
        };
        // Always reap the command and terminate remaining group members, including
        // a daemon retaining stdout after its parent exits. No detached reader wait.
        // SAFETY: the group was assigned to this child's PID by process_group(0).
        unsafe {
            libc::kill(-group, libc::SIGKILL);
        }
        let _ = child.wait();
        let stdout = stdout.join().map_err(|_| BuilderRefusal::BuildFailed)??;
        let stderr = stderr.join().map_err(|_| BuilderRefusal::BuildFailed)??;
        self.stdout_bytes = self.stdout_bytes.saturating_add(stdout.1);
        self.stderr_bytes = self.stderr_bytes.saturating_add(stderr.1);
        self.evidence.update(b"stdout\0");
        self.evidence.update(stdout.1.to_le_bytes());
        self.evidence.update(&stdout.0);
        self.evidence.update(b"stderr\0");
        self.evidence.update(stderr.1.to_le_bytes());
        self.evidence.update(&stderr.0);
        outcome?;
        if self.stdout_bytes + self.stderr_bytes > self.bounds.maximum_evidence_bytes {
            return Err(BuilderRefusal::BoundExhausted);
        }
        self.check_bounds()?;
        Ok(Captured { stdout: stdout.0 })
    }
    pub fn saved_evidence(&self) -> ExecutionEvidence {
        ExecutionEvidence {
            stdout_bytes: self.stdout_bytes,
            stderr_bytes: self.stderr_bytes,
            hash: self.evidence.clone(),
            process_sequence: self.process_sequence,
        }
    }
    pub fn inherit_evidence(mut self, evidence: &ExecutionEvidence) -> Self {
        self.stdout_bytes = evidence.stdout_bytes;
        self.stderr_bytes = evidence.stderr_bytes;
        self.evidence = evidence.hash.clone();
        self.process_sequence = evidence.process_sequence;
        self
    }
    pub fn evidence(&self) -> (u64, u64, String) {
        (
            self.stdout_bytes,
            self.stderr_bytes,
            format!("sha256:{:x}", self.evidence.clone().finalize()),
        )
    }
}
fn read_pipe(
    mut pipe: impl Read + Send + 'static,
    mut diagnostic: fs::File,
    total: Arc<AtomicU64>,
    limit: u64,
) -> thread::JoinHandle<Result<(Vec<u8>, u64), BuilderRefusal>> {
    thread::spawn(move || {
        let mut captured = Vec::new();
        let mut bytes = 0_u64;
        let mut buffer = [0_u8; 8192];
        loop {
            let n = pipe
                .read(&mut buffer)
                .map_err(|_| BuilderRefusal::BuildFailed)?;
            if n == 0 {
                break;
            }
            bytes = bytes.saturating_add(n as u64);
            let offset = total.fetch_add(n as u64, Ordering::AcqRel);
            let allowed = limit.saturating_sub(offset).min(n as u64) as usize;
            diagnostic
                .write_all(&buffer[..allowed])
                .map_err(|_| BuilderRefusal::WorkspaceUnavailable)?;
            captured.extend_from_slice(&buffer[..allowed]);
        }
        diagnostic
            .sync_all()
            .map_err(|_| BuilderRefusal::WorkspaceUnavailable)?;
        Ok((captured, bytes))
    })
}
fn directory_bytes(root: &Path, limit: u64) -> Result<u64, BuilderRefusal> {
    directory_bytes_observing(root, limit, &mut |_| {})
}
fn directory_bytes_observing(
    root: &Path,
    limit: u64,
    observed: &mut impl FnMut(&Path),
) -> Result<u64, BuilderRefusal> {
    let mut pending = vec![root.to_owned()];
    let mut bytes = 0_u64;
    let mut entries = 0_u64;
    while let Some(directory) = pending.pop() {
        let children = match fs::read_dir(&directory) {
            Ok(children) => children,
            // Cargo atomically renames/removes temporary descendants while compiling.
            // A disappeared descendant contributes zero at this observation; losing
            // the admitted root or any other I/O failure still refuses accounting.
            Err(e) if directory != root && e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(BuilderRefusal::WorkspaceUnavailable),
        };
        for entry in children {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(_) => return Err(BuilderRefusal::WorkspaceUnavailable),
            };
            entries += 1;
            if entries > 2_000_000 {
                return Err(BuilderRefusal::BoundExhausted);
            }
            observed(&entry.path());
            let metadata = match entry.path().symlink_metadata() {
                Ok(metadata) => metadata,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(_) => return Err(BuilderRefusal::WorkspaceUnavailable),
            };
            // Do not follow symlinks out of the admitted workspace during accounting.
            if metadata.is_dir() {
                pending.push(entry.path());
            } else {
                bytes = bytes.saturating_add(metadata.len());
            }
            if bytes > limit {
                return Ok(bytes);
            }
        }
    }
    Ok(bytes)
}
fn process_group_size(group: i32) -> Result<u64, BuilderRefusal> {
    let mut count = 0;
    for entry in fs::read_dir("/proc").map_err(|_| BuilderRefusal::BuildFailed)? {
        let Ok(entry) = entry else {
            continue;
        };
        if entry.file_name().to_string_lossy().parse::<u32>().is_err() {
            continue;
        }
        let Ok(stat) = fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        let Some((_, fields)) = stat.rsplit_once(") ") else {
            continue;
        };
        if fields
            .split_whitespace()
            .nth(2)
            .and_then(|s| s.parse::<i32>().ok())
            == Some(group)
        {
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bounds() -> BuilderBounds {
        BuilderBounds {
            maximum_source_files: 10,
            maximum_source_bytes: 1024,
            maximum_workspace_bytes: 1024 * 1024,
            maximum_output_bytes: 1024,
            maximum_processes: 16,
            maximum_seconds: 2,
            maximum_evidence_bytes: 128,
        }
    }
    #[test]
    fn output_pressure_and_cancellation_are_distinct() {
        let root =
            std::env::temp_dir().join(format!("conduit-linux-process-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let bounds = bounds();
        let cancelled = AtomicBool::new(false);
        let mut execution = Execution::new(&root, None, &bounds, &cancelled);
        let mut command = Command::new("printf");
        command.arg("x".repeat(256));
        assert!(matches!(
            execution.run(&mut command),
            Err(BuilderRefusal::BoundExhausted)
        ));
        assert_eq!(
            fs::read(root.join("execution-000001-stdout.log")).unwrap(),
            vec![b'x'; 128]
        );
        assert_eq!(
            fs::metadata(root.join("execution-000001-stderr.log"))
                .unwrap()
                .len(),
            0
        );
        cancelled.store(true, Ordering::Release);
        assert!(matches!(
            execution.run(&mut Command::new("true")),
            Err(BuilderRefusal::Interrupted)
        ));
        fs::remove_dir_all(&root).unwrap();
    }
    #[test]
    fn cancellation_terminates_a_running_command() {
        let root =
            std::env::temp_dir().join(format!("conduit-linux-cancel-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let bounds = bounds();
        let cancelled = Arc::new(AtomicBool::new(false));
        let trigger = cancelled.clone();
        let cancellation = thread::spawn(move || {
            thread::sleep(Duration::from_millis(75));
            trigger.store(true, Ordering::Release);
        });
        let mut execution = Execution::new(&root, None, &bounds, &cancelled);
        let mut command = Command::new("sleep");
        command.arg("30");
        let started = Instant::now();
        assert!(matches!(
            execution.run(&mut command),
            Err(BuilderRefusal::Interrupted)
        ));
        assert!(started.elapsed() < Duration::from_secs(2));
        cancellation.join().unwrap();
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn failing_command_retains_exact_stderr_without_overwrite() {
        let root =
            std::env::temp_dir().join(format!("conduit-linux-stderr-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let bounds = bounds();
        let cancelled = AtomicBool::new(false);
        let mut execution = Execution::new(&root, None, &bounds, &cancelled);
        let mut command = Command::new("sh");
        command.args(["-c", "printf reviewed-build-error >&2; exit 7"]);
        assert!(matches!(
            execution.run(&mut command),
            Err(BuilderRefusal::BuildFailed)
        ));
        let diagnostic = root.join("execution-000001-stderr.log");
        assert_eq!(fs::read(&diagnostic).unwrap(), b"reviewed-build-error");
        // A new execution cannot silently replace retained proof from an earlier one.
        let mut duplicate = Execution::new(&root, None, &bounds, &cancelled);
        assert!(matches!(
            duplicate.run(&mut Command::new("true")),
            Err(BuilderRefusal::WorkspaceUnavailable)
        ));
        assert_eq!(fs::read(diagnostic).unwrap(), b"reviewed-build-error");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn accounting_tolerates_disappearing_children_but_preserves_real_errors() {
        let root =
            std::env::temp_dir().join(format!("conduit-linux-accounting-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("cargo-temporary"), b"temporary").unwrap();
        let mut removed = false;
        let bytes = directory_bytes_observing(&root, 1024, &mut |path| {
            // Deterministic actual disappearance between readdir and metadata.
            fs::remove_file(path).unwrap();
            removed = true;
        })
        .unwrap();
        assert!(removed);
        assert_eq!(bytes, 0);
        let file = root.join("not-a-directory");
        fs::write(&file, b"retained").unwrap();
        assert_eq!(
            directory_bytes(&file, 1024),
            Err(BuilderRefusal::WorkspaceUnavailable)
        );
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(
            directory_bytes(&root, 1024),
            Err(BuilderRefusal::WorkspaceUnavailable)
        );
    }
    #[test]
    fn process_group_stat_parser_counts_this_process() {
        // SAFETY: getpgrp has no preconditions and does not mutate process state.
        assert!(process_group_size(unsafe { libc::getpgrp() }).unwrap() >= 1);
    }
}
