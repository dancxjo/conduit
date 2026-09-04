use std::path::Path;
use std::process::Command;

#[derive(Debug)]
pub(super) struct CandidateChangeSet {
    pub(super) comparison_base: String,
    pub(super) paths: Vec<String>,
}

pub(super) fn candidate_changed_paths(
    root: &Path,
    requested_base: &str,
    head: &str,
) -> Result<CandidateChangeSet, String> {
    for (value, label) in [(requested_base, "base"), (head, "head")] {
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

    let merge_base_output = Command::new("git")
        .args(["merge-base", "--all", requested_base, head])
        .current_dir(root)
        .output()
        .map_err(|error| error.to_string())?;
    if !merge_base_output.status.success() {
        return Err("git merge-base failed".to_owned());
    }
    let merge_base_stdout =
        String::from_utf8(merge_base_output.stdout).map_err(|error| error.to_string())?;
    let merge_bases = merge_base_stdout
        .lines()
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    let [comparison_base] = merge_bases.as_slice() else {
        return Err(format!(
            "expected one merge base, found {}",
            merge_bases.len()
        ));
    };

    let output = Command::new("git")
        .args([
            "diff",
            "--name-only",
            "-z",
            "--diff-filter=ACDMRTUXB",
            comparison_base,
            head,
        ])
        .current_dir(root)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err("git diff failed".to_owned());
    }
    let paths = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .map(|entry| String::from_utf8(entry.to_vec()).map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CandidateChangeSet {
        comparison_base: (*comparison_base).to_owned(),
        paths,
    })
}

#[cfg(test)]
#[path = "git_changes_tests.rs"]
mod tests;
