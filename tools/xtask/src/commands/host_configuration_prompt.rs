use std::fmt::Write as _;

use conduit_host_fabrication::{
    canonical_host_configuration_conduit, check_host_configuration,
    compatible_base_implementations, CheckedHostConfiguration, ConfigurationBase,
    ConfigurationTarget, HostConfiguration, TargetDescriptor, HOST_CONFIGURATION_SCHEMA,
};

pub(crate) struct PromptedHostConfiguration {
    pub(crate) checked: CheckedHostConfiguration,
    pub(crate) source: String,
}

pub(crate) fn prompt(
    suggested_name: Option<&str>,
    existing: Option<&HostConfiguration>,
) -> Result<PromptedHostConfiguration, Box<dyn std::error::Error>> {
    let default_name = existing
        .map(|configuration| configuration.name.as_str())
        .or(suggested_name)
        .unwrap_or("new-host");
    let name: String = cliclack::input("Host recipe name")
        .default_input(default_name)
        .validate(|value: &String| validate_name(value))
        .interact()?;

    let packages = conduit_workspace_fabrication::package_set();
    let descriptors = packages.target_descriptors();
    let initial_target = existing
        .and_then(|configuration| descriptor_index(&descriptors, &configuration.target))
        .unwrap_or(0);
    let mut target_prompt = cliclack::select("Choose the Host architecture")
        .initial_value(initial_target)
        .filter_mode();
    for (index, descriptor) in descriptors.iter().enumerate() {
        target_prompt = target_prompt.item(
            index,
            descriptor.label.as_str(),
            format!(
                "{}/{}/{}",
                descriptor.family, descriptor.architecture, descriptor.machine
            ),
        );
    }
    let target_index = target_prompt.interact()?;
    let descriptor = &descriptors[target_index];
    let same_target =
        existing.is_some_and(|configuration| descriptor_matches(descriptor, &configuration.target));
    let choices = compatible_base_implementations(descriptor, &packages);
    let selected_kinds = if choices.is_empty() {
        cliclack::note(
            "Bases",
            "This architecture currently offers no configurable Base implementations.",
        )?;
        Vec::new()
    } else {
        let initial = existing
            .filter(|_| same_target)
            .into_iter()
            .flat_map(|configuration| configuration.bases.iter())
            .filter(|base| choices.iter().any(|(kind, _)| kind == &base.kind))
            .map(|base| base.kind.clone())
            .collect::<Vec<_>>();
        let mut base_prompt = cliclack::multiselect("Choose the Bases this Host implements")
            .initial_values(initial)
            .max_rows(8);
        for (kind, implementations) in &choices {
            base_prompt = base_prompt.item(
                kind.clone(),
                kind,
                format!("{} compatible implementation(s)", implementations.len()),
            );
        }
        base_prompt.interact()?
    };

    let mut bases = Vec::new();
    for kind in selected_kinds {
        let implementations = choices
            .iter()
            .find(|(candidate, _)| candidate == &kind)
            .map(|(_, implementations)| implementations)
            .ok_or_else(|| format!("selected Base '{kind}' is no longer compatible"))?;
        let existing_implementation = existing
            .filter(|_| same_target)
            .and_then(|configuration| configuration.bases.iter().find(|base| base.kind == kind))
            .and_then(|base| {
                base.implementation
                    .as_ref()
                    .or_else(|| base.implementations.first())
            })
            .filter(|implementation| implementations.contains(implementation));
        let selected = if implementations.len() == 1 {
            implementations[0].clone()
        } else {
            let mut implementation_prompt =
                cliclack::select(format!("Choose the implementation for Base '{kind}'"));
            if let Some(initial) = existing_implementation {
                implementation_prompt = implementation_prompt.initial_value(initial.clone());
            }
            for implementation in implementations {
                implementation_prompt = implementation_prompt.item(
                    implementation.clone(),
                    implementation,
                    "compatible with the selected architecture",
                );
            }
            implementation_prompt.interact()?
        };
        bases.push(ConfigurationBase {
            kind,
            implementation: Some(exact_implementation(implementations, &selected)?),
            implementations: Vec::new(),
        });
    }

    let configuration = HostConfiguration {
        schema: HOST_CONFIGURATION_SCHEMA,
        name,
        target: ConfigurationTarget {
            architecture: descriptor.architecture.clone(),
            machine: descriptor.machine.clone(),
            board: descriptor.board.clone(),
            os: descriptor.os.clone(),
            fabrication_descriptor: exact_single_descriptor(descriptor),
        },
        bases,
        resources: existing
            .map(|configuration| configuration.resources.clone())
            .unwrap_or_default(),
        limits: existing
            .filter(|_| same_target)
            .map(|configuration| configuration.limits.clone())
            .unwrap_or_else(|| descriptor.maxima.clone()),
    };
    let prompted = prepare(configuration)?;
    cliclack::note("Host recipe", summary(&prompted.checked))?;
    Ok(prompted)
}

