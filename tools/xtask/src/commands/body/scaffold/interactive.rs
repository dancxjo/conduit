use std::{fmt::Write as _, path::Path};

use clap::ValueEnum as _;
use console::style;

use super::{
    catalog::{
        insert_host_recipe, load_host_recipes, output_name, template_description, template_name,
        BodyTemplate, HostRecipe,
    },
    CreationReport, HostAssignment, PendingHostRecipe,
};
use crate::commands::host_configuration_prompt;

#[derive(Clone, PartialEq, Eq)]
enum RecipeChoice {
    Create,
    Existing(String),
}

pub(super) struct Request {
    pub(super) name: String,
    pub(super) template: Option<BodyTemplate>,
    pub(super) assignments: Vec<HostAssignment>,
    pub(super) pending_recipes: Vec<PendingHostRecipe>,
}

pub(super) fn collect(
    root: &Path,
    supplied_name: Option<&str>,
    mut template: Option<BodyTemplate>,
    mut assignments: Vec<HostAssignment>,
) -> Result<Request, Box<dyn std::error::Error>> {
    cliclack::intro(style(" Conduit body new ").on_cyan().black())?;
    let name = match supplied_name {
        Some(name) => name.to_owned(),
        None => cliclack::input("What should this Body be called?")
            .placeholder("pete")
            .validate(|value: &String| validate_name(value))
            .interact()?,
    };

    if template.is_none() && assignments.is_empty() {
        let mut prompt =
            cliclack::select("Choose a starting point").initial_value(BodyTemplate::Minimal);
        for candidate in BodyTemplate::value_variants() {
            prompt = prompt.item(
                *candidate,
                template_name(*candidate),
                template_description(*candidate),
            );
        }
        template = Some(prompt.interact()?);
    }

    let mut recipes = load_host_recipes(root)?;
    let mut pending_recipes = Vec::new();
    if cliclack::confirm("Add or replace a Host recipe?")
        .initial_value(false)
        .interact()?
    {
        loop {
            let mut prompt = cliclack::select("Choose a Host recipe")
                .item(
                    RecipeChoice::Create,
                    "Create a new Host recipe",
                    "choose architecture, Bases, and exact implementations",
                )
                .filter_mode();
            for (selector, recipe) in &recipes {
                let outputs = recipe
                    .outputs
                    .iter()
                    .map(output_name)
                    .collect::<Vec<_>>()
                    .join(" or ");
                prompt = prompt.item(
                    RecipeChoice::Existing(selector.clone()),
                    selector,
                    format!("{} → {outputs}", recipe.target),
                );
            }
            let (host_name, configuration) = match prompt.interact()? {
                RecipeChoice::Existing(configuration) => {
                    (part_name(&configuration)?, configuration)
                }
                RecipeChoice::Create => {
                    let host_name = part_name("new-part")?;
                    let prompted = host_configuration_prompt::prompt(Some(&host_name), None)?;
                    let configuration = prompted.checked.configuration().name.clone();
                    let destination = root
                        .join("targets")
                        .join(&prompted.checked.profile().target.family)
                        .join("profiles")
                        .join(format!("{configuration}.host.conduit"));
                    if destination.exists() || recipes.contains_key(&configuration) {
                        return Err(format!(
                            "Host recipe '{configuration}' already exists; choose it from the existing recipes"
                        )
                        .into());
                    }
                    insert_host_recipe(
                        &mut recipes,
                        &configuration,
                        destination.clone(),
                        prompted.checked.clone(),
                    )?;
                    pending_recipes.push(PendingHostRecipe {
                        selector: configuration.clone(),
                        destination,
                        checked: prompted.checked,
                        source: prompted.source,
                    });
                    (host_name, configuration)
                }
            };
            let output = choose_output(
                recipes
                    .get(&configuration)
                    .expect("selected or newly prepared Host recipe"),
            )?;
            upsert(
                &mut assignments,
                HostAssignment {
                    name: host_name,
                    configuration,
                    output,
                },
            );
            pending_recipes.retain(|pending| {
                assignments
                    .iter()
                    .any(|assignment| assignment.configuration == pending.selector)
            });
            if !cliclack::confirm("Add another Host recipe?")
                .initial_value(false)
                .interact()?
            {
                break;
            }
        }
    }

    Ok(Request {
        name,
        template,
        assignments,
        pending_recipes,
    })
}

