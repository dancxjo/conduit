//! An installed zero-Body Birth followed by the existing live three-Host proof.

use std::{path::PathBuf, process::Command};

use clap::Args as ClapArgs;

use crate::cli::GlobalOpts;

use super::{profile::Paths, ConduitosArch, ConduitosError};

#[derive(ClapArgs, Debug, Clone)]
pub(super) struct Args {
    /// Executable named by the installed Linux Host release.
    #[arg(long)]
    owner: PathBuf,
    /// Fresh installed Host state with no retained Body.
    #[arg(long)]
    owner_state: PathBuf,
    /// Exact-source static Handbook package.
    #[arg(long)]
    handbook: PathBuf,
    /// Exact-source ConduitOS x86_64 live build.
    #[arg(long)]
    build: PathBuf,
    /// Reviewed ConduitOS Host profile.
    #[arg(long)]
    host_profile: PathBuf,
    /// Private TLS leaf valid for the guest-visible route.
    #[arg(long)]
    route_tls_cert: PathBuf,
    /// Private key for that leaf; never retained in evidence.
    #[arg(long)]
    route_tls_key: PathBuf,
    /// Private IPv4 owner listener reachable by the QEMU forwarding helper.
    #[arg(long)]
    owner_forward: std::net::SocketAddr,
    /// Guest-visible WSS URL pinned by the invitation.
    #[arg(long)]
    route_url: String,
    /// New private output directory, including invitation and spore.
    #[arg(long)]
    output_dir: PathBuf,
    /// Pinned Playwright module installed for the browser proof project.
    #[arg(long)]
    playwright: PathBuf,
    /// Name selected through the screen-free Crèche Face.
    #[arg(long, default_value = "One Body Clock")]
    body_name: String,
    /// Optionally select a currently advertised ALSA speaker.
    #[arg(long, requires_all = ["speaker_device", "speech_executable"])]
    speaker_card: Option<String>,
    #[arg(long, requires = "speaker_card")]
    speaker_device: Option<u16>,
    #[arg(long, requires_all = ["speech_data", "speech_engine", "speech_language_coverage"])]
    speech_executable: Option<PathBuf>,
    #[arg(long, requires = "speech_executable")]
    speech_data: Option<PathBuf>,
    #[arg(long, requires = "speech_executable")]
    speech_engine: Option<PathBuf>,
    /// Explicit provider-bound Language coverage declaration.
    #[arg(long, requires = "speech_executable")]
    speech_language_coverage: Option<PathBuf>,
    /// Already-local Ollama model for a same-run validated spoken chapter.
    #[arg(long, requires = "speech_executable")]
    model: Option<String>,
    /// Explicit loopback Ollama origin, reached through a producer-owned route.
    #[arg(long, default_value = "http://127.0.0.1:11434")]
    ollama_endpoint: String,
    /// Finite model memory admission in MiB.
    #[arg(long, default_value_t = 2048)]
    admitted_memory_mib: u32,
}

pub(super) fn execute(args: &Args, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "screen-free-three-host-requires-live-input",
            "screen-free Birth and the three-host proof require live products",
        ));
    }
    let root = Paths::new(ConduitosArch::X86_64)?.root;
    let script = root.join("proof/browser/screen-free-three-host-journey.mjs");
    for (name, path) in [
        ("producer", &script),
        ("owner", &args.owner),
        ("installation", &args.owner_state.join("installation.json")),
        (
            "handbook",
            &args.handbook.join("application.application.json"),
        ),
        ("build", &args.build.join("build-manifest.json")),
        ("host-profile", &args.host_profile),
        ("route-tls-cert", &args.route_tls_cert),
        ("route-tls-key", &args.route_tls_key),
        ("playwright", &args.playwright),
    ] {
        if !path.is_file() {
            return Err(ConduitosError::refusal(
                "screen-free-three-host-prerequisite",
                format!("{name} is unavailable: {}", path.display()),
            ));
        }
    }
    if args.output_dir.exists() {
        return Err(ConduitosError::refusal(
            "screen-free-three-host-output-exists",
            "provide a new private output directory",
        ));
    }
    let current = std::env::current_exe().map_err(|error| {
        ConduitosError::refusal("screen-free-three-host-xtask", error.to_string())
    })?;
    let mut command = Command::new("node");
    command.current_dir(root).arg(script).arg(current);
    for path in [
        &args.owner,
        &args.owner_state,
        &args.handbook,
        &args.build,
        &args.host_profile,
        &args.route_tls_cert,
        &args.route_tls_key,
    ] {
        command.arg(path);
    }
    command
        .arg(args.owner_forward.to_string())
        .arg(&args.route_url)
        .arg(&args.output_dir)
        .arg(&args.playwright)
        .arg(&args.body_name);
    command
        .arg(args.speaker_card.as_deref().unwrap_or("-"))
        .arg(
            args.speaker_device
                .map_or_else(|| "-".into(), |device| device.to_string()),
        );
    for path in [
        &args.speech_executable,
        &args.speech_data,
        &args.speech_engine,
        &args.speech_language_coverage,
    ] {
        if let Some(path) = path {
            command.arg(path);
        } else {
            command.arg("-");
        }
    }
    command.arg(args.model.as_deref().unwrap_or("-"));
    if args.model.is_some() {
        command
            .arg(&args.ollama_endpoint)
            .arg(args.admitted_memory_mib.to_string());
    }
    let status = command.status().map_err(|error| {
        ConduitosError::refusal("screen-free-three-host-launch", error.to_string())
    })?;
    if !status.success() {
        return Err(ConduitosError::refusal(
            "screen-free-three-host-failed",
            format!("installed screen-free journey exited with {status}"),
        ));
    }
    Ok(())
}
