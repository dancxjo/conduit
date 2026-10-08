//! Verified emulator/firmware pair for supervisor entropy and protected execution.
use super::{profile::Paths, qemu_source, report::sha256_file, ConduitosArch, ConduitosError};
use crate::cli::GlobalOpts;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
const QEMU_SHA: &str = "a3717477d8e2c84d630bfffbc20f6cd3293eb45aa1e6dac6d0cc27689991c9e1";
const SBI_PACKAGE_SHA: &str = "ba78a00c48dfc5d2adf2ea5f6d01dcbfe7019e132ff02d3c7207ead51d645a32";
const SBI_SHA: &str = "b296d711ecb879489689109c0fd7e0698b5b9550725b4e50fac456bbd3ecbcdf";
const VERSION: &str = "QEMU emulator version 10.2.1 (conduit-protected-riscv64)";
const CONFIGURE: &[&str] = &[
    "--target-list=riscv64-softmmu",
    "--with-pkgversion=conduit-protected-riscv64",
    "--disable-docs",
    "--disable-tools",
    "--disable-guest-agent",
    "--disable-user",
    "--disable-werror",
    "--disable-gtk",
    "--disable-sdl",
    "--disable-vnc",
    "--disable-opengl",
    "--disable-pixman",
    "--disable-capstone",
    "--disable-slirp",
    "--disable-download",
];
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct Inputs {
    schema: String,
    qemu_source_sha256: String,
    opensbi_package_sha256: String,
    opensbi_firmware_sha256: String,
    configure: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Receipt {
    inputs: Inputs,
    proof_class: String,
    version: String,
    executable_sha256: String,
    cc_version: String,
    ninja_version: String,
    glib_version: String,
}
fn inputs() -> Inputs {
    Inputs {
        schema: "conduit.conduitos/riscv64-protected-emulator@1".into(),
        qemu_source_sha256: QEMU_SHA.into(),
        opensbi_package_sha256: SBI_PACKAGE_SHA.into(),
        opensbi_firmware_sha256: SBI_SHA.into(),
        configure: CONFIGURE.iter().map(|s| (*s).into()).collect(),
    }
}
fn directory(paths: &Paths, inputs: &Inputs) -> Result<PathBuf, ConduitosError> {
    let key = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(inputs).map_err(refusal)?)
    );
    Ok(paths
        .root
        .join("target/conduitos/toolchain/riscv64-domain-qemu")
        .join(key))
}
fn validate(
    receipt: &Receipt,
    expected: &Inputs,
    executable: &str,
    firmware: &str,
    version: &str,
) -> Result<(), ConduitosError> {
    if receipt.inputs != *expected
        || receipt.executable_sha256 != executable
        || firmware != expected.opensbi_firmware_sha256
        || receipt.version != VERSION
        || version != VERSION
        || receipt.proof_class != "emulator-tool-only"
        || receipt.cc_version.is_empty()
        || receipt.ninja_version.is_empty()
        || receipt.glib_version.is_empty()
    {
        return Err(refusal("prepared emulator or firmware identity mismatch"));
    }
    Ok(())
}
pub(super) fn selected(paths: &Paths) -> Result<Option<(PathBuf, PathBuf)>, ConduitosError> {
    let expected = inputs();
    let directory = directory(paths, &expected)?;
    let receipt = directory.join("receipt.json");
    if !receipt.exists() {
        return Ok(None);
    }
    let receipt: Receipt =
        serde_json::from_slice(&fs::read(receipt).map_err(refusal)?).map_err(refusal)?;
    let executable = directory.join("qemu-system-riscv64");
    let firmware = directory.join("fw_dynamic.bin");
    let executable_sha = sha256_file(&executable)?;
    let firmware_sha = sha256_file(&firmware)?;
    // Verify content before executing even the version query.
    if receipt.inputs != expected
        || receipt.executable_sha256 != executable_sha
        || firmware_sha != SBI_SHA
    {
        return Err(refusal("prepared emulator or firmware content mismatch"));
    }
    validate(
        &receipt,
        &expected,
        &executable_sha,
        &firmware_sha,
        &qemu_source::version(&executable, &["--version"])?,
    )?;
    Ok(Some((executable, firmware)))
}
pub(super) fn prepare(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    let paths = Paths::new(ConduitosArch::Riscv64)?;
    let expected = inputs();
    let destination = directory(&paths, &expected)?;
    if opts.dry_run {
        println!(
            "prepare verified QEMU 10.2.1 and OpenSBI 1.8.1 at {}",
            destination.display()
        );
        return Ok(());
    }
    if selected(&paths)?.is_some() {
        retain_receipt(&paths, &destination)?;
        if opts.json {
            println!(
                "{}",
                fs::read_to_string(destination.join("receipt.json")).map_err(refusal)?
            );
        } else if !opts.quiet {
            println!(
                "Verified protected RISC-V64 tools: {}",
                destination.display()
            );
        }
        return Ok(());
    }
    fs::create_dir_all(&destination).map_err(refusal)?;
    for name in ["qemu-system-riscv64", "fw_dynamic.bin"] {
        if destination.join(name).exists() {
            return Err(refusal("unreceipted tool preserved for inspection"));
        }
    }
    let built = qemu_source::build(
        &destination,
        QEMU_SHA,
        CONFIGURE,
        &[],
        "qemu-system-riscv64",
    )?;
    let package = destination.join("opensbi_1.8.1-1_all.deb");
    qemu_source::download(
        "https://archive.ubuntu.com/ubuntu/pool/main/o/opensbi/opensbi_1.8.1-1_all.deb",
        &package,
        SBI_PACKAGE_SHA,
    )?;
    let extracted = destination.join(format!("firmware-{}", std::process::id()));
    qemu_source::run(
        Command::new("dpkg-deb")
            .arg("--extract")
            .arg(&package)
            .arg(&extracted),
    )?;
    let firmware = extracted.join("usr/lib/riscv64-linux-gnu/opensbi/generic/fw_dynamic.bin");
    qemu_source::verify(&firmware, SBI_SHA)?;
    let version = qemu_source::version(&built, &["--version"])?;
    let receipt = Receipt {
        inputs: expected.clone(),
        proof_class: "emulator-tool-only".into(),
        version: version.clone(),
        executable_sha256: sha256_file(&built)?,
        cc_version: qemu_source::version(Path::new("cc"), &["--version"])?,
        ninja_version: qemu_source::version(Path::new("ninja"), &["--version"])?,
        glib_version: qemu_source::version(Path::new("pkg-config"), &["--modversion", "glib-2.0"])?,
    };
    validate(
        &receipt,
        &expected,
        &receipt.executable_sha256,
        SBI_SHA,
        &version,
    )?;
    fs::copy(built, destination.join("qemu-system-riscv64")).map_err(refusal)?;
    fs::copy(firmware, destination.join("fw_dynamic.bin")).map_err(refusal)?;
    fs::write(
        destination.join("receipt.json"),
        serde_json::to_vec_pretty(&receipt).map_err(refusal)?,
    )
    .map_err(refusal)?;
    selected(&paths)?.ok_or_else(|| refusal("prepared tool receipt absent"))?;
    retain_receipt(&paths, &destination)?;
    if opts.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&receipt).map_err(refusal)?
        );
    } else if !opts.quiet {
        println!(
            "Prepared protected RISC-V64 tools: {}",
            destination.display()
        );
    }
    Ok(())
}
fn retain_receipt(paths: &Paths, destination: &Path) -> Result<(), ConduitosError> {
    fs::create_dir_all(&paths.target).map_err(refusal)?;
    fs::copy(
        destination.join("receipt.json"),
        paths.target.join("riscv64-protected-emulator.json"),
    )
    .map_err(refusal)?;
    Ok(())
}

