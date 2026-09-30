//! Concrete std-Host source workspace for the bounded make Base.

use conduit_host_make::{
    BuilderBounds, BuilderHostAdapter, BuilderRefusal, BuilderSource, HostImage,
    RealizedBuilderArtifact, SealedBuilderSource, ToolchainGrant,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

/// Package-owned build machinery. Implementations expose reviewed operations,
/// not an ambient command or shell string.
pub trait ReviewedBuilderBackend {
    fn prepare_exact_toolchain(
        &mut self,
        identity: &str,
        allow_acquisition: bool,
        permitted_origins: &[String],
        workspace: &Path,
        bounds: &BuilderBounds,
    ) -> Result<(), BuilderRefusal>;

    fn build_exact_image(
        &mut self,
        image: &HostImage,
        image_description: &[u8],
        source_root: &Path,
        workspace: &Path,
        output: &Path,
        bounds: &BuilderBounds,
    ) -> Result<RealizedBuilderArtifact, BuilderRefusal>;
}

/// The std-only adapter grants source reads and exact package operations. It
/// deliberately contains no repository mutation or arbitrary shell entrance.
pub struct StdBuilderAdapter<B> {
    permitted_local_roots: Vec<PathBuf>,
    permitted_network_origins: Vec<String>,
    backend: B,
}

impl<B> StdBuilderAdapter<B> {
    pub fn new(
        permitted_local_roots: Vec<PathBuf>,
        permitted_network_origins: Vec<String>,
        backend: B,
    ) -> Self {
        Self {
            permitted_local_roots,
            permitted_network_origins,
            backend,
        }
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    fn acquire_source(
        &self,
        source: &BuilderSource,
        workspace: &Path,
        bounds: &BuilderBounds,
    ) -> Result<SealedBuilderSource, BuilderRefusal> {
        match source {
            BuilderSource::CurrentCheckout { root } | BuilderSource::LocalCheckout { root } => {
                let root = canonical_existing(root)?;
                let authorized = self.permitted_local_roots.iter().any(|allowed| {
                    allowed
                        .canonicalize()
                        .is_ok_and(|allowed| root.starts_with(allowed))
                });
                if !authorized {
                    return Err(BuilderRefusal::UnauthorizedSource);
                }
                seal_git_tree(root, None, bounds)
            }
            BuilderSource::RemoteGit {
                https_url,
                revision,
            } => {
                if !exact_git_revision(revision)
                    || !self
                        .permitted_network_origins
                        .iter()
                        .any(|origin| url_belongs_to_origin(https_url, origin))
                {
                    return Err(BuilderRefusal::UnauthorizedNetwork);
                }
                let root = workspace.join("source");
                if root.exists() {
                    return Err(BuilderRefusal::WorkspaceUnavailable);
                }
                fs::create_dir_all(workspace).map_err(|_| BuilderRefusal::WorkspaceUnavailable)?;
                run_git(workspace, &["init", "--quiet", "source"], bounds)?;
                run_git(&root, &["remote", "add", "origin", https_url], bounds)?;
                run_git(
                    &root,
                    &["fetch", "--quiet", "--depth", "1", "origin", revision],
                    bounds,
                )?;
                run_git(
                    &root,
                    &["checkout", "--quiet", "--detach", "FETCH_HEAD"],
                    bounds,
                )?;
                let sealed = seal_git_tree(root, Some(https_url.clone()), bounds)?;
                if sealed.revision != *revision {
                    return Err(BuilderRefusal::StaleRevision);
                }
                Ok(sealed)
            }
        }
    }
}

impl<B: ReviewedBuilderBackend> BuilderHostAdapter for StdBuilderAdapter<B> {
    fn seal_source(
        &mut self,
        source: &BuilderSource,
        workspace: &Path,
        bounds: &BuilderBounds,
    ) -> Result<SealedBuilderSource, BuilderRefusal> {
        self.acquire_source(source, workspace, bounds)
    }

    fn prepare_toolchain(
        &mut self,
        required_identity: &str,
        grant: &ToolchainGrant,
        workspace: &Path,
        bounds: &BuilderBounds,
    ) -> Result<(), BuilderRefusal> {
        if required_identity != grant.identity {
            return Err(BuilderRefusal::ToolchainUnauthorized);
        }
        self.backend.prepare_exact_toolchain(
            required_identity,
            grant.allow_acquisition,
            &grant.permitted_origins,
            workspace,
            bounds,
        )
    }

    fn realize(
        &mut self,
        image: &HostImage,
        image_description: &[u8],
        source: &SealedBuilderSource,
        workspace: &Path,
        output: &Path,
        bounds: &BuilderBounds,
    ) -> Result<RealizedBuilderArtifact, BuilderRefusal> {
        self.backend.build_exact_image(
            image,
            image_description,
            &source.root,
            workspace,
            output,
            bounds,
        )
    }
}

fn canonical_existing(root: &Path) -> Result<PathBuf, BuilderRefusal> {
    if !root.exists() {
        return Err(BuilderRefusal::MissingSource);
    }
    root.canonicalize()
        .map_err(|_| BuilderRefusal::MissingSource)
}

fn seal_git_tree(
    root: PathBuf,
    remote_origin: Option<String>,
    bounds: &BuilderBounds,
) -> Result<SealedBuilderSource, BuilderRefusal> {
    let revision = git_text(&root, &["rev-parse", "HEAD"], bounds)?;
    if !exact_git_revision(&revision) {
        return Err(BuilderRefusal::MalformedArtifact);
    }
    let dirty = !git_bytes(
        &root,
        &["status", "--porcelain=v1", "--untracked-files=all"],
        bounds,
    )?
    .is_empty();
    let names = git_bytes(
        &root,
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ],
        bounds,
    )?;
    let mut hasher = Sha256::new();
    let mut files = 0_u32;
    let mut bytes = 0_u64;
    for raw in names
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
    {
        let relative = std::str::from_utf8(raw).map_err(|_| BuilderRefusal::MalformedArtifact)?;
        let content = fs::read(root.join(relative)).map_err(|_| BuilderRefusal::MissingSource)?;
        files = files
            .checked_add(1)
            .ok_or(BuilderRefusal::SourceBoundExhausted)?;
        bytes = bytes
            .checked_add(content.len() as u64)
            .ok_or(BuilderRefusal::SourceBoundExhausted)?;
        if files > bounds.maximum_source_files || bytes > bounds.maximum_source_bytes {
            return Err(BuilderRefusal::SourceBoundExhausted);
        }
        hasher.update((raw.len() as u64).to_le_bytes());
        hasher.update(raw);
        hasher.update((content.len() as u64).to_le_bytes());
        hasher.update(content);
    }
    let content_sha256 = format!("sha256:{:x}", hasher.finalize());
    Ok(SealedBuilderSource {
        source_identity: format!("git:{revision}+{content_sha256}"),
        revision,
        content_sha256,
        root,
        files,
        bytes,
        dirty,
        remote_origin,
    })
}

fn run_git(
    root: &Path,
    arguments: &[&str],
    bounds: &BuilderBounds,
) -> Result<Output, BuilderRefusal> {
    if arguments.len() > bounds.maximum_processes as usize + 8 {
        return Err(BuilderRefusal::BoundExhausted);
    }
    let output = Command::new("git")
        .current_dir(root)
        .args(arguments)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|_| BuilderRefusal::MissingSource)?;
    let evidence = output.stdout.len() as u64 + output.stderr.len() as u64;
    if evidence > bounds.maximum_evidence_bytes {
        return Err(BuilderRefusal::BoundExhausted);
    }
    if !output.status.success() {
        return Err(BuilderRefusal::BuildFailed);
    }
    Ok(output)
}

