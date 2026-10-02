use std::{fs, path::Path};

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

#[path = "host_cli.rs"]
mod host_cli;
pub use host_cli::HostArgs;
use host_cli::{HostCommand, HostConfigCommand, RpiHostAction};
#[path = "host_speech.rs"]
mod host_speech;
#[path = "host_speech_proof.rs"]
mod host_speech_proof;

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
        HostCommand::ProveSpeech(request) => host_speech_proof::prove(request, opts),
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
            speech,
        } => host_local_model::prove(
            &model,
            &ollama_endpoint,
            admitted_memory_mib,
            orifina_presenter,
            journey_documentary,
            &speech,
            opts,
        ),
        HostCommand::Verify {
            output,
            boot,
            journey,
        } => {
            let manifest = host_target::verify_target(&output)?;
            if boot {
                host_target::boot_target(&output, &manifest, opts)?;
            }
            if journey {
                host_target::prove_journey(&output, &manifest, opts)?;
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