pub(crate) fn prepare(
    configuration: HostConfiguration,
) -> Result<PromptedHostConfiguration, Box<dyn std::error::Error>> {
    let checked = check_host_configuration(
        configuration,
        &conduit_workspace_fabrication::catalog(),
        &conduit_workspace_fabrication::package_set(),
    )
    .map_err(|items| format!("Host configuration refused: {items:?}"))?;
    let source = canonical_host_configuration_conduit(checked.configuration())
        .map_err(|item| format!("Host configuration encode refused: {item:?}"))?;
    Ok(PromptedHostConfiguration { checked, source })
}

fn summary(checked: &CheckedHostConfiguration) -> String {
    let mut summary = format!("target  {}\n", checked.profile().target.key());
    if checked.resolved_bases().is_empty() {
        summary.push_str("Bases   none\n");
    } else {
        for (kind, implementation) in checked.resolved_bases() {
            let _ = writeln!(&mut summary, "Base    {kind} → {implementation}");
        }
    }
    let _ = write!(
        &mut summary,
        "limits  architecture defaults or preserved checked bounds"
    );
    summary
}

fn descriptor_index(
    descriptors: &[&TargetDescriptor],
    target: &ConfigurationTarget,
) -> Option<usize> {
    descriptors
        .iter()
        .position(|descriptor| descriptor_matches(descriptor, target))
}

fn descriptor_matches(descriptor: &TargetDescriptor, target: &ConfigurationTarget) -> bool {
    descriptor.architecture == target.architecture
        && descriptor.machine == target.machine
        && descriptor.board == target.board
        && descriptor.os == target.os
}

fn exact_single_descriptor(descriptor: &TargetDescriptor) -> Option<String> {
    match descriptor.fabrication_descriptors.as_slice() {
        [binding] => Some(binding.clone()),
        _ => None,
    }
}

fn exact_implementation(
    available: &[String],
    selected: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    available
        .iter()
        .find(|implementation| implementation.as_str() == selected)
        .cloned()
        .ok_or_else(|| format!("implementation '{selected}' is not in the compatible set").into())
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

    #[test]
    fn a_competing_base_implementation_must_be_selected_exactly() {
        let available = vec!["driver/a@1".into(), "driver/b@1".into()];
        assert_eq!(
            exact_implementation(&available, "driver/b@1").unwrap(),
            "driver/b@1"
        );
        assert!(exact_implementation(&available, "driver/c@1").is_err());
    }

    #[test]
    fn prepared_configuration_uses_shared_descriptor_and_catalog_truth() {
        let packages = conduit_workspace_fabrication::package_set();
        let descriptor = packages
            .target_descriptors()
            .into_iter()
            .find(|descriptor| !compatible_base_implementations(descriptor, &packages).is_empty())
            .expect("workspace must retain one target with configurable Bases");
        let choices = compatible_base_implementations(descriptor, &packages);
        let (kind, implementations) = choices.first().unwrap();
        let prompted = prepare(HostConfiguration {
            schema: HOST_CONFIGURATION_SCHEMA,
            name: "shared-host".into(),
            target: ConfigurationTarget {
                architecture: descriptor.architecture.clone(),
                machine: descriptor.machine.clone(),
                board: descriptor.board.clone(),
                os: descriptor.os.clone(),
                fabrication_descriptor: exact_single_descriptor(descriptor),
            },
            bases: vec![ConfigurationBase {
                kind: kind.clone(),
                implementation: Some(implementations[0].clone()),
                implementations: Vec::new(),
            }],
            resources: Vec::new(),
            limits: descriptor.maxima.clone(),
        })
        .unwrap();
        assert_eq!(prompted.checked.resolved_bases()[0].1, implementations[0]);
        assert!(prompted.source.starts_with("host shared-host {"));
    }
}
