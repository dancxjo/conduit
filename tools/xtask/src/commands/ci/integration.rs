use serde::Serialize;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

pub(super) const INTEGRATION_SCHEMA: &str = "conduit.ci.integration/v1";

#[derive(Debug, Serialize)]
pub(super) struct Integration {
    pub(super) schema: &'static str,
    pub(super) base_sha: String,
    pub(super) candidate_sha: String,
    pub(super) candidate_tree: String,
    pub(super) status: &'static str,
    pub(super) integration_tree: Option<String>,
    pub(super) effective_merge_base_sha: Option<String>,
    pub(super) effective_merge_base_tree: Option<String>,
    pub(super) merge_base_method: &'static str,
}

pub(super) fn run(
    base: &str,
    head: &str,
    json_out: Option<&Path>,
    summary_out: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = crate::workspace::workspace_root()?;
    let integration = resolve(&root, base, head)?;
    let json = format!("{}\n", serde_json::to_string_pretty(&integration)?);
    if let Some(path) = json_out {
        write_parent(path)?;
        fs::write(path, json)?;
    } else {
        print!("{json}");
    }
    if let Some(path) = summary_out {
        write_parent(path)?;
        fs::write(path, summary(&integration))?;
    }
    Ok(())
}

pub(super) fn resolve(root: &Path, base: &str, head: &str) -> Result<Integration, String> {
    let base_sha = resolve_commit(root, base)?;
    let candidate_sha = resolve_commit(root, head)?;
    let candidate_tree = resolve_tree(root, &candidate_sha)?;
    match merge_tree(root, &base_sha, &candidate_sha, None)? {
        MergeTree::Clean(tree) => {
            let merge_base = git_text(root, &["merge-base", &base_sha, &candidate_sha])?;
            Ok(clean(
                base_sha,
                candidate_sha,
                candidate_tree,
                tree,
                merge_base,
                "genealogical",
                root,
            )?)
        }
        MergeTree::Conflict => {
            let Some(effective) = exact_parent_tree_on_base(root, &base_sha, &candidate_sha)?
            else {
                return Ok(conflict(base_sha, candidate_sha, candidate_tree));
            };
            match merge_tree(root, &base_sha, &candidate_sha, Some(&effective))? {
                MergeTree::Clean(tree) => Ok(clean(
                    base_sha,
                    candidate_sha,
                    candidate_tree,
                    tree,
                    effective,
                    "content-equivalent-candidate-parent",
                    root,
                )?),
                MergeTree::Conflict => Ok(conflict(base_sha, candidate_sha, candidate_tree)),
            }
        }
    }
}

fn clean(
    base_sha: String,
    candidate_sha: String,
    candidate_tree: String,
    integration_tree: String,
    effective_merge_base_sha: String,
    method: &'static str,
    root: &Path,
) -> Result<Integration, String> {
    let effective_merge_base_tree = resolve_tree(root, &effective_merge_base_sha)?;
    Ok(Integration {
        schema: INTEGRATION_SCHEMA,
        base_sha,
        candidate_sha,
        candidate_tree,
        status: "clean",
        integration_tree: Some(integration_tree),
        effective_merge_base_sha: Some(effective_merge_base_sha),
        effective_merge_base_tree: Some(effective_merge_base_tree),
        merge_base_method: method,
    })
}

fn conflict(base_sha: String, candidate_sha: String, candidate_tree: String) -> Integration {
    Integration {
        schema: INTEGRATION_SCHEMA,
        base_sha,
        candidate_sha,
        candidate_tree,
        status: "conflict",
        integration_tree: None,
        effective_merge_base_sha: None,
        effective_merge_base_tree: None,
        merge_base_method: "none",
    }
}

fn exact_parent_tree_on_base(
    root: &Path,
    base: &str,
    head: &str,
) -> Result<Option<String>, String> {
    let parents = git_text(root, &["show", "-s", "--format=%P", head])?;
    let parent_trees = parents
        .split_whitespace()
        .map(|parent| resolve_tree(root, parent))
        .collect::<Result<Vec<_>, _>>()?;
    if parent_trees.is_empty() {
        return Ok(None);
    }
    let history = git_text(root, &["log", "--format=%H %T", base])?;
    for line in history.lines() {
        let mut fields = line.split_whitespace();
        let (Some(commit), Some(tree), None) = (fields.next(), fields.next(), fields.next()) else {
            return Err("git log emitted an invalid commit/tree record".to_owned());
        };
        if parent_trees.iter().any(|parent_tree| parent_tree == tree) {
            return Ok(Some(commit.to_owned()));
        }
    }
    Ok(None)
}

enum MergeTree {
    Clean(String),
    Conflict,
}

fn merge_tree(
    root: &Path,
    base: &str,
    head: &str,
    merge_base: Option<&str>,
) -> Result<MergeTree, String> {
    let mut command = git_command(root);
    command.args(["merge-tree", "--write-tree"]);
    if let Some(merge_base) = merge_base {
        command.arg(format!("--merge-base={merge_base}"));
    }
    let output = command
        .args([base, head])
        .output()
        .map_err(|error| format!("run git merge-tree: {error}"))?;
    if output.status.success() {
        let tree = String::from_utf8(output.stdout)
            .map_err(|_| "git merge-tree emitted non-UTF-8".to_owned())?
            .lines()
            .next()
            .unwrap_or_default()
            .trim()
            .to_owned();
        if tree.len() != 40 && tree.len() != 64 {
            return Err("git merge-tree did not emit an integration tree".to_owned());
        }
        Ok(MergeTree::Clean(tree))
    } else if output.status.code() == Some(1) {
        Ok(MergeTree::Conflict)
    } else {
        Err(command_error("git merge-tree", &output))
    }
}

fn resolve_commit(root: &Path, revision: &str) -> Result<String, String> {
    git_text(
        root,
        &["rev-parse", "--verify", &format!("{revision}^{{commit}}")],
    )
}

fn resolve_tree(root: &Path, commit: &str) -> Result<String, String> {
    git_text(
        root,
        &["rev-parse", "--verify", &format!("{commit}^{{tree}}")],
    )
}

fn git_text(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = git_command(root)
        .args(args)
        .output()
        .map_err(|error| format!("run git {}: {error}", args.join(" ")))?;
    if !output.status.success() {
        return Err(command_error(&format!("git {}", args.join(" ")), &output));
    }
    String::from_utf8(output.stdout)
        .map_err(|_| "git emitted non-UTF-8".to_owned())
        .map(|text| text.trim().to_owned())
}

fn git_command(root: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .current_dir(root)
        .arg("-c")
        .arg(format!("safe.directory={}", root.display()));
    command
}

fn command_error(label: &str, output: &Output) -> String {
    format!(
        "{label} failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    )
}

fn summary(integration: &Integration) -> String {
    format!(
        "## CI integration\n\n- candidate: `{}`\n- candidate tree: `{}`\n- base: `{}`\n- status: **{}**\n- integration tree: `{}`\n- effective merge base: `{}`\n- effective merge-base tree: `{}`\n- merge-base method: **{}**\n",
        integration.candidate_sha,
        integration.candidate_tree,
        integration.base_sha,
        integration.status,
        integration.integration_tree.as_deref().unwrap_or("unavailable"),
        integration.effective_merge_base_sha.as_deref().unwrap_or("unavailable"),
        integration.effective_merge_base_tree.as_deref().unwrap_or("unavailable"),
        integration.merge_base_method,
    )
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
#[path = "integration/tests.rs"]
mod tests;
