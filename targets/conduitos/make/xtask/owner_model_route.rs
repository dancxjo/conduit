//! A controlled loopback route selected by a fresh installed owner before Boot.

#[cfg(unix)]
use std::os::unix::process::CommandExt;
#[cfg(unix)]
use std::sync::atomic::{AtomicI32, Ordering};
use std::{
    ffi::OsString,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

use clap::Args as ClapArgs;

use crate::cli::GlobalOpts;

use super::{profile::Paths, ConduitosArch, ConduitosError};

#[cfg(unix)]
static INTERRUPTED: AtomicI32 = AtomicI32::new(0);

#[cfg(unix)]
extern "C" fn record_signal(signal: libc::c_int) {
    INTERRUPTED.store(signal, Ordering::SeqCst);
}

fn abort_route(route: &mut Child, socket: &Path) {
    let _ = route.kill();
    let _ = route.wait();
    let _ = std::fs::remove_file(socket);
}

#[derive(ClapArgs, Debug)]
pub(super) struct Args {
    /// Existing local Ollama origin; no model is downloaded or started.
    #[arg(long)]
    upstream: String,
    /// New socket under a private directory, retained while this command runs.
    #[arg(long)]
    control_socket: PathBuf,
    /// Fixed loopback port selected by the installed owner; required for a supervised run.
    #[arg(long)]
    listen_port: Option<u16>,
    /// Command to run while this route is alive. The route is stopped when it ends.
    #[arg(last = true)]
    command: Vec<OsString>,
}

pub(super) fn execute(args: &Args, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "owner-model-route-requires-live-provider",
            "the controlled route must connect to an existing local provider",
        ));
    }
    let root = Paths::new(ConduitosArch::X86_64)?.root;
    let script = root.join("proof/browser/local-model-route.mjs");
    if !script.is_file() {
        return Err(ConduitosError::refusal(
            "owner-model-route-script-unavailable",
            format!("route script is unavailable: {}", script.display()),
        ));
    }
    if args.control_socket.exists() {
        return Err(ConduitosError::refusal(
            "owner-model-route-socket-exists",
            "provide a new private control socket path",
        ));
    }
    let parent = args.control_socket.parent().ok_or_else(|| {
        ConduitosError::refusal(
            "owner-model-route-directory",
            "control socket needs a parent",
        )
    })?;
    let metadata = std::fs::metadata(parent).map_err(|error| {
        ConduitosError::refusal("owner-model-route-directory", error.to_string())
    })?;
    if !metadata.is_dir() {
        return Err(ConduitosError::refusal(
            "owner-model-route-directory",
            "control socket parent must be a private directory",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(ConduitosError::refusal(
                "owner-model-route-directory",
                "control socket parent must exclude group and other access",
            ));
        }
    }
    let control_socket = parent
        .canonicalize()
        .map_err(|error| ConduitosError::refusal("owner-model-route-directory", error.to_string()))?
        .join(
            args.control_socket
                .file_name()
                .expect("control socket has a filename"),
        );
    if !args.command.is_empty() && args.listen_port.is_none() {
        return Err(ConduitosError::refusal(
            "owner-model-route-fixed-endpoint-required",
            "a supervised command requires --listen-port matching the installed endpoint",
        ));
    }
    if args.listen_port == Some(0) {
        return Err(ConduitosError::refusal(
            "owner-model-route-invalid-port",
            "the fixed listener port must be nonzero",
        ));
    }
    let mut route = Command::new("node")
        .arg(script)
        .arg(&args.upstream)
        .arg(&control_socket)
        .args(args.listen_port.map(|port| port.to_string()))
        .args((!args.command.is_empty()).then_some("--supervised"))
        .current_dir(root)
        .stdin(if args.command.is_empty() {
            Stdio::inherit()
        } else {
            Stdio::piped()
        })
        .stdout(if args.command.is_empty() {
            Stdio::inherit()
        } else {
            Stdio::piped()
        })
        .spawn()
        .map_err(|error| ConduitosError::refusal("owner-model-route-launch", error.to_string()))?;
    if args.command.is_empty() {
        let status = route.wait().map_err(|error| {
            ConduitosError::refusal("owner-model-route-wait", error.to_string())
        })?;
        if status.success() {
            return Ok(());
        }
        return Err(ConduitosError::refusal(
            "owner-model-route-failed",
            format!("controlled route exited with {status}"),
        ));
    }
    let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
    let stdout = route.stdout.take().expect("piped route stdout");
    thread::spawn(move || {
        let mut ready = String::new();
        let result = BufReader::new(stdout).read_line(&mut ready).map(|_| ready);
        let _ = ready_sender.send(result);
    });
    let ready = match ready_receiver.recv_timeout(Duration::from_secs(10)) {
        Ok(Ok(ready)) => ready,
        Ok(Err(error)) => {
            abort_route(&mut route, &control_socket);
            return Err(ConduitosError::refusal(
                "owner-model-route-readiness",
                error.to_string(),
            ));
        }
        Err(error) => {
            abort_route(&mut route, &control_socket);
            return Err(ConduitosError::refusal(
                "owner-model-route-readiness",
                format!("route did not report its selected endpoint within ten seconds: {error}"),
            ));
        }
    };
    let expected = format!(
        "http://127.0.0.1:{}",
        args.listen_port.expect("checked above")
    );
    let reported: serde_json::Value = serde_json::from_str(&ready).map_err(|error| {
        abort_route(&mut route, &control_socket);
        ConduitosError::refusal("owner-model-route-readiness", error.to_string())
    })?;
    if reported["endpoint"] != expected
        || reported["control_socket"] != control_socket.to_string_lossy().as_ref()
    {
        abort_route(&mut route, &control_socket);
        return Err(ConduitosError::refusal(
            "owner-model-route-readiness",
            "route did not bind the selected endpoint and private control socket",
        ));
    }
    println!("{ready}");
    let mut command = Command::new(&args.command[0]);
    #[cfg(unix)]
    command.process_group(0);
    #[cfg(unix)]
    unsafe {
        libc::signal(
            libc::SIGINT,
            record_signal as *const () as libc::sighandler_t,
        );
        libc::signal(
            libc::SIGTERM,
            record_signal as *const () as libc::sighandler_t,
        );
    }
    let mut child = command.args(&args.command[1..]).spawn().map_err(|error| {
        abort_route(&mut route, &control_socket);
        ConduitosError::refusal("owner-model-route-command", error.to_string())
    })?;
    loop {
        #[cfg(unix)]
        {
            let signal = INTERRUPTED.swap(0, Ordering::SeqCst);
            if signal != 0 {
                unsafe {
                    libc::kill(-(child.id() as i32), signal);
                }
                drop(route.stdin.take());
                // A stopped run must not strand its guest or browser.
                thread::sleep(Duration::from_millis(250));
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
                let _ = child.wait();
                let _ = route.wait();
                return Err(ConduitosError::refusal(
                    "owner-model-route-interrupted",
                    "supervised command and selected route were interrupted",
                ));
            }
        }
        if let Some(status) = route
            .try_wait()
            .map_err(|error| ConduitosError::refusal("owner-model-route-wait", error.to_string()))?
        {
            // The proof command launches Node/QEMU descendants. Retire its
            // process group when the selected route disappears.
            #[cfg(unix)]
            {
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGTERM);
                }
                thread::sleep(Duration::from_secs(2));
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
            }
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(&control_socket);
            return Err(ConduitosError::refusal(
                "owner-model-route-lost",
                format!("selected model route exited during the supervised proof: {status}"),
            ));
        }
        if let Some(status) = child.try_wait().map_err(|error| {
            ConduitosError::refusal("owner-model-route-command", error.to_string())
        })? {
            // Closing the private pipe asks the route to unlink its control socket.
            drop(route.stdin.take());
            let route_status = route.wait().map_err(|error| {
                ConduitosError::refusal("owner-model-route-wait", error.to_string())
            })?;
            if !route_status.success() {
                return Err(ConduitosError::refusal(
                    "owner-model-route-failed",
                    format!("controlled route exited with {route_status}"),
                ));
            }
            if status.success() {
                return Ok(());
            }
            return Err(ConduitosError::refusal(
                "owner-model-route-command-failed",
                format!("supervised command exited with {status}"),
            ));
        }
        thread::sleep(Duration::from_millis(250));
    }
}
