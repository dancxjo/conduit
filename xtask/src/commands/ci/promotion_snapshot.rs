use std::{
    io::Write,
    process::{Command, Stdio},
};

const SNAPSHOT_PREFIX: &str = "promote/";

pub fn run(
    dev_ref: &str,
    main_ref: &str,
    remote: &str,
    push: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let dev_sha = resolve_commit(dev_ref)?;
    let main_sha = resolve_commit(main_ref)?;
    let dev_tree = resolve_tree(&dev_sha)?;
    if dev_tree == resolve_tree(&main_sha)? {
        return Err(
            "development and stable trees are already identical; no promotion needed".into(),
        );
    }
    let promotion_sha = create_promotion_commit(&dev_sha, &main_sha, &dev_tree)?;
    let branch = snapshot_branch(&promotion_sha)?;

    if push {
        let destination = format!("refs/heads/{branch}");
        let source_destination = format!("{promotion_sha}:{destination}");
        let status = Command::new("git")
            .args(["push", remote, &source_destination])
            .status()?;
        if !status.success() {
            return Err(format!(
                "refused to overwrite {remote}/{branch}; fetch dev and choose a fresh snapshot"
            )
            .into());
        }
    }

    println!("snapshot_sha={dev_sha}");
    println!("promotion_sha={promotion_sha}");
    println!("snapshot_branch={branch}");
    println!("promotion_base=main");
    Ok(())
}

fn resolve_commit(reference: &str) -> Result<String, Box<dyn std::error::Error>> {
    let sha = git_output(["rev-parse", "--verify", &format!("{reference}^{{commit}}")])?;
    validate_commit_sha(&sha)?;
    Ok(sha)
}

fn resolve_tree(commit: &str) -> Result<String, Box<dyn std::error::Error>> {
    git_output(["rev-parse", "--verify", &format!("{commit}^{{tree}}")])
}

fn create_promotion_commit(
    dev_sha: &str,
    main_sha: &str,
    tree: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let timestamp = git_output(["show", "-s", "--format=%aI", dev_sha])?;
    let mut child = Command::new("git")
        .args(["commit-tree", tree, "-p", main_sha, "-p", dev_sha])
        .env("GIT_AUTHOR_NAME", "Conduit Promotion")
        .env("GIT_AUTHOR_EMAIL", "promotion@conduit.invalid")
        .env("GIT_AUTHOR_DATE", &timestamp)
        .env("GIT_COMMITTER_NAME", "Conduit Promotion")
        .env("GIT_COMMITTER_EMAIL", "promotion@conduit.invalid")
        .env("GIT_COMMITTER_DATE", &timestamp)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    write!(
        child
            .stdin
            .as_mut()
            .ok_or("cannot write promotion commit")?,
        "Promote frozen dev snapshot {dev_sha}\n\nConduit-Dev-Snapshot: {dev_sha}\n"
    )?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr)
            .trim()
            .to_owned()
            .into());
    }
    let sha = String::from_utf8(output.stdout)?.trim().to_owned();
    validate_commit_sha(&sha)?;
    Ok(sha)
}

fn git_output<const N: usize>(args: [&str; N]) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git").args(args).output()?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr)
            .trim()
            .to_owned()
            .into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn validate_commit_sha(sha: &str) -> Result<(), Box<dyn std::error::Error>> {
    if sha.len() != 40 || !sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("development ref resolved to invalid commit identity {sha:?}").into());
    }
    Ok(())
}

fn snapshot_branch(sha: &str) -> Result<String, Box<dyn std::error::Error>> {
    validate_commit_sha(sha)?;
    Ok(format!("{SNAPSHOT_PREFIX}{sha}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_ref_contains_the_complete_immutable_commit_identity() {
        let sha = "0123456789abcdef0123456789abcdef01234567";
        assert_eq!(snapshot_branch(sha).unwrap(), format!("promote/{sha}"));
    }

    #[test]
    fn abbreviated_or_non_hex_identities_fail_closed() {
        assert!(snapshot_branch("0123456").is_err());
        assert!(snapshot_branch("g123456789abcdef0123456789abcdef01234567").is_err());
    }

    #[test]
    fn promotion_commit_has_exact_tree_and_both_histories_as_parents() {
        let dev = resolve_commit("HEAD").unwrap();
        let main = resolve_commit("HEAD^").unwrap();
        let tree = resolve_tree(&dev).unwrap();
        let promotion = create_promotion_commit(&dev, &main, &tree).unwrap();

        assert_eq!(
            git_output(["show", "-s", "--format=%P", &promotion]).unwrap(),
            format!("{main} {dev}")
        );
        assert_eq!(resolve_tree(&promotion).unwrap(), tree);
        assert_eq!(
            git_output([
                "show",
                "-s",
                "--format=%(trailers:key=Conduit-Dev-Snapshot,valueonly)",
                &promotion,
            ])
            .unwrap(),
            dev
        );
    }
}
