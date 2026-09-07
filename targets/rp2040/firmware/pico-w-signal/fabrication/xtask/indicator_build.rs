//! Exact indicator peripheral build, with no fabricated firmware Plan identity.
use super::doctor::{repo_root, sha256_file, verify_sha256, CYW43_ASSETS, CYW43_ASSET_DIR};
use super::firmware::{identity_manifest_path, uf2_path, FIRMWARE_PACKAGE, TARGET};
use super::{PicoArgs, PicoResult};
use std::{
    path::Path,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) fn run(args: &PicoArgs) -> PicoResult<()> {
    let root = repo_root();
    let firmware = root.join("targets/rp2040/firmware/pico-w-signal");
    let uf2 = uf2_path(&root);
    let elf = uf2.with_extension("");
    let identity_path = identity_manifest_path(&root);
    let generated = uf2.with_extension("indicator-build.json");
    if args.dry_run {
        println!(
            "Would build indicator-resource for {TARGET}, convert {} to {}, and seal {}",
            elf.display(),
            uf2.display(),
            identity_path.display()
        );
        return Ok(());
    }
    let status = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=normal"])
        .current_dir(&root)
        .output()?;
    if !status.status.success() || !status.stdout.is_empty() {
        return Err(
            "indicator firmware requires a clean committed tree for exact build provenance".into(),
        );
    }
    for (name, expected) in CYW43_ASSETS {
        verify_sha256(&root.join(CYW43_ASSET_DIR).join(name), expected)?;
    }
    std::fs::create_dir_all(generated.parent().ok_or("missing artifact parent")?)?;
    // Force a fresh compiler-emitted identity; do not infer the running build
    // from a prior sidecar or the checkout revision observed after compilation.
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_nanos()
        .to_string();
    let status = Command::new("cargo")
        .args([
            "build",
            "--locked",
            "--release",
            "--target",
            TARGET,
            "--no-default-features",
            "--features",
            "indicator-resource",
            "--bin",
            FIRMWARE_PACKAGE,
            "--manifest-path",
        ])
        .arg(firmware.join("Cargo.toml"))
        .env("CONDUIT_PICO_SIGNAL_IDENTITY_RERUN", nonce)
        .env("CONDUIT_PICO_INDICATOR_IDENTITY_SIDECAR", &generated)
        .status()?;
    if !status.success() {
        return Err("indicator firmware build failed".into());
    }
    let mut identity: serde_json::Value = serde_json::from_slice(&std::fs::read(&generated)?)?;
    if identity["firmware_mode"] != "indicator-resource" || identity["tree_state"] != "clean" {
        return Err("compiler-emitted indicator identity is not a clean exact image".into());
    }
    let status = crate::process::command_for("elf2uf2-rs")
        .arg(&elf)
        .arg(&uf2)
        .status()?;
    if !status.success() {
        return Err("indicator UF2 conversion failed".into());
    }
    identity["firmware_sha256"] = sha256_file(&elf)?.into();
    identity["uf2_sha256"] = sha256_file(&uf2)?.into();
    std::fs::write(&identity_path, serde_json::to_vec_pretty(&identity)?)?;
    println!("Indicator identity: {}", identity_path.display());
    println!(
        "Expected firmware build: {}",
        identity["firmware_build_id"]
            .as_str()
            .ok_or("missing compiler build ID")?
    );
    Ok(())
}

pub(super) fn verify_artifact(root: &Path) -> PicoResult<()> {
    let identity: serde_json::Value =
        serde_json::from_slice(&std::fs::read(identity_manifest_path(root))?)?;
    if identity["schema"] != "conduit.pico-indicator/image@1" || identity["tree_state"] != "clean" {
        return Err("indicator image identity is missing or not exact".into());
    }
    let expected = identity["uf2_sha256"]
        .as_str()
        .ok_or("indicator identity has no UF2 digest")?;
    verify_sha256(&uf2_path(root), expected)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indicator_build_dry_run_does_not_require_hardware_or_clean_tree() {
        run(&PicoArgs {
            indicator_resource: true,
            dry_run: true,
            ..Default::default()
        })
        .unwrap();
    }

    #[test]
    fn indicator_mode_cannot_enter_local_proof_or_mix_firmware_modes() {
        let args = PicoArgs {
            indicator_resource: true,
            dry_run: true,
            ..Default::default()
        };
        assert!(super::super::run_local(args.clone()).is_err());
        assert!(super::super::run(PicoArgs {
            usb_remote: true,
            ..args
        })
        .is_err());
    }
}
