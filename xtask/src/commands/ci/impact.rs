use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::suites::workspace_shards::WorkspaceShard;

mod cargo_graph;
use cargo_graph::{affected_tests, dependency_closure, discover, normalize, Package};

const SUITES: [&str; 3] = ["esp32", "browser", "conduitos"];
const CONDUITOS_X86_PROOFS: [&str; 8] = [
    "kernel",
    "xhci",
    "usb",
    "hid",
    "keyboard",
    "front-door",
    "product-journey",
    "rescue",
];
const CONDUITOS_ARCHITECTURES: [&str; 4] = ["aarch64", "ia32", "riscv64", "loongarch64"];
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

#[derive(Debug, Serialize)]
struct ImpactPlan {
    esp32_required: bool,
    browser_required: bool,
    conduitos_required: bool,
    conduitos_x86_proofs: Vec<String>,
    conduitos_architectures: Vec<String>,
    conduitos_aarch64_product_required: bool,
    full_fallback: bool,
    reason: String,
    changed_paths: Vec<String>,
    changed_packages: Vec<String>,
    affected_test_packages: Vec<String>,
    workspace_shards: BTreeMap<String, bool>,
    suite_reasons: BTreeMap<String, Vec<String>>,
}

struct WorkspaceImpact {
    changed_packages: BTreeSet<String>,
    affected_test_packages: BTreeSet<String>,
    shards: BTreeMap<String, bool>,
}

#[derive(Default)]
struct ConduitosImpact {
    x86_proofs: BTreeSet<String>,
    architectures: BTreeSet<String>,
    aarch64_product: bool,
}

impl ConduitosImpact {
    fn all() -> Self {
        Self {
            x86_proofs: CONDUITOS_X86_PROOFS
                .map(str::to_owned)
                .into_iter()
                .collect(),
            architectures: CONDUITOS_ARCHITECTURES
                .map(str::to_owned)
                .into_iter()
                .collect(),
            aarch64_product: true,
        }
    }

    fn required(&self) -> bool {
        !self.x86_proofs.is_empty() || !self.architectures.is_empty() || self.aarch64_product
    }
}

