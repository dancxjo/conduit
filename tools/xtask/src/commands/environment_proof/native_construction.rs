//! CPU construction, then actual compilation of the Make-emitted resident data.
//! Identity/entropy/native providers remain fixtures; this builds no firmware.
use super::execute;
use std::{error::Error, fs, path::Path, process::Command};
pub(super) fn prove(root: &Path) -> Result<(), Box<dyn Error>> {
    let proof = root.join("target/environment-resident-recipe-proof");
    fs::create_dir_all(proof.join("src"))?;
    let generated = proof.join("src/resident.rs");
    let source_generated = proof.join("src/source_recipe.rs");
    if source_generated.exists() {
        fs::remove_file(&source_generated)?;
    }
    if generated.exists() {
        fs::remove_file(&generated)?;
    }
    execute(
        Command::new("cargo")
            .current_dir(root)
            .args([
                "test",
                "--locked",
                "--manifest-path",
                "bodies/environment/make/Cargo.toml",
                "--test",
                "native",
                "--test",
                "network_assignment",
                "--test",
                "selected_port_shapes",
                "--test",
                "credential_owner",
                "--test",
                "network_play",
                "--test",
                "root_control",
                "--",
                "--nocapture",
            ])
            .env("CARGO_TARGET_DIR", root.join("target"))
            .env("CONDUIT_RECIPE_PROOF_OUTPUT", &generated)
            .env("CONDUIT_SOURCE_RECIPE_PROOF_OUTPUT", &source_generated),
    )?;
    if !generated.is_file() || !source_generated.is_file() {
        return Err("Make did not emit the resident recipe".into());
    }
    fs::write(
        proof.join("src/lib.rs"),
        "pub mod resident;\npub mod source_recipe;\n",
    )?;
    fs::write(
        proof.join("Cargo.toml"),
        r#"[package]
name = "conduit-environment-resident-recipe-proof"
version = "0.0.0"
edition = "2021"
publish = false
[workspace]
[dependencies]
conduit-core = { path = "../../architecture/core" }
conduit-time = { path = "../../semantics/time", default-features = false }
conduit-kernel = { path = "../../architecture/kernel", features = ["alloc"] }
conduit-environment-runtime = { path = "../../bodies/environment/runtime", features = ["appliance"] }
"#,
    )?;
    execute(
        Command::new("cargo")
            .current_dir(&proof)
            .args(["generate-lockfile", "--offline"]),
    )?;
    execute(
        Command::new("cargo")
            .current_dir(&proof)
            .args(["check", "--locked", "--offline"])
            .env("CARGO_TARGET_DIR", root.join("target/environment-runtime")),
    )
}
