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
    sync::Arc,
};
pub struct PreparedNativeModel {
    child: Child,
    input: ChildStdin,
    output: ChildStdout,
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
            Prepared::Ready(v) => v,
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
        let child = guard.into_child();
        Ok(Self {
            child,
            input,
            output,
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
        let mut request = [0u8; 9];
        request[0] = 2;
        request[1..5].copy_from_slice(&self.next.to_le_bytes());
        self.input
            .write_all(&request)
            .and_then(|()| self.input.flush())
            .map_err(|_| NativeBackingRefusal::Io)?;
        let mut reply = [0u8; 8];
        let result = self.output.read_exact(&mut reply);
        self.failed = true;
        let status = self.child.wait().map_err(|_| NativeBackingRefusal::Io)?;
        use std::os::unix::process::ExitStatusExt;
        if result.is_err() && status.signal() == Some(libc::SIGSYS) {
            Err(NativeBackingRefusal::ExecutionBoundary)
        } else {
            Err(NativeBackingRefusal::Protocol)
        }
    }
    pub fn infer_f32(&mut self, value: f32) -> Result<f32, NativeBackingRefusal> {
        if self.failed {
            return Err(NativeBackingRefusal::ProviderLost);
        }
        if !value.is_finite() {
            return Err(NativeBackingRefusal::InvalidInput);
        }
        if self.next > self.maximum {
            return Err(NativeBackingRefusal::Capacity);
        }
        let mut request = [0u8; 9];
        request[0] = 1;
        request[1..5].copy_from_slice(&self.next.to_le_bytes());
        request[5..].copy_from_slice(&value.to_bits().to_le_bytes());
        let result = (|| {
            self.input
                .write_all(&request)
                .and_then(|()| self.input.flush())
                .map_err(|_| NativeBackingRefusal::Io)?;
            let mut reply = [0u8; 8];
            self.output
                .read_exact(&mut reply)
                .map_err(|_| NativeBackingRefusal::Io)?;
            if u32::from_le_bytes(reply[..4].try_into().unwrap()) != self.next {
                return Err(NativeBackingRefusal::Protocol);
            }
            let output = f32::from_bits(u32::from_le_bytes(reply[4..].try_into().unwrap()));
            // This exact reviewed checkpoint is Identity, not a general evaluator.
            if output.to_bits() != value.to_bits() {
                return Err(NativeBackingRefusal::InvalidOutput);
            }
            self.input
                .write_all(b"C")
                .and_then(|()| self.input.flush())
                .map_err(|_| NativeBackingRefusal::Io)?;
            Ok(output)
        })();
        match result {
            Ok(value) => {
                self.next += 1;
                Ok(value)
            }
            Err(error) => {
                self.failed = true;
                let _ = self.child.kill();
                let _ = self.child.wait();
                Err(classify(&mut self.child, error))
            }
        }
    }
}
impl Drop for PreparedNativeModel {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
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
