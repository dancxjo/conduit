use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use clap::{Args, Subcommand};
use conduit_body_fabrication::{
    build_body_spores, check_body_description, deployment_receipt, parse_body_description_conduit,
    BuiltSpore, CheckedBodyDescription, DeploymentDisposition, SporeJoinMode,
};
use conduit_host_fabrication::{
    parse_host_configuration_conduit, HostConfiguration, SporeOutputKind,
};
use serde::Serialize;

use crate::cli::GlobalOpts;

mod scaffold;

use scaffold::{BodyTemplate, HostAssignment};

#[derive(Args, Debug)]
pub struct BodyArgs {
    #[command(subcommand)]
    command: BodyCommand,
}

#[derive(Subcommand, Debug)]
enum BodyCommand {
    /// Create one checked canonical Body description from repository Host recipes.
    New {
        /// Body name; prompted for in an interactive terminal when omitted.
        name: Option<String>,
        /// Start from a named composition; defaults to minimal unless --host is supplied.
        #[arg(long, value_enum)]
        template: Option<BodyTemplate>,
        /// Add or replace one Host entry as NAME=HOST-CONFIGURATION.
        #[arg(long = "host", value_name = "NAME=CONFIGURATION")]
        hosts: Vec<HostAssignment>,
        /// Destination `.body.conduit` file.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Disable terminal prompts for automation and scripted use.
        #[arg(long)]
        no_interactive: bool,
    },
    /// List the built-in Body compositions and repository Host recipes they use.
    Templates,
    /// Validate a Body description and every referenced Host configuration without artifacts.
    Check { path: PathBuf },
    /// Display the checked Body, target packages, Bases, join modes, and deployment readiness.
    Show { path: PathBuf },
    /// Build one Spore per selected Host through the existing Host IMAGE path.
    Build {
        path: PathBuf,
        #[arg(long)]
        host: Option<String>,
        #[arg(long, default_value = "target/body-build")]
        output: PathBuf,
        #[arg(long)]
        deploy: bool,
    },
    /// Deploy checked, freshly built Spore artifacts through their selected adapters.
    Deploy {
        path: PathBuf,
        #[arg(long)]
        host: Option<String>,
        #[arg(long, default_value = "target/body-build")]
        output: PathBuf,
    },
}

#[derive(Serialize)]
struct BodyReport<'a> {
    body_description_id: &'a str,
    body_id: &'a str,
    hosts: Vec<HostReport<'a>>,
}

#[derive(Serialize)]
struct HostReport<'a> {
    name: &'a str,
    target: String,
    configuration: &'a str,
    configuration_id: &'a str,
    bases: &'a [(String, String)],
    join_mode: &'a SporeJoinMode,
    output: &'a SporeOutputKind,
    fabrication_package: String,
    features: Vec<String>,
    deployment_complete: bool,
}

pub fn run(args: BodyArgs, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    match args.command {
        BodyCommand::New {
            name,
            template,
            hosts,
            output,
            no_interactive,
        } => scaffold::create(
            name.as_deref(),
            template,
            &hosts,
            output.as_deref(),
            no_interactive,
            opts,
        ),
        BodyCommand::Templates => scaffold::list_templates(opts),
        BodyCommand::Check { path } => print_checked(&load(&path)?, opts),
        BodyCommand::Show { path } => show(&load(&path)?, opts),
        BodyCommand::Build {
            path,
            host,
            output,
            deploy,
        } => build(&load(&path)?, host.as_deref(), &output, deploy, opts),
        BodyCommand::Deploy { path, host, output } => {
            build(&load(&path)?, host.as_deref(), &output, true, opts)
        }
    }
}

