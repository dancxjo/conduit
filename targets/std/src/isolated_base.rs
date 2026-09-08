//! Linux process-isolated file Base used by the hosted confinement proof.
//!
//! The provider applies Landlock before accepting operations and a seccomp
//! deny-list for process creation, execution, networking, and tracing. The
//! universal Host/Base contract remains platform-neutral.

use conduit_core::{
    BaseCapabilityRefusal, BaseCapabilityTable, BaseOperationClaim, CapabilityIssueRequest,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::path::Path;
use std::process::{Child, Command, Stdio};

pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_FRAME_BYTES: usize = 4096;
pub const MAX_OPERATIONS: u16 = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum SupervisorFrame {
    Bootstrap {
        protocol_version: u16,
        issue: Box<CapabilityIssueRequest>,
        allowed_directory: String,
        allowed_file: String,
        protected_sibling: String,
    },
    Read {
        claim: Box<BaseOperationClaim>,
    },
    Revoke,
    ProbeRawSibling,
    ProbeProcessSpawn,
    ProbeNetworkSocket,
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum ProviderFrame {
    Ready {
        protocol_version: u16,
        enforcement: String,
        environment_entries: u16,
        descriptor_ceiling: u16,
    },
    ReadCompleted {
        bytes: Vec<u8>,
    },
    Refused {
        reason: String,
    },
    Revoked,
    ProbeDenied {
        probe: String,
        os_error: i32,
    },
    Stopped,
}

pub fn provider_main() -> Result<(), String> {
    let bootstrap: SupervisorFrame = read_frame(&mut io::stdin()).map_err(format_io)?;
    let SupervisorFrame::Bootstrap {
        protocol_version,
        issue,
        allowed_directory,
        allowed_file,
        protected_sibling,
    } = bootstrap
    else {
        return Err("first frame was not bootstrap".into());
    };
    if protocol_version != PROTOCOL_VERSION {
        return Err("protocol version mismatch".into());
    }

    let mut issuer_key = [0_u8; 32];
    File::open("/dev/urandom")
        .and_then(|mut random| random.read_exact(&mut issuer_key))
        .map_err(format_io)?;
    let mut table = BaseCapabilityTable::new(
        issue.scope.host_id.clone(),
        issue.scope.boot_id.clone(),
        issue.scope.base_instance_id.clone(),
        issue.scope.base_provider_generation,
        issuer_key,
        1,
    )
    .map_err(format_refusal)?;
    let handle = table.issue(*issue).map_err(format_refusal)?;

    apply_landlock(Path::new(&allowed_directory))?;
    apply_seccomp()?;
    write_frame(
        &mut io::stdout(),
        &ProviderFrame::Ready {
            protocol_version: PROTOCOL_VERSION,
            enforcement: "process-isolated/landlock+seccomp".into(),
            environment_entries: std::env::vars_os().count() as u16,
            descriptor_ceiling: current_descriptor_ceiling()?,
        },
    )
    .map_err(format_io)?;

    for _ in 0..MAX_OPERATIONS {
        match read_frame::<_, SupervisorFrame>(&mut io::stdin()).map_err(format_io)? {
            SupervisorFrame::Read { claim } => match table.authorize(&handle, &claim) {
                Ok(lease) => match File::open(&allowed_file) {
                    Ok(file) => {
                        let mut bytes = Vec::new();
                        file.take(claim.parameter_bytes as u64)
                            .read_to_end(&mut bytes)
                            .map_err(format_io)?;
                        table
                            .complete(lease, bytes.len() as u32)
                            .map_err(format_refusal)?;
                        write_frame(&mut io::stdout(), &ProviderFrame::ReadCompleted { bytes })
                            .map_err(format_io)?;
                    }
                    Err(error) => write_frame(
                        &mut io::stdout(),
                        &ProviderFrame::Refused {
                            reason: format!("os-denied:{:?}", error.kind()),
                        },
                    )
                    .map_err(format_io)?,
                },
                Err(refusal) => write_frame(
                    &mut io::stdout(),
                    &ProviderFrame::Refused {
                        reason: format!("capability:{refusal:?}"),
                    },
                )
                .map_err(format_io)?,
            },
            SupervisorFrame::Revoke => {
                table.revoke(&handle).map_err(format_refusal)?;
                write_frame(&mut io::stdout(), &ProviderFrame::Revoked).map_err(format_io)?;
            }
            SupervisorFrame::ProbeRawSibling => {
                let error = File::open(&protected_sibling)
                    .expect_err("Landlock unexpectedly permitted the sibling resource");
                write_probe_denied(&mut io::stdout(), "raw-sibling", error.raw_os_error())?;
            }
            SupervisorFrame::ProbeProcessSpawn => {
                let error = Command::new("/bin/true")
                    .status()
                    .expect_err("seccomp unexpectedly permitted process execution");
                write_probe_denied(&mut io::stdout(), "process-spawn", error.raw_os_error())?;
            }
            SupervisorFrame::ProbeNetworkSocket => {
                let error = std::net::TcpStream::connect("127.0.0.1:9")
                    .expect_err("seccomp unexpectedly permitted a network socket");
                write_probe_denied(&mut io::stdout(), "network-socket", error.raw_os_error())?;
            }
            SupervisorFrame::Shutdown => {
                write_frame(&mut io::stdout(), &ProviderFrame::Stopped).map_err(format_io)?;
                return Ok(());
            }
            SupervisorFrame::Bootstrap { .. } => return Err("duplicate bootstrap".into()),
        }
    }
    Err("operation bound exhausted".into())
}

pub fn spawn_provider(executable: &Path) -> io::Result<Child> {
    use std::os::unix::process::CommandExt;
    let mut command = Command::new(executable);
    command
        .env_clear()
        .current_dir("/")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    unsafe {
        command.pre_exec(|| {
            set_limit(libc::RLIMIT_AS, 128 * 1024 * 1024)?;
            set_limit(libc::RLIMIT_CPU, 2)?;
            set_limit(libc::RLIMIT_NOFILE, 8)?;
            set_limit(libc::RLIMIT_NPROC, 1)?;
            set_limit(libc::RLIMIT_FSIZE, 1024 * 1024)?;
            Ok(())
        });
    }
    command.spawn()
}

fn set_limit(resource: libc::__rlimit_resource_t, value: libc::rlim_t) -> io::Result<()> {
    let limit = libc::rlimit {
        rlim_cur: value,
        rlim_max: value,
    };
    if unsafe { libc::setrlimit(resource, &limit) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn write_probe_denied(
    writer: &mut impl Write,
    probe: &str,
    os_error: Option<i32>,
) -> Result<(), String> {
    let os_error = os_error.ok_or_else(|| format!("{probe} denial had no OS error"))?;
    write_frame(
        writer,
        &ProviderFrame::ProbeDenied {
            probe: probe.into(),
            os_error,
        },
    )
    .map_err(format_io)
}

pub fn write_frame(writer: &mut impl Write, frame: &impl Serialize) -> io::Result<()> {
    let bytes = serde_json::to_vec(frame).map_err(io::Error::other)?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame too large",
        ));
    }
    writer.write_all(&(bytes.len() as u32).to_le_bytes())?;
    writer.write_all(&bytes)?;
    writer.flush()
}

pub fn read_frame<R: Read, T: DeserializeOwned>(reader: &mut R) -> io::Result<T> {
    let mut length = [0_u8; 4];
    reader.read_exact(&mut length)?;
    let length = u32::from_le_bytes(length) as usize;
    if length == 0 || length > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid frame length",
        ));
    }
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes)?;
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

