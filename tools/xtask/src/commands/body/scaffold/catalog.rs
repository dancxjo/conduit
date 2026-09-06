use std::{collections::BTreeMap, fs, path::Path};

use clap::ValueEnum;
use conduit_host_fabrication::{
    check_host_configuration, parse_host_configuration_conduit, CheckedHostConfiguration,
    FabricationAnchor, SporeOutputKind,
};
use serde::Serialize;

use super::HostAssignment;

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(in crate::commands::body) enum BodyTemplate {
    Minimal,
    Hosted,
    Robot,
    Distributed,
}

#[derive(Clone, Copy)]
struct TemplateHost {
    name: &'static str,
    configuration: &'static str,
    join_mode: TemplateJoinMode,
}

#[derive(Clone, Copy)]
pub(super) enum TemplateJoinMode {
    Prejoined,
    SelfJoining,
}

#[derive(Clone)]
pub(super) struct HostSeed {
    pub(super) name: String,
    pub(super) configuration: String,
    pub(super) join_mode: TemplateJoinMode,
    pub(super) output: Option<SporeOutputKind>,
}

pub(super) struct Composition {
    pub(super) template: Option<&'static str>,
    pub(super) seeds: Vec<HostSeed>,
}

pub(super) struct HostRecipe {
    pub(super) checked: CheckedHostConfiguration,
    pub(super) source_path: std::path::PathBuf,
    pub(super) target: String,
    pub(super) package: FabricationAnchor,
    pub(super) outputs: Vec<SporeOutputKind>,
}

#[derive(Serialize)]
pub(super) struct TemplateCatalogReport {
    pub(super) templates: Vec<TemplateReport>,
    pub(super) host_configurations: Vec<HostConfigurationReport>,
}

#[derive(Serialize)]
pub(super) struct TemplateReport {
    pub(super) name: &'static str,
    pub(super) description: &'static str,
    pub(super) hosts: Vec<TemplateHostReport>,
}

#[derive(Serialize)]
pub(super) struct TemplateHostReport {
    pub(super) name: &'static str,
    pub(super) configuration: &'static str,
    pub(super) join_mode: &'static str,
}

#[derive(Serialize)]
pub(super) struct HostConfigurationReport {
    pub(super) name: String,
    pub(super) target: String,
    pub(super) outputs: Vec<&'static str>,
}

pub(super) fn compose(
    requested_template: Option<BodyTemplate>,
    assignments: &[HostAssignment],
) -> Result<Composition, Box<dyn std::error::Error>> {
    let effective_template =
        requested_template.or_else(|| assignments.is_empty().then_some(BodyTemplate::Minimal));
    let mut seeds = effective_template
        .map(template_hosts)
        .unwrap_or_default()
        .into_iter()
        .map(|host| HostSeed {
            name: host.name.into(),
            configuration: host.configuration.into(),
            join_mode: host.join_mode,
            output: None,
        })
        .collect::<Vec<_>>();
    let mut explicit = std::collections::BTreeSet::new();
    for assignment in assignments {
        if !explicit.insert(&assignment.name) {
            return Err(format!("duplicate --host assignment for '{}'", assignment.name).into());
        }
        if let Some(seed) = seeds.iter_mut().find(|seed| seed.name == assignment.name) {
            seed.configuration.clone_from(&assignment.configuration);
            seed.output.clone_from(&assignment.output);
        } else {
            seeds.push(HostSeed {
                name: assignment.name.clone(),
                configuration: assignment.configuration.clone(),
                join_mode: TemplateJoinMode::Prejoined,
                output: assignment.output.clone(),
            });
        }
    }
    seeds.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(Composition {
        template: effective_template.map(template_name),
        seeds,
    })
}

pub(super) fn load_host_recipes(
    root: &Path,
) -> Result<BTreeMap<String, HostRecipe>, Box<dyn std::error::Error>> {
    let directory = root.join("targets");
    let mut paths = Vec::new();
    for target in fs::read_dir(&directory)? {
        let profiles = target?.path().join("profiles");
        if profiles.is_dir() {
            for entry in fs::read_dir(profiles)? {
                paths.push(entry?.path());
            }
        }
    }
    paths.sort();
    let catalog = conduit_workspace_fabrication::catalog();
    let mut recipes = BTreeMap::new();
    for path in paths {
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(selector) = file_name.strip_suffix(".host.conduit").map(str::to_owned) else {
            continue;
        };
        let source = fs::read_to_string(&path)?;
        let configuration = parse_host_configuration_conduit(&source).map_err(|item| {
            format!(
                "Host configuration {} decode refused: {item:?}",
                path.display()
            )
        })?;
        let checked = check_host_configuration(
            configuration,
            &catalog,
            &conduit_workspace_fabrication::package_set(),
        )
        .map_err(|items| format!("Host configuration {} refused: {items:?}", path.display()))?;
        insert_host_recipe(&mut recipes, &selector, path, checked)?;
    }
    if recipes.is_empty() {
        return Err(format!("no canonical Host recipes found in {}", directory.display()).into());
    }
    Ok(recipes)
}

