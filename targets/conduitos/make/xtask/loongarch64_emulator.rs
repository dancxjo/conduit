//! Pinned preparation of the reviewed diagnostic emulator, separate from stock QEMU.
use super::qemu_source::{graphics, version};
use super::{profile::Paths, report::sha256_file, ConduitosArch, ConduitosError};
use crate::cli::GlobalOpts;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

const ARCHIVE_SHA256: &str = "a3717477d8e2c84d630bfffbc20f6cd3293eb45aa1e6dac6d0cc27689991c9e1";
const GRAPHICAL_VERSION: &str =
    "QEMU emulator version 10.2.1 (conduit-diagnostic-misc-drdtl-graphics)";
const VERSION: &str = "QEMU emulator version 10.2.1 (conduit-diagnostic-misc-drdtl)";
const PATCHES: [&str; 2] = ["qemu-misc-drdtl.patch", "qemu-nonfault-badi.patch"];
const CONFIGURE: &[&str] = &[
    "--target-list=loongarch64-softmmu",
    "--with-pkgversion=conduit-diagnostic-misc-drdtl",
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
    archive_sha256: String,
    patch_sha256: [String; 2],
    configure: Vec<String>,
}
#[derive(Serialize, Deserialize)]
struct Receipt {
    inputs: Inputs,
    proof_class: String,
    version: String,
    executable_sha256: String,
    cc_version: String,
    ninja_version: String,
    glib_version: String,
    diagnostic_correction: bool,
    upstream_release_claimed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    graphics: Option<graphics::Evidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sdl_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pixman_version: Option<String>,
}

fn configure(graphical: bool) -> Vec<String> {
    let mut arguments = CONFIGURE
        .iter()
        .map(|argument| {
            match (*argument, graphical) {
                ("--with-pkgversion=conduit-diagnostic-misc-drdtl", true) => {
                    "--with-pkgversion=conduit-diagnostic-misc-drdtl-graphics"
                }
                ("--disable-sdl", true) => "--enable-sdl",
                ("--disable-pixman", true) => "--enable-pixman",
                (argument, _) => argument,
            }
            .to_owned()
        })
        .collect::<Vec<_>>();
    if graphical {
        arguments.extend(["--disable-modules".into(), "--disable-debug-info".into()]);
    }
    arguments
}
fn graphical(inputs: &Inputs) -> bool {
    inputs.configure.iter().any(|arg| arg == "--enable-pixman")
}
fn expected_version(inputs: &Inputs) -> &'static str {
    if graphical(inputs) {
        GRAPHICAL_VERSION
    } else {
        VERSION
    }
}
fn inputs(paths: &Paths, graphical: bool) -> Result<Inputs, ConduitosError> {
    Ok(Inputs {
        schema: "conduit.conduitos/loongarch64-diagnostic-emulator@1".into(),
        archive_sha256: ARCHIVE_SHA256.into(),
        patch_sha256: [
            sha256_file(&patch(paths, PATCHES[0]))?,
            sha256_file(&patch(paths, PATCHES[1]))?,
        ],
        configure: configure(graphical),
    })
}
fn patch(paths: &Paths, name: &str) -> PathBuf {
    paths
        .root
        .join("targets/conduitos/proof/tools/loongarch64")
        .join(name)
}
fn directory(paths: &Paths, inputs: &Inputs) -> Result<PathBuf, ConduitosError> {
    let encoded = serde_json::to_vec(inputs).map_err(refusal)?;
    let key = format!("{:x}", Sha256::digest(encoded));
    Ok(paths
        .root
        .join("target/conduitos/toolchain/loongarch64-domain-qemu")
        .join(key))
}
fn validate(
    receipt: &Receipt,
    inputs: &Inputs,
    digest: &str,
    version: &str,
) -> Result<(), ConduitosError> {
    if receipt.inputs != *inputs
        || receipt.executable_sha256 != digest
        || receipt.version != expected_version(inputs)
        || version != expected_version(inputs)
        || receipt.proof_class != "diagnostic-emulator-tool-only"
        || !receipt.diagnostic_correction
        || receipt.upstream_release_claimed
        || receipt.cc_version.is_empty()
        || receipt.ninja_version.is_empty()
        || receipt.glib_version.is_empty()
        || (graphical(inputs)
            && (receipt.graphics.as_ref().is_none_or(|v| !v.valid())
                || receipt.sdl_version.as_ref().is_none_or(String::is_empty)
                || receipt.pixman_version.as_ref().is_none_or(String::is_empty)))
        || (!graphical(inputs)
            && (receipt.graphics.is_some()
                || receipt.sdl_version.is_some()
                || receipt.pixman_version.is_some()))
    {
        return Err(refusal("prepared diagnostic emulator identity mismatch"));
    }
    Ok(())
}
fn cached(paths: &Paths, inputs: &Inputs) -> Result<Option<PathBuf>, ConduitosError> {
    let directory = directory(paths, inputs)?;
    let receipt = directory.join("receipt.json");
    if !receipt.exists() {
        return Ok(None);
    }
    let receipt: Receipt =
        serde_json::from_slice(&fs::read(receipt).map_err(refusal)?).map_err(refusal)?;
    let binary = directory.join("qemu-system-loongarch64");
    // Verify the executable before running even its version query.
    let digest = sha256_file(&binary)?;
    if receipt.inputs != *inputs || receipt.executable_sha256 != digest {
        return Err(refusal("prepared diagnostic emulator content mismatch"));
    }
    validate(
        &receipt,
        inputs,
        &digest,
        &version(&binary, &["--version"])?,
    )?;
    if graphical(inputs)
        && receipt.graphics.as_ref() != Some(&graphics::probe(&binary, &directory)?)
    {
        return Err(refusal("prepared graphical emulator capabilities changed"));
    }
    Ok(Some(binary))
}

