use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const SUITES: [&str; 3] = ["esp32", "browser", "conduitos"];
const SKIP_DIRECTORIES: [&str; 3] = [".git", "target", "node_modules"];
const GLOBAL_PREFIXES: [&str; 4] = [".github/", ".cargo/", "xtask/", "scripts/ci/"];
const GLOBAL_FILES: [&str; 4] = [
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain",
    "rust-toolchain.toml",
];
const HARMLESS_PREFIXES: [&str; 2] = ["docs/", "examples/"];
const HARMLESS_FILES: [&str; 6] = [
    "README.md",
    "STATUS.md",
    "AGENTS.md",
    "LICENSE",
    "justfile",
    "Justfile",
];

#[derive(Debug, Clone)]
struct Package {
    name: String,
    directory: PathBuf,
    dependencies: BTreeSet<String>,
}

#[derive(Debug, Serialize)]
struct ImpactPlan {
    esp32_required: bool,
    browser_required: bool,
    conduitos_required: bool,
    full_fallback: bool,
    reason: String,
    changed_paths: Vec<String>,
    changed_packages: Vec<String>,
    suite_reasons: BTreeMap<String, Vec<String>>,
}

pub(super) fn run(
    base: &str,
    head: &str,
    json_out: Option<&Path>,
    summary_out: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = crate::workspace::workspace_root()?;
    let plan = match changed_paths(&root, base, head)
        .and_then(|paths| discover_packages(&root).map(|packages| (paths, packages)))
        .and_then(|(paths, packages)| plan_for_paths(&root, paths, &packages))
    {
        Ok(plan) => plan,
        Err(error) => full_plan(format!("planner-error:{error}"), Vec::new()),
    };

    write_github_outputs(&plan);
    if let Some(path) = json_out {
        write_parent(path)?;
        fs::write(path, format!("{}\n", serde_json::to_string_pretty(&plan)?))?;
    }
    if let Some(path) = summary_out {
        write_parent(path)?;
        fs::write(path, markdown_summary(&plan))?;
    }
    Ok(())
}

fn suite_roots() -> BTreeMap<&'static str, BTreeSet<&'static str>> {
    BTreeMap::from([
        (
            "esp32",
            BTreeSet::from([
                "conduit-esp32-c3-signal",
                "conduit-esp32-s3-signal",
                "conduit-esp32-wroom-signal",
                "conduit-host-esp32-fabrication",
            ]),
        ),
        (
            "browser",
            BTreeSet::from([
                "conduit-browser-host",
                "conduit-browser-runtime",
                "patchbay-html",
                "patchbay-hosted",
                "patchbay-model",
                "patchbay-native",
            ]),
        ),
        (
            "conduitos",
            BTreeSet::from([
                "conduitos",
                "conduit-host-conduitos-fabrication",
                "conduit-workspace-fabrication",
            ]),
        ),
    ])
}

fn direct_prefixes(suite: &str) -> &'static [&'static str] {
    match suite {
        "esp32" => &["firmware/conduit-esp32-", "targets/esp32/"],
        "browser" => &[
            "hosts/browser-",
            "apps/patchbay/",
            "proof/browser/",
            "assets/",
        ],
        "conduitos" => &["hosts/conduitos/", "profiles/hosts/conduitos"],
        _ => &[],
    }
}

fn discover_packages(root: &Path) -> Result<BTreeMap<String, Package>, String> {
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
        collect_build_dependencies(&value, &directory, &names_by_directory, &mut dependencies);
        packages.insert(
            name.clone(),
            Package {
                name,
                directory,
                dependencies,
            },
        );
    }
    Ok(packages)
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

