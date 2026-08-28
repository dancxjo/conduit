use std::{fs, path::Path};

use conduit_body_fabrication::build_body_spores;
use conduit_host_fabrication::{
    build_default_host_image, check_host_configuration, parse_host_configuration_conduit,
    BuildInputs, CheckedHostConfiguration, SporeOutputKind,
};

use crate::cli::{BodyCommand, HostCommand};

pub(crate) fn host(command: HostCommand) -> Result<(), String> {
    match command {
        HostCommand::Check { source } => {
            let checked = load_host(&source)?;
            println!(
                "CHECKED {} ({})",
                source.display(),
                checked.configuration_id()
            );
            Ok(())
        }
        HostCommand::Show { source } => {
            let checked = load_host(&source)?;
            let profile = checked.profile();
            println!("Host configuration: {}", checked.configuration_id());
            println!("target: {}", profile.target.key());
            println!("bases: {:?}", checked.resolved_bases());
            println!("bounds: {:?}", profile.bounds);
            Ok(())
        }
        HostCommand::Build { source, output } => {
            let checked = load_host(&source)?;
            let source_identity = format!("configuration:{}", checked.configuration_id());
            let (image, bytes) = build_default_host_image(
                checked.into_profile(),
                &conduit_workspace_fabrication::catalog(),
                &conduit_workspace_fabrication::package_set(),
                &BuildInputs {
                    source_identity,
                    toolchain_available: true,
                },
            )
            .map_err(|diagnostics| format!("Host BUILD refused: {diagnostics:?}"))?;
            if matches!(
                image.manifest.output,
                SporeOutputKind::DiskImage | SporeOutputKind::EfiArtifact | SporeOutputKind::Uf2
            ) {
                return Err(format!(
                    "target {} requires its guarded repository fabrication adapter; use cargo xtask host build",
                    image.manifest.target
                ));
            }
            fs::create_dir_all(&output).map_err(|error| error.to_string())?;
            fs::write(output.join("image.json"), bytes).map_err(|error| error.to_string())?;
            fs::write(
                output.join("build-manifest.json"),
                serde_json::to_vec_pretty(&image.manifest).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
            println!(
                "BUILT {} ({:?})\nIMAGE: {}\nmanifest: {}",
                image.manifest.image_id,
                image.manifest.output,
                output.join("image.json").display(),
                output.join("build-manifest.json").display()
            );
            Ok(())
        }
    }
}

pub(crate) fn body(command: BodyCommand) -> Result<(), String> {
    match command {
        BodyCommand::Check { source } => {
            let checked = crate::body_product::load(&source)?;
            println!(
                "CHECKED {} ({}) hosts={}",
                checked.description().body.id,
                checked.description_id(),
                checked.hosts().len()
            );
            Ok(())
        }
        BodyCommand::Show { source } => {
            let checked = crate::body_product::load(&source)?;
            println!("Body: {}", checked.description().body.id);
            println!("description: {}", checked.description_id());
            for host in checked.hosts() {
                println!(
                    "host {} target={} configuration={} join={:?} output={:?}",
                    host.description.name,
                    host.configuration.profile().target.key(),
                    host.configuration.configuration_id(),
                    host.description.spore.join_mode,
                    host.description.spore.output
                );
            }
            Ok(())
        }
        BodyCommand::Build { source, output } => {
            let checked = crate::body_product::load(&source)?;
            let spores = build_body_spores(
                &checked,
                None,
                checked.description_id(),
                &conduit_workspace_fabrication::catalog(),
                &conduit_workspace_fabrication::package_set(),
            )
            .map_err(|diagnostics| format!("Body BUILD refused: {diagnostics:?}"))?;
            for spore in &spores {
                let host_output = output.join(&spore.manifest.host_entry_name);
                fs::create_dir_all(&host_output).map_err(|error| error.to_string())?;
                fs::write(host_output.join("image.json"), &spore.image_bytes)
                    .map_err(|error| error.to_string())?;
                fs::write(
                    host_output.join("spore-manifest.json"),
                    serde_json::to_vec_pretty(&spore.manifest)
                        .map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?;
                println!(
                    "BUILT Spore {} image={} package={}",
                    spore.manifest.spore_id,
                    spore.manifest.image_id,
                    spore.manifest.fabrication.fabrication_package_id
                );
            }
            Ok(())
        }
    }
}

fn load_host(path: &Path) -> Result<CheckedHostConfiguration, String> {
    if !path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".host.conduit"))
    {
        return Err(format!(
            "Host construction source must use the canonical .host.conduit suffix: {}",
            path.display()
        ));
    }
    let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let configuration = parse_host_configuration_conduit(&source)
        .map_err(|diagnostic| format!("Host configuration decode refused: {diagnostic:?}"))?;
    check_host_configuration(
        configuration,
        &conduit_workspace_fabrication::catalog(),
        &conduit_workspace_fabrication::package_set(),
    )
    .map_err(|diagnostics| format!("Host configuration refused: {diagnostics:?}"))
}
