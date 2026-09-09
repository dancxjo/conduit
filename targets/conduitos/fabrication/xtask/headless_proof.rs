//! Exact final headless IMAGE exclusion and explicit unsupported boot evidence.
mod validation;

use super::{report::sha256_file, ConduitosError};
use crate::{cli::GlobalOpts, commands::host::host_target};
use serde_json::{json, Value};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

pub fn execute(output: PathBuf, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(refusal(
            "dry-run-has-no-headless-proof",
            "a final IMAGE and actual boot are required",
        ));
    }
    let manifest =
        host_target::verify_target(&output).map_err(|e| refusal("headless-artifact-invalid", e))?;
    let resolved = &manifest.resolved_build;
    if manifest.target != "conduitos/x86_64/pc"
        || !resolved.presenters.is_empty()
        || !resolved.facilities.is_empty()
        || !resolved.resource_budgets.is_empty()
        || !resolved.base_selections.is_empty()
        || !resolved.driver_selections.is_empty()
        || !resolved.host_operations.is_empty()
    {
        return Err(refusal(
            "headless-profile-mismatch",
            "the exact x86 headless profile must exclude graphical resources and providers",
        ));
    }
    let kernel = output.join(&manifest.kernel.file);
    let symbols = Command::new("nm")
        .args(["-C", "--defined-only"])
        .arg(&kernel)
        .output()
        .map_err(|e| refusal("headless-symbol-inspection-unavailable", e))?;
    if !symbols.status.success() || symbols.stdout.is_empty() {
        return Err(refusal(
            "headless-symbol-inspection-failed",
            "the final ELF must have an inspectable symbol table",
        ));
    }
    let symbols_text = String::from_utf8_lossy(&symbols.stdout);
    validation::validate_symbols(&symbols_text)?;
    let excluded_fonts = inspect_font_assets(&kernel)?;
    fs::write(output.join("headless-elf-symbols.txt"), &symbols.stdout).map_err(io_error)?;
    let first = boot(&output, &manifest, 1)?;
    let second = boot(&output, &manifest, 2)?;
    if first["host_id"] == second["host_id"] || first["boot_id"] == second["boot_id"] {
        return Err(refusal(
            "headless-stale-identity",
            "independent boots must have fresh Host and Boot identities",
        ));
    }
    let receipt = json!({
        "schema":"conduit.conduitos/headless-proof@1", "proof_class":"freestanding-emulator",
        "image_sha256":manifest.image.sha256, "kernel_sha256":sha256_file(&kernel)?,
        "symbol_table_sha256":sha256_file(&output.join("headless-elf-symbols.txt"))?,
        "graphical_symbols_absent":true, "first":first, "second":second,
        "excluded_font_assets":excluded_fonts,
        "workload_execution_claimed":false, "expected_exit_code":35
    });
    fs::write(
        output.join("headless-proof.json"),
        serde_json::to_vec_pretty(&receipt).map_err(|e| refusal("headless-receipt-invalid", e))?,
    )
    .map_err(io_error)?;
    if !opts.quiet {
        println!(
            "Headless unsupported-startup proof: {}",
            output.join("headless-proof.json").display()
        );
    }
    Ok(())
}

fn boot(
    output: &std::path::Path,
    manifest: &host_target::TargetBuildManifest,
    index: usize,
) -> Result<Value, ConduitosError> {
    let serial = output.join(format!("headless-boot-{index}.log"));
    let stderr = output.join(format!("headless-boot-{index}-stderr.log"));
    let serial_arg = format!("file:{}", serial.display());
    let mut command = Command::new("qemu-system-x86_64");
    command
        .args([
            "-M",
            "q35",
            "-cpu",
            "max",
            "-m",
            "64M",
            "-smp",
            "1",
            "-display",
            "none",
            "-vga",
            "none",
            "-monitor",
            "none",
            "-serial",
            &serial_arg,
            "-no-reboot",
            "-net",
            "none",
            "-device",
            "isa-debug-exit,iobase=0xf4,iosize=0x04",
            "-cdrom",
        ])
        .arg(output.join(&manifest.image.file))
        .args(["-boot", "d"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(fs::File::create(stderr).map_err(io_error)?);
    let argv = command
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let mut child = command
        .spawn()
        .map_err(|e| refusal("headless-qemu-unavailable", e))?;
    let result = (|| {
        let deadline = Instant::now() + Duration::from_secs(45);
        loop {
            if let Some(status) = child.try_wait().map_err(io_error)? {
                if status.code() != Some(35) {
                    return Err(refusal("headless-unexpected-exit", status));
                }
                let text = fs::read_to_string(&serial).map_err(io_error)?;
                let sign = validation::validate(
                    &text,
                    &manifest.profile_id,
                    &manifest.build_id,
                    &manifest.resolved_description_binding,
                )?;
                let lowered = super::target_lowering::lower(&manifest.resolved_build)?;
                let bounds = &manifest.resolved_build.bounds;
                let arena_ceiling = if bounds.heap_arena_bytes == 0 {
                    bounds.static_memory_bytes
                } else {
                    bounds.heap_arena_bytes
                };
                if sign["compiled_implementations"] != lowered.implementations
                    || sign["runtime_arena_ceiling"] != arena_ceiling
                {
                    return Err(refusal(
                        "headless-inventory-mismatch",
                        "boot inventory must match the exact resolved build",
                    ));
                }
                return Ok(
                    json!({"host_id":sign["host_id"], "boot_id":sign["boot_id"], "sign":sign, "qemu_argv":argv, "serial_sha256":sha256_file(&serial)?}),
                );
            }
            if Instant::now() >= deadline {
                return Err(refusal(
                    "headless-boot-timeout",
                    "no bounded terminal startup outcome",
                ));
            }
            thread::sleep(Duration::from_millis(20));
        }
    })();
    if child.try_wait().map_err(io_error)?.is_none() {
        child.kill().map_err(io_error)?;
        child.wait().map_err(io_error)?;
    }
    result
}

fn inspect_font_assets(kernel: &std::path::Path) -> Result<Vec<Value>, ConduitosError> {
    let root = crate::workspace::workspace_root()
        .map_err(|e| refusal("headless-workspace-unavailable", e))?;
    let fonts = root.join("targets/conduitos/assets/fonts");
    let mut pending = if fonts.exists() {
        vec![fonts]
    } else {
        Vec::new()
    };
    let elf = fs::read(kernel).map_err(io_error)?;
    let mut inspected = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).map_err(io_error)? {
            let entry = entry.map_err(io_error)?;
            let path = entry.path();
            if entry.file_type().map_err(io_error)?.is_dir() {
                pending.push(path);
            } else if matches!(
                path.extension().and_then(|s| s.to_str()),
                Some("ttf" | "otf")
            ) {
                let font = fs::read(&path).map_err(io_error)?;
                if font.is_empty() || elf.windows(font.len()).any(|bytes| bytes == font) {
                    return Err(refusal("headless-font-asset-leaked", path.display()));
                }
                inspected.push(json!({"asset":path.strip_prefix(&root).unwrap_or(&path), "sha256":sha256_file(&path)?}));
            }
        }
    }
    inspected.sort_by_key(|entry| entry["asset"].as_str().unwrap_or_default().to_owned());
    Ok(inspected)
}

fn refusal(code: &'static str, detail: impl std::fmt::Display) -> ConduitosError {
    ConduitosError::refusal(code, detail.to_string())
}
fn io_error(error: std::io::Error) -> ConduitosError {
    refusal("headless-proof-io", error)
}
