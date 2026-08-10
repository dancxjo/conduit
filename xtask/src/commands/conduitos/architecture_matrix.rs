use std::{collections::BTreeSet, fs};

use serde::Serialize;

use crate::cli::GlobalOpts;

use super::{
    aarch64_a0, image, loongarch64_a0,
    profile::{Paths, LIMINE_ARCHIVE_SHA256, LIMINE_ARCHIVE_URL, LIMINE_VERSION},
    riscv64_a0, ConduitosArch, ConduitosError,
};

const SCHEMA: &str = "conduit.conduitos/architecture-matrix@1";
const MAXIMUM_ARCHITECTURES: usize = 8;

#[derive(Serialize)]
struct ArchitectureRow {
    architecture: &'static str,
    limine_artifact: &'static str,
    in_pinned_limine_matrix: bool,
    shared_backbone_target: &'static str,
    shared_backbone_profile_known: bool,
    executable_backend_present: bool,
    a0_compile_link: bool,
    a1_boot: bool,
    a2_machine_wake: bool,
    a3_ordinary_form: bool,
    a4_observatory_patchbay: bool,
    blocker: Option<&'static str>,
}

#[derive(Serialize)]
struct ArchitectureMatrix {
    schema: &'static str,
    maximum_architectures: usize,
    limine_version: &'static str,
    limine_archive_url: &'static str,
    limine_archive_sha256: &'static str,
    matrix_basis: &'static str,
    architectures: Vec<ArchitectureRow>,
}

pub fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-matrix-evidence",
            "architecture-matrix must inspect the digest-verified pinned Limine archive",
        ));
    }
    let paths = Paths::new(ConduitosArch::X86_64)?;
    image::prepare_limine(&paths)?;
    let artifacts = pinned_efi_artifacts(&paths)?;
    let matrix = build_matrix(&artifacts)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&matrix).map_err(|error| {
            ConduitosError::refusal("architecture-matrix-encoding-failed", error.to_string())
        })?
    );
    Ok(())
}

fn pinned_efi_artifacts(paths: &Paths) -> Result<BTreeSet<String>, ConduitosError> {
    let entries = fs::read_dir(&paths.limine).map_err(|error| {
        ConduitosError::refusal("pinned-limine-matrix-unavailable", error.to_string())
    })?;
    let mut artifacts = BTreeSet::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            ConduitosError::refusal("pinned-limine-matrix-unavailable", error.to_string())
        })?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("BOOT") && name.ends_with(".EFI") {
            artifacts.insert(name);
        }
    }
    Ok(artifacts)
}

fn build_matrix(artifacts: &BTreeSet<String>) -> Result<ArchitectureMatrix, ConduitosError> {
    if ConduitosArch::ALL.len() > MAXIMUM_ARCHITECTURES {
        return Err(ConduitosError::refusal(
            "architecture-matrix-capacity-exceeded",
            ConduitosArch::ALL.len().to_string(),
        ));
    }
    let architectures: Vec<_> = ConduitosArch::ALL.into_iter().map(row).collect();
    let expected: BTreeSet<_> = architectures
        .iter()
        .map(|row| row.limine_artifact.to_owned())
        .collect();
    if artifacts != &expected {
        return Err(ConduitosError::refusal(
            "pinned-limine-command-matrix-mismatch",
            format!("archive={artifacts:?}, command={expected:?}"),
        ));
    }
    Ok(ArchitectureMatrix {
        schema: SCHEMA,
        maximum_architectures: MAXIMUM_ARCHITECTURES,
        limine_version: LIMINE_VERSION,
        limine_archive_url: LIMINE_ARCHIVE_URL,
        limine_archive_sha256: LIMINE_ARCHIVE_SHA256,
        matrix_basis: "digest-verified top-level BOOT*.EFI artifacts",
        architectures,
    })
}