fn apply_landlock(allowed: &Path) -> Result<(), String> {
    const CREATE_RULESET_VERSION: u32 = 1;
    const RULE_PATH_BENEATH: u32 = 1;
    const ACCESS_FS_READ_FILE: u64 = 1 << 2;
    const ACCESS_FS_READ_DIR: u64 = 1 << 3;
    const HANDLED_FS_V1: u64 = (1 << 13) - 1;
    #[repr(C)]
    struct RulesetAttr {
        handled_access_fs: u64,
    }
    #[repr(C)]
    struct PathBeneathAttr {
        allowed_access: u64,
        parent_fd: i32,
        reserved: u32,
    }

    let abi = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            std::ptr::null::<RulesetAttr>(),
            0,
            CREATE_RULESET_VERSION,
        )
    };
    if abi < 1 {
        return Err("Landlock ABI unavailable".into());
    }
    let ruleset_attr = RulesetAttr {
        handled_access_fs: HANDLED_FS_V1,
    };
    let ruleset_fd = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            &ruleset_attr,
            std::mem::size_of::<RulesetAttr>(),
            0,
        ) as i32
    };
    if ruleset_fd < 0 {
        return Err(format_io(io::Error::last_os_error()));
    }
    let allowed_dir = File::open(allowed).map_err(format_io)?;
    let path_attr = PathBeneathAttr {
        allowed_access: ACCESS_FS_READ_FILE | ACCESS_FS_READ_DIR,
        parent_fd: allowed_dir.as_raw_fd(),
        reserved: 0,
    };
    let add_result = unsafe {
        libc::syscall(
            libc::SYS_landlock_add_rule,
            ruleset_fd,
            RULE_PATH_BENEATH,
            &path_attr,
            0,
        )
    };
    if add_result != 0 {
        unsafe { libc::close(ruleset_fd) };
        return Err(format_io(io::Error::last_os_error()));
    }
    let no_new_privileges = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
    if no_new_privileges != 0 {
        unsafe { libc::close(ruleset_fd) };
        return Err(format_io(io::Error::last_os_error()));
    }
    let restrict_result = unsafe { libc::syscall(libc::SYS_landlock_restrict_self, ruleset_fd, 0) };
    unsafe { libc::close(ruleset_fd) };
    if restrict_result != 0 {
        return Err(format_io(io::Error::last_os_error()));
    }
    Ok(())
}

