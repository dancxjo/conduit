use std::{
    fs,
    path::{Path, PathBuf},
};

use clap::{Args, Subcommand};
use conduit_host_make::{
    build_default_host_image, check_host_configuration, make_chooser_catalog,
    parse_host_configuration_conduit, BuildInputs, HostImage,
};

use crate::cli::GlobalOpts;

#[path = "host_browser_sdk_package.rs"]
mod host_browser_sdk_package;
#[path = "host_capstone.rs"]
mod host_capstone;
#[path = "host_configurator.rs"]
mod host_configurator;
#[path = "host_esp32_inspection.rs"]
mod host_esp32_inspection;
#[cfg(test)]
#[path = "host_esp32_inspection_tests.rs"]
mod host_esp32_inspection_tests;
#[path = "host_local_model.rs"]
mod host_local_model;
#[path = "host_local_model_journey.rs"]
mod host_local_model_journey;
#[path = "host_microphone.rs"]
mod host_microphone;
#[path = "host_release.rs"]
mod host_release;
#[path = "host_release_catalog.rs"]
mod host_release_catalog;
#[path = "host_spoken_birth.rs"]
mod host_spoken_birth;
#[path = "host_target.rs"]
pub(crate) mod host_target;
#[path = "host_whisper.rs"]
mod host_whisper;

#[derive(Args, Debug)]
pub struct HostArgs {
    #[command(subcommand)]
    command: Option<HostCommand>,
}

#[derive(Subcommand, Debug)]
enum HostCommand {
    /// Launch the ordinary std Host (the default Host target).
    Std,
    /// Build and launch one independent browser page/WASM Host.
    Browser,
    /// Build, flash, or physically verify an exact Raspberry Pi Host IMAGE.
    Rpi(RpiHostArgs),
    /// Create or revise one canonical Host construction document interactively.
    Configure {
        /// Existing configuration to edit, or destination offered when creating.
        path: Option<PathBuf>,
    },
    /// Check or display one canonical Host construction document.
    Config {
        #[command(subcommand)]
        command: HostConfigCommand,
    },
    /// Emit the checked target and Base chooser catalog as portable JSON.
    Catalog,
    /// Resolve one PROFILE and emit its exact IMAGE and build manifest.
    Build {
        profile: PathBuf,
        #[arg(long, default_value = "target/host-build")]
        output: PathBuf,
        /// Exact source identity; defaults to the current Git commit.
        #[arg(long)]
        source_identity: Option<String>,
    },
    /// Compile and seal the reviewed generic existing-computer release bundles.
    Release {
        #[arg(long, default_value = "target/creche-host-releases")]
        output: PathBuf,
        /// Native platform to compile on this runner.
        #[arg(long, value_enum, default_value_t = host_release::ReleasePlatform::Linux)]
        platform: host_release::ReleasePlatform,
        /// Exact source identity; defaults to the current Git commit.
        #[arg(long)]
        source_identity: Option<String>,
    },
    /// Seal reviewed release manifests into the installed target catalog.
    ReleaseCatalog {
        /// Directory containing reviewed release manifests and their artifacts.
        #[arg(long)]
        root: PathBuf,
        /// Monotonically increasing release-channel generation.
        #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
        generation: u64,
    },
    /// Package one exact reviewed BrowserBundle for the zero-dependency ESM SDK.
    BrowserSdkPackage {
        /// Directory containing the sealed BrowserBundle and IMAGE.
        #[arg(long)]
        bundle: PathBuf,
        /// New output directory for the npm-style package.
        #[arg(long)]
        output: PathBuf,
    },
    /// Verify one final target IMAGE and its exact BUILD closure.
    Verify {
        output: PathBuf,
        /// Boot the verified IMAGE through the deterministic x86_64 QEMU appliance.
        #[arg(long)]
        boot: bool,
    },
    /// Prove one body across native, browser, and headless PROFILE-built Hosts.
    Capstone {
        #[arg(long, default_value = "target/host-make-capstone")]
        output: PathBuf,
        #[arg(long, default_value = "workspace-head")]
        source_identity: String,
    },
    /// Inspect one attached ESP32 without writing its flash.
    InspectEsp32 {
        /// Stable serial device path, preferably beneath /dev/serial/by-id.
        #[arg(long)]
        port: PathBuf,
        /// Exact SoC class expected from the attached board.
        #[arg(long, value_enum)]
        expected_soc: host_esp32_inspection::Esp32SocClass,
        /// Literal text observed on the development-board PCB.
        #[arg(long)]
        board_marking: String,
        /// Literal text observed on the module's RF shield.
        #[arg(long)]
        module_marking: String,
        /// Literal board revision, or `unmarked` when inspection finds none.
        #[arg(long)]
        board_revision: String,
        #[arg(long, default_value = "target/esp32-inspection/inspection.json")]
        output: PathBuf,
    },
    /// Inspect one already-local Ollama model without loading or downloading it.
    InspectLocalModel {
        /// Exact local model name or its local `:latest` alias.
        #[arg(long)]
        model: String,
        /// Exact loopback Ollama origin.
        #[arg(long, default_value = conduit_std_host::hosted_local_model::DEFAULT_OLLAMA_ENDPOINT)]
        ollama_endpoint: String,
    },
    /// Initialize and warm one already-local Ollama model under finite Host limits.
    ProveLocalModel {
        /// Exact local model name or its local `:latest` alias.
        #[arg(long)]
        model: String,
        /// Exact loopback Ollama origin.
        #[arg(long, default_value = conduit_std_host::hosted_local_model::DEFAULT_OLLAMA_ENDPOINT)]
        ollama_endpoint: String,
        /// Finite admitted RAM/VRAM ceiling expressed in MiB.
        #[arg(long)]
        admitted_memory_mib: u32,
        /// Run the fixed-policy Orifina comparison over one exact Workspace tutorial state.
        #[arg(long)]
        orifina_presenter: bool,
        /// Retain every tutorial-state manifestation for an explicit documentary run.
        #[arg(long, requires = "orifina_presenter")]
        journey_documentary: bool,
    },
    /// Speak through Tongues as Host, perform one Birth, then speak as the Body.
    ProveSpokenBirth {
        /// Explicitly consume the one allowed Birth action.
        #[arg(long)]
        confirm_birth: bool,
        #[arg(long, default_value = "No Body exists yet. Confirm to begin.")]
        bootstrap_text: String,
        #[arg(long, default_value = "I am now speaking as the born Body.")]
        body_text: String,
    },
    /// Carry a finite recorded PCM clip through ordinary Whisper Plan/Play.
    ProveWhisper {
        /// Exact local whisper.cpp-compatible executable.
        #[arg(long)]
        executable: PathBuf,
        /// Exact already-local Whisper model file.
        #[arg(long)]
        model: PathBuf,
        /// Raw mono signed-16-le 16 kHz PCM recording, at most six seconds.
        #[arg(long)]
        pcm_s16le_16000_mono: PathBuf,
        #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u8).range(1..=32))]
        threads: u8,
        #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u64).range(1..=120))]
        timeout_seconds: u64,
    },
    /// Explicitly capture one bounded microphone clip and recognize it through Whisper.
    ProveMicrophoneWhisper {
        #[arg(long)]
        arecord_executable: PathBuf,
        #[arg(long)]
        card_id: String,
        #[arg(long)]
        device: u16,
        #[arg(long, default_value_t = 3_000)]
        capture_milliseconds: u32,
        #[arg(long, default_value_t = 10)]
        capture_timeout_seconds: u64,
        #[arg(long)]
        whisper_executable: PathBuf,
        #[arg(long)]
        whisper_model: PathBuf,
        #[arg(long, default_value_t = 2)]
        whisper_threads: u8,
        #[arg(long, default_value_t = 30)]
        whisper_timeout_seconds: u64,
        #[arg(long)]
        authorize_capture: bool,
    },
}

