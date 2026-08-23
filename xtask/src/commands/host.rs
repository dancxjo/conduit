use std::{fs, path::PathBuf};

use clap::{Args, Subcommand};
use conduit_host_fabrication::{
    build_host_image, check_host_configuration, parse_host_configuration, BuildInputs,
    FabricationCatalog, HostBounds, HostProfile,
};

use crate::cli::GlobalOpts;

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
#[path = "host_target.rs"]
mod host_target;

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
    /// Create or revise one durable Host configuration TOML interactively.
    Configure {
        /// Existing configuration to edit, or destination offered when creating.
        path: Option<PathBuf>,
    },
    /// Check or display one durable Host configuration TOML.
    Config {
        #[command(subcommand)]
        command: HostConfigCommand,
    },
    /// Resolve one PROFILE and emit its exact IMAGE and build manifest.
    Build {
        profile: PathBuf,
        #[arg(long, default_value = "target/host-build")]
        output: PathBuf,
        /// Exact source identity; defaults to the current Git commit.
        #[arg(long)]
        source_identity: Option<String>,
        /// Exact toolchain identity; defaults to `rustc --version --verbose`.
        #[arg(long)]
        toolchain_identity: Option<String>,
    },
    /// Verify one final target IMAGE and its exact BUILD closure.
    Verify {
        output: PathBuf,
        /// Boot the verified IMAGE through the deterministic x86_64 QEMU appliance.
        #[arg(long)]
        boot: bool,
    },
    /// Prove one Body across native, browser, and headless PROFILE-built Hosts.
    Capstone {
        #[arg(long, default_value = "target/host-fabrication-capstone")]
        output: PathBuf,
        #[arg(long, default_value = "workspace-head")]
        source_identity: String,
        #[arg(long, default_value = "rustc:workspace")]
        toolchain_identity: String,
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
    },
    /// Initialize and warm one already-local Ollama model under finite Host limits.
    ProveLocalModel {
        /// Exact local model name or its local `:latest` alias.
        #[arg(long)]
        model: String,
        /// Finite admitted RAM/VRAM ceiling expressed in MiB.
        #[arg(long)]
        admitted_memory_mib: u32,
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
    /// Validate without saving or fabricating an IMAGE.
    Check { path: PathBuf },
    /// Print the resolved target, Bases, variants, limits, and identity.
    Show { path: PathBuf },
}

pub fn run(args: HostArgs, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    match args.command.unwrap_or(HostCommand::Std) {
        HostCommand::Std => super::demo::run_std(opts),
        HostCommand::Browser => super::browser::run(opts),
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
        HostCommand::Build {
            profile: profile_path,
            output,
            source_identity,
            toolchain_identity,
        } => {
            let source_identity = source_identity
                .map(Ok)
                .unwrap_or_else(|| command_identity("git", &["rev-parse", "HEAD"]))?;
            let toolchain_identity = toolchain_identity
                .map(Ok)
                .unwrap_or_else(|| command_identity("rustc", &["--version", "--verbose"]))?;
            let source = fs::read_to_string(&profile_path)?;
            let profile = if profile_path
                .extension()
                .is_some_and(|extension| extension == "toml")
            {
                let configuration = parse_host_configuration(&source).map_err(|diagnostic| {
                    format!("Host configuration decode refused: {diagnostic:?}")
                })?;
                check_host_configuration(configuration, &FabricationCatalog::canonical())
                    .map_err(|diagnostics| format!("Host configuration refused: {diagnostics:?}"))?
                    .into_profile()
            } else {
                serde_json::from_str::<HostProfile>(&source)?
            };
            let maxima = repository_target_maxima(&profile)?;
            let inputs = BuildInputs {
                source_identity,
                toolchain_identity,
                toolchain_available: true,
                maxima,
            };
            let (image, bytes) =
                build_host_image(profile, &FabricationCatalog::canonical(), &inputs)
                    .map_err(|diagnostics| format!("Host BUILD refused: {diagnostics:?}"))?;
            if opts.dry_run {
                println!(
                    "would BUILD {} from resolved binding {}",
                    profile_path.display(),
                    image.manifest.image_id
                );
            }
            if matches!(
                image.manifest.target.as_str(),
                "conduitos/x86_64/pc" | "conduitos/aarch64/virt"
            ) {
                let target = host_target::build_target(&image, &bytes, &output, opts)?;
                if opts.json {
                    println!("{}", serde_json::to_string(&target)?);
                } else if !opts.quiet {
                    println!("BUILT {} ({:?})", target.image_id, image.manifest.image_use);
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
                        image.manifest.image_id, image.manifest.image_use
                    );
                    println!("IMAGE: {}", output.join("image.json").display());
                    println!("manifest: {}", output.join("build-manifest.json").display());
                }
            }
            Ok(())
        }
        HostCommand::Capstone {
            output,
            source_identity,
            toolchain_identity,
        } => host_capstone::run(&output, &source_identity, &toolchain_identity, opts),
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
        HostCommand::InspectLocalModel { model } => host_local_model::inspect(&model, opts),
        HostCommand::ProveLocalModel {
            model,
            admitted_memory_mib,
        } => host_local_model::prove(&model, admitted_memory_mib, opts),
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
) -> Result<conduit_host_fabrication::CheckedHostConfiguration, Box<dyn std::error::Error>> {
    let source = fs::read_to_string(path)?;
    let configuration = parse_host_configuration(&source)
        .map_err(|diagnostic| format!("Host configuration decode refused: {diagnostic:?}"))?;
    check_host_configuration(configuration, &FabricationCatalog::canonical())
        .map_err(|diagnostics| format!("Host configuration refused: {diagnostics:?}").into())
}

fn repository_target_maxima(
    profile: &HostProfile,
) -> Result<HostBounds, Box<dyn std::error::Error>> {
    let limits = match (
        profile.target.family.as_str(),
        profile.target.machine.as_str(),
    ) {
        ("conduitos", "pico-w") => HostBounds {
            static_memory_bytes: 2 * 1024 * 1024,
            heap_arena_bytes: 256 * 1024,
            queue_items: 512,
            buffered_bytes: 512 * 1024,
            active_instances: 64,
            operation_slots: 64,
            timer_slots: 32,
            line_sessions: 8,
            evidence_items: 512,
        },
        ("conduitos", "pc" | "virt") => hosted_limits(512 * 1024 * 1024),
        ("std", _) | ("browser", _) => hosted_limits(2 * 1024 * 1024 * 1024),
        _ => {
            return Err(
                format!("no BUILD ceiling table for target {}", profile.target.key()).into(),
            )
        }
    };
    Ok(limits)
}

fn hosted_limits(memory: u64) -> HostBounds {
    HostBounds {
        static_memory_bytes: memory,
        heap_arena_bytes: memory,
        queue_items: 1_048_576,
        buffered_bytes: memory,
        active_instances: 65_536,
        operation_slots: 65_536,
        timer_slots: 65_536,
        line_sessions: 65_536,
        evidence_items: 1_048_576,
    }
}
