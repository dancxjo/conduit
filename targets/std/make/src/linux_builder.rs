//! Reviewed Linux source builder. This is trusted-source execution with monitored
//! limits, not a sandbox or hostile-code confinement boundary. Build scripts run
//! with the builder user's authority. Cargo is offline. The owning operation must
//! explicitly admit a local pinned boot archive; no package or toolchain network
//! acquisition is admitted. These restrictions are not OS network confinement.

use crate::ReviewedBuilderBackend;
use conduit_host_make::{
    canonical_host_configuration_conduit, canonical_profile_json, verify_image_binding,
    BuilderBounds, BuilderRefusal, CheckedHostConfiguration, HostImage, RealizedBuilderArtifact,
    SporeOutputKind,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};

#[path = "linux_builder_assets.rs"]
mod assets;
pub use assets::ReviewedConduitosBootAssets;
#[path = "linux_builder_process.rs"]
mod process;
#[path = "linux_builder_source.rs"]
mod source;
#[path = "linux_builder_verification.rs"]
mod verification;
use process::Execution;

/// A finite supported target operation, selected from checked Host construction.
/// The source must contain the ordinary reviewed xtask implementation. This
/// backend never accepts a shell command, executable override, or raw PROFILE.
pub struct LinuxSourceBuilder {
    configuration: CheckedHostConfiguration,
    cancelled: Arc<AtomicBool>,
    prepared: Option<String>,
    started: Option<Instant>,
    prepared_evidence: Option<process::ExecutionEvidence>,
    boot_assets: Option<ReviewedConduitosBootAssets>,
}

impl LinuxSourceBuilder {
    pub fn new(configuration: CheckedHostConfiguration, cancelled: Arc<AtomicBool>) -> Self {
        Self {
            configuration,
            cancelled,
            prepared: None,
            started: None,
            prepared_evidence: None,
            boot_assets: None,
        }
    }

    /// Explicit local boot-package custody supplied by the admitted Host owner.
    /// A source-read or installed-toolchain grant never implies this authority.
    pub fn with_reviewed_boot_assets(mut self, assets: ReviewedConduitosBootAssets) -> Self {
        self.boot_assets = Some(assets);
        self
    }

    fn selection(&self) -> Result<Selection, BuilderRefusal> {
        Selection::for_target(&self.configuration.profile().target.key())
    }
}

#[derive(Clone, Copy)]
struct Selection {
    architecture: &'static str,
    rust_target: &'static str,
}
impl Selection {
    fn for_target(target: &str) -> Result<Self, BuilderRefusal> {
        let (architecture, rust_target) = match target {
            "conduitos/x86_64/pc" => ("x86_64", "x86_64-unknown-none"),
            "conduitos/aarch64/virt" => ("aarch64", "aarch64-unknown-none"),
            "conduitos/riscv64/virt" => ("riscv64", "riscv64gc-unknown-none-elf"),
            "conduitos/loongarch64/virt" => ("loongarch64", "loongarch64-unknown-none"),
            // IA32's object/linker preparation is a different toolchain contract.
            _ => return Err(BuilderRefusal::UnsupportedAdapter),
        };
        Ok(Self {
            architecture,
            rust_target,
        })
    }
    fn toolchain(self) -> String {
        format!("rustc:stable+{}+llvm-tools", self.rust_target)
    }
    fn adapter(self) -> String {
        format!("conduit-host-conduitos/build-{}@1", self.architecture)
    }
    fn image_file(self) -> String {
        format!("conduitos-{}.iso", self.architecture)
    }
}

impl ReviewedBuilderBackend for LinuxSourceBuilder {
    fn seal_exact_source(
        &mut self,
        selected: &conduit_host_make::BuilderSource,
        workspace: &Path,
        bounds: &BuilderBounds,
        local_roots: &[PathBuf],
        network_origins: &[String],
    ) -> Option<Result<conduit_host_make::SealedBuilderSource, BuilderRefusal>> {
        Some((|| {
            if !bounds.valid() {
                return Err(BuilderRefusal::InvalidRequest);
            }
            if self.cancelled.load(Ordering::Acquire) {
                return Err(BuilderRefusal::Interrupted);
            }
            let started = *self.started.get_or_insert_with(Instant::now);
            fs::create_dir_all(workspace).map_err(|_| BuilderRefusal::WorkspaceUnavailable)?;
            let mut execution = Execution::new(workspace, None, bounds, &self.cancelled)
                .minimum_free_bytes(4 * 1024 * 1024 * 1024)
                .started_at(started);
            if let Some(evidence) = &self.prepared_evidence {
                execution = execution.inherit_evidence(evidence);
            }
            let result = source::acquire(
                selected,
                workspace,
                bounds,
                local_roots,
                network_origins,
                &mut execution,
            );
            self.prepared_evidence = Some(execution.saved_evidence());
            result
        })())
    }