#[derive(Args, Debug)]
struct RpiHostArgs {
    /// Exact currently supported Raspberry Pi board profile.
    #[arg(long, value_enum, default_value_t = super::conduitos::Armv6RpiBoard::default())]
    board: super::conduitos::Armv6RpiBoard,

    #[command(subcommand)]
    action: Option<RpiHostAction>,
}

#[derive(Subcommand, Debug)]
enum RpiHostAction {
    /// Generate and independently verify the exact SD-card IMAGE (default).
    Image,
    /// Erase, write, and byte-verify one explicitly confirmed removable device.
    Flash {
        /// Exact whole removable block device to erase and write.
        #[arg(long)]
        device: PathBuf,
        /// Repeat the exact device path to acknowledge destructive erasure.
        #[arg(long)]
        confirm_device: PathBuf,
    },
    /// Capture and validate one exact physical Raspberry Pi UART boot.
    PhysicalProof {
        /// Exact UART character device connected through a 3.3 V TTL adapter.
        #[arg(long)]
        serial_device: PathBuf,
        /// Finite capture deadline in seconds.
        #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u64).range(1..=120))]
        timeout_seconds: u64,
    },
}

#[derive(Subcommand, Debug)]
enum HostConfigCommand {
    /// Validate canonical source without saving or making an IMAGE.
    Check { path: PathBuf },
    /// Print the resolved target, Bases, variants, limits, and identity.
    Show { path: PathBuf },
}

