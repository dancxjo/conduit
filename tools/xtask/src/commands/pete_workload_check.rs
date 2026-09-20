//! Non-actuating check of Pete's reviewed workload and reusable Host closure.

use std::{fs, process::Command};

use conduit_host_fabrication::{
    build_default_host_image, check_host_configuration, parse_host_configuration_conduit,
    BuildInputs,
};
use serde::Serialize;

use crate::{cli::GlobalOpts, workspace::workspace_root};

const SCHEMA: &str = "conduit.pete/workload-closure@1";

#[derive(Serialize)]
struct WorkloadClosureReport {
    schema: &'static str,
    proof_class: &'static str,
    physical_access: bool,
    authority_acquired: bool,
    planning_closure_proven: bool,
    source_identity: String,
    initial_workload_revision: u64,
    initial_forms: Vec<FormReport>,
    later_navigation_form: String,
    hosts: Vec<HostReport>,
}

#[derive(Serialize)]
struct FormReport {
    role: &'static str,
    source_document_id: String,
    checked_form_id: String,
    required_kinds: Vec<String>,
    may_request_motion: bool,
}

#[derive(Serialize)]
struct HostReport {
    role: &'static str,
    required_at_birth: bool,
    intended_workload_roles: Vec<&'static str>,
    target_owned_configuration: &'static str,
    configuration_id: String,
    build_id: String,
    image_id: String,
    target: String,
    bases: Vec<String>,
    implementations: Vec<String>,
    host_operations: Vec<String>,
    resources: Vec<String>,
    resource_budgets: serde_json::Value,
    bounds: serde_json::Value,
}

pub fn run(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    if opts.dry_run {
        if !opts.quiet {
            println!(
                "would check Pete's reviewed forms and target-owned Host profiles without accessing physical devices"
            );
        }
        return Ok(());
    }
    let workload = conduit_pete::reviewed_pete_workload()
        .map_err(|error| format!("Pete workload refused: {error:?}"))?;
    let source_identity = git_identity(&root)?;
    let catalog = conduit_workspace_fabrication::catalog();
    let packages = conduit_workspace_fabrication::package_set();
    let mut hosts = Vec::new();
    for requirement in conduit_pete::reviewed_pete_host_requirements() {
        let path = root.join(requirement.target_owned_configuration);
        let source = fs::read_to_string(&path)?;
        let configuration = parse_host_configuration_conduit(&source).map_err(|error| {
            format!(
                "decode Pete Host configuration {}: {error:?}",
                path.display()
            )
        })?;
        let checked =
            check_host_configuration(configuration, &catalog, &packages).map_err(|errors| {
                format!(
                    "check Pete Host configuration {}: {errors:?}",
                    path.display()
                )
            })?;
        let configuration_id = checked.configuration_id().to_owned();
        let (image, _) = build_default_host_image(
            checked.into_profile(),
            &catalog,
            &packages,
            &BuildInputs {
                source_identity: source_identity.clone(),
                toolchain_available: true,
            },
        )
        .map_err(|errors| format!("build Pete Host manifest {}: {errors:?}", path.display()))?;
        hosts.push(HostReport {
            role: host_role(requirement.role),
            required_at_birth: requirement.required_at_birth,
            intended_workload_roles: requirement
                .contributes_to
                .iter()
                .copied()
                .map(workload_role)
                .collect(),
            target_owned_configuration: requirement.target_owned_configuration,
            configuration_id,
            build_id: image.manifest.build_id,
            image_id: image.manifest.image_id,
            target: image.manifest.target,
            bases: image.manifest.bases,
            implementations: image.manifest.implementations,
            host_operations: image.manifest.host_operations,
            resources: image.manifest.resources,
            resource_budgets: serde_json::to_value(image.manifest.resource_budgets)?,
            bounds: serde_json::to_value(image.manifest.bounds)?,
        });
    }
    let initial_forms = workload
        .resident_forms
        .iter()
        .filter(|item| workload.initial.contains(&item.form))
        .map(|item| FormReport {
            role: workload_role(item.role),
            source_document_id: item.form.source_document_id.as_str().to_owned(),
            checked_form_id: item.form.checked_form_id.as_str().to_owned(),
            required_kinds: item
                .required_kinds
                .iter()
                .map(|kind| kind.as_str().to_owned())
                .collect(),
            may_request_motion: item.may_request_motion,
        })
        .collect();
    let report = WorkloadClosureReport {
        schema: SCHEMA,
        proof_class: "configuration-check",
        physical_access: false,
        authority_acquired: false,
        planning_closure_proven: false,
        source_identity,
        initial_workload_revision: 0,
        initial_forms,
        later_navigation_form: workload.navigation.checked_form_id.as_str().to_owned(),
        hosts,
    };
    if opts.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else if !opts.quiet {
        println!(
            "Pete workload closure checked: {} initial Forms, {} Host roles; no physical access or authority",
            report.initial_forms.len(),
            report.hosts.len()
        );
    }
    Ok(())
}

fn git_identity(root: &std::path::Path) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err("cannot derive Pete workload source identity".into());
    }
    let revision = String::from_utf8(output.stdout)?.trim().to_owned();
    if revision.is_empty() {
        return Err("Pete workload source identity is empty".into());
    }
    Ok(format!("git:{revision}"))
}

const fn host_role(role: conduit_pete::PeteHostRole) -> &'static str {
    match role {
        conduit_pete::PeteHostRole::Forebrain => "forebrain",
        conduit_pete::PeteHostRole::Motherbrain => "motherbrain",
        conduit_pete::PeteHostRole::Brainstem => "brainstem",
        conduit_pete::PeteHostRole::OptionalBrowser => "optional-browser",
    }
}

const fn workload_role(role: conduit_pete::PeteWorkloadRole) -> &'static str {
    match role {
        conduit_pete::PeteWorkloadRole::Situation => "situation",
        conduit_pete::PeteWorkloadRole::AutobiographicalMemory => "autobiographical-memory",
        conduit_pete::PeteWorkloadRole::HistoricalIndex => "historical-index",
        conduit_pete::PeteWorkloadRole::Conversation => "conversation",
        conduit_pete::PeteWorkloadRole::Navigation => "navigation",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_report_is_explicitly_non_actuating() {
        let opts = GlobalOpts {
            dry_run: false,
            quiet: true,
            json: false,
            locked: false,
        };
        run(&opts).unwrap();
    }
}