    fn prepare_exact_toolchain(
        &mut self,
        identity: &str,
        _allow_acquisition: bool,
        _permitted_origins: &[String],
        workspace: &Path,
        bounds: &BuilderBounds,
    ) -> Result<(), BuilderRefusal> {
        self.prepared = None;
        let started = *self.started.get_or_insert_with(Instant::now);
        if identity != self.selection()?.toolchain() {
            return Err(BuilderRefusal::ToolchainUnauthorized);
        }
        if !bounds.valid() {
            return Err(BuilderRefusal::InvalidRequest);
        }
        fs::create_dir_all(workspace).map_err(|_| BuilderRefusal::WorkspaceUnavailable)?;
        let workspace = workspace
            .canonicalize()
            .map_err(|_| BuilderRefusal::WorkspaceUnavailable)?;
        let mut execution = Execution::new(&workspace, None, bounds, &self.cancelled)
            .minimum_free_bytes(4 * 1024 * 1024 * 1024)
            .started_at(started);
        if let Some(evidence) = &self.prepared_evidence {
            execution = execution.inherit_evidence(evidence);
        }
        let mut probe = Command::new("rustup");
        probe.args(["run", "stable", "rustc", "--version"]);
        execution.run(&mut probe).map_err(toolchain_error)?;
        let mut components = Command::new("rustup");
        components.args(["component", "list", "--installed", "--toolchain", "stable"]);
        let output = execution.run(&mut components).map_err(toolchain_error)?;
        if !String::from_utf8_lossy(&output.stdout)
            .lines()
            .any(|line| line.starts_with("llvm-tools"))
        {
            return Err(BuilderRefusal::ToolchainUnavailable);
        }
        let mut targets = Command::new("rustup");
        targets.args(["target", "list", "--installed", "--toolchain", "stable"]);
        let output = execution.run(&mut targets).map_err(toolchain_error)?;
        let rust_target = self.selection()?.rust_target;
        if !String::from_utf8_lossy(&output.stdout)
            .lines()
            .any(|line| line == rust_target)
        {
            return Err(BuilderRefusal::ToolchainUnavailable);
        }
        self.prepared_evidence = Some(execution.saved_evidence());
        self.prepared = Some(identity.to_owned());
        Ok(())
    }