pub fn run(args: HostArgs, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    match args.command.unwrap_or(HostCommand::Std) {
        HostCommand::Std => super::demo::run_std(opts),
        HostCommand::Browser => super::browser::run(opts),
        HostCommand::ProveSpokenBirth {
            confirm_birth,
            bootstrap_text,
            body_text,
        } => host_spoken_birth::prove(confirm_birth, &bootstrap_text, &body_text, opts),
        HostCommand::ProveWhisper {
            executable,
            model,
            pcm_s16le_16000_mono,
            threads,
            timeout_seconds,
        } => host_whisper::prove(
            host_whisper::WhisperProofRequest {
                executable,
                model,
                pcm_s16le_16000_mono,
                threads,
                timeout_seconds,
            },
            opts,
        ),
        HostCommand::ProveMicrophoneWhisper {
            arecord_executable,
            card_id,
            device,
            capture_milliseconds,
            capture_timeout_seconds,
            whisper_executable,
            whisper_model,
            whisper_threads,
            whisper_timeout_seconds,
            authorize_capture,
        } => host_microphone::prove(
            host_microphone::MicrophoneWhisperRequest {
                arecord_executable,
                card_id,
                device,
                capture_milliseconds,
                capture_timeout_seconds,
                whisper_executable,
                whisper_model,
                whisper_threads,
                whisper_timeout_seconds,
                authorize_capture,
            },
            opts,
        ),
        HostCommand::Rpi(args) => match args.action.unwrap_or(RpiHostAction::Image) {
            RpiHostAction::Image => {
                super::conduitos::build_rpi_image(args.board, opts).map_err(Into::into)
            }
            RpiHostAction::Flash {
                device,
                confirm_device,
            } => super::conduitos::flash_rpi_image(args.board, &device, &confirm_device, opts)
                .map_err(Into::into),
            RpiHostAction::PhysicalProof {
                serial_device,
                timeout_seconds,
            } => super::conduitos::prove_physical_rpi(
                args.board,
                &serial_device,
                timeout_seconds,
                opts,
            )
            .map_err(Into::into),
        },
        HostCommand::Configure { path } => host_configurator::run(path.as_deref(), opts),
        HostCommand::Config { command } => {
            let path = match &command {
                HostConfigCommand::Check { path } | HostConfigCommand::Show { path } => path,
            };
            let checked = load_configuration(path)?;
            match command {
                HostConfigCommand::Check { .. } => {
                    if opts.json {
                        println!(
                            "{{\"configuration_id\":{:?},\"valid\":true}}",
                            checked.configuration_id()
                        );
                    } else if !opts.quiet {
                        println!(
                            "CHECKED {} ({})",
                            path.display(),
                            checked.configuration_id()
                        );
                    }
                }
                HostConfigCommand::Show { .. } => host_configurator::print_summary(&checked, opts)?,
            }
            Ok(())
        }
        HostCommand::Catalog => {
            let catalog = make_chooser_catalog(&conduit_workspace_make::package_set());
            println!("{}", serde_json::to_string_pretty(&catalog)?);
            Ok(())
        }
        HostCommand::Build {
            profile: profile_path,
            output,
            source_identity,
        } => {
            let source_identity = source_identity
                .map(Ok)
                .unwrap_or_else(|| command_identity("git", &["rev-parse", "HEAD"]))?;
            let (image, bytes) = resolve_profile(&profile_path, source_identity)?;
            if opts.dry_run {
                println!(
                    "would BUILD {} from resolved binding {}",
                    profile_path.display(),
                    image.manifest.image_id
                );
            }
            if crate::commands::conduitos::target_backend::find(&image.manifest.target).is_some() {
                let target = host_target::build_target(&image, &bytes, &output, opts)?;
                if opts.json {
                    println!("{}", serde_json::to_string(&target)?);
                } else if !opts.quiet {
                    println!("BUILT {} ({:?})", target.image_id, image.manifest.output);
                    println!("IMAGE: {}", output.join(&target.image.file).display());
                    println!("manifest: {}", output.join("build-manifest.json").display());
                }
            } else if !opts.dry_run {
                fs::create_dir_all(&output)?;
                fs::write(output.join("image.json"), &bytes)?;
                fs::write(
                    output.join("build-manifest.json"),
                    serde_json::to_vec_pretty(&image.manifest)?,
                )?;
                if opts.json {
                    println!("{}", serde_json::to_string(&image.manifest)?);
                } else if !opts.quiet {
                    println!(
                        "BUILT {} ({:?})",
                        image.manifest.image_id, image.manifest.output
                    );
                    println!("IMAGE: {}", output.join("image.json").display());
                    println!("manifest: {}", output.join("build-manifest.json").display());
                }
            }
            Ok(())
        }
        HostCommand::Release {
            output,
            platform,
            source_identity,
        } => {
            let source_identity = source_identity
                .map(Ok)
                .unwrap_or_else(|| command_identity("git", &["rev-parse", "HEAD"]))?;
            host_release::run(
                &output,
                platform,
                &source_identity,
                &host_release::ReleaseOptions {
                    json: opts.json,
                    quiet: opts.quiet,
                },
            )
        }
        HostCommand::ReleaseCatalog { root, generation } => {
            host_release_catalog::run(&root, generation, opts.dry_run, opts.json, opts.quiet)
        }
        HostCommand::BrowserSdkPackage { bundle, output } => {
            host_browser_sdk_package::run(&bundle, &output, opts)
        }
        HostCommand::Capstone {
            output,
            source_identity,
        } => host_capstone::run(&output, &source_identity, opts),
        HostCommand::InspectEsp32 {
            port,
            expected_soc,
            board_marking,
            module_marking,
            board_revision,
            output,
        } => host_esp32_inspection::run(
            &port,
            expected_soc,
            &board_marking,
            &module_marking,
            &board_revision,
            &output,
            opts,
        ),
        HostCommand::InspectLocalModel {
            model,
            ollama_endpoint,
        } => host_local_model::inspect(&model, &ollama_endpoint, opts),
        HostCommand::ProveLocalModel {
            model,
            ollama_endpoint,
            admitted_memory_mib,
            orifina_presenter,
            journey_documentary,
        } => host_local_model::prove(
            &model,
            &ollama_endpoint,
            admitted_memory_mib,
            orifina_presenter,
            journey_documentary,
            opts,
        ),
        HostCommand::Verify { output, boot } => {
            let manifest = host_target::verify_target(&output)?;
            if boot {
                host_target::boot_target(&output, &manifest, opts)?;
            }
            if opts.json {
                println!("{}", serde_json::to_string(&manifest)?);
            } else if !opts.quiet {
                println!("VERIFIED {}", manifest.image_id);
            }
            Ok(())
        }
    }
}