fn part_name(suggestion: &str) -> Result<String, Box<dyn std::error::Error>> {
    cliclack::input("Name this part")
        .default_input(suggestion)
        .validate(|value: &String| validate_name(value))
        .interact()
        .map_err(Into::into)
}

fn choose_output(
    recipe: &HostRecipe,
) -> Result<Option<conduit_host_fabrication::SporeOutputKind>, Box<dyn std::error::Error>> {
    if recipe.outputs.len() == 1 {
        return Ok(None);
    }
    let mut prompt = cliclack::select("Choose the Spore output for this Host");
    for output in &recipe.outputs {
        prompt = prompt.item(
            output.clone(),
            output_name(output),
            recipe.package.package_id.as_str(),
        );
    }
    Ok(Some(prompt.interact()?))
}

pub(super) fn confirm_creation(
    report: &CreationReport,
) -> Result<bool, Box<dyn std::error::Error>> {
    let mut summary = String::new();
    if let Some(template) = report.template {
        writeln!(&mut summary, "template  {template}")?;
    }
    for host in &report.hosts {
        writeln!(
            &mut summary,
            "{:<12} {:<20} {:<14} {}",
            host.name, host.configuration, host.output, host.join_mode
        )?;
    }
    writeln!(&mut summary, "\n{}", report.output)?;
    for recipe in &report.host_recipes {
        writeln!(&mut summary, "{recipe}")?;
    }
    cliclack::note(format!("Review {}", report.body_id), summary.trim_end())?;
    let confirmed = cliclack::confirm("Create this Body?")
        .initial_value(true)
        .interact()?;
    if !confirmed {
        cliclack::outro_cancel("Nothing was created")?;
    }
    Ok(confirmed)
}

pub(super) fn finish(report: &CreationReport) -> Result<(), Box<dyn std::error::Error>> {
    let mut created = report.output.clone();
    for recipe in &report.host_recipes {
        writeln!(&mut created, "{recipe}")?;
    }
    cliclack::outro_note(
        format!("Created {}", report.body_id),
        format!(
            "{created}\n\nNext\n  cargo xtask body show {}",
            report.output
        ),
    )?;
    Ok(())
}

fn upsert(assignments: &mut Vec<HostAssignment>, assignment: HostAssignment) {
    if let Some(existing) = assignments
        .iter_mut()
        .find(|existing| existing.name == assignment.name)
    {
        existing.configuration = assignment.configuration;
        existing.output = assignment.output;
    } else {
        assignments.push(assignment);
    }
}

fn validate_name(value: &str) -> Result<(), &'static str> {
    if value.is_empty()
        || !value
            .chars()
            .all(|character| character.is_alphanumeric() || matches!(character, '_' | '-'))
    {
        Err("Use only letters, numbers, '_' or '-'.")
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::body::scaffold::catalog::{compose, join_name};

    #[test]
    fn repeated_interactive_host_choice_replaces_the_prior_choice() {
        let mut assignments = vec![HostAssignment {
            name: "brainstem".into(),
            configuration: "pico-w".into(),
            output: None,
        }];
        upsert(
            &mut assignments,
            HostAssignment {
                name: "brainstem".into(),
                configuration: "browser-page".into(),
                output: None,
            },
        );
        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].configuration, "browser-page");
    }

    #[test]
    fn interactive_names_share_the_source_name_contract() {
        assert!(validate_name("pete-r1").is_ok());
        assert!(validate_name("pete r1").is_err());
        assert!(validate_name("").is_err());
    }

    #[test]
    fn composed_review_keeps_join_modes_visible() {
        let composition = compose(Some(BodyTemplate::Distributed), &[]).unwrap();
        assert!(composition
            .seeds
            .iter()
            .any(|seed| join_name(seed.join_mode) == "self-joining"));
    }
}