fn refusal(error: impl std::fmt::Display) -> ConduitosError {
    ConduitosError::refusal("riscv64-emulator-preparation-refused", error.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_recipe_executable_and_firmware_must_match() {
        let expected = inputs();
        let receipt = Receipt {
            inputs: expected.clone(),
            proof_class: "emulator-tool-only".into(),
            version: VERSION.into(),
            executable_sha256: "binary".into(),
            cc_version: "cc".into(),
            ninja_version: "ninja".into(),
            glib_version: "glib".into(),
        };
        assert!(validate(&receipt, &expected, "binary", SBI_SHA, VERSION).is_ok());
        assert!(validate(&receipt, &expected, "foreign", SBI_SHA, VERSION).is_err());
        assert!(validate(&receipt, &expected, "binary", "foreign-firmware", VERSION).is_err());
        assert!(validate(
            &receipt,
            &expected,
            "binary",
            SBI_SHA,
            "QEMU emulator version 8.2.2"
        )
        .is_err());
        let mut altered = expected.clone();
        altered.configure.clear();
        assert!(validate(&receipt, &altered, "binary", SBI_SHA, VERSION).is_err());
        let mut altered = receipt.clone();
        altered.proof_class = "freestanding-emulator".into();
        assert!(validate(&altered, &expected, "binary", SBI_SHA, VERSION).is_err());
    }
}
