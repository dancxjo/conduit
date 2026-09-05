use std::{
    ffi::OsString,
    path::{Component, Path, PathBuf},
};

use crate::workspace::workspace_root;

use super::{ConduitosArch, ConduitosError};

pub const LIMINE_VERSION: &str = "12.5.2";
pub const LIMINE_ARCHIVE_SHA256: &str =
    "4c760c09c53560d859b362319a3dc63b79cca3d47f35d69ab0106a13b8057055";
pub const LIMINE_ARCHIVE_URL: &str =
    "https://github.com/limine-bootloader/limine/releases/download/v12.5.2/limine-binary.tar.gz";
pub const QEMU_PROFILE: &str = "q35-single-cpu-64m-headless-xhci-usb-kbd-adlib";
pub const AARCH64_QEMU_PROFILE: &str = "qemu-virt-single-cpu-256m-uefi-semihosting";
pub const EXPECTED_QEMU_SUCCESS: i32 = 33;
pub const COMMON_BACKBONE_TARGETS: &[&str] = &[
    "i686-unknown-uefi",
    "x86_64-unknown-none",
    "aarch64-unknown-none",
    "armv6-none-eabi",
    "riscv64gc-unknown-none-elf",
    "loongarch64-unknown-none",
];
pub const IA32_SHARED_BACKBONE_TARGET: &str = "i686-unknown-uefi";
pub const IA32_OBJECT_TARGET: &str = "i686-unknown-linux-gnu";
pub const IA32_LINK_PROFILE: &str = "rust-elf-object+rust-lld-elf_i386";

pub struct Paths {
    pub root: PathBuf,
    pub target: PathBuf,
    pub kernel: PathBuf,
    pub iso_root: PathBuf,
    pub iso: PathBuf,
    pub limine_archive: PathBuf,
    pub limine: PathBuf,
    pub proof: PathBuf,
    pub observatory_snapshot: PathBuf,
    pub xhci_proof: PathBuf,
    pub pc_speaker_proof: PathBuf,
    pub usb_proof: PathBuf,
    pub hid_proof: PathBuf,
    pub keyboard_proof: PathBuf,
    pub rescue_proof: PathBuf,
    pub hotplug_proof: PathBuf,
    pub opl2_proof: PathBuf,
    pub prepared_proof_image: PathBuf,
}

impl Paths {
    pub fn new(arch: ConduitosArch) -> Result<Self, ConduitosError> {
        let root = workspace_root()
            .map_err(|error| ConduitosError::refusal("workspace-unavailable", error))?;
        let target_root = target_root(&root, std::env::var_os("CONDUIT_CONDUITOS_TARGET_ROOT"))?;
        let target = target_root.join(arch.as_str());
        Ok(Self {
            kernel: target.join("conduitos"),
            iso_root: target.join("iso-root"),
            iso: target.join("conduitos.iso"),
            limine_archive: root
                .join("target/conduitos/vendor")
                .join(format!("limine-binary-{LIMINE_VERSION}.tar.gz")),
            limine: root
                .join("target/conduitos/vendor")
                .join(format!("limine-binary-{LIMINE_VERSION}")),
            proof: target.join("kernel-proof.json"),
            observatory_snapshot: target.join("observatory-snapshot.json"),
            xhci_proof: target.join("xhci-proof.json"),
            pc_speaker_proof: target.join("pc-speaker-proof.json"),
            usb_proof: target.join("usb-proof.json"),
            hid_proof: target.join("hid-proof.json"),
            keyboard_proof: target.join("keyboard-proof.json"),
            rescue_proof: target.join("rescue-proof.json"),
            hotplug_proof: target.join("hotplug-proof.json"),
            opl2_proof: target.join("opl2-proof.json"),
            prepared_proof_image: target.join("prepared-proof-image.json"),
            root,
            target,
        })
    }
}

fn target_root(root: &Path, requested: Option<OsString>) -> Result<PathBuf, ConduitosError> {
    let Some(requested) = requested else {
        return Ok(root.join("target").join("conduitos"));
    };
    let requested = PathBuf::from(requested);
    if requested.as_os_str().is_empty()
        || requested
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(ConduitosError::refusal(
            "conduitos-target-root-invalid",
            "isolated target root must be nonempty and contain no parent traversal",
        ));
    }
    Ok(if requested.is_absolute() {
        requested
    } else {
        root.join(requested)
    })
}

pub fn command(
    program: &str,
    args: &[&str],
    cwd: &Path,
    reason: &'static str,
) -> Result<std::process::Output, ConduitosError> {
    command_with_env(program, args, cwd, reason, &[])
}

pub fn command_with_env(
    program: &str,
    args: &[&str],
    cwd: &Path,
    reason: &'static str,
    environment: &[(&str, &str)],
) -> Result<std::process::Output, ConduitosError> {
    let mut command = std::process::Command::new(program);
    command.args(args).current_dir(cwd);
    for (name, value) in environment {
        command.env(name, value);
    }
    let output = command.output().map_err(|error| {
        ConduitosError::refusal(reason, format!("cannot launch {program}: {error}"))
    })?;
    if !output.status.success() {
        return Err(ConduitosError::refusal(
            reason,
            format!(
                "{program} {} exited {}; stderr: {}",
                args.join(" "),
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolated_target_root_is_explicit_and_traversal_free() {
        let root = Path::new("/repo");
        assert_eq!(
            target_root(root, Some(OsString::from("target/batch"))).unwrap(),
            Path::new("/repo/target/batch")
        );
        assert!(target_root(root, Some(OsString::from("../escape"))).is_err());
        assert!(target_root(root, Some(OsString::new())).is_err());
    }
}
