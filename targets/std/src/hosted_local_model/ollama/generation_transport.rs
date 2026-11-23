//! Cancellation of one already-admitted asynchronous GenerateFlow effect.
//!
//! The existing process owner polls at 2 ms and allows 100 ms for pipe retirement
//! after killing its process group. The selected Unix curl profile is qualified
//! with a two-second cancellation-to-reaped-worker test ceiling. This is a
//! measured hosted profile, not a hard real-time guarantee against OS failure,
//! uninterruptible kernel work, hostile escaped descendants, or continued remote
//! inference. Closing curl does not unload the shared Ollama server's model.
//!
//! Inventory, initialization, synchronous model operations and text streaming
//! retain their existing transports. One worker admits the existing 16 MiB raw
//! response ceiling plus 4096 diagnostic bytes, separately from model weights.
//! Exceeding the wire ceiling fails instead of parsing a truncated body. The
//! ordinary process entrance retains its smaller capture policy.
use super::LocalModelAdapterTerminal;
use crate::hosted_process::{
    run_process_with_input_and_output_bounds, ProcessReport, ProcessRequest, ProcessTerminal,
};
use std::{
    ffi::OsString,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

const MAXIMUM_REQUEST_BYTES: usize = 8 * 1024 * 1024;
const MAXIMUM_RESPONSE_BYTES: usize = super::MAXIMUM_INVENTORY_BYTES;
const MAXIMUM_DIAGNOSTIC_BYTES: usize = 4_096;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Clone)]
pub(super) struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub(super) fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    pub(super) fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub(super) fn reset(&self) {
        self.0.store(false, Ordering::Release);
    }

    fn requested(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

pub(super) struct Transport {
    executable: PathBuf,
    cancellation: Cancellation,
    timeout: Duration,
}

impl Transport {
    pub(super) fn prepare() -> Result<Self, String> {
        let executable = std::env::var_os("PATH")
            .into_iter()
            .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
            .map(|directory| directory.join("curl"))
            .find(|candidate| executable(candidate))
            .and_then(|candidate| candidate.canonicalize().ok())
            .ok_or("prepare generation transport: curl executable is unavailable")?;
        Ok(Self {
            executable,
            cancellation: Cancellation::new(),
            timeout: REQUEST_TIMEOUT,
        })
    }

    pub(super) fn cancellation(&self) -> Cancellation {
        self.cancellation.clone()
    }

    pub(super) fn generate(
        &self,
        endpoint: &str,
        body: &[u8],
    ) -> Result<Vec<u8>, LocalModelAdapterTerminal> {
        let arguments = [
            "--silent".into(),
            "--show-error".into(),
            "--fail".into(),
            "--max-time".into(),
            super::REQUEST_TIMEOUT_SECONDS.into(),
            "--header".into(),
            "content-type: application/json".into(),
            "--data-binary".into(),
            "@-".into(),
            format!("{endpoint}/api/generate").into(),
        ];
        let environment = [(OsString::from("LC_ALL"), OsString::from("C"))];
        let request = ProcessRequest {
            program: &self.executable,
            arguments: &arguments,
            environment: &environment,
            stdin: body,
            maximum_stdout_bytes: MAXIMUM_RESPONSE_BYTES,
            maximum_stderr_bytes: MAXIMUM_DIAGNOSTIC_BYTES,
            timeout: self.timeout,
            require_process_group: true,
        };
        let report = run_process_with_input_and_output_bounds(
            &request,
            MAXIMUM_REQUEST_BYTES,
            MAXIMUM_RESPONSE_BYTES,
            || self.cancellation.requested(),
        )
        .map_err(|_| LocalModelAdapterTerminal::ProviderLost)?;
        finish(report)
    }
}

fn finish(report: ProcessReport) -> Result<Vec<u8>, LocalModelAdapterTerminal> {
    match report.terminal {
        ProcessTerminal::Cancelled => Err(LocalModelAdapterTerminal::Cancelled),
        ProcessTerminal::TimedOut => Err(LocalModelAdapterTerminal::TimedOut),
        ProcessTerminal::Exited(status) if status.code() == Some(28) => {
            Err(LocalModelAdapterTerminal::TimedOut)
        }
        _ if report.stdout_bound_retired => Err(LocalModelAdapterTerminal::ResponseBoundExceeded),
        ProcessTerminal::Exited(status) if status.success() => {
            if report.stdout.observed_bytes > MAXIMUM_RESPONSE_BYTES as u64 {
                Err(LocalModelAdapterTerminal::ResponseBoundExceeded)
            } else {
                Ok(report.stdout.retained)
            }
        }
        ProcessTerminal::Exited(_) | ProcessTerminal::ProviderLost(_) => {
            Err(LocalModelAdapterTerminal::ProviderLost)
        }
    }
}

fn executable(path: &std::path::Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        metadata.is_file()
    }
}

#[cfg(test)]
#[path = "generation_transport_tests.rs"]
mod tests;

#[cfg(all(test, unix))]
#[path = "generation_test_support.rs"]
pub(super) mod test_support;
