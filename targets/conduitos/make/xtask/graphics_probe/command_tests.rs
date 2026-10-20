use super::*;
use crate::cli::Cli;
use clap::Parser;
fn args(arch: ConduitosArch) -> ProbeArgs {
    ProbeArgs {
        image_directory: "product".into(),
        output_directory: "new-output".into(),
        arch,
        provider: Provider::Ramfb,
        firmware: "firmware".into(),
        vars_template: None,
        boot_kernel: None,
        emulator: None,
    }
}
#[test]
fn firmware_pairs_and_boot_kernels_are_architecture_specific_inputs() {
    let mut ia32 = args(ConduitosArch::Ia32);
    assert!(ia32.validate_firmware_inputs().is_err());
    ia32.vars_template = Some("vars".into());
    assert!(ia32.validate_firmware_inputs().is_ok());
    ia32.boot_kernel = Some("unexpected".into());
    assert!(ia32.validate_firmware_inputs().is_err());
    let mut riscv = args(ConduitosArch::Riscv64);
    assert!(riscv.validate_firmware_inputs().is_err());
    riscv.boot_kernel = Some("uboot".into());
    assert!(riscv.validate_firmware_inputs().is_ok());
    for arch in [
        ConduitosArch::Aarch64,
        ConduitosArch::Loongarch64,
        ConduitosArch::X86_64,
    ] {
        let mut ordinary = args(arch);
        assert!(ordinary.validate_firmware_inputs().is_ok());
        ordinary.vars_template = Some("unexpected".into());
        assert!(ordinary.validate_firmware_inputs().is_err());
    }
}
#[test]
fn supported_repository_entrance_requires_explicit_candidate_and_artifact_paths() {
    let command = [
        "xtask",
        "make",
        "conduitos",
        "framebuffer-probe",
        "--arch",
        "aarch64",
        "--provider",
        "ramfb",
        "--firmware",
        "firmware.fd",
        "--image-directory",
        "normal-product",
        "--output-directory",
        "fresh-evidence",
    ];
    assert!(Cli::try_parse_from(command).is_ok());
    assert!(Cli::try_parse_from(&command[..command.len() - 2]).is_err());
    let mut invalid = command;
    invalid[7] = "imaginary-gpu";
    assert!(Cli::try_parse_from(invalid).is_err());
}

#[test]
fn live_media_selection_does_not_relabel_pi_boards_as_virt() {
    assert_eq!(
        live_host(ConduitosArch::Aarch64).unwrap().row().host,
        "conduitos/aarch64/virt"
    );
    assert!(live_host(ConduitosArch::Armv6).is_err());
}

#[test]
fn kernel_machine_identity_is_not_inferred_from_a_product_directory_name() {
    let mut image = vec![0; 64];
    image[..6].copy_from_slice(b"\x7fELF\x02\x01");
    image[18..20].copy_from_slice(&183_u16.to_le_bytes());
    assert!(require_kernel_arch(ConduitosArch::Aarch64, &image).is_ok());
    assert!(require_kernel_arch(ConduitosArch::Riscv64, &image).is_err());
}