fn load(path: &Path) -> Result<CheckedBodyDescription, Box<dyn std::error::Error>> {
    let source = fs::read_to_string(path)?;
    if !is_conduit_source(path, "body") {
        return Err(format!(
            "Body construction source must use the canonical .body.conduit suffix: {}",
            path.display()
        )
        .into());
    }
    let description = parse_body_description_conduit(&source)
        .map_err(|item| format!("Body description decode refused: {item:?}"))?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut configurations = BTreeMap::<String, HostConfiguration>::new();
    for host in &description.hosts {
        if configurations.contains_key(&host.configuration) {
            continue;
        }
        let config_path = parent.join(&host.configuration);
        let config_source = match fs::read_to_string(&config_path) {
            Ok(source) => source,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!(
                    "Host configuration {} unavailable: {error}",
                    config_path.display()
                )
                .into())
            }
        };
        if !is_conduit_source(&config_path, "host") {
            return Err(format!(
                "Referenced Host construction source must use the canonical .host.conduit suffix: {}",
                config_path.display()
            )
            .into());
        }
        let configuration = parse_host_configuration_conduit(&config_source).map_err(|item| {
            format!(
                "Host configuration {} decode refused: {item:?}",
                config_path.display()
            )
        })?;
        configurations.insert(host.configuration.clone(), configuration);
    }
    check_body_description(
        description,
        &configurations,
        &conduit_workspace_fabrication::catalog(),
        &conduit_workspace_fabrication::package_set(),
    )
    .map_err(|items| format!("Body description refused: {items:?}").into())
}

fn is_conduit_source(path: &Path, role: &str) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(&format!(".{role}.conduit")))
}

fn report(body: &CheckedBodyDescription) -> BodyReport<'_> {
    let packages = conduit_workspace_fabrication::package_set();
    let hosts = body
        .hosts()
        .iter()
        .map(|host| {
            let selection = packages
                .derive_build_selection(
                    host.configuration.profile(),
                    &host.description.spore.output,
                )
                .expect("checked architecture selection");
            HostReport {
                name: &host.description.name,
                target: host.configuration.profile().target.key(),
                configuration: &host.description.configuration,
                configuration_id: host.configuration.configuration_id(),
                bases: host.configuration.resolved_bases(),
                join_mode: &host.description.spore.join_mode,
                output: &host.description.spore.output,
                fabrication_package: selection.fabrication_package_id,
                features: selection.features,
                deployment_complete: host.description.deployment.is_some()
                    && selection.deployment_adapter.is_some(),
            }
        })
        .collect();
    BodyReport {
        body_description_id: body.description_id(),
        body_id: &body.description().body.id,
        hosts,
    }
}

fn print_checked(
    body: &CheckedBodyDescription,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    if opts.json {
        println!("{}", serde_json::to_string(&report(body))?);
    } else if !opts.quiet {
        println!(
            "CHECKED {} ({}) hosts={}",
            body.description().body.id,
            body.description_id(),
            body.hosts().len()
        );
    }
    Ok(())
}

fn show(
    body: &CheckedBodyDescription,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    let report = report(body);
    if opts.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    if opts.quiet {
        return Ok(());
    }
    println!(
        "Body description: {} ({})",
        body.description().name,
        body.description_id()
    );
    println!("Body: {}", report.body_id);
    println!("\nHosts");
    for host in report.hosts {
        println!("\n{}", host.name);
        println!("  target               {}", host.target);
        println!(
            "  configuration        {} ({})",
            host.configuration, host.configuration_id
        );
        println!("  bases                {:?}", host.bases);
        println!("  join mode            {:?}", host.join_mode);
        println!("  output               {:?}", host.output);
        println!("  fabrication package {}", host.fabrication_package);
        println!("  derived features     {:?}", host.features);
        println!("  deployment complete  {}", host.deployment_complete);
    }
    Ok(())
}

