use std::{collections::BTreeMap, io::IsTerminal as _, path::Path, str::FromStr};

use conduit_body_fabrication::{
    canonical_body_description_conduit, check_body_description, BodyBindingTarget, BodyDescription,
    BodyHostDescription, SporeDescription, SporeJoinMode, BODY_DESCRIPTION_SCHEMA,
};
use conduit_host_fabrication::{CheckedHostConfiguration, SporeOutputKind};
use console::{style, Emoji};
use serde::Serialize;

use crate::{cli::GlobalOpts, workspace::workspace_root};

mod catalog;
mod interactive;
mod paths;

pub(super) use catalog::BodyTemplate;
use catalog::{
    compose, join_name, list as template_catalog, load_host_recipes, output_name, HostSeed,
    TemplateCatalogReport, TemplateJoinMode,
};
use paths::{display_path, output_path, path_text, relative_path, write_new};

static CHECK: Emoji<'_, '_> = Emoji("✓", "ok");

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HostAssignment {
    name: String,
    configuration: String,
    output: Option<SporeOutputKind>,
}

impl FromStr for HostAssignment {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (name, configuration) = value
            .split_once('=')
            .ok_or_else(|| "expected NAME=CONFIGURATION".to_owned())?;
        if !is_name(name) {
            return Err("Host entry name must contain only letters, numbers, '_' or '-'".into());
        }
        let configuration = configuration
            .strip_suffix(".host.conduit")
            .unwrap_or(configuration);
        if !is_name(configuration) {
            return Err(
                "Host configuration must be a repository recipe name such as 'pico-w'".into(),
            );
        }
        Ok(Self {
            name: name.into(),
            configuration: configuration.into(),
            output: None,
        })
    }
}

struct PreparedBody {
    source: String,
    report: CreationReport,
}

struct PendingHostRecipe {
    selector: String,
    destination: std::path::PathBuf,
    checked: CheckedHostConfiguration,
    source: String,
}

#[derive(Serialize)]
struct CreationReport {
    created: bool,
    body_id: String,
    template: Option<&'static str>,
    output: String,
    hosts: Vec<CreatedHostReport>,
    host_recipes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
}

#[derive(Serialize)]
struct CreatedHostReport {
    name: String,
    configuration: String,
    target: String,
    join_mode: &'static str,
    output: &'static str,
    fabrication_package: String,
    features: Vec<String>,
}

pub(super) fn create(
    name: Option<&str>,
    mut template: Option<BodyTemplate>,
    assignments: &[HostAssignment],
    output: Option<&Path>,
    no_interactive: bool,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let guided = !no_interactive
        && !opts.dry_run
        && !opts.json
        && !opts.quiet
        && std::io::stdin().is_terminal()
        && std::io::stdout().is_terminal();
    let mut assignments = assignments.to_vec();
    let mut pending_recipes = Vec::new();
    let name = if guided {
        let request = interactive::collect(&root, name, template, assignments)?;
        template = request.template;
        assignments = request.assignments;
        pending_recipes = request.pending_recipes;
        request.name
    } else {
        name.map(str::to_owned).ok_or(
            "body new requires NAME outside an interactive terminal; pass --no-interactive to make that intent explicit",
        )?
    };
    let output = output_path(&root, &name, output)?;
    let mut prepared = prepare(
        &root,
        &name,
        template,
        &assignments,
        &pending_recipes,
        &output,
    )?;
    paths::ensure_absent(&output, "Body description")?;
    for recipe in &pending_recipes {
        paths::ensure_absent(&recipe.destination, "Host configuration")?;
    }
    if guided && !interactive::confirm_creation(&prepared.report)? {
        return Ok(());
    }
    prepared.report.created = !opts.dry_run;
    if opts.dry_run {
        prepared.report.source = Some(prepared.source.clone());
    } else {
        for recipe in &pending_recipes {
            write_new(
                &recipe.destination,
                recipe.source.as_bytes(),
                "Host configuration",
            )?;
        }
        write_new(&output, prepared.source.as_bytes(), "Body description")?;
    }

    if guided {
        interactive::finish(&prepared.report)?;
    } else if opts.json {
        println!("{}", serde_json::to_string_pretty(&prepared.report)?);
    } else if !opts.quiet {
        if opts.dry_run {
            print!("{}", prepared.source);
        } else {
            print_receipt(&prepared.report);
        }
    }
    Ok(())
}

pub(super) fn list_templates(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let report = template_catalog(&load_host_recipes(&workspace_root()?)?)?;
    if opts.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else if !opts.quiet {
        print_template_catalog(&report);
    }
    Ok(())
}

