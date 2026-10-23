//! Owns one prepared native reservation; replies become visible only after checks.
use super::{
    protocol::{self, Bootstrap, Prepared},
    worker::{file_digest, Snapshot},
    BackingLedger, NativeBackingGrant, NativeBackingRefusal, NativeCpuProfile,
};
use conduit_ai::imported_onnx::ImportedOnnxResource;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::Path,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
mod transport;
use transport::Exchange;
static NEXT_TRANSPORT: AtomicU64 = AtomicU64::new(1);

/// Opaque transport custody, subordinate to the admitted generic session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeRequestId {
    transport: u64,
    sequence: u32,
}

pub struct PreparedNativeModel {
    child: Child,
    input: Option<ChildStdin>,
    output: Option<ChildStdout>,
    transport: u64,
    exchange: Exchange,
    current: Option<NativeRequestId>,
    last: Option<NativeRequestId>,
    ledger: BackingLedger,
    resource: Arc<ImportedOnnxResource>,
    next: u32,
    maximum: u32,
    failed: bool,
}
impl PreparedNativeModel {
    pub fn prepare(
        executable: &Path,
        profile: NativeCpuProfile,
        grant: NativeBackingGrant,
        resource: Arc<ImportedOnnxResource>,
    ) -> Result<Self, NativeBackingRefusal> {
        profile.validate()?;
        grant.validate()?;
        if resource.resource().bytes().len() != 77
            || Sha256::digest(resource.resource().bytes()).as_slice()
                != super::profile::MODEL_SHA256
        {
            return Err(NativeBackingRefusal::IdentityMismatch);
        }
        if file_digest(Path::new(&profile.runtime_library))? != profile.runtime_sha256 {
            return Err(NativeBackingRefusal::IdentityMismatch);
        }
        let mut command = Command::new(executable);
        command
            .env_clear()
            .current_dir("/")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                let limit = libc::rlimit {
                    rlim_cur: 0,
                    rlim_max: 0,
                };
                if libc::setrlimit(libc::RLIMIT_CORE, &limit) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = command.spawn().map_err(|_| NativeBackingRefusal::Io)?;
        let maximum = u32::from(profile.maximum_calls);
        let mut guard = AdmissionChild { child };
        let mut input = guard
            .child
            .stdin
            .take()
            .ok_or(NativeBackingRefusal::Protocol)?;
        let mut output = guard
            .child
            .stdout
            .take()
            .ok_or(NativeBackingRefusal::Protocol)?;
        protocol::send(
            &mut input,
            &Bootstrap {
                profile,
                grant,
                graph: resource.resource().bytes().to_vec(),
            },
        )?;
        let ready: Prepared =
            protocol::receive(&mut output).map_err(|error| classify(&mut guard.child, error))?;
        let ledger = match ready {
            Prepared::Ready(v) => *v,
            Prepared::Refused(e) => return Err(e),
        };
        ledger.validate(grant)?;
        // The kernel's stopped-at-ready process map, not a declared byte count,
        // verifies the exact backing and domain spans before this object exists.
        let observed = Snapshot::capture(guard.child.id())?
            .ledger(ledger.fixed_io_span, ledger.execution_stack_span)?;
        if observed != ledger {
            return Err(NativeBackingRefusal::InvalidLedger);
        }
        input
            .write_all(b"G")
            .and_then(|()| input.flush())
            .map_err(|_| NativeBackingRefusal::Io)?;
        nonblocking(&input)?;
        nonblocking(&output)?;
        let transport = NEXT_TRANSPORT
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .map_err(|_| NativeBackingRefusal::Capacity)?;
        let child = guard.into_child();
        Ok(Self {
            child,
            input: Some(input),
            output: Some(output),
            transport,
            exchange: Exchange::new(),
            current: None,
            last: None,
            ledger,
            resource,
            next: 1,
            maximum,
            failed: false,
        })
    }
    pub fn ledger(&self) -> &BackingLedger {
        &self.ledger
    }
    pub fn verify_backing(&self) -> Result<(), NativeBackingRefusal> {
        if self.failed {
            return Err(NativeBackingRefusal::ProviderLost);
        }
        let observed = Snapshot::capture(self.child.id())?
            .ledger(self.ledger.fixed_io_span, self.ledger.execution_stack_span)?;
        if observed == self.ledger {
            Ok(())
        } else {
            Err(NativeBackingRefusal::InvalidLedger)
        }
    }
    pub fn resource(&self) -> &ImportedOnnxResource {
        &self.resource
    }
    pub(crate) fn probe_capacity(&mut self) -> Result<(), NativeBackingRefusal> {
        if self.failed {
            return Err(NativeBackingRefusal::ProviderLost);
        }
        if self.current.is_some() {
            return Err(NativeBackingRefusal::Capacity);
        }
        let mut request = [0u8; 9];
        request[0] = 2;
        request[1..5].copy_from_slice(&self.next.to_le_bytes());
        self.input
            .as_mut()
            .ok_or(NativeBackingRefusal::ProviderLost)?
            .write_all(&request)
            .map_err(|_| NativeBackingRefusal::Io)?;
        let mut reply = [0u8; 8];
        let result = self
            .output
            .as_mut()
            .ok_or(NativeBackingRefusal::ProviderLost)?
            .read_exact(&mut reply);
        self.failed = true;
        let status = self.child.wait().map_err(|_| NativeBackingRefusal::Io)?;
        use std::os::unix::process::ExitStatusExt;
        if result.is_err() && status.signal() == Some(libc::SIGSYS) {
            Err(NativeBackingRefusal::ExecutionBoundary)
        } else {
            Err(NativeBackingRefusal::Protocol)
        }
    }
    /// Reserve the sole in-flight slot; poll performs at most one write/read.
    pub fn begin_f32(&mut self, value: f32) -> Result<NativeRequestId, NativeBackingRefusal> {
        if self.failed {
            return Err(NativeBackingRefusal::ProviderLost);
        }
        if self.current.is_some() {
            return Err(NativeBackingRefusal::Capacity);
        }
        if self.next > self.maximum {
            return Err(NativeBackingRefusal::Capacity);
        }
        let id = NativeRequestId {
            transport: self.transport,
            sequence: self.next,
        };
        self.exchange.begin(self.next, value)?;
        self.current = Some(id);
        Ok(id)
    }
    /// Ready is not publication: the validated reply remains unacknowledged.
    pub fn poll_f32(&mut self, id: NativeRequestId) -> Result<bool, NativeBackingRefusal> {
        self.check(id)?;
        let result = self.exchange.poll(
            self.input
                .as_mut()
                .ok_or(NativeBackingRefusal::ProviderLost)?,
            self.output
                .as_mut()
                .ok_or(NativeBackingRefusal::ProviderLost)?,
        );
        self.checked(result)
    }
    /// Publish only after the exact current reply and its one-byte ACK complete.
    pub fn take_f32(&mut self, id: NativeRequestId) -> Result<Option<f32>, NativeBackingRefusal> {
        self.check(id)?;
        let result = self.exchange.take(
            self.input
                .as_mut()
                .ok_or(NativeBackingRefusal::ProviderLost)?,
        );
        let result = self.checked(result)?;
        if result.is_some() {
            self.current = None;
            self.last = Some(id);
            self.next += 1; // maximum is u16; cannot overflow u32.
        }
        Ok(result)
    }
    /// Revoke before closing pipes and killing/reaping. Repeating the same
    /// terminal token is harmless and cannot cancel a subsequent request.
    pub fn cancel(&mut self, id: NativeRequestId) -> Result<(), NativeBackingRefusal> {
        if self.last == Some(id) {
            return Ok(());
        }
        self.check(id)?;
        self.unload()
    }
    /// Idempotent terminal retirement; this transport is never silently renewed.
    pub fn unload(&mut self) -> Result<(), NativeBackingRefusal> {
        self.failed = true;
        self.exchange.revoke();
        if let Some(id) = self.current.take() {
            self.last = Some(id);
        }
        drop(self.input.take());
        drop(self.output.take());
        let _ = self.child.kill();
        self.child
            .wait()
            .map(|_| ())
            .map_err(|_| NativeBackingRefusal::Io)
    }
    fn check(&self, id: NativeRequestId) -> Result<(), NativeBackingRefusal> {
        if self.current != Some(id) {
            return Err(NativeBackingRefusal::Protocol);
        }
        if self.failed {
            return Err(NativeBackingRefusal::ProviderLost);
        }
        Ok(())
    }
    fn checked<T>(
        &mut self,
        result: Result<T, NativeBackingRefusal>,
    ) -> Result<T, NativeBackingRefusal> {
        result.map_err(|error| {
            let error = classify(&mut self.child, error);
            let _ = self.unload();
            error
        })
    }
    /// Compatibility convenience only. Async Play must use begin/poll/take so
    /// its existing scheduler can process cancellation while the child runs.
    pub fn infer_f32(&mut self, value: f32) -> Result<f32, NativeBackingRefusal> {
        let id = self.begin_f32(value)?;
        loop {
            if self.poll_f32(id)? {
                if let Some(value) = self.take_f32(id)? {
                    return Ok(value);
                }
            }
            if let Err(error) = self.wait_io() {
                return self.checked(Err(error));
            }
        }
    }
    fn wait_io(&self) -> Result<(), NativeBackingRefusal> {
        use std::os::fd::AsRawFd;
        let fd = if self.exchange.wants_write() {
            libc::pollfd {
                fd: self
                    .input
                    .as_ref()
                    .ok_or(NativeBackingRefusal::ProviderLost)?
                    .as_raw_fd(),
                events: libc::POLLOUT,
                revents: 0,
            }
        } else {
            libc::pollfd {
                fd: self
                    .output
                    .as_ref()
                    .ok_or(NativeBackingRefusal::ProviderLost)?
                    .as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            }
        };
        let mut fd = fd;
        // Compatibility wait has no scheduler authority or implicit timeout.
        let result = unsafe { libc::poll(&mut fd, 1, -1) };
        if result < 0 && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
            Err(NativeBackingRefusal::Io)
        } else {
            Ok(())
        }
    }
}
impl Drop for PreparedNativeModel {
    fn drop(&mut self) {
        let _ = self.unload();
    }
}
struct AdmissionChild {
    child: Child,
}
impl AdmissionChild {
    fn into_child(self) -> Child {
        let this = std::mem::ManuallyDrop::new(self);
        // Move ownership without dropping the armed preparation guard.
        unsafe { std::ptr::read(&this.child) }
    }
}
impl Drop for AdmissionChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn classify(child: &mut Child, fallback: NativeBackingRefusal) -> NativeBackingRefusal {
    if !matches!(fallback, NativeBackingRefusal::Io) {
        return fallback;
    }
    use std::os::unix::process::ExitStatusExt;
    match child.try_wait() {
        Ok(Some(status)) if status.signal() == Some(libc::SIGSYS) => {
            NativeBackingRefusal::ExecutionBoundary
        }
        Ok(Some(_)) => NativeBackingRefusal::ProviderLost,
        _ => fallback,
    }
}

fn nonblocking(fd: &impl std::os::fd::AsRawFd) -> Result<(), NativeBackingRefusal> {
    let fd = fd.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        Err(NativeBackingRefusal::Io)
    } else {
        Ok(())
    }
}