pub(super) fn selected(paths: &Paths) -> Result<PathBuf, ConduitosError> {
    if let Some(binary) = cached(paths, &inputs(paths, false)?)? {
        return Ok(binary);
    }
    [
        paths
            .root
            .join("target/conduitos/toolchain/loongarch64-misc-drdtl/qemu-system-loongarch64"),
        paths
            .root
            .join("target/conduitos/toolchain/riscv64-root/usr/bin/qemu-system-loongarch64"),
        PathBuf::from("/usr/bin/qemu-system-loongarch64"),
    ]
    .into_iter()
    .find(|path| path.is_file())
    .ok_or_else(|| {
        ConduitosError::refusal(
            "unavailable-loongarch64-emulator",
            "qemu-system-loongarch64 is required",
        )
    })
}

pub(super) fn selected_graphical(paths: &Paths) -> Result<PathBuf, ConduitosError> {
    cached(paths, &inputs(paths, true)?)?.ok_or_else(|| {
        ConduitosError::refusal(
            "unavailable-loongarch64-graphical-emulator",
            "run `cargo xtask make conduitos prepare-loongarch64-domain-emulator --graphical`",
        )
    })
}
pub(super) fn prepare(graphical: bool, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    let paths = Paths::new(ConduitosArch::Loongarch64)?;
    let inputs = inputs(&paths, graphical)?;
    let destination = directory(&paths, &inputs)?;
    if opts.dry_run {
        println!(
            "prepare pinned QEMU 10.2.1 with both reviewed LoongArch corrections at {}",
            destination.display()
        );
        return Ok(());
    }
    if graphical {
        super::loongarch64_firmware::prepare(&paths)?;
    }
    if let Some(binary) = cached(&paths, &inputs)? {
        if opts.json {
            println!(
                "{}",
                fs::read_to_string(destination.join("receipt.json")).map_err(refusal)?
            );
        } else if !opts.quiet {
            println!("Verified diagnostic emulator: {}", binary.display());
        }
        return Ok(());
    }
    let library = |name| version(Path::new("pkg-config"), &["--modversion", name]);
    let (sdl_version, pixman_version) = if graphical {
        (Some(library("sdl2")?), Some(library("pixman-1")?))
    } else {
        (None, None)
    };
    let configure = inputs
        .configure
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let built = super::qemu_source::build(
        &destination,
        ARCHIVE_SHA256,
        &configure,
        &PATCHES.map(|name| patch(&paths, name)),
        "qemu-system-loongarch64",
    )?;
    let actual_version = version(&built, &["--version"])?;
    let receipt = Receipt {
        inputs: inputs.clone(),
        proof_class: "diagnostic-emulator-tool-only".into(),
        version: actual_version.clone(),
        executable_sha256: sha256_file(&built)?,
        cc_version: version(Path::new("cc"), &["--version"])?,
        ninja_version: version(Path::new("ninja"), &["--version"])?,
        glib_version: version(Path::new("pkg-config"), &["--modversion", "glib-2.0"])?,
        diagnostic_correction: true,
        upstream_release_claimed: false,
        graphics: if graphical {
            Some(graphics::probe(&built, &destination)?)
        } else {
            None
        },
        sdl_version,
        pixman_version,
    };
    validate(
        &receipt,
        &inputs,
        &receipt.executable_sha256,
        &actual_version,
    )?;
    let binary = destination.join("qemu-system-loongarch64");
    if binary.exists() {
        return Err(refusal(
            "unreceipted diagnostic executable already exists; preserved for inspection",
        ));
    }
    fs::copy(&built, &binary).map_err(refusal)?;
    fs::write(
        destination.join("receipt.json"),
        serde_json::to_vec_pretty(&receipt).map_err(refusal)?,
    )
    .map_err(refusal)?;
    if graphical {
        selected_graphical(&paths)?;
    } else {
        cached(&paths, &inputs)?.ok_or_else(|| refusal("prepared executable receipt missing"))?;
    }
    if opts.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&receipt).map_err(refusal)?
        );
    } else if !opts.quiet {
        println!("Prepared diagnostic emulator: {}", binary.display());
    }
    Ok(())
}
fn refusal(error: impl std::fmt::Display) -> ConduitosError {
    ConduitosError::refusal(
        "loongarch64-diagnostic-emulator-preparation-refused",
        error.to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pinned_inputs(graphical: bool) -> Inputs {
        Inputs {
            schema: "conduit.conduitos/loongarch64-diagnostic-emulator@1".into(),
            archive_sha256: ARCHIVE_SHA256.into(),
            patch_sha256: [
                "abd2875c1db4a3a7897fda4967c2bddcce60f63db8e46f08d18774748a64cc5e".into(),
                "df89114623b6d2467a2a5bc77532981ce6b0cccc64b6e4585a7503e4b7af8347".into(),
            ],
            configure: configure(graphical),
        }
    }
    #[test]
    fn serial_preparation_preserves_the_existing_cache_identity() {
        let encoded = serde_json::to_vec(&pinned_inputs(false)).unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(encoded)),
            "83b88accc2d317b10d86224fef8969c3aaa901eba19e5b8dd171a0df5ca7e3fb"
        );
        assert_ne!(pinned_inputs(false), pinned_inputs(true));
        assert_eq!(expected_version(&pinned_inputs(true)), GRAPHICAL_VERSION);
    }
    #[test]
    fn graphical_receipt_cannot_substitute_serial_or_claim_a_product_frame() {
        let inputs = pinned_inputs(true);
        let mut receipt = Receipt {
            inputs: inputs.clone(),
            proof_class: "diagnostic-emulator-tool-only".into(),
            version: GRAPHICAL_VERSION.into(),
            executable_sha256: "binary".into(),
            cc_version: "cc".into(),
            ninja_version: "ninja".into(),
            glib_version: "glib".into(),
            diagnostic_correction: true,
            upstream_release_claimed: false,
            graphics: Some(graphics::Evidence {
                display_backends: vec!["none".into(), "sdl".into()],
                screendump_command_available: true,
                guest_instructions_executed: false,
                product_frame_proved: false,
            }),
            sdl_version: Some("sdl".into()),
            pixman_version: Some("pixman".into()),
        };
        assert!(validate(&receipt, &inputs, "binary", GRAPHICAL_VERSION).is_ok());
        assert!(validate(&receipt, &pinned_inputs(false), "binary", GRAPHICAL_VERSION).is_err());
        assert!(validate(&receipt, &inputs, "binary", VERSION).is_err());
        receipt.graphics.as_mut().unwrap().product_frame_proved = true;
        assert!(validate(&receipt, &inputs, "binary", GRAPHICAL_VERSION).is_err());
        receipt.graphics.as_mut().unwrap().product_frame_proved = false;
        receipt.sdl_version = None;
        assert!(validate(&receipt, &inputs, "binary", GRAPHICAL_VERSION).is_err());
        receipt.sdl_version = Some("sdl".into());
        receipt.graphics = None;
        assert!(validate(&receipt, &inputs, "binary", GRAPHICAL_VERSION).is_err());
    }
    #[test]
    fn changed_source_patch_recipe_executable_or_claims_refuse() {
        let inputs = Inputs {
            schema: "schema".into(),
            archive_sha256: "source".into(),
            patch_sha256: ["misc".into(), "badi".into()],
            configure: vec!["configure".into()],
        };
        let mut receipt = Receipt {
            inputs: inputs.clone(),
            proof_class: "diagnostic-emulator-tool-only".into(),
            version: VERSION.into(),
            executable_sha256: "binary".into(),
            cc_version: "cc".into(),
            ninja_version: "ninja".into(),
            glib_version: "glib".into(),
            diagnostic_correction: true,
            upstream_release_claimed: false,
            graphics: None,
            sdl_version: None,
            pixman_version: None,
        };
        assert!(validate(&receipt, &inputs, "binary", VERSION).is_ok());
        assert!(validate(&receipt, &inputs, "changed", VERSION).is_err());
        assert!(validate(&receipt, &inputs, "binary", "QEMU emulator version 10.2.1").is_err());
        for altered in [
            Inputs {
                archive_sha256: "changed".into(),
                ..inputs.clone()
            },
            Inputs {
                patch_sha256: ["misc".into(), "changed".into()],
                ..inputs.clone()
            },
            Inputs {
                configure: vec![],
                ..inputs.clone()
            },
        ] {
            assert!(validate(&receipt, &altered, "binary", VERSION).is_err());
        }
        receipt.upstream_release_claimed = true;
        assert!(validate(&receipt, &inputs, "binary", VERSION).is_err());
        receipt.upstream_release_claimed = false;
        receipt.diagnostic_correction = false;
        assert!(validate(&receipt, &inputs, "binary", VERSION).is_err());
    }
}