    fn build_exact_image(
        &mut self,
        image: &HostImage,
        image_description: &[u8],
        source_root: &Path,
        workspace: &Path,
        output: &Path,
        bounds: &BuilderBounds,
    ) -> Result<RealizedBuilderArtifact, BuilderRefusal> {
        if !bounds.valid() {
            return Err(BuilderRefusal::InvalidRequest);
        }
        let boot_assets = self
            .boot_assets
            .as_ref()
            .ok_or(BuilderRefusal::UnauthorizedNetwork)?;
        let selection = self.selection()?;
        if self.prepared.as_deref() != Some(image.manifest.toolchain_identity.as_str())
            || image.manifest.toolchain_identity != selection.toolchain()
        {
            return Err(BuilderRefusal::ToolchainUnauthorized);
        }
        if image.manifest.builder_adapter != selection.adapter()
            || image.manifest.output != SporeOutputKind::DiskImage
        {
            return Err(BuilderRefusal::UnsupportedAdapter);
        }
        verify_image_binding(image, image_description)
            .map_err(|_| BuilderRefusal::MalformedArtifact)?;
        let profile = canonical_profile_json(self.configuration.profile())
            .map_err(|_| BuilderRefusal::InvalidRequest)?;
        let profile_id = verification::digest(&profile);
        if profile_id != image.manifest.profile_id
            || image.manifest.source_configuration_id.as_deref()
                != Some(self.configuration.configuration_id())
        {
            return Err(BuilderRefusal::InvalidRequest);
        }
        let workspace = workspace
            .canonicalize()
            .map_err(|_| BuilderRefusal::WorkspaceUnavailable)?;
        let source_root = source_root
            .canonicalize()
            .map_err(|_| BuilderRefusal::MissingSource)?;
        let output = fresh_output(output, &workspace, &source_root)?;
        let snapshot = workspace.join("build-source");
        if snapshot.exists() {
            return Err(BuilderRefusal::WorkspaceUnavailable);
        }
        let mut execution = Execution::new(&workspace, Some(&output), bounds, &self.cancelled)
            .minimum_free_bytes(4 * 1024 * 1024 * 1024)
            .started_at(self.started.ok_or(BuilderRefusal::ToolchainUnavailable)?)
            .inherit_evidence(
                self.prepared_evidence
                    .as_ref()
                    .ok_or(BuilderRefusal::ToolchainUnavailable)?,
            );
        // Re-observe the local checkout and then independently verify the detached
        // snapshot. No hardlinks/shared Git object alternates or source build outputs.
        let revision = verification::verify_source(
            &source_root,
            &image.manifest.source_identity,
            bounds,
            &mut execution,
        )?;
        let mut clone = Command::new("git");
        clone
            .args([
                "-c",
                "protocol.file.allow=always",
                "clone",
                "--quiet",
                "--no-local",
                "--depth",
                "1",
                "--no-checkout",
            ])
            .arg(&source_root)
            .arg(&snapshot);
        execution.run(&mut clone)?;
        let mut checkout = Command::new("git");
        checkout
            .arg("-C")
            .arg(&snapshot)
            .args(["checkout", "--quiet", "--detach"])
            .arg(&revision);
        execution.run(&mut checkout)?;
        verification::verify_source(
            &snapshot,
            &image.manifest.source_identity,
            bounds,
            &mut execution,
        )?;
        boot_assets.stage(&snapshot, bounds, &mut || execution.check_progress())?;
        execution.check_bounds()?;
        let configuration_path = workspace.join("selected.host.conduit");
        let configuration =
            canonical_host_configuration_conduit(self.configuration.configuration())
                .map_err(|_| BuilderRefusal::InvalidRequest)?;
        use std::io::Write;
        let mut selected = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&configuration_path)
            .map_err(|_| BuilderRefusal::WorkspaceUnavailable)?;
        selected
            .write_all(configuration.as_bytes())
            .map_err(|_| BuilderRefusal::WorkspaceUnavailable)?;
        let mut build = Command::new("cargo");
        build
            .arg("+stable")
            .args(["xtask", "make", "host", "build"])
            .arg(&configuration_path)
            .arg("--output")
            .arg(&output)
            .arg("--source-identity")
            .arg(&image.manifest.source_identity)
            .current_dir(&snapshot)
            .env("CARGO_TARGET_DIR", workspace.join("cargo-target"))
            .env(
                "CONDUIT_CONDUITOS_TARGET_ROOT",
                workspace.join("conduitos-target"),
            )
            // The monitored process budget includes Cargo, xtask, linkers and
            // build scripts. Giving every slot to rustc jobs exhausts it before
            // ordinary driver overhead can run. Keep finite headroom and avoid
            // multiplying this Host's compiler memory demand on large machines.
            .env("CARGO_INCREMENTAL", "0")
            .env(
                "CARGO_BUILD_JOBS",
                (bounds.maximum_processes / 4).clamp(1, 4).to_string(),
            )
            .env_remove("RUSTC")
            .env_remove("RUSTC_WRAPPER")
            .env_remove("RUSTC_WORKSPACE_WRAPPER")
            .env_remove("RUSTFLAGS")
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .env_remove("CARGO_BUILD_RUSTC")
            .env("CARGO_NET_OFFLINE", "true")
            .env("RUSTUP_AUTO_INSTALL", "0")
            .env("RUSTUP_TOOLCHAIN", "stable");
        execution.run(&mut build)?;
        verification::verify_source(
            &snapshot,
            &image.manifest.source_identity,
            bounds,
            &mut execution,
        )?;
        if self.cancelled.load(Ordering::Acquire) {
            return Err(BuilderRefusal::Interrupted);
        }
        execution.check_bounds()?;
        let artifact = verification::verify_output(
            &output,
            image,
            &selection.image_file(),
            bounds,
            &mut || execution.check_progress(),
        )?;
        execution.check_bounds()?;
        let (stdout_bytes, stderr_bytes, evidence_sha256) = execution.evidence();
        Ok(RealizedBuilderArtifact {
            path: artifact.0,
            bytes: artifact.1,
            sha256: artifact.2,
            evidence_sha256,
            stdout_bytes,
            stderr_bytes,
        })
    }
}

fn toolchain_error(error: BuilderRefusal) -> BuilderRefusal {
    match error {
        BuilderRefusal::BuildFailed => BuilderRefusal::ToolchainUnavailable,
        other => other,
    }
}

fn fresh_output(output: &Path, workspace: &Path, source: &Path) -> Result<PathBuf, BuilderRefusal> {
    // Exclusive creation prevents accepting a previous or concurrently produced image.
    if output.exists() {
        return Err(BuilderRefusal::WorkspaceUnavailable);
    }
    let parent = output
        .parent()
        .ok_or(BuilderRefusal::WorkspaceUnavailable)?;
    fs::create_dir_all(parent).map_err(|_| BuilderRefusal::WorkspaceUnavailable)?;
    let parent = parent
        .canonicalize()
        .map_err(|_| BuilderRefusal::WorkspaceUnavailable)?;
    let name = output
        .file_name()
        .ok_or(BuilderRefusal::WorkspaceUnavailable)?;
    let output = parent.join(name);
    if output.starts_with(source) || workspace.starts_with(&output) {
        return Err(BuilderRefusal::WorkspaceUnavailable);
    }
    fs::create_dir(&output).map_err(|_| BuilderRefusal::WorkspaceUnavailable)?;
    Ok(output)
}
