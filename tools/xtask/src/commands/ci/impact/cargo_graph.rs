use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const SKIP_DIRECTORIES: [&str; 3] = [".git", "target", "node_modules"];

#[derive(Debug, Clone)]
pub(super) struct Package {
    pub(super) name: String,
    pub(super) directory: PathBuf,
    pub(super) dependencies: BTreeSet<String>,
    pub(super) workspace_member: bool,
    test_dependencies: BTreeSet<String>,
}

pub(super) fn discover(root: &Path) -> Result<BTreeMap<String, Package>, String> {
    let workspace_manifest = fs::read_to_string(root.join("Cargo.toml"))
        .map_err(|error| format!("workspace Cargo.toml: {error}"))?;
    let workspace_value: toml::Value = toml::from_str(&workspace_manifest)
        .map_err(|error| format!("workspace Cargo.toml: {error}"))?;
    let workspace_directories: BTreeSet<PathBuf> = workspace_value
        .get("workspace")
        .and_then(|workspace| workspace.get("members"))
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "workspace Cargo.toml omits workspace.members".to_owned())?
        .iter()
        .map(|member| {
            member
                .as_str()
                .map(|member| normalize(&root.join(member)))
                .ok_or_else(|| "workspace member is not a string".to_owned())
        })
        .collect::<Result<_, _>>()?;
    let mut manifests = Vec::new();
    collect_manifests(root, &mut manifests).map_err(|error| error.to_string())?;
    let mut parsed = Vec::new();
    for manifest in manifests {
        let source = fs::read_to_string(&manifest)
            .map_err(|error| format!("{}: {error}", manifest.display()))?;
        let value: toml::Value =
            toml::from_str(&source).map_err(|error| format!("{}: {error}", manifest.display()))?;
        if value
            .get("package")
            .and_then(|item| item.get("name"))
            .and_then(toml::Value::as_str)
            .is_some()
        {
            parsed.push((manifest, value));
        }
    }

    let names_by_directory: BTreeMap<PathBuf, String> = parsed
        .iter()
        .map(|(manifest, value)| {
            let name = value["package"]["name"]
                .as_str()
                .expect("checked package name")
                .to_owned();
            (normalize(manifest.parent().expect("manifest parent")), name)
        })
        .collect();

    let mut packages = BTreeMap::new();
    for (manifest, value) in parsed {
        let directory = normalize(manifest.parent().expect("manifest parent"));
        let name = value["package"]["name"]
            .as_str()
            .expect("checked package name")
            .to_owned();
        let mut dependencies = BTreeSet::new();
        collect_dependencies(
            &value,
            &directory,
            &names_by_directory,
            &mut dependencies,
            false,
        );
        let mut test_dependencies = dependencies.clone();
        collect_dependencies(
            &value,
            &directory,
            &names_by_directory,
            &mut test_dependencies,
            true,
        );
        packages.insert(
            name.clone(),
            Package {
                name,
                workspace_member: workspace_directories.contains(&directory),
                directory,
                dependencies,
                test_dependencies,
            },
        );
    }
    Ok(packages)
}

pub(super) fn dependency_closure(
    packages: &BTreeMap<String, Package>,
    roots: &BTreeSet<&str>,
) -> Result<BTreeSet<String>, String> {
    let missing: Vec<_> = roots
        .iter()
        .filter(|root| !packages.contains_key(**root))
        .copied()
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "configured CI root package(s) missing: {}",
            missing.join(", ")
        ));
    }
    let mut seen = BTreeSet::new();
    let mut pending: Vec<String> = roots.iter().map(|name| (*name).to_owned()).collect();
    while let Some(name) = pending.pop() {
        if seen.insert(name.clone()) {
            pending.extend(
                packages[&name]
                    .dependencies
                    .iter()
                    .filter(|dep| !seen.contains(*dep))
                    .cloned(),
            );
        }
    }
    Ok(seen)
}

pub(super) fn affected_tests(
    packages: &BTreeMap<String, Package>,
    changed: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut affected = changed.clone();
    loop {
        let additions: Vec<_> = packages
            .values()
            .filter(|package| {
                !affected.contains(&package.name)
                    && package
                        .test_dependencies
                        .iter()
                        .any(|dependency| affected.contains(dependency))
            })
            .map(|package| package.name.clone())
            .collect();
        if additions.is_empty() {
            break;
        }
        affected.extend(additions);
    }
    affected
}

fn collect_manifests(directory: &Path, manifests: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name();
            if !SKIP_DIRECTORIES.iter().any(|skip| name == *skip) {
                collect_manifests(&path, manifests)?;
            }
        } else if entry.file_name() == "Cargo.toml" {
            manifests.push(path);
        }
    }
    Ok(())
}

fn collect_dependencies(
    value: &toml::Value,
    directory: &Path,
    names: &BTreeMap<PathBuf, String>,
    dependencies: &mut BTreeSet<String>,
    dev_only: bool,
) {
    let keys = if dev_only {
        &["dev-dependencies"][..]
    } else {
        &["dependencies", "build-dependencies"][..]
    };
    for key in keys {
        collect_dependency_table(value.get(*key), directory, names, dependencies);
    }
    if let Some(targets) = value.get("target").and_then(toml::Value::as_table) {
        for target in targets.values() {
            for key in keys {
                collect_dependency_table(target.get(*key), directory, names, dependencies);
            }
        }
    }
}

fn collect_dependency_table(
    value: Option<&toml::Value>,
    directory: &Path,
    names: &BTreeMap<PathBuf, String>,
    dependencies: &mut BTreeSet<String>,
) {
    let Some(table) = value.and_then(toml::Value::as_table) else {
        return;
    };
    for specification in table.values() {
        let Some(relative) = specification.get("path").and_then(toml::Value::as_str) else {
            continue;
        };
        if let Some(name) = names.get(&normalize(&directory.join(relative))) {
            dependencies.insert(name.clone());
        }
    }
}

pub(super) fn normalize(path: &Path) -> PathBuf {
    path.canonicalize()
        .unwrap_or_else(|_| path.components().collect())
}
