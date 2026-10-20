//! G0 discovery only: exact product artifacts and read-only firmware handoff.
use super::{live_media, report, ConduitosArch, ConduitosError};
use crate::{cli::GlobalOpts, workspace::workspace_root};
use clap::{Args, ValueEnum};
use serde_json::{json, Value};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

mod capture;
mod debugger;
mod elf;
mod guest;
mod limine;
mod multiboot;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum Provider {
    None,
    Ramfb,
    Vga,
}
#[derive(Args, Debug)]
pub(super) struct ProbeArgs {
    /// Exact normal PROFILE-built product directory; never a proof appliance.
    #[arg(long)]
    image_directory: PathBuf,
    /// New directory; an existing directory refuses to protect prior evidence.
    #[arg(long)]
    output_directory: PathBuf,
    #[arg(long, value_enum)]
    arch: ConduitosArch,
    /// Candidate only. Device presence never establishes graphical admission.
    #[arg(long, value_enum)]
    provider: Provider,
    /// Existing local firmware (CODE for IA-32); execution never downloads.
    #[arg(long)]
    firmware: PathBuf,
    /// Immutable IA-32 template; copied to a private writable per-probe store.
    #[arg(long)]
    vars_template: Option<PathBuf>,
    /// Existing RISC-V U-Boot EFI kernel paired with the OpenSBI firmware.
    #[arg(long)]
    boot_kernel: Option<PathBuf>,
    /// Optional exact local QEMU executable, otherwise the architecture's PATH tool.
    #[arg(long)]
    emulator: Option<PathBuf>,
}

