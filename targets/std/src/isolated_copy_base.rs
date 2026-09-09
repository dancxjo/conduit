use crate::copy_task::base::{CopyFiles, ExecutionFaults};
use crate::CopyResult;
use conduit_core::{
    BaseCapabilityRefusal, BaseCapabilityTable, BaseOperationClaim, CapabilityIssueRequest,
    ProtectedResourceCommitPolicy,
};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout};

mod host;
pub use host::{IsolatedFileBaseInspection, IsolatedFileHost};

const PROTOCOL_VERSION: u16 = 1;
const MAX_PROVIDER_FRAMES: u32 = 4_100;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CopyBootstrap {
    pub issue: CapabilityIssueRequest,
    pub source_path: PathBuf,
    pub destination_path: PathBuf,
    pub commit_policy: ProtectedResourceCommitPolicy,
    pub maximum_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
enum RequestFrame {
    Bootstrap {
        protocol_version: u16,
        bootstrap: Box<CopyBootstrap>,
    },
    Step {
        claim: Box<BaseOperationClaim>,
    },
    Cancel,
    #[cfg(feature = "isolated-base-proof")]
    ProbeReadPath {
        path: PathBuf,
    },
    #[cfg(feature = "isolated-base-proof")]
    ProbeWritePath {
        path: PathBuf,
    },
    #[cfg(feature = "isolated-base-proof")]
    ProbeProcessSpawn,
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
enum ResponseFrame {
    Ready {
        protocol_version: u16,
        enforcement: String,
        provider_generation: u64,
    },
    Progress {
        bytes_copied: u64,
    },
    Terminal {
        result: WireCopyResult,
    },
    Refused {
        reason: String,
    },
    #[cfg(feature = "isolated-base-proof")]
    ProbeDenied {
        probe: String,
        os_error: i32,
    },
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum WireCopyResult {
    Success {
        bytes_copied: u64,
    },
    DestinationExists,
    Denied,
    StaleHandle,
    Oversized {
        source_bytes: u64,
        maximum_bytes: u64,
    },
    Partial {
        bytes_copied: u64,
    },
    Cancelled {
        bytes_copied: u64,
    },
    CleanupFailed {
        bytes_copied: u64,
    },
}

impl From<CopyResult> for WireCopyResult {
    fn from(value: CopyResult) -> Self {
        match value {
            CopyResult::Success { bytes_copied } => Self::Success { bytes_copied },
            CopyResult::DestinationExists => Self::DestinationExists,
            CopyResult::Denied => Self::Denied,
            CopyResult::StaleHandle => Self::StaleHandle,
            CopyResult::Oversized {
                source_bytes,
                maximum_bytes,
            } => Self::Oversized {
                source_bytes,
                maximum_bytes,
            },
            CopyResult::Partial { bytes_copied } => Self::Partial { bytes_copied },
            CopyResult::Cancelled { bytes_copied } => Self::Cancelled { bytes_copied },
            CopyResult::CleanupFailed { bytes_copied } => Self::CleanupFailed { bytes_copied },
        }
    }
}

impl From<WireCopyResult> for CopyResult {
    fn from(value: WireCopyResult) -> Self {
        match value {
            WireCopyResult::Success { bytes_copied } => Self::Success { bytes_copied },
            WireCopyResult::DestinationExists => Self::DestinationExists,
            WireCopyResult::Denied => Self::Denied,
            WireCopyResult::StaleHandle => Self::StaleHandle,
            WireCopyResult::Oversized {
                source_bytes,
                maximum_bytes,
            } => Self::Oversized {
                source_bytes,
                maximum_bytes,
            },
            WireCopyResult::Partial { bytes_copied } => Self::Partial { bytes_copied },
            WireCopyResult::Cancelled { bytes_copied } => Self::Cancelled { bytes_copied },
            WireCopyResult::CleanupFailed { bytes_copied } => Self::CleanupFailed { bytes_copied },
        }
    }
}

pub fn provider_main() -> Result<(), String> {
    let request: RequestFrame =
        crate::isolated_base::read_frame(&mut io::stdin()).map_err(io_error)?;
    let RequestFrame::Bootstrap {
        protocol_version,
        bootstrap,
    } = request
    else {
        return Err("first copy Base frame was not bootstrap".into());
    };
    if protocol_version != PROTOCOL_VERSION {
        return Err("copy Base protocol version mismatch".into());
    }
    validate_bootstrap(&bootstrap)?;
    let source_directory = bootstrap
        .source_path
        .parent()
        .ok_or("source has no parent")?;
    let destination_directory = bootstrap
        .destination_path
        .parent()
        .ok_or("destination has no parent")?;

    let mut issuer_key = [0_u8; 32];
    File::open("/dev/urandom")
        .and_then(|mut random| random.read_exact(&mut issuer_key))
        .map_err(io_error)?;
    let mut table = BaseCapabilityTable::new(
        bootstrap.issue.scope.host_id.clone(),
        bootstrap.issue.scope.boot_id.clone(),
        bootstrap.issue.scope.base_instance_id.clone(),
        bootstrap.issue.scope.base_provider_generation,
        issuer_key,
        1,
    )
    .map_err(capability_error)?;
    let handle = table
        .issue(bootstrap.issue.clone())
        .map_err(capability_error)?;
    apply_copy_landlock(source_directory, destination_directory)?;
    crate::isolated_base::apply_seccomp()?;

    let (mut files, mut startup_terminal) = match CopyFiles::prepare(
        &bootstrap.source_path,
        &bootstrap.destination_path,
        bootstrap.commit_policy,
        bootstrap.maximum_bytes,
        ExecutionFaults::default(),
    ) {
        Ok(files) => (Some(files), None),
        Err(result) => (None, Some(WireCopyResult::from(result))),
    };
    crate::isolated_base::write_frame(
        &mut io::stdout(),
        &ResponseFrame::Ready {
            protocol_version: PROTOCOL_VERSION,
            enforcement: "process-isolated/landlock-v3+seccomp".into(),
            provider_generation: bootstrap.issue.scope.base_provider_generation,
        },
    )
    .map_err(io_error)?;

    for _ in 0..MAX_PROVIDER_FRAMES {
        match crate::isolated_base::read_frame::<_, RequestFrame>(&mut io::stdin())
            .map_err(io_error)?
        {
            RequestFrame::Step { claim } => {
                if let Some(result) = startup_terminal.take() {
                    crate::isolated_base::write_frame(
                        &mut io::stdout(),
                        &ResponseFrame::Terminal { result },
                    )
                    .map_err(io_error)?;
                    continue;
                }
                let Some(copy_files) = files.as_mut() else {
                    write_refusal("copy already terminal")?;
                    continue;
                };
                let lease = match table.authorize(&handle, &claim) {
                    Ok(lease) => lease,
                    Err(error) => {
                        write_refusal(&format!("capability:{error:?}"))?;
                        continue;
                    }
                };
                let terminal = match copy_files.step() {
                    Ok(true) => {
                        table.complete(lease, 1).map_err(capability_error)?;
                        crate::isolated_base::write_frame(
                            &mut io::stdout(),
                            &ResponseFrame::Progress {
                                bytes_copied: copy_files.bytes_copied,
                            },
                        )
                        .map_err(io_error)?;
                        false
                    }
                    Ok(false) => {
                        table.complete(lease, 1).map_err(capability_error)?;
                        let result = CopyResult::Success {
                            bytes_copied: copy_files.bytes_copied,
                        };
                        crate::isolated_base::write_frame(
                            &mut io::stdout(),
                            &ResponseFrame::Terminal {
                                result: result.into(),
                            },
                        )
                        .map_err(io_error)?;
                        true
                    }
                    Err(result) => {
                        table.complete(lease, 1).map_err(capability_error)?;
                        crate::isolated_base::write_frame(
                            &mut io::stdout(),
                            &ResponseFrame::Terminal {
                                result: result.into(),
                            },
                        )
                        .map_err(io_error)?;
                        true
                    }
                };
                if terminal {
                    files = None;
                }
            }
            RequestFrame::Cancel => {
                let result = files.take().map(|mut files| {
                    if files.cleanup() {
                        CopyResult::Cancelled {
                            bytes_copied: files.bytes_copied,
                        }
                    } else {
                        CopyResult::CleanupFailed {
                            bytes_copied: files.bytes_copied,
                        }
                    }
                });
                table.revoke(&handle).map_err(capability_error)?;
                if let Some(result) = result {
                    crate::isolated_base::write_frame(
                        &mut io::stdout(),
                        &ResponseFrame::Terminal {
                            result: result.into(),
                        },
                    )
                    .map_err(io_error)?;
                }
            }
            #[cfg(feature = "isolated-base-proof")]
            RequestFrame::ProbeReadPath { path } => {
                let error = File::open(path)
                    .expect_err("copy Landlock unexpectedly permitted sibling read");
                write_probe("raw-sibling-read", error)?;
            }
            #[cfg(feature = "isolated-base-proof")]
            RequestFrame::ProbeWritePath { path } => {
                let error = std::fs::OpenOptions::new()
                    .write(true)
                    .open(path)
                    .expect_err("copy Landlock unexpectedly permitted sibling write");
                write_probe("raw-sibling-write", error)?;
            }
            #[cfg(feature = "isolated-base-proof")]
            RequestFrame::ProbeProcessSpawn => {
                let error = std::process::Command::new("/bin/true")
                    .status()
                    .expect_err("copy seccomp unexpectedly permitted process spawn");
                write_probe("process-spawn", error)?;
            }
            RequestFrame::Shutdown => {
                crate::isolated_base::write_frame(&mut io::stdout(), &ResponseFrame::Stopped)
                    .map_err(io_error)?;
                return Ok(());
            }
            RequestFrame::Bootstrap { .. } => return Err("duplicate copy bootstrap".into()),
        }
    }
    Err("copy Base frame bound exhausted".into())
}

#[cfg(feature = "isolated-base-proof")]
mod adversarial;
#[cfg(feature = "isolated-base-proof")]
pub use adversarial::{run_adversarial_copy_proof, AdversarialCopyReport};

pub(crate) struct IsolatedCopyClient {
    child: Child,
    input: ChildStdin,
    output: ChildStdout,
}

pub(crate) struct IsolatedCopyStep {
    pub continues: bool,
    pub bytes_copied: u64,
}

impl IsolatedCopyClient {
    pub fn start(executable: &Path, bootstrap: CopyBootstrap) -> Result<Self, CopyResult> {
        let mut child =
            crate::isolated_base::spawn_provider(executable).map_err(|_| CopyResult::Denied)?;
        let input = child.stdin.take().ok_or(CopyResult::Denied)?;
        let output = child.stdout.take().ok_or(CopyResult::Denied)?;
        let mut client = Self {
            child,
            input,
            output,
        };
        crate::isolated_base::write_frame(
            &mut client.input,
            &RequestFrame::Bootstrap {
                protocol_version: PROTOCOL_VERSION,
                bootstrap: Box::new(bootstrap),
            },
        )
        .map_err(|_| CopyResult::Denied)?;
        match crate::isolated_base::read_frame::<_, ResponseFrame>(&mut client.output)
            .map_err(|_| CopyResult::Denied)?
        {
            ResponseFrame::Ready { enforcement, .. }
                if enforcement == "process-isolated/landlock-v3+seccomp" =>
            {
                Ok(client)
            }
            ResponseFrame::Terminal { result } => Err(result.into()),
            _ => Err(CopyResult::Denied),
        }
    }

    pub fn step(&mut self, claim: BaseOperationClaim) -> Result<IsolatedCopyStep, CopyResult> {
        crate::isolated_base::write_frame(
            &mut self.input,
            &RequestFrame::Step {
                claim: Box::new(claim),
            },
        )
        .map_err(|_| CopyResult::Partial { bytes_copied: 0 })?;
        match crate::isolated_base::read_frame::<_, ResponseFrame>(&mut self.output)
            .map_err(|_| CopyResult::Partial { bytes_copied: 0 })?
        {
            ResponseFrame::Progress { bytes_copied } => Ok(IsolatedCopyStep {
                continues: true,
                bytes_copied,
            }),
            ResponseFrame::Terminal {
                result: WireCopyResult::Success { bytes_copied },
            } => Ok(IsolatedCopyStep {
                continues: false,
                bytes_copied,
            }),
            ResponseFrame::Terminal { result } => Err(result.into()),
            ResponseFrame::Refused { .. } => Err(CopyResult::Denied),
            _ => Err(CopyResult::Partial { bytes_copied: 0 }),
        }
    }

    pub fn cancel(&mut self) -> CopyResult {
        if crate::isolated_base::write_frame(&mut self.input, &RequestFrame::Cancel).is_err() {
            return CopyResult::Partial { bytes_copied: 0 };
        }
        match crate::isolated_base::read_frame::<_, ResponseFrame>(&mut self.output) {
            Ok(ResponseFrame::Terminal { result }) => result.into(),
            _ => CopyResult::CleanupFailed { bytes_copied: 0 },
        }
    }
}

impl Drop for IsolatedCopyClient {
    fn drop(&mut self) {
        let _ = crate::isolated_base::write_frame(&mut self.input, &RequestFrame::Shutdown);
        let _ = self.child.wait();
    }
}

fn validate_bootstrap(bootstrap: &CopyBootstrap) -> Result<(), String> {
    if bootstrap.maximum_bytes == 0
        || bootstrap.source_path == bootstrap.destination_path
        || !matches!(
            bootstrap.commit_policy,
            ProtectedResourceCommitPolicy::CreateOnly
                | ProtectedResourceCommitPolicy::ReplaceExisting
        )
    {
        return Err("invalid isolated copy bootstrap".into());
    }
    Ok(())
}

fn write_refusal(reason: &str) -> Result<(), String> {
    crate::isolated_base::write_frame(
        &mut io::stdout(),
        &ResponseFrame::Refused {
            reason: reason.into(),
        },
    )
    .map_err(io_error)
}

#[cfg(feature = "isolated-base-proof")]
fn write_probe(probe: &str, error: io::Error) -> Result<(), String> {
    crate::isolated_base::write_frame(
        &mut io::stdout(),
        &ResponseFrame::ProbeDenied {
            probe: probe.into(),
            os_error: error.raw_os_error().ok_or("probe denial lacked OS errno")?,
        },
    )
    .map_err(io_error)
}

mod sandbox;
use sandbox::apply_copy_landlock;

fn io_error(error: io::Error) -> String {
    error.to_string()
}
fn capability_error(error: BaseCapabilityRefusal) -> String {
    format!("{error:?}")
}