fn row(arch: ConduitosArch) -> ArchitectureRow {
    let boot_accepted = matches!(arch, ConduitosArch::X86_64 | ConduitosArch::Aarch64);
    let full_spine_accepted = arch == ConduitosArch::X86_64;
    let ordinary_form_accepted = full_spine_accepted || arch == ConduitosArch::Aarch64;
    let observatory_patchbay_accepted = ordinary_form_accepted;
    let machine_wake_accepted = full_spine_accepted || arch == ConduitosArch::Aarch64;
    let compile_link_accepted = boot_accepted
        || matches!(
            arch,
            ConduitosArch::Ia32
                | ConduitosArch::Aarch64
                | ConduitosArch::Riscv64
                | ConduitosArch::Loongarch64
        );
    let (artifact, target, blocker) = match arch {
        ConduitosArch::Ia32 => (
            "BOOTIA32.EFI",
            "i686-unknown-uefi",
            "A0 compile/link accepted; A1 boot not established",
        ),
        ConduitosArch::X86_64 => ("BOOTX64.EFI", "x86_64-unknown-none", ""),
        ConduitosArch::Aarch64 => ("BOOTAA64.EFI", aarch64_a0::TARGET, ""),
        ConduitosArch::Riscv64 => (
            "BOOTRISCV64.EFI",
            riscv64_a0::TARGET,
            "A0 compile/link accepted; A1 boot not established",
        ),
        ConduitosArch::Loongarch64 => (
            "BOOTLOONGARCH64.EFI",
            loongarch64_a0::TARGET,
            "A0 compile/link accepted; A1 boot not established",
        ),
    };
    ArchitectureRow {
        architecture: arch.as_str(),
        limine_artifact: artifact,
        in_pinned_limine_matrix: true,
        shared_backbone_target: target,
        shared_backbone_profile_known: true,
        executable_backend_present: compile_link_accepted,
        a0_compile_link: compile_link_accepted,
        a1_boot: boot_accepted,
        a2_machine_wake: machine_wake_accepted,
        a3_ordinary_form: ordinary_form_accepted,
        a4_observatory_patchbay: observatory_patchbay_accepted,
        blocker: (!observatory_patchbay_accepted).then_some(blocker),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_archive_artifacts_and_command_enum_must_agree() {
        let artifacts = ConduitosArch::ALL
            .into_iter()
            .map(row)
            .map(|row| row.limine_artifact.to_owned())
            .collect();
        let matrix = build_matrix(&artifacts).unwrap();
        assert_eq!(matrix.architectures.len(), 5);
        assert_eq!(
            matrix
                .architectures
                .iter()
                .filter(|row| row.executable_backend_present)
                .count(),
            5
        );
        let aarch64 = matrix
            .architectures
            .iter()
            .find(|row| row.architecture == "aarch64")
            .unwrap();
        assert!(aarch64.a0_compile_link);
        assert!(aarch64.a1_boot);
        assert!(aarch64.a2_machine_wake);
        assert!(aarch64.a3_ordinary_form);
        assert!(aarch64.a4_observatory_patchbay);
        let ia32 = matrix
            .architectures
            .iter()
            .find(|row| row.architecture == "ia32")
            .unwrap();
        assert!(ia32.executable_backend_present);
        assert!(ia32.a0_compile_link);
        assert!(!ia32.a1_boot);
        let riscv64 = matrix
            .architectures
            .iter()
            .find(|row| row.architecture == "riscv64")
            .unwrap();
        assert!(riscv64.executable_backend_present);
        assert!(riscv64.a0_compile_link);
        assert!(!riscv64.a1_boot);
        let loongarch64 = matrix
            .architectures
            .iter()
            .find(|row| row.architecture == "loongarch64")
            .unwrap();
        assert!(loongarch64.executable_backend_present);
        assert!(loongarch64.a0_compile_link);
        assert!(!loongarch64.a1_boot);
    }

    #[test]
    fn unknown_or_missing_archive_artifact_refuses() {
        let mut artifacts: BTreeSet<_> = ConduitosArch::ALL
            .into_iter()
            .map(row)
            .map(|row| row.limine_artifact.to_owned())
            .collect();
        artifacts.remove("BOOTAA64.EFI");
        artifacts.insert("BOOTUNKNOWN.EFI".to_owned());
        assert!(build_matrix(&artifacts).is_err());
    }
}