pub(crate) fn apply_seccomp() -> Result<(), String> {
    const BPF_LD_W_ABS: u16 = 0x20;
    const BPF_JMP_JEQ_K: u16 = 0x15;
    const BPF_RET_K: u16 = 0x06;
    const SECCOMP_RET_ALLOW: u32 = 0x7fff_0000;
    const SECCOMP_RET_ERRNO: u32 = 0x0005_0000;
    let denied = [
        libc::SYS_clone,
        libc::SYS_clone3,
        libc::SYS_fork,
        libc::SYS_vfork,
        libc::SYS_execve,
        libc::SYS_execveat,
        libc::SYS_socket,
        libc::SYS_socketpair,
        libc::SYS_ptrace,
    ];
    let mut filters = Vec::<libc::sock_filter>::with_capacity(denied.len() * 2 + 2);
    filters.push(libc::sock_filter {
        code: BPF_LD_W_ABS,
        jt: 0,
        jf: 0,
        k: 0,
    });
    for syscall in denied {
        filters.push(libc::sock_filter {
            code: BPF_JMP_JEQ_K,
            jt: 0,
            jf: 1,
            k: syscall as u32,
        });
        filters.push(libc::sock_filter {
            code: BPF_RET_K,
            jt: 0,
            jf: 0,
            k: SECCOMP_RET_ERRNO | libc::EPERM as u32,
        });
    }
    filters.push(libc::sock_filter {
        code: BPF_RET_K,
        jt: 0,
        jf: 0,
        k: SECCOMP_RET_ALLOW,
    });
    let program = libc::sock_fprog {
        len: filters.len() as u16,
        filter: filters.as_mut_ptr(),
    };
    let result = unsafe {
        libc::prctl(
            libc::PR_SET_SECCOMP,
            libc::SECCOMP_MODE_FILTER,
            &program as *const libc::sock_fprog,
        )
    };
    if result != 0 {
        return Err(format_io(io::Error::last_os_error()));
    }
    Ok(())
}

fn current_descriptor_ceiling() -> Result<u16, String> {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } != 0 {
        return Err(format_io(io::Error::last_os_error()));
    }
    Ok(limit.rlim_cur.min(u16::MAX as u64) as u16)
}

fn format_io(error: io::Error) -> String {
    error.to_string()
}

fn format_refusal(refusal: BaseCapabilityRefusal) -> String {
    format!("{refusal:?}")
}