fn git_bytes(
    root: &Path,
    arguments: &[&str],
    bounds: &BuilderBounds,
) -> Result<Vec<u8>, BuilderRefusal> {
    Ok(run_git(root, arguments, bounds)?.stdout)
}

fn git_text(
    root: &Path,
    arguments: &[&str],
    bounds: &BuilderBounds,
) -> Result<String, BuilderRefusal> {
    String::from_utf8(git_bytes(root, arguments, bounds)?)
        .map(|value| value.trim().to_owned())
        .map_err(|_| BuilderRefusal::MalformedArtifact)
}

fn exact_git_revision(revision: &str) -> bool {
    matches!(revision.len(), 40 | 64) && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn url_belongs_to_origin(url: &str, origin: &str) -> bool {
    url.starts_with("https://")
        && (url == origin
            || url
                .strip_prefix(origin)
                .is_some_and(|suffix| suffix.starts_with('/')))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn remote_source_requires_https_permitted_origin_and_exact_revision() {
        assert!(url_belongs_to_origin(
            "https://code.example/conduit.git",
            "https://code.example"
        ));
        assert!(!url_belongs_to_origin(
            "https://code.example.evil/conduit.git",
            "https://code.example"
        ));
        assert!(exact_git_revision(&"a".repeat(40)));
        assert!(!exact_git_revision("main"));
    }

    #[test]
    fn local_checkout_is_content_sealed_and_dirty_state_is_explicit() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("conduit-builder-source-{suffix}"));
        fs::create_dir_all(&root).unwrap();
        let cleanup = Cleanup(root.clone());
        Command::new("git")
            .current_dir(&root)
            .args(["init", "--quiet"])
            .status()
            .unwrap();
        Command::new("git")
            .current_dir(&root)
            .args(["config", "user.email", "builder@example.invalid"])
            .status()
            .unwrap();
        Command::new("git")
            .current_dir(&root)
            .args(["config", "user.name", "Builder Test"])
            .status()
            .unwrap();
        fs::write(root.join("source.txt"), b"exact source\n").unwrap();
        Command::new("git")
            .current_dir(&root)
            .args(["add", "source.txt"])
            .status()
            .unwrap();
        Command::new("git")
            .current_dir(&root)
            .args(["commit", "--quiet", "-m", "source"])
            .status()
            .unwrap();
        let sealed = seal_git_tree(root.clone(), None, &test_bounds()).unwrap();
        assert!(!sealed.dirty);
        assert!(sealed.source_identity.contains(&sealed.revision));
        assert!(sealed.source_identity.contains(&sealed.content_sha256));

        fs::write(root.join("source.txt"), b"changed source\n").unwrap();
        let dirty = seal_git_tree(root, None, &test_bounds()).unwrap();
        assert!(dirty.dirty);
        assert_ne!(sealed.content_sha256, dirty.content_sha256);
        drop(cleanup);
    }

    fn test_bounds() -> BuilderBounds {
        BuilderBounds {
            maximum_source_files: 100,
            maximum_source_bytes: 1_000_000,
            maximum_workspace_bytes: 2_000_000,
            maximum_output_bytes: 1_000_000,
            maximum_processes: 8,
            maximum_seconds: 60,
            maximum_evidence_bytes: 100_000,
        }
    }

    struct Cleanup(PathBuf);

    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
