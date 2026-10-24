//! Human window lifetime is independent of bounded boot-verification receipts.
use super::{
    demo, ia32_firmware, profile::Paths, report, riscv64_emulator, riscv64_firmware, ConduitosArch,
    ConduitosError,
};
use crate::{cli::GlobalOpts, commands::host::host_target};
use std::{
    fs,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub(crate) fn launch(output: &Path, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    let manifest = host_target::verify_target(output).map_err(io)?;
    if !manifest
        .resolved_build
        .presenters
        .iter()
        .any(|p| p == "presenter/native-graphical@1")
    {
        return Err(io("select a graphical PROFILE-built product; this IMAGE has no native graphical Presenter"));
    }
    let image = fs::canonicalize(output.join(&manifest.image.file)).map_err(io)?;
    if manifest.target == "conduitos/x86_64/pc" {
        demo::boot_visible_image(&image, Some(&manifest.image.sha256), opts)?;
        host_target::verify_target(output).map_err(io)?;
        return Ok(());
    }
    let arch = match manifest.target.as_str() {
        "conduitos/ia32/pc" => ConduitosArch::Ia32,
        "conduitos/aarch64/virt" => ConduitosArch::Aarch64,
        "conduitos/riscv64/virt" => ConduitosArch::Riscv64,
        _ => {
            return Err(io(
                "persistent graphical launch is not yet qualified for this target",
            ))
        }
    };
    let paths = Paths::new(arch)?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(io)?
        .as_nanos();
    let directory = paths
        .target
        .join(format!("interactive-{}-{nonce}", std::process::id()));
    let session = Session {
        directory,
        vars: None,
    };
    if !opts.dry_run {
        fs::create_dir_all(&paths.target).map_err(io)?;
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&session.directory)
            .map_err(io)?;
    }
    let (mut command, firmware, session) = configure(arch, &paths, &image, session, opts.dry_run)?;
    let firmware_hashes = firmware
        .iter()
        .map(|p| Ok((p.clone(), report::sha256_file(p)?)))
        .collect::<Result<Vec<_>, ConduitosError>>()?;
    let record = serde_json::json!({
        "schema":"conduit.conduitos/persistent-launch@1", "proof_class":"human-session-launch-only",
        "target":manifest.target,"source_identity":manifest.source_identity,
        "profile_id":manifest.profile_id,"build_id":manifest.build_id,"image_id":manifest.image_id,
        "image_sha256":manifest.image.sha256,"program":command.get_program().to_string_lossy(),
        "arguments":command.get_args().map(|a|a.to_string_lossy().into_owned()).collect::<Vec<_>>(),
        "firmware":firmware_hashes,"local_input":false,"stopped_by_boot_verifier":false
    });
    if opts.dry_run {
        println!("{}", serde_json::to_string_pretty(&record).map_err(io)?);
        return Ok(());
    }
    fs::write(
        session.directory.join("launch.json"),
        serde_json::to_vec_pretty(&record).map_err(io)?,
    )
    .map_err(io)?;
    if !opts.quiet {
        println!(
            "{} — read-only graphical snapshot; local keyboard and pointer are unavailable.",
            manifest.target
        );
        println!("Close the QEMU window to stop. This launch does not issue a boot-verification receipt.");
    }
    let status = command
        .current_dir(&paths.root)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .env("QEMU_AUDIO_DRV", "none")
        .env_remove("QEMU_MODULE_DIR")
        .status()
        .map_err(io)?;
    for (path, hash) in firmware_hashes {
        if report::sha256_file(&path)? != hash {
            return Err(io("immutable firmware changed during session"));
        }
    }
    host_target::verify_target(output).map_err(io)?;
    fs::write(
        session.directory.join("exit.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema":"conduit.conduitos/persistent-launch-exit@1","success":status.success(),
            "exit_code":status.code(),"image_unchanged":true,"firmware_unchanged":true,
            "proof_class":"human-session-exit-only"
        }))
        .map_err(io)?,
    )
    .map_err(io)?;
    if !status.success() {
        return Err(io(format!(
            "QEMU exited {status}; check graphical display access"
        )));
    }
    Ok(())
}