pub(super) fn execute(args: &ProbeArgs, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(fail(
            "graphics-probe-needs-execution",
            "discovery needs an actual guest; no receipt is fabricated for dry-run",
        ));
    }
    let root = workspace_root().map_err(|e| fail("workspace-root", e))?;
    let host = live_host(args.arch)?;
    let manifest = crate::commands::host::host_target::verify_target(&args.image_directory)
        .map_err(|e| fail("graphics-probe-product-invalid", e.to_string()))?;
    if manifest.target != host.row().host || manifest.boot_assets.architecture != args.arch.as_str()
    {
        return Err(fail(
            "graphics-probe-target-mismatch",
            "select the exact live-media product Host, not another board or architecture",
        ));
    }
    args.validate_firmware_inputs()?;
    if manifest.boot_assets.limine_version != super::profile::LIMINE_VERSION
        || manifest.boot_assets.limine_archive_sha256 != super::profile::LIMINE_ARCHIVE_SHA256
    {
        return Err(fail(
            "graphics-probe-bootloader-version",
            "the discovery decoder requires the repository's pinned Limine layout",
        ));
    }
    let kernel = args
        .image_directory
        .join(&manifest.kernel.file)
        .canonicalize()
        .map_err(io)?;
    let image = args
        .image_directory
        .join(&manifest.image.file)
        .canonicalize()
        .map_err(io)?;
    let mut kernel_bytes = Vec::new();
    fs::File::open(&kernel)
        .map_err(io)?
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut kernel_bytes)
        .map_err(io)?;
    require_kernel_arch(args.arch, &kernel_bytes)?;
    let checkpoint = if args.arch == ConduitosArch::Ia32 {
        elf::entry_point(&kernel_bytes)
    } else {
        elf::framebuffer_request(&kernel_bytes)
    }
    .map_err(|reason| fail(reason, "retained kernel request/entry could not be located"))?;
    let executable = guest::executable(args)?;
    let mut inputs = vec![
        identity(&executable)?,
        identity(&args.firmware)?,
        identity(&kernel)?,
        identity(&image)?,
    ];
    for path in [&args.vars_template, &args.boot_kernel]
        .into_iter()
        .flatten()
    {
        inputs.push(identity(path)?);
    }
    if let Some(parent) = args
        .output_directory
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(io)?;
    }
    fs::create_dir(&args.output_directory).map_err(io)?;
    let output = args.output_directory.canonicalize().map_err(io)?;
    if output.to_str().is_none_or(|path| path.contains(',')) {
        return Err(fail(
            "graphics-probe-output-path",
            "choose a UTF-8 output directory without commas for QEMU's serial file entrance",
        ));
    }
    let mut record = json!({"schema":"conduit.conduitos/framebuffer-discovery@1","probe_status":"preparing",
        "capture_source_commit":report::git_head(&root)?,"capture_source_dirty":source_dirty(&root)?,"product":manifest,"inputs":inputs,
        "architecture":args.arch.as_str(),"candidate_provider":format!("{:?}",args.provider).to_lowercase(),
        "graphical_show_proved":false,"physical_scanout_proved":false,"gpu_acceleration_proved":false,
        "discovery_limits":{"guest_handoff_seconds":if args.arch==ConduitosArch::Ia32 {debugger::ENTRY_SECONDS}else{capture::HANDOFF_SECONDS},"framebuffers":limine::MAX_FRAMEBUFFERS,"modes_per_framebuffer":limine::MAX_MODES,"maximum_ppm_bytes":super::qmp_display::MAXIMUM_FRAME_BYTES}});
    let result = (|| {
        let mut guest = guest::Guest::start(args, &executable, &image, &output)?;
        record["launch"] = json!({"program":executable,"arguments":&guest.command});
        record["emulator_version"] = json!(&guest.version);
        let observed = capture::observe(args, checkpoint, &mut guest, &output)?;
        record["observation"] = observed;
        // Verify that diagnostics did not mutate either retained product artifact.
        if report::sha256_file(&image)? != record["product"]["image"]["sha256"]
            || report::sha256_file(&kernel)? != record["product"]["kernel"]["sha256"]
        {
            return Err(fail(
                "graphics-probe-product-changed",
                "retained image/kernel changed during discovery",
            ));
        }
        for input in record["inputs"]
            .as_array()
            .ok_or_else(|| fail("graphics-probe-input-record", "missing input identities"))?
        {
            let path = input["path"]
                .as_str()
                .ok_or_else(|| fail("graphics-probe-input-record", "missing input path"))?;
            if report::sha256_file(Path::new(path))? != input["sha256"] {
                return Err(fail(
                    "graphics-probe-input-changed",
                    "a toolchain, firmware, template or retained artifact changed during discovery",
                ));
            }
        }
        record["probe_status"] = json!("observed");
        Ok(())
    })();
    if let Err(error) = &result {
        record["probe_status"] = json!("refused");
        record["refusal"] = json!({"code":error.reason,"detail":error.detail});
    }
    fs::write(
        output.join("discovery.json"),
        serde_json::to_vec_pretty(&record)
            .map_err(|e| fail("graphics-probe-encoding", e.to_string()))?,
    )
    .map_err(io)?;
    if opts.json {
        println!("{}", record);
    } else if !opts.quiet {
        println!(
            "Framebuffer discovery (not graphical Show proof): {}",
            output.join("discovery.json").display()
        );
    }
    result
}
fn require_kernel_arch(arch: ConduitosArch, image: &[u8]) -> Result<(), ConduitosError> {
    // ELF machine constants from the platform ABI, independent of a path label.
    let expected = match arch {
        ConduitosArch::Ia32 => (1, 3),
        ConduitosArch::X86_64 => (2, 62),
        ConduitosArch::Aarch64 => (2, 183),
        ConduitosArch::Riscv64 => (2, 243),
        ConduitosArch::Loongarch64 => (2, 258),
        ConduitosArch::Armv6 => {
            return Err(fail(
                "graphics-probe-board-unsupported",
                "board-specific image",
            ))
        }
    };
    if elf::identity(image).map_err(|reason| fail(reason, "kernel ELF identity unavailable"))?
        != expected
    {
        return Err(fail(
            "graphics-probe-kernel-architecture",
            "kernel ELF class/machine does not match the selected live Host",
        ));
    }
    Ok(())
}
fn live_host(arch: ConduitosArch) -> Result<live_media::LiveHost, ConduitosError> {
    Ok(match arch {
        ConduitosArch::X86_64 => live_media::LiveHost::X86_64,
        ConduitosArch::Ia32 => live_media::LiveHost::Ia32,
        ConduitosArch::Aarch64 => live_media::LiveHost::Aarch64,
        ConduitosArch::Riscv64 => live_media::LiveHost::Riscv64,
        ConduitosArch::Loongarch64 => live_media::LiveHost::Loongarch64,
        ConduitosArch::Armv6 => {
            return Err(fail(
                "graphics-probe-board-unsupported",
                "ARMv6 Raspberry Pi has a separate board-specific framebuffer proof track",
            ))
        }
    })
}
fn identity(path: &Path) -> Result<Value, ConduitosError> {
    let metadata = fs::metadata(path).map_err(io)?;
    if !metadata.is_file() || metadata.len() > 2 * 1024 * 1024 * 1024 {
        return Err(fail(
            "graphics-probe-input-bound",
            "inputs must be finite regular files within 2 GiB",
        ));
    }
    let canonical = path.canonicalize().map_err(io)?;
    let encoded = canonical
        .to_str()
        .ok_or_else(|| fail("graphics-probe-path-encoding", "inputs require UTF-8 paths"))?;
    Ok(json!({"path":encoded,"bytes":metadata.len(),"sha256":report::sha256_file(path)?}))
}

fn fail(reason: &'static str, detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal(reason, detail)
}
fn io(error: std::io::Error) -> ConduitosError {
    fail("graphics-probe-artifact-io", error.to_string())
}

impl ProbeArgs {
    fn validate_firmware_inputs(&self) -> Result<(), ConduitosError> {
        if self.vars_template.is_some() != (self.arch == ConduitosArch::Ia32)
            || self.boot_kernel.is_some() != (self.arch == ConduitosArch::Riscv64)
        {
            return Err(fail("graphics-probe-firmware-inputs","IA-32 requires --vars-template; RISC-V64 requires --boot-kernel; other architectures accept neither"));
        }
        Ok(())
    }
}
fn source_dirty(root: &Path) -> Result<bool, ConduitosError> {
    let result = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(root)
        .output()
        .map_err(io)?;
    if !result.status.success() {
        return Err(fail(
            "graphics-probe-source-identity",
            "Git source status query failed",
        ));
    }
    Ok(!result.stdout.is_empty())
}
#[cfg(test)]
#[path = "graphics_probe/command_tests.rs"]
mod command_tests;
