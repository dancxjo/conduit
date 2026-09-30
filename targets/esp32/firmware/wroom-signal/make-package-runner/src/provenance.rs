use std::{fs, path::Path};

use sha2::{Digest, Sha256};

use crate::process::{run_capture, ExecutedCommand};

#[derive(Debug)]
pub struct InputProvenance {
    pub source_sha: String,
    pub tracked_input_count: usize,
    pub tracked_inputs_sha256: String,
    pub input_state: &'static str,
    pub dirty_status_sha256: Option<String>,
}

pub fn inspect(
    repo_root: &Path,
    allow_dirty: bool,
    commands: &mut Vec<ExecutedCommand>,
) -> Result<InputProvenance, Box<dyn std::error::Error>> {
    let canonical = repo_root.canonicalize()?;
    if canonical != repo_root {
        return Err("--repo-root must be an absolute canonical path".into());
    }
    let top = text(run_capture(
        ExecutedCommand::new(
            "resolve-repository-root",
            repo_root,
            "git",
            ["rev-parse", "--show-toplevel"],
        ),
        commands,
    )?)?;
    if Path::new(&top).canonicalize()? != repo_root {
        return Err("--repo-root is not the exact Git repository root".into());
    }

    let status = run_capture(
        ExecutedCommand::new(
            "inspect-input-cleanliness",
            repo_root,
            "git",
            ["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        ),
        commands,
    )?;
    if !status.is_empty() && !allow_dirty {
        return Err(
            "repository inputs are dirty; commit them or use --allow-dirty for local development"
                .into(),
        );
    }

    let source_sha = text(run_capture(
        ExecutedCommand::new(
            "resolve-source-commit",
            repo_root,
            "git",
            ["rev-parse", "HEAD"],
        ),
        commands,
    )?)?;
    let tracked = run_capture(
        ExecutedCommand::new(
            "enumerate-tracked-inputs",
            repo_root,
            "git",
            ["ls-files", "-z"],
        ),
        commands,
    )?;
    let paths = tracked
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .collect::<Vec<_>>();
    let tracked_inputs_sha256 = digest_tracked(repo_root, &paths)?;
    Ok(InputProvenance {
        source_sha,
        tracked_input_count: paths.len(),
        tracked_inputs_sha256,
        input_state: if status.is_empty() {
            "clean"
        } else {
            "dirty-override"
        },
        dirty_status_sha256: (!status.is_empty()).then(|| sha256(&status)),
    })
}

fn digest_tracked(repo_root: &Path, paths: &[&[u8]]) -> Result<String, Box<dyn std::error::Error>> {
    let mut digest = Sha256::new();
    for path_bytes in paths {
        let path_text = std::str::from_utf8(path_bytes)?;
        let path = repo_root.join(path_text);
        let metadata = fs::symlink_metadata(&path)?;
        let contents = if metadata.file_type().is_symlink() {
            fs::read_link(&path)?.to_string_lossy().as_bytes().to_vec()
        } else {
            fs::read(&path)?
        };
        frame(&mut digest, path_bytes);
        frame(&mut digest, &contents);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn frame(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_le_bytes());
    digest.update(value);
}

pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn text(bytes: Vec<u8>) -> Result<String, Box<dyn std::error::Error>> {
    Ok(String::from_utf8(bytes)?.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framing_distinguishes_ambiguous_concatenation() {
        let mut left = Sha256::new();
        frame(&mut left, b"ab");
        frame(&mut left, b"c");
        let mut right = Sha256::new();
        frame(&mut right, b"a");
        frame(&mut right, b"bc");
        assert_ne!(
            format!("{:x}", left.finalize()),
            format!("{:x}", right.finalize())
        );
    }
}
