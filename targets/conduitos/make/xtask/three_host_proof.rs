//! Correlate native and browser product proofs without constructing their state.

use std::{path::PathBuf, process::Command};

use clap::Args as ClapArgs;

use crate::cli::GlobalOpts;

use super::{profile::Paths, ConduitosArch, ConduitosError};

#[derive(ClapArgs, Debug, Clone)]
pub(super) struct Args {
    /// Product executable named by the installed Linux Host release.
    #[arg(long)]
    owner: PathBuf,
    /// State directory of that installed, running Body owner.
    #[arg(long)]
    owner_state: PathBuf,
    /// Exact-source static Handbook package.
    #[arg(long)]
    handbook: PathBuf,
    /// Private ISO provisioned for that Body and owner route.
    #[arg(long)]
    spore: PathBuf,
    /// Routed candidate ID contained in that ISO.
    #[arg(long)]
    candidate_id: String,
    /// Explicit private IPv4 TLS owner listener.
    #[arg(long)]
    owner_forward: std::net::SocketAddr,
    /// New private directory for correlated browser and QMP evidence.
    #[arg(long)]
    output_dir: PathBuf,
    /// Pinned Playwright module installed for the browser proof project.
    #[arg(long)]
    playwright: PathBuf,
    /// Optional installed eSpeak executable for a same-run direct reading.
    #[arg(long, requires_all = ["speech_data", "speech_engine", "speech_language_coverage"])]
    speech_executable: Option<PathBuf>,
    /// Exact installed eSpeak voice data tree.
    #[arg(long, requires = "speech_executable")]
    speech_data: Option<PathBuf>,
    /// Exact installed eSpeak engine library, not a symlink.
    #[arg(long, requires = "speech_executable")]
    speech_engine: Option<PathBuf>,
    /// Native coverage bound to the exact selected speech provider source.
    #[arg(long, requires = "speech_executable")]
    speech_language_coverage: Option<PathBuf>,
    /// Already-local Ollama model for a same-run validated spoken chapter.
    #[arg(long, requires = "speech_executable")]
    model: Option<String>,
    /// Explicit loopback Ollama origin; the producer uses a private forwarding route.
    #[arg(long, default_value = "http://127.0.0.1:11434")]
    ollama_endpoint: String,
    /// Finite model memory admitted by the existing spoken chapter producer.
    #[arg(long, default_value_t = 2048)]
    admitted_memory_mib: u32,
    /// Private control socket for the installed owner's selected model endpoint.
    #[arg(long, requires = "model")]
    owner_model_route_control: Option<PathBuf>,
}

pub(super) fn execute(args: &Args, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "three-host-proof-requires-live-input",
            "the three-host proof must launch the real browser and QMP guest",
        ));
    }
    let root = Paths::new(ConduitosArch::X86_64)?.root;
    let script = root.join("proof/browser/three-host-owner-journey.mjs");
    for (name, path) in [
        ("script", script.clone()),
        ("owner", args.owner.clone()),
        (
            "handbook",
            args.handbook.join("application.application.json"),
        ),
        ("spore", args.spore.clone()),
        ("playwright", args.playwright.clone()),
        ("installation", args.owner_state.join("installation.json")),
    ] {
        if !path.is_file() {
            return Err(ConduitosError::refusal(
                "three-host-proof-prerequisite",
                format!("{name} is unavailable: {}", path.display()),
            ));
        }
    }
    if args.output_dir.exists() {
        return Err(ConduitosError::refusal(
            "three-host-proof-output-exists",
            "provide a new private evidence directory",
        ));
    }
    if let Some(executable) = &args.speech_executable {
        for (name, path) in [
            ("speech-executable", executable),
            (
                "speech-data",
                args.speech_data
                    .as_ref()
                    .expect("Clap requires speech data"),
            ),
            (
                "speech-engine",
                args.speech_engine
                    .as_ref()
                    .expect("Clap requires speech engine"),
            ),
            (
                "speech-language-coverage",
                args.speech_language_coverage
                    .as_ref()
                    .expect("Clap requires speech Language coverage"),
            ),
        ] {
            if !path.exists() {
                return Err(ConduitosError::refusal(
                    "three-host-proof-speech-prerequisite",
                    format!("{name} is unavailable: {}", path.display()),
                ));
            }
        }
    }
    if args.model.is_some() && args.admitted_memory_mib == 0 {
        return Err(ConduitosError::refusal(
            "three-host-proof-model-memory",
            "model memory admission must be positive",
        ));
    }
    let current = std::env::current_exe()
        .map_err(|error| ConduitosError::refusal("three-host-proof-xtask", error.to_string()))?;
    let mut command = Command::new("node");
    command
        .arg(&script)
        .arg(current)
        .arg(&args.owner)
        .arg(&args.owner_state)
        .arg(&args.handbook)
        .arg(&args.spore)
        .arg(&args.candidate_id)
        .arg(args.owner_forward.to_string())
        .arg(&args.output_dir)
        .arg(&args.playwright);
    if let Some(executable) = &args.speech_executable {
        command
            .arg(executable)
            .arg(
                args.speech_data
                    .as_ref()
                    .expect("Clap requires speech data"),
            )
            .arg(
                args.speech_engine
                    .as_ref()
                    .expect("Clap requires speech engine"),
            )
            .arg(
                args.speech_language_coverage
                    .as_ref()
                    .expect("Clap requires speech Language coverage"),
            );
    }
    if let Some(model) = &args.model {
        command
            .arg(model)
            .arg(&args.ollama_endpoint)
            .arg(args.admitted_memory_mib.to_string())
            .arg(
                args.owner_model_route_control
                    .as_ref()
                    .map_or_else(|| "-".into(), |path| path.display().to_string()),
            );
    }
    let status = command
        .current_dir(&root)
        .status()
        .map_err(|error| ConduitosError::refusal("three-host-proof-launch", error.to_string()))?;
    if !status.success() {
        return Err(ConduitosError::refusal(
            "three-host-proof-failed",
            format!("browser and QMP journey exited with {status}"),
        ));
    }
    Ok(())
}
