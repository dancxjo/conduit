use std::collections::BTreeSet;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug)]
pub(super) struct CandidateChangeSet {
    pub(super) comparison_base: String,
    pub(super) paths: Vec<String>,
    pub(super) test_extraction_parents: Vec<String>,
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
    let test_extraction_parents = test_extraction_parents(root, comparison_base, head, &paths)?;
    Ok(CandidateChangeSet {
        comparison_base: (*comparison_base).to_owned(),
        paths,
        test_extraction_parents,
    })
}

fn test_extraction_parents(
    root: &Path,
    base: &str,
    head: &str,
    paths: &[String],
) -> Result<Vec<String>, String> {
    let changed = paths.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let children = paths
        .iter()
        .filter(|path| path.ends_with("/tests.rs"))
        .collect::<Vec<_>>();
    if children.is_empty() || children.len() * 2 != paths.len() {
        return Ok(Vec::new());
    }

    let mut parents = Vec::new();
    for child in children {
        let parent = format!("{}.rs", child.trim_end_matches("/tests.rs"));
        if !changed.contains(parent.as_str())
            || git_path_exists(root, base, child)?
            || !git_path_exists(root, head, child)?
            || !exact_test_module_extraction(
                &git_file(root, base, &parent)?,
                &git_file(root, head, &parent)?,
                &git_file(root, head, child)?,
            )
        {
            return Ok(Vec::new());
        }
        parents.push(parent);
    }
    parents.sort();
    Ok(parents)
}

fn git_path_exists(root: &Path, commit: &str, path: &str) -> Result<bool, String> {
    let status = Command::new("git")
        .args(["cat-file", "-e", &format!("{commit}:{path}")])
        .current_dir(root)
        .stderr(Stdio::null())
        .status()
        .map_err(|error| error.to_string())?;
    Ok(status.success())
}

fn git_file(root: &Path, commit: &str, path: &str) -> Result<String, String> {
    let output = Command::new("git")
        .args(["show", &format!("{commit}:{path}")])
        .current_dir(root)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!("cannot read {path} at {commit}"));
    }
    String::from_utf8(output.stdout).map_err(|error| error.to_string())
}

fn exact_test_module_extraction(before: &str, after: &str, child: &str) -> bool {
    const INLINE: &str = "\n#[cfg(test)]\nmod tests {\n";
    const EXTERNAL: &str = "\n#[cfg(test)]\nmod tests;\n";
    let Some((prefix, body_with_close)) = before.rsplit_once(INLINE) else {
        return false;
    };
    if body_with_close.contains(INLINE) || !body_with_close.ends_with("}\n") {
        return false;
    }
    if after != format!("{prefix}{EXTERNAL}") {
        return false;
    }
    let body = &body_with_close[..body_with_close.len() - 2];
    let Some(extracted) = deindent_one_level(body) else {
        return false;
    };
    equivalent_test_source(&extracted, child) && has_only_location_insensitive_test_macros(child)
}

fn equivalent_test_source(before: &str, after: &str) -> bool {
    let normalized = |source: &str| {
        let mut compact = source
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        for suffix in [",)", ",]", ",}"] {
            while compact.contains(suffix) {
                compact = compact.replace(suffix, &suffix[1..]);
            }
        }
        compact
    };
    normalized(before) == normalized(after)
        && ordinary_string_literals(before).is_some()
        && ordinary_string_literals(before) == ordinary_string_literals(after)
}

fn ordinary_string_literals(source: &str) -> Option<Vec<String>> {
    if contains_raw_string(source) {
        return None;
    }
    let bytes = source.as_bytes();
    let mut strings = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'"' {
            index += 1;
            continue;
        }
        let start = index;
        index += 1;
        let mut escaped = false;
        let mut closed = false;
        while index < bytes.len() {
            let byte = bytes[index];
            index += 1;
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                strings.push(source[start..index].to_owned());
                closed = true;
                break;
            }
        }
        if !closed {
            return None;
        }
    }
    Some(strings)
}

fn contains_raw_string(source: &str) -> bool {
    source.match_indices('r').any(|(index, _)| {
        let suffix = &source[index + 1..];
        let starts_raw_delimiter = suffix.starts_with('"') || suffix.starts_with('#');
        let starts_token = source[..index]
            .chars()
            .next_back()
            .is_none_or(|before| !before.is_ascii_alphanumeric() && before != '_');
        starts_raw_delimiter && starts_token
    })
}

fn deindent_one_level(body: &str) -> Option<String> {
    if !body.ends_with('\n') {
        return None;
    }
    let mut result = String::new();
    for line in body.split_inclusive('\n') {
        if line == "\n" {
            result.push('\n');
        } else {
            result.push_str(line.strip_prefix("    ")?);
        }
    }
    Some(result)
}

fn has_only_location_insensitive_test_macros(source: &str) -> bool {
    const ALLOWED: &[&str] = &[
        "assert",
        "assert_eq",
        "assert_ne",
        "debug_assert",
        "debug_assert_eq",
        "debug_assert_ne",
        "format",
        "matches",
        "panic",
        "unreachable",
        "vec",
    ];
    for (index, _) in source.match_indices('!') {
        let identifier = source[..index]
            .chars()
            .rev()
            .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();
        if !identifier.is_empty() && !ALLOWED.contains(&identifier.as_str()) {
            return false;
        }
    }
    !source.contains("#[path")
        && !source.contains("include!")
        && !source.contains("include_str!")
        && !source.contains("include_bytes!")
        && !source.contains("file!")
        && !source.contains("line!")
        && !source.contains("column!")
        && !source.contains("\nmod ")
}

#[cfg(test)]
#[path = "git_changes_tests.rs"]
mod tests;