pub(super) fn run(
    base: &str,
    head: &str,
    json_out: Option<&Path>,
    summary_out: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = crate::workspace::workspace_root()?;
    let plan = match changed_paths(&root, base, head)
        .and_then(|paths| discover(&root).map(|packages| (paths, packages)))
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
    let mut conduitos = ConduitosImpact::default();
    let mut reasons = empty_reasons();
    let mut changed_packages = BTreeSet::new();
    let substantive: Vec<_> = paths.iter().filter(|path| !path.ends_with(".md")).collect();
    if substantive.is_empty() {
        return Ok(plan(
            selected,
            conduitos,
            false,
            "markdown-only".to_owned(),
            paths,
            WorkspaceImpact {
                changed_packages,
                affected_test_packages: BTreeSet::new(),
                shards: WorkspaceShard::ALL
                    .into_iter()
                    .map(|shard| (shard.name().to_owned(), false))
                    .collect(),
            },
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
                if suite == "conduitos" {
                    select_conduitos_path(path, &mut conduitos);
                }
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
                    if suite == "conduitos" && !starts_with_any(path, direct_prefixes("conduitos"))
                    {
                        conduitos = ConduitosImpact::all();
                    }
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
    let affected = affected_tests(packages, &changed_packages);
    let workspace_shards = workspace_shards_for(&affected);
    selected.insert("conduitos".to_owned(), conduitos.required());
    Ok(plan(
        selected,
        conduitos,
        false,
        reason,
        paths,
        WorkspaceImpact {
            changed_packages,
            affected_test_packages: affected,
            shards: workspace_shards,
        },
        reasons,
    ))
}

fn select_conduitos_path(path: &str, impact: &mut ConduitosImpact) {
    if path == "hosts/conduitos/src/bin/aarch64_product.rs"
        || path == "profiles/hosts/conduitos-aarch64-headless.profile.json"
    {
        impact.aarch64_product = true;
        return;
    }
    if path.starts_with("hosts/conduitos/src/arch/x86_64/xhci") {
        // Every retained x86 appliance boots through the shared xHCI-enabled
        // image, including the front-door and product-journey keyboard paths.
        impact
            .x86_proofs
            .extend(CONDUITOS_X86_PROOFS.map(str::to_owned));
        return;
    }
    if path.starts_with("hosts/conduitos/src/arch/x86_64/") {
        impact
            .x86_proofs
            .extend(CONDUITOS_X86_PROOFS.map(str::to_owned));
        return;
    }
    for architecture in CONDUITOS_ARCHITECTURES {
        if path.starts_with(&format!("hosts/conduitos/proof-appliances/{architecture}/")) {
            impact.architectures.insert(architecture.to_owned());
            return;
        }
        if path.starts_with(&format!("hosts/conduitos/src/arch/{architecture}/")) {
            impact.architectures.insert(architecture.to_owned());
            if architecture == "aarch64" {
                impact.aarch64_product = true;
            }
            return;
        }
        if path.starts_with(&format!(
            "hosts/conduitos/fabrication/xtask/{architecture}_"
        )) {
            impact.architectures.insert(architecture.to_owned());
            return;
        }
    }
    *impact = ConduitosImpact::all();
}

fn workspace_shards_for(affected: &BTreeSet<String>) -> BTreeMap<String, bool> {
    WorkspaceShard::ALL
        .into_iter()
        .map(|shard| {
            let required = matches!(
                shard,
                WorkspaceShard::Lint | WorkspaceShard::Portable | WorkspaceShard::Pico
            ) || shard
                .test_packages()
                .iter()
                .any(|package| affected.contains(*package));
            (shard.name().to_owned(), required)
        })
        .collect()
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
    conduitos: ConduitosImpact,
    full_fallback: bool,
    reason: String,
    changed_paths: Vec<String>,
    workspace: WorkspaceImpact,
    suite_reasons: BTreeMap<String, Vec<String>>,
) -> ImpactPlan {
    ImpactPlan {
        esp32_required: selected["esp32"],
        browser_required: selected["browser"],
        conduitos_required: selected["conduitos"],
        conduitos_x86_proofs: conduitos.x86_proofs.into_iter().collect(),
        conduitos_architectures: conduitos.architectures.into_iter().collect(),
        conduitos_aarch64_product_required: conduitos.aarch64_product,
        full_fallback,
        reason,
        changed_paths,
        changed_packages: workspace.changed_packages.into_iter().collect(),
        affected_test_packages: workspace.affected_test_packages.into_iter().collect(),
        workspace_shards: workspace.shards,
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
        ConduitosImpact::all(),
        true,
        reason,
        paths,
        WorkspaceImpact {
            changed_packages: BTreeSet::new(),
            affected_test_packages: BTreeSet::new(),
            shards: WorkspaceShard::ALL
                .into_iter()
                .map(|shard| (shard.name().to_owned(), true))
                .collect(),
        },
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
    println!(
        "conduitos_x86_matrix={}",
        serde_json::to_string(&plan.conduitos_x86_proofs).expect("ConduitOS x86 matrix serializes")
    );
    println!(
        "conduitos_architecture_matrix={}",
        serde_json::to_string(&plan.conduitos_architectures)
            .expect("ConduitOS architecture matrix serializes")
    );
    println!(
        "conduitos_aarch64_product_required={}",
        plan.conduitos_aarch64_product_required
    );
    println!("full_fallback={}", plan.full_fallback);
    println!("impact_reason={}", plan.reason);
    let matrix: Vec<_> = WorkspaceShard::ALL
        .into_iter()
        .filter(|shard| plan.workspace_shards[shard.name()])
        .map(WorkspaceShard::name)
        .collect();
    println!(
        "workspace_matrix={}",
        serde_json::to_string(&matrix).expect("workspace matrix serializes")
    );
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
    for shard in WorkspaceShard::ALL {
        rows.push_str(&format!(
            "| workspace/{} | {} | {} |\n",
            shard.name(),
            if plan.workspace_shards[shard.name()] {
                "run"
            } else {
                "skip on PR"
            },
            if matches!(
                shard,
                WorkspaceShard::Lint | WorkspaceShard::Portable | WorkspaceShard::Pico
            ) {
                "permanent product spine"
            } else if plan.workspace_shards[shard.name()] {
                "affected reverse-dependent package"
            } else {
                "no affected package assigned to shard"
            }
        ));
    }
    for proof in CONDUITOS_X86_PROOFS {
        let selected = plan
            .conduitos_x86_proofs
            .iter()
            .any(|candidate| candidate == proof);
        rows.push_str(&format!(
            "| conduitos/x86/{proof} | {} | {} |\n",
            if selected { "run" } else { "skip on PR" },
            if selected {
                "affected ConduitOS x86 claim"
            } else {
                "no dependency/ownership path to proof"
            }
        ));
    }
    for architecture in CONDUITOS_ARCHITECTURES {
        let selected = plan
            .conduitos_architectures
            .iter()
            .any(|candidate| candidate == architecture);
        rows.push_str(&format!(
            "| conduitos/architecture/{architecture} | {} | {} |\n",
            if selected { "run" } else { "skip on PR" },
            if selected {
                "affected architecture claim"
            } else {
                "no dependency/ownership path to architecture"
            }
        ));
    }
    rows.push_str(&format!(
        "| conduitos/aarch64-product | {} | {} |\n",
        if plan.conduitos_aarch64_product_required {
            "run"
        } else {
            "skip on PR"
        },
        if plan.conduitos_aarch64_product_required {
            "affected product claim"
        } else {
            "no dependency/ownership path to product"
        }
    ));
    format!("## CI impact plan\n\nReason: `{}`\n\nChanged packages: {}\n\nAffected test packages: {}\n\n| obligation | decision | reason |\n| --- | --- | --- |\n{}\n> Pull requests may use this plan selectively. Main and merge-queue runs remain exhaustive.\n", plan.reason, if plan.changed_packages.is_empty() { "(none)".to_owned() } else { plan.changed_packages.join(", ") }, if plan.affected_test_packages.is_empty() { "(none)".to_owned() } else { plan.affected_test_packages.join(", ") }, rows)
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
