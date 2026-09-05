use std::process::Command;

const SNAPSHOT_PREFIX: &str = "promote/";

pub fn run(dev_ref: &str, remote: &str, push: bool) -> Result<(), Box<dyn std::error::Error>> {
    let sha = git_output(["rev-parse", "--verify", &format!("{dev_ref}^{{commit}}")])?;
    validate_commit_sha(&sha)?;
    let branch = snapshot_branch(&sha)?;

    if push {
        let destination = format!("refs/heads/{branch}");
        let source_destination = format!("{sha}:{destination}");
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

    println!("snapshot_sha={sha}");
    println!("snapshot_branch={branch}");
    println!("promotion_base=main");
    Ok(())
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
}