struct Session {
    directory: PathBuf,
    vars: Option<PathBuf>,
}
impl Drop for Session {
    fn drop(&mut self) {
        if let Some(path) = &self.vars {
            let _ = fs::remove_file(path);
        }
    }
}
fn configure(
    arch: ConduitosArch,
    paths: &Paths,
    image: &Path,
    mut session: Session,
    dry: bool,
) -> Result<(Command, Vec<PathBuf>, Session), ConduitosError> {
    let mut firmware = Vec::new();
    let mut command = match arch {
        ConduitosArch::Ia32 => {
            let (code, template) = super::ia32_a1::firmware_paths(paths)?;
            let vars = private_vars(&template, &mut session, dry)?;
            let mut c = Command::new("qemu-system-i386");
            c.args([
                "-machine",
                "q35",
                "-cpu",
                "max",
                "-m",
                "512M",
                "-smp",
                "1",
                "-vga",
                "std",
                "-display",
                "sdl",
                "-serial",
                "none",
                "-debugcon",
                "stdio",
                "-global",
                "isa-debugcon.iobase=0xe9",
            ])
            .arg("-drive")
            .arg(format!(
                "if=pflash,format=raw,readonly=on,file={}",
                filename(&code)?
            ))
            .arg("-drive")
            .arg(format!("if=pflash,format=raw,file={}", filename(&vars)?));
            firmware.extend([code, template]);
            c
        }
        ConduitosArch::Aarch64 => {
            let code = std::env::var_os("CONDUITOS_AARCH64_FIRMWARE")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/usr/share/qemu-efi-aarch64/QEMU_EFI.fd"));
            let mut c = Command::new("qemu-system-aarch64");
            c.args([
                "-machine",
                "virt",
                "-cpu",
                "neoverse-n2",
                "-m",
                "512M",
                "-smp",
                "1",
                "-display",
                "sdl",
                "-serial",
                "stdio",
                "-device",
                "ramfb",
                "-bios",
            ])
            .arg(&code);
            firmware.push(code);
            c
        }
        ConduitosArch::Riscv64 => {
            let (qemu, sbi) = riscv64_emulator::selected_graphical(paths)?;
            let (code, template) = riscv64_firmware::paths(&paths.root).map_err(io)?;
            let vars = private_vars(&template, &mut session, dry)?;
            let mut c = Command::new(qemu);
            c.args([
                "-machine",
                "virt,acpi=off",
                "-cpu",
                "rv64,zkr=true,sv57=off,sv48=off",
                "-m",
                "2G",
                "-smp",
                "1",
                "-accel",
                "tcg,thread=single",
                "-display",
                "sdl",
                "-serial",
                "stdio",
                "-device",
                "ramfb",
                "-bios",
            ])
            .arg(&sbi)
            .arg("-blockdev")
            .arg(format!(
                "node-name=pflash0,driver=file,read-only=on,filename={}",
                filename(&code)?
            ))
            .arg("-blockdev")
            .arg(format!(
                "node-name=pflash1,driver=file,filename={}",
                filename(&vars)?
            ))
            .args(["-machine", "pflash0=pflash0,pflash1=pflash1"]);
            firmware.extend([sbi, code, template]);
            c
        }
        _ => return Err(io("unsupported persistent launch architecture")),
    };
    command.args(["-monitor", "none", "-net", "none", "-no-reboot"]);
    if arch == ConduitosArch::Riscv64 {
        command
            .arg("-drive")
            .arg(format!(
                "file={},format=raw,if=none,id=boot,readonly=on",
                filename(image)?
            ))
            .args(["-device", "virtio-blk-pci,drive=boot,bootindex=0"]);
    } else {
        command.arg("-cdrom").arg(image).args(["-boot", "d"]);
    }
    Ok((command, firmware, session))
}
fn private_vars(
    template: &Path,
    session: &mut Session,
    dry: bool,
) -> Result<PathBuf, ConduitosError> {
    let vars = session.directory.join("vars.fd");
    if !dry {
        ia32_firmware::copy_vars(template, &vars).map_err(io)?;
        session.vars = Some(vars.clone());
    }
    Ok(vars)
}
fn filename(path: &Path) -> Result<String, ConduitosError> {
    path.to_str()
        .map(|v| v.replace(',', ",,"))
        .ok_or_else(|| io("QEMU file path is not UTF-8"))
}
fn io(error: impl std::fmt::Display) -> ConduitosError {
    ConduitosError::refusal("persistent-launch-unavailable", error.to_string())
}

#[cfg(test)]
#[path = "persistent_launch_tests.rs"]
mod tests;
