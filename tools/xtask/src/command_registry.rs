use std::ffi::OsString;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleClass {
    Prove,
    Check,
    Integrate,
    Make,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandAlias {
    pub spelling: &'static [&'static str],
    pub deprecated: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepositoryCommand {
    pub canonical: &'static [&'static str],
    pub lifecycle: LifecycleClass,
    pub aliases: &'static [CommandAlias],
}

pub const REPOSITORY_COMMANDS: &[RepositoryCommand] = &[
    RepositoryCommand {
        canonical: &["integrate"],
        lifecycle: LifecycleClass::Integrate,
        aliases: &[],
    },
    RepositoryCommand {
        canonical: &["make", "host", "browser"],
        lifecycle: LifecycleClass::Make,
        aliases: &[],
    },
    RepositoryCommand {
        canonical: &["make", "host", "std"],
        lifecycle: LifecycleClass::Make,
        aliases: &[],
    },
    RepositoryCommand {
        canonical: &["check", "browser"],
        lifecycle: LifecycleClass::Check,
        aliases: &[],
    },
    RepositoryCommand {
        canonical: &["prove", "journey", "triple"],
        lifecycle: LifecycleClass::Prove,
        aliases: &[],
    },
    RepositoryCommand {
        canonical: &["prove", "browser-host"],
        lifecycle: LifecycleClass::Prove,
        aliases: &[],
    },
    RepositoryCommand {
        canonical: &["make", "host", "build"],
        lifecycle: LifecycleClass::Make,
        aliases: &[],
    },
];

pub fn normalize_compatibility_aliases(mut args: Vec<OsString>) -> Vec<OsString> {
    debug_assert!(validate_registry(REPOSITORY_COMMANDS).is_ok());
    let Some(command_index) = args
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, item)| !item.to_string_lossy().starts_with('-'))
        .map(|(index, _)| index)
    else {
        return args;
    };

    for command in REPOSITORY_COMMANDS {
        for alias in command.aliases {
            let end = command_index + alias.spelling.len();
            if end <= args.len()
                && args[command_index..end]
                    .iter()
                    .zip(alias.spelling)
                    .all(|(actual, expected)| actual == expected)
            {
                args.splice(
                    command_index..end,
                    command.canonical.iter().map(OsString::from),
                );
                return args;
            }
        }
    }
    args
}

pub fn validate_registry(commands: &[RepositoryCommand]) -> Result<(), String> {
    use std::collections::{BTreeMap, BTreeSet};

    let mut destinations = BTreeSet::new();
    let mut aliases = BTreeMap::new();
    for command in commands {
        let _lifecycle = command.lifecycle;
        let canonical = command.canonical.join(" ");
        if !destinations.insert(canonical.clone()) {
            return Err(format!("duplicate canonical destination: {canonical}"));
        }
        for alias in command.aliases {
            let _deprecated = alias.deprecated;
            let spelling = alias.spelling.join(" ");
            if aliases
                .insert(spelling.clone(), canonical.clone())
                .is_some()
            {
                return Err(format!("duplicate alias: {spelling}"));
            }
        }
    }
    for (alias, destination) in &aliases {
        if aliases.contains_key(destination) {
            return Err(format!("alias cycle or chain: {alias} -> {destination}"));
        }
        if destinations.contains(alias) {
            return Err(format!("alias shadows canonical destination: {alias}"));
        }
    }
    Ok(())
}

#[cfg(test)]
fn just_recipe_bodies(source: &str) -> Result<std::collections::BTreeMap<String, String>, String> {
    let mut recipe = None;
    let mut bodies = std::collections::BTreeMap::new();
    for line in source.lines() {
        if !line.starts_with(' ')
            && !line.starts_with('\t')
            && !line.starts_with('#')
            && line.ends_with(':')
        {
            recipe = line
                .split_whitespace()
                .next()
                .map(|name| name.trim_end_matches(':').to_owned());
            continue;
        }
        if let Some(name) = recipe.take() {
            if line.starts_with("    ") {
                let body = line.trim();
                if !(body == "conduit {{args}}"
                    || body.starts_with("conduit ")
                    || body == "cargo xtask {{args}}"
                    || body.starts_with("cargo xtask "))
                {
                    return Err(format!(
                        "just recipe {name} contains independent execution logic: {body}"
                    ));
                }
                bodies.insert(name, body.to_owned());
            }
        }
    }
    Ok(bodies)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn every_registered_alias_resolves_to_exactly_one_canonical_command() {
        validate_registry(REPOSITORY_COMMANDS).unwrap();
        for command in REPOSITORY_COMMANDS {
            for alias in command.aliases {
                let mut argv = vec![OsString::from("xtask")];
                argv.extend(alias.spelling.iter().map(OsString::from));
                let normalized = normalize_compatibility_aliases(argv);
                assert_eq!(
                    &normalized[1..],
                    &command
                        .canonical
                        .iter()
                        .map(OsString::from)
                        .collect::<Vec<_>>()
                );
                crate::cli::Cli::try_parse_from(normalized).unwrap();
            }
        }
    }

    #[test]
    fn registry_rejects_duplicate_destinations_aliases_and_cycles() {
        const TO_B: &[CommandAlias] = &[CommandAlias {
            spelling: &["b"],
            deprecated: false,
        }];
        const TO_A: &[CommandAlias] = &[CommandAlias {
            spelling: &["a"],
            deprecated: false,
        }];
        let duplicate = [
            RepositoryCommand {
                canonical: &["a"],
                lifecycle: LifecycleClass::Check,
                aliases: &[],
            },
            RepositoryCommand {
                canonical: &["a"],
                lifecycle: LifecycleClass::Prove,
                aliases: &[],
            },
        ];
        assert!(validate_registry(&duplicate)
            .unwrap_err()
            .contains("duplicate canonical"));

        let duplicate_alias = [
            RepositoryCommand {
                canonical: &["a"],
                lifecycle: LifecycleClass::Check,
                aliases: TO_B,
            },
            RepositoryCommand {
                canonical: &["c"],
                lifecycle: LifecycleClass::Prove,
                aliases: TO_B,
            },
        ];
        assert!(validate_registry(&duplicate_alias)
            .unwrap_err()
            .contains("duplicate alias"));

        let cycle = [
            RepositoryCommand {
                canonical: &["a"],
                lifecycle: LifecycleClass::Check,
                aliases: TO_B,
            },
            RepositoryCommand {
                canonical: &["b"],
                lifecycle: LifecycleClass::Make,
                aliases: TO_A,
            },
        ];
        assert!(validate_registry(&cycle).unwrap_err().contains("cycle"));
    }

    #[test]
    fn justfile_recipes_are_thin_registered_entrance_delegations() {
        let bodies = just_recipe_bodies(include_str!("../../../justfile")).unwrap();
        assert_eq!(
            bodies,
            std::collections::BTreeMap::from([
                (
                    "check".to_owned(),
                    "cargo xtask check {{ args }}".to_owned()
                ),
                ("conduit".to_owned(), "conduit {{ args }}".to_owned()),
                ("integrate".to_owned(), "cargo xtask integrate".to_owned()),
                ("xtask".to_owned(), "cargo xtask {{ args }}".to_owned()),
            ])
        );
        assert!(just_recipe_bodies("bad:\n    cargo test --workspace\n")
            .unwrap_err()
            .contains("independent execution logic"));
    }
}