fn prepare(
    root: &Path,
    name: &str,
    requested_template: Option<BodyTemplate>,
    assignments: &[HostAssignment],
    pending_recipes: &[PendingHostRecipe],
    output: &Path,
) -> Result<PreparedBody, Box<dyn std::error::Error>> {
    if !is_name(name) {
        return Err(
            "Body name must contain only letters, numbers, '_' or '-' and must not be empty".into(),
        );
    }
    let mut recipes = load_host_recipes(root)?;
    for pending in pending_recipes {
        catalog::insert_host_recipe(
            &mut recipes,
            &pending.selector,
            pending.destination.clone(),
            pending.checked.clone(),
        )?;
    }
    let composition = compose(requested_template, assignments)?;
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    let mut configurations = BTreeMap::new();
    let mut hosts = Vec::new();
    let mut host_reports = Vec::new();
    for seed in composition.seeds {
        let recipe = recipes.get(&seed.configuration).ok_or_else(|| {
            format!(
                "unknown Host configuration '{}'; available recipes: {}",
                seed.configuration,
                recipes.keys().cloned().collect::<Vec<_>>().join(", ")
            )
        })?;
        let output_kind = match seed.output.as_ref() {
            Some(output) if recipe.outputs.contains(output) => output,
            Some(output) => {
                return Err(format!(
                    "Host configuration '{}' cannot produce output {output:?}",
                    seed.configuration
                )
                .into())
            }
            None => {
                let [output] = recipe.outputs.as_slice() else {
                    return Err(format!(
                        "Host configuration '{}' has {} legal outputs; choose one explicitly",
                        seed.configuration,
                        recipe.outputs.len()
                    )
                    .into());
                };
                output
            }
        };
        let selection = conduit_workspace_fabrication::package_set()
            .derive_build_selection(recipe.checked.profile(), output_kind)
            .map_err(|item| {
                format!(
                    "Host configuration '{}' output refused: {item:?}",
                    seed.configuration
                )
            })?;
        let configuration_path = relative_path(parent, &recipe.source_path);
        let configuration = path_text(&configuration_path)?;
        configurations.insert(
            configuration.clone(),
            recipe.checked.configuration().clone(),
        );
        let (join_mode, part, invitation) = spore_join(name, &seed);
        hosts.push(BodyHostDescription {
            name: seed.name.clone(),
            part,
            configuration,
            spore: SporeDescription {
                join_mode,
                output: output_kind.clone(),
                invitation,
            },
            deployment: None,
        });
        host_reports.push(CreatedHostReport {
            name: seed.name,
            configuration: seed.configuration,
            target: recipe.target.clone(),
            join_mode: join_name(seed.join_mode),
            output: output_name(output_kind),
            fabrication_package: recipe.package.package_id.clone(),
            features: selection.features,
        });
    }
    let checked = check_body_description(
        BodyDescription {
            schema: BODY_DESCRIPTION_SCHEMA,
            name: name.into(),
            body: BodyBindingTarget {
                id: format!("body:{name}"),
            },
            hosts,
        },
        &configurations,
        &conduit_workspace_fabrication::catalog(),
        &conduit_workspace_fabrication::package_set(),
    )
    .map_err(|items| format!("generated Body description refused: {items:?}"))?;
    host_reports.sort_by(|left, right| left.name.cmp(&right.name));
    let source = canonical_body_description_conduit(checked.description())
        .map_err(|item| format!("generated Body source refused: {item:?}"))?;
    Ok(PreparedBody {
        source,
        report: CreationReport {
            created: false,
            body_id: checked.description().body.id.clone(),
            template: composition.template,
            output: display_path(output),
            hosts: host_reports,
            host_recipes: pending_recipes
                .iter()
                .map(|recipe| display_path(&recipe.destination))
                .collect(),
            source: None,
        },
    })
}

fn spore_join(body_name: &str, seed: &HostSeed) -> (SporeJoinMode, Option<String>, Option<String>) {
    match seed.join_mode {
        TemplateJoinMode::Prejoined => (
            SporeJoinMode::Prejoined,
            Some(format!("part:{}", seed.name)),
            None,
        ),
        TemplateJoinMode::SelfJoining => (
            SporeJoinMode::SelfJoining,
            None,
            Some(format!("invitation:{body_name}-{}:single-use", seed.name)),
        ),
    }
}

fn print_receipt(report: &CreationReport) {
    println!(
        "{} {} {}",
        style(CHECK).green().bold(),
        style("Created").green().bold(),
        style(&report.body_id).cyan().bold()
    );
    println!("  {}", style(&report.output).underlined());
    if let Some(template) = report.template {
        println!("\n{} {}", style("Template").dim(), style(template).yellow());
    }
    println!("\n{}", style("Hosts").bold());
    for host in &report.hosts {
        println!("\n  {} {}", style(CHECK).green(), style(&host.name).bold());
        println!("    {:<8} {}", style("recipe").dim(), host.configuration);
        println!("    {:<8} {}", style("target").dim(), host.target);
        println!("    {:<8} {}", style("join").dim(), host.join_mode);
        println!("    {:<8} {}", style("output").dim(), host.output);
        if !host.features.is_empty() {
            println!(
                "    {:<8} {}",
                style("features").dim(),
                host.features.join(", ")
            );
        }
    }
    if !report.host_recipes.is_empty() {
        println!("\n{}", style("New Host recipes").bold());
        for recipe in &report.host_recipes {
            println!("  {} {}", style(CHECK).green(), recipe);
        }
    }
    println!(
        "\n{}\n  {}",
        style("Next").bold(),
        style(format!("cargo xtask body show {}", report.output)).cyan()
    );
}

fn print_template_catalog(report: &TemplateCatalogReport) {
    println!("{}\n", style("Body templates").bold());
    for template in &report.templates {
        println!("{}", style(template.name).cyan().bold());
        println!("  {}", template.description);
        for host in &template.hosts {
            println!(
                "  {} {}={} {}",
                style(CHECK).green(),
                style(host.name).bold(),
                host.configuration,
                style(format!("({})", host.join_mode)).dim()
            );
        }
        println!();
    }
    println!("{}", style("Host configurations").bold());
    for configuration in &report.host_configurations {
        println!(
            "  {}  {}  {}",
            style(&configuration.name).cyan(),
            configuration.target,
            style(configuration.outputs.join(", ")).dim()
        );
    }
}

fn is_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_alphanumeric() || matches!(character, '_' | '-'))
}

#[cfg(test)]
mod tests;