fn collect_build_dependencies(
    value: &toml::Value,
    directory: &Path,
    names: &BTreeMap<PathBuf, String>,
    dependencies: &mut BTreeSet<String>,
) {
    for key in ["dependencies", "build-dependencies"] {
        collect_dependency_table(value.get(key), directory, names, dependencies);
    }
    if let Some(targets) = value.get("target").and_then(toml::Value::as_table) {
        for target in targets.values() {
            for key in ["dependencies", "build-dependencies"] {
                collect_dependency_table(target.get(key), directory, names, dependencies);
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

fn normalize(path: &Path) -> PathBuf {
    path.canonicalize()
        .unwrap_or_else(|_| path.components().collect())
}

fn dependency_closure(
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

fn plan_for_paths(
    root: &Path,
    paths: Vec<String>,
    packages: &BTreeMap<String, Package>,
) -> Result<ImpactPlan, String> {
    let closures: BTreeMap<_, _> = suite_roots()
        .iter()
        .map(|(suite, roots)| {
            dependency_closure(packages, roots).map(|closure| ((*suite).to_owned(), closure))
        })
        .collect::<Result<_, _>>()?;
    let mut selected = BTreeMap::from(SUITES.map(|suite| (suite.to_owned(), false)));
    let mut reasons = empty_reasons();
    let mut changed_packages = BTreeSet::new();
    let substantive: Vec<_> = paths.iter().filter(|path| !path.ends_with(".md")).collect();
    if substantive.is_empty() {
        return Ok(plan(
            selected,
            false,
            "markdown-only".to_owned(),
            paths,
            changed_packages,
            reasons,
        ));
    }

    for path in substantive {
        if GLOBAL_FILES.contains(&path.as_str()) || starts_with_any(path, &GLOBAL_PREFIXES) {
            return Ok(full_plan(format!("global-change:{path}"), paths));
        }
        let mut direct = false;
        for suite in SUITES {
            if starts_with_any(path, direct_prefixes(suite)) {
                selected.insert(suite.to_owned(), true);
                reasons
                    .get_mut(suite)
                    .expect("known suite")
                    .push(format!("owned-path:{path}"));
                direct = true;
            }
        }
        if let Some(package) = package_for_path(root, path, packages) {
            changed_packages.insert(package.to_owned());
            for suite in SUITES {
                if closures[suite].contains(package) {
                    selected.insert(suite.to_owned(), true);
                    reasons
                        .get_mut(suite)
                        .expect("known suite")
                        .push(format!("package-dependency:{package}"));
                }
            }
        } else if !direct
            && !HARMLESS_FILES.contains(&path.as_str())
            && !starts_with_any(path, &HARMLESS_PREFIXES)
        {
            return Ok(full_plan(format!("unclassified-path:{path}"), paths));
        }
    }
    let names: Vec<_> = SUITES
        .into_iter()
        .filter(|suite| selected[*suite])
        .collect();
    let reason = if names.is_empty() {
        "no-heavyweight-obligations".to_owned()
    } else {
        format!("selected:{}", names.join(","))
    };
    Ok(plan(
        selected,
        false,
        reason,
        paths,
        changed_packages,
        reasons,
    ))
}

fn package_for_path<'a>(
    root: &Path,
    path: &str,
    packages: &'a BTreeMap<String, Package>,
) -> Option<&'a str> {
    let absolute = normalize(&root.join(path));
    packages
        .values()
        .filter(|package| absolute.starts_with(&package.directory))
        .max_by_key(|package| package.directory.components().count())
        .map(|package| package.name.as_str())
}

fn starts_with_any(path: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|prefix| path.starts_with(prefix))
}

fn plan(
    selected: BTreeMap<String, bool>,
    full_fallback: bool,
    reason: String,
    changed_paths: Vec<String>,
    changed_packages: BTreeSet<String>,
    suite_reasons: BTreeMap<String, Vec<String>>,
) -> ImpactPlan {
    ImpactPlan {
        esp32_required: selected["esp32"],
        browser_required: selected["browser"],
        conduitos_required: selected["conduitos"],
        full_fallback,
        reason,
        changed_paths,
        changed_packages: changed_packages.into_iter().collect(),
        suite_reasons,
    }
}

fn empty_reasons() -> BTreeMap<String, Vec<String>> {
    BTreeMap::from(SUITES.map(|suite| (suite.to_owned(), Vec::new())))
}

fn full_plan(reason: String, paths: Vec<String>) -> ImpactPlan {
    let selected = BTreeMap::from(SUITES.map(|suite| (suite.to_owned(), true)));
    let suite_reasons =
        BTreeMap::from(SUITES.map(|suite| (suite.to_owned(), vec![reason.clone()])));
    plan(
        selected,
        true,
        reason,
        paths,
        BTreeSet::new(),
        suite_reasons,
    )
}

fn changed_paths(root: &Path, base: &str, head: &str) -> Result<Vec<String>, String> {
    for (value, label) in [(base, "base"), (head, "head")] {
        if value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!("invalid {label} SHA"));
        }
        let status = Command::new("git")
            .args(["cat-file", "-e", &format!("{value}^{{commit}}")])
            .current_dir(root)
            .status()
            .map_err(|error| error.to_string())?;
        if !status.success() {
            return Err(format!("unknown {label} commit"));
        }
    }
    let output = Command::new("git")
        .args([
            "diff",
            "--name-only",
            "-z",
            "--diff-filter=ACDMRTUXB",
            base,
            head,
        ])
        .current_dir(root)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err("git diff failed".to_owned());
    }
    output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .map(|entry| String::from_utf8(entry.to_vec()).map_err(|error| error.to_string()))
        .collect()
}

fn write_github_outputs(plan: &ImpactPlan) {
    println!("esp32_required={}", plan.esp32_required);
    println!("browser_required={}", plan.browser_required);
    println!("conduitos_required={}", plan.conduitos_required);
    println!("full_fallback={}", plan.full_fallback);
    println!("impact_reason={}", plan.reason);
}

fn markdown_summary(plan: &ImpactPlan) -> String {
    let mut rows = String::new();
    for suite in SUITES {
        let reasons = &plan.suite_reasons[suite];
        let why = if reasons.is_empty() {
            "no dependency/ownership path".to_owned()
        } else {
            reasons.join(", ")
        };
        rows.push_str(&format!(
            "| {suite} | {} | {why} |\n",
            if required(plan, suite) {
                "run"
            } else {
                "skip on PR"
            }
        ));
    }
    format!("## CI impact plan\n\nReason: `{}`\n\nChanged packages: {}\n\n| heavyweight suite | decision | reason |\n| --- | --- | --- |\n{}\n> Pull requests may use this plan selectively. Main and merge-queue runs remain exhaustive in this slice.\n", plan.reason, if plan.changed_packages.is_empty() { "(none)".to_owned() } else { plan.changed_packages.join(", ") }, rows)
}

fn required(plan: &ImpactPlan, suite: &str) -> bool {
    match suite {
        "esp32" => plan.esp32_required,
        "browser" => plan.browser_required,
        "conduitos" => plan.conduitos_required,
        _ => false,
    }
}

fn write_parent(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
