use std::ffi::OsString;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleClass {
    Host,
    Demo,
    Prove,
    Check,
    Fabricate,
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

const BROWSER_ALIASES: &[CommandAlias] = &[CommandAlias {
    spelling: &["browser"],
    deprecated: true,
}];
const STD_ALIASES: &[CommandAlias] = &[CommandAlias {
    spelling: &["demo", "std"],
    deprecated: true,
}];
const BROWSER_CHECK_ALIASES: &[CommandAlias] = &[CommandAlias {
    spelling: &["check", "browser"],
    deprecated: true,
}];

pub const REPOSITORY_COMMANDS: &[RepositoryCommand] = &[
    RepositoryCommand {
        canonical: &["host", "browser"],
        lifecycle: LifecycleClass::Host,
        aliases: BROWSER_ALIASES,
    },
    RepositoryCommand {
        canonical: &["host", "std"],
        lifecycle: LifecycleClass::Host,
        aliases: STD_ALIASES,
    },
    RepositoryCommand {
        canonical: &["check", "browser-host"],
        lifecycle: LifecycleClass::Check,
        aliases: BROWSER_CHECK_ALIASES,
    },
    RepositoryCommand {
        canonical: &["demo", "triple"],
        lifecycle: LifecycleClass::Demo,
        aliases: &[],
    },
    RepositoryCommand {
        canonical: &["prove", "browser-host"],
        lifecycle: LifecycleClass::Prove,
        aliases: &[],
    },
    RepositoryCommand {
        canonical: &["host", "build"],
        lifecycle: LifecycleClass::Fabricate,
        aliases: &[],
    },
];

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JustAlias {
    pub recipe: &'static str,
    pub canonical_body: &'static str,
}

#[cfg(test)]
pub const FRIENDLY_JUST_ALIASES: &[JustAlias] = &[
    JustAlias {
        recipe: "patchbay",
        canonical_body: "cargo xtask demo patchbay --on native",
    },
    JustAlias {
        recipe: "browser",
        canonical_body: "cargo xtask host browser",
    },
    JustAlias {
        recipe: "std-host",
        canonical_body: "cargo xtask host std",
    },
    JustAlias {
        recipe: "demo-std",
        canonical_body: "cargo xtask host std",
    },
    JustAlias {
        recipe: "demo-triple-local",
        canonical_body: "cargo xtask demo triple",
    },
    JustAlias {
        recipe: "check-kernel-s1",
        canonical_body: "cargo xtask check kernel-takeover",
    },
    JustAlias {
        recipe: "check-kernel-takeover",
        canonical_body: "cargo xtask check kernel-takeover",
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
                lifecycle: LifecycleClass::Demo,
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
                lifecycle: LifecycleClass::Demo,
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
                lifecycle: LifecycleClass::Demo,
                aliases: TO_B,
            },
            RepositoryCommand {
                canonical: &["b"],
                lifecycle: LifecycleClass::Fabricate,
                aliases: TO_A,
            },
        ];
        assert!(validate_registry(&cycle).unwrap_err().contains("cycle"));
    }

    #[test]
    fn justfile_recipes_are_thin_registered_entrance_delegations() {
        let bodies = just_recipe_bodies(include_str!("../../../justfile")).unwrap();
        for alias in FRIENDLY_JUST_ALIASES {
            assert_eq!(
                bodies.get(alias.recipe).map(String::as_str),
                Some(alias.canonical_body),
                "friendly recipe must delegate to its registered canonical command"
            );
        }
        assert!(just_recipe_bodies("bad:\n    cargo test --workspace\n")
            .unwrap_err()
            .contains("independent execution logic"));
    }
}