pub(crate) fn build_conduitos_live(
    profile_path: &Path,
    output: &std::path::Path,
    opts: &GlobalOpts,
) -> Result<host_target::TargetBuildManifest, Box<dyn std::error::Error>> {
    let source_identity = command_identity("git", &["rev-parse", "HEAD"])?;
    let (image, bytes) = resolve_profile(profile_path, source_identity)?;
    if crate::commands::conduitos::target_backend::find(&image.manifest.target).is_none() {
        return Err(format!(
            "{} does not resolve to a bootable ConduitOS product target",
            image.manifest.target
        )
        .into());
    }
    host_target::build_target(&image, &bytes, output, opts)
}

fn resolve_profile(
    profile_path: &std::path::Path,
    source_identity: String,
) -> Result<(HostImage, Vec<u8>), Box<dyn std::error::Error>> {
    let source = fs::read_to_string(profile_path)?;
    if !is_conduit_source(profile_path, "host") {
        return Err(format!(
            "Host construction source must use the canonical .host.conduit suffix: {}",
            profile_path.display()
        )
        .into());
    }
    let configuration = parse_host_configuration_conduit(&source)
        .map_err(|diagnostic| format!("Host configuration decode refused: {diagnostic:?}"))?;
    let profile = check_host_configuration(
        configuration,
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .map_err(|diagnostics| format!("Host configuration refused: {diagnostics:?}"))?
    .into_profile();
    let inputs = BuildInputs {
        source_identity,
        toolchain_available: true,
    };
    build_default_host_image(
        profile,
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
        &inputs,
    )
    .map_err(|diagnostics| format!("Host BUILD refused: {diagnostics:?}").into())
}

fn command_identity(
    program: &str,
    arguments: &[&str],
) -> Result<String, Box<dyn std::error::Error>> {
    let output = std::process::Command::new(program)
        .args(arguments)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "cannot derive exact identity from {program}: {}",
            output.status
        )
        .into());
    }
    let identity = String::from_utf8(output.stdout)?.trim().to_owned();
    if identity.is_empty() {
        return Err(format!("{program} returned an empty identity").into());
    }
    Ok(identity)
}

fn load_configuration(
    path: &std::path::Path,
) -> Result<conduit_host_make::CheckedHostConfiguration, Box<dyn std::error::Error>> {
    let source = fs::read_to_string(path)?;
    if !is_conduit_source(path, "host") {
        return Err(format!(
            "Host construction source must use the canonical .host.conduit suffix: {}",
            path.display()
        )
        .into());
    }
    let configuration = parse_host_configuration_conduit(&source)
        .map_err(|diagnostic| format!("Host configuration decode refused: {diagnostic:?}"))?;
    check_host_configuration(
        configuration,
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .map_err(|diagnostics| format!("Host configuration refused: {diagnostics:?}").into())
}

fn is_conduit_source(path: &std::path::Path, role: &str) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(&format!(".{role}.conduit")))
}