fn build(
    body: &CheckedBodyDescription,
    selected: Option<&str>,
    output: &Path,
    deploy: bool,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    let source_identity = command_identity()?;
    let spores = build_body_spores(
        body,
        selected,
        &source_identity,
        &conduit_workspace_fabrication::catalog(),
        &conduit_workspace_fabrication::package_set(),
    )
    .map_err(|items| format!("Body BUILD refused: {items:?}"))?;
    if opts.dry_run {
        for spore in &spores {
            println!(
                "would BUILD Spore {} with {} features={:?}",
                spore.manifest.host_entry_name,
                spore.manifest.fabrication.fabrication_package_id,
                spore.manifest.fabrication.features
            );
            if deploy {
                let receipt = deployment_receipt(body, spore, DeploymentDisposition::Prepared)
                    .map_err(|item| format!("Spore deployment refused: {item:?}"))?;
                println!(
                    "would PREPARE deployment via {} at {}; does not prove boot/join/presence",
                    receipt.adapter, receipt.destination
                );
            }
        }
        return Ok(());
    }
    let receipts = if deploy {
        spores
            .iter()
            .map(|spore| {
                deployment_receipt(body, spore, DeploymentDisposition::Prepared)
                    .map_err(|item| format!("Spore deployment refused: {item:?}"))
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };
    for spore in &spores {
        write_spore(output, spore)?;
    }
    if deploy {
        for (spore, receipt) in spores.iter().zip(&receipts) {
            let destination = PathBuf::from(&receipt.destination);
            fs::create_dir_all(&destination)?;
            write_new_or_same(
                &destination.join(format!("{}.image.json", spore.manifest.host_entry_name)),
                &spore.image_bytes,
            )?;
            let host_root = output.join(&spore.manifest.host_entry_name);
            write_new_or_same(
                &host_root.join("deployment-receipt.json"),
                &serde_json::to_vec_pretty(&receipt)?,
            )?;
        }
    }
    if opts.json {
        println!(
            "{}",
            serde_json::to_string(&spores.iter().map(|item| &item.manifest).collect::<Vec<_>>())?
        );
    } else if !opts.quiet {
        for spore in &spores {
            println!(
                "BUILT Spore {} image={} package={}",
                spore.manifest.spore_id,
                spore.manifest.image_id,
                spore.manifest.fabrication.fabrication_package_id
            );
        }
        for receipt in receipts {
            println!(
                "PREPARED deployment {} via {} at {}; does not prove boot/join/presence",
                receipt.spore_id, receipt.adapter, receipt.destination
            );
        }
    }
    Ok(())
}

fn write_spore(output: &Path, spore: &BuiltSpore) -> Result<(), Box<dyn std::error::Error>> {
    let root = output.join(&spore.manifest.host_entry_name);
    fs::create_dir_all(&root)?;
    write_new_or_same(&root.join("image.json"), &spore.image_bytes)?;
    write_new_or_same(
        &root.join("build-manifest.json"),
        &serde_json::to_vec_pretty(&spore.image.manifest)?,
    )?;
    write_new_or_same(
        &root.join("spore-manifest.json"),
        &serde_json::to_vec_pretty(&spore.manifest)?,
    )?;
    Ok(())
}

fn write_new_or_same(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() {
        let prior = fs::read(path)?;
        if prior != bytes {
            return Err(format!(
                "refusing to replace different Body-build artifact {}",
                path.display()
            )
            .into());
        }
        return Ok(());
    }
    fs::write(path, bytes)?;
    Ok(())
}

fn command_identity() -> Result<String, Box<dyn std::error::Error>> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()?;
    if !output.status.success() {
        return Err("cannot derive Body BUILD source identity".into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

#[cfg(test)]
mod tests {
    #[test]
    fn check_and_show_remain_descriptor_only_without_target_processes() {
        let source = include_str!("body.rs");
        let descriptor_half = source.split("fn build(").next().unwrap();
        assert!(!descriptor_half.contains("std::process::Command"));
        assert!(!descriptor_half.contains("build_body_spores("));
        assert!(!descriptor_half.contains("fs::write("));
        assert!(!descriptor_half.contains("cargo "));
    }

    #[test]
    fn body_orchestration_names_no_target_sdk_crate() {
        let source = include_str!("../../Cargo.toml");
        for forbidden in ["esp-idf", "embassy-rp", "wasm-bindgen", "arpabet_cmudict"] {
            assert!(!source.contains(forbidden));
        }
    }
}