pub(super) fn insert_host_recipe(
    recipes: &mut BTreeMap<String, HostRecipe>,
    selector: &str,
    source_path: std::path::PathBuf,
    checked: CheckedHostConfiguration,
) -> Result<(), Box<dyn std::error::Error>> {
    if recipes.contains_key(selector) {
        return Err(format!("duplicate Host recipe selector '{selector}'").into());
    }
    let packages = conduit_workspace_fabrication::package_set();
    let package = packages
        .anchor_for_target(&checked.profile().target.key())
        .ok_or_else(|| {
            format!(
                "Host configuration {} has no fabrication anchor",
                source_path.display()
            )
        })?
        .clone();
    let target = checked.profile().target.key();
    let outputs = package
        .targets
        .iter()
        .find(|descriptor| descriptor.key() == target)
        .expect("resolved anchor owns exact target")
        .outputs
        .iter()
        .filter(|output| {
            packages
                .derive_build_selection(checked.profile(), output)
                .is_ok()
        })
        .cloned()
        .collect::<Vec<_>>();
    if outputs.is_empty() {
        return Err(format!(
            "Host configuration {} has no compatible Body output",
            source_path.display()
        )
        .into());
    }
    recipes.insert(
        selector.into(),
        HostRecipe {
            checked,
            source_path,
            target,
            package,
            outputs,
        },
    );
    Ok(())
}

pub(super) fn list(
    recipes: &BTreeMap<String, HostRecipe>,
) -> Result<TemplateCatalogReport, Box<dyn std::error::Error>> {
    let templates = BodyTemplate::value_variants()
        .iter()
        .map(|template| {
            let hosts = template_hosts(*template)
                .iter()
                .map(|host| {
                    if !recipes.contains_key(host.configuration) {
                        return Err(format!(
                            "template '{}' requires missing Host recipe '{}'",
                            template_name(*template),
                            host.configuration
                        ));
                    }
                    Ok(TemplateHostReport {
                        name: host.name,
                        configuration: host.configuration,
                        join_mode: join_name(host.join_mode),
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(TemplateReport {
                name: template_name(*template),
                description: template_description(*template),
                hosts,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let host_configurations = recipes
        .iter()
        .map(|(name, recipe)| HostConfigurationReport {
            name: name.clone(),
            target: recipe.target.clone(),
            outputs: recipe.outputs.iter().map(output_name).collect(),
        })
        .collect();
    Ok(TemplateCatalogReport {
        templates,
        host_configurations,
    })
}

fn template_hosts(template: BodyTemplate) -> Vec<TemplateHost> {
    match template {
        BodyTemplate::Minimal => vec![host("main", "linux-computer")],
        BodyTemplate::Hosted => vec![host("app", "linux-computer"), host("page", "browser-page")],
        BodyTemplate::Robot => vec![
            host("forebrain", "linux-computer"),
            host("brainstem", "pico-w"),
        ],
        BodyTemplate::Distributed => vec![
            host("coordinator", "linux-computer"),
            TemplateHost {
                name: "peer",
                configuration: "browser-page",
                join_mode: TemplateJoinMode::SelfJoining,
            },
        ],
    }
}

const fn host(name: &'static str, configuration: &'static str) -> TemplateHost {
    TemplateHost {
        name,
        configuration,
        join_mode: TemplateJoinMode::Prejoined,
    }
}

pub(super) const fn template_name(template: BodyTemplate) -> &'static str {
    match template {
        BodyTemplate::Minimal => "minimal",
        BodyTemplate::Hosted => "hosted",
        BodyTemplate::Robot => "robot",
        BodyTemplate::Distributed => "distributed",
    }
}

pub(super) const fn template_description(template: BodyTemplate) -> &'static str {
    match template {
        BodyTemplate::Minimal => "One native Host: the smallest checked Body starting point.",
        BodyTemplate::Hosted => "A native application Host plus a prejoined browser page.",
        BodyTemplate::Robot => "A native forebrain plus a Pico W brainstem.",
        BodyTemplate::Distributed => {
            "A native coordinator plus a self-joining browser peer with a bounded invitation."
        }
    }
}

pub(super) const fn join_name(mode: TemplateJoinMode) -> &'static str {
    match mode {
        TemplateJoinMode::Prejoined => "prejoined",
        TemplateJoinMode::SelfJoining => "self-joining",
    }
}

pub(super) const fn output_name(output: &SporeOutputKind) -> &'static str {
    match output {
        SporeOutputKind::NativeBundle => "native-bundle",
        SporeOutputKind::BrowserBundle => "browser-bundle",
        SporeOutputKind::IntelHex => "intel-hex",
        SporeOutputKind::Uf2 => "uf2",
        SporeOutputKind::DiskImage => "disk-image",
        SporeOutputKind::EfiArtifact => "efi-artifact",
        SporeOutputKind::Esp32Image => "esp32-image",
        SporeOutputKind::SdImage => "sd-image",
    }
}
