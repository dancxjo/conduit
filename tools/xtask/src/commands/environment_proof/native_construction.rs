//! CPU construction, then actual compilation of the Make-emitted resident data.
//! Identity/entropy/native providers remain fixtures; this builds no firmware.
use super::execute;
use std::{error::Error, fs, path::Path, process::Command};
pub(super) fn prove(root: &Path) -> Result<(), Box<dyn Error>> {
    execute(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "-p",
        "conduit-assigned-plan",
        "--lib",
    ]))?;
    execute(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "-p",
        "conduit-embedded-build",
        "--test",
        "assigned_plan",
    ]))?;
    let proof = root.join("target/environment-resident-recipe-proof");
    fs::create_dir_all(proof.join("src"))?;
    let layout = proof.join("src/layout.rs");
    if layout.exists() {
        fs::remove_file(&layout)?;
    }
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
                "assignment_transfer",
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
            .env("CONDUIT_LAYOUT_PROOF_OUTPUT", &layout)
            .env("CONDUIT_SOURCE_RECIPE_PROOF_OUTPUT", &source_generated),
    )?;
    if !generated.is_file() || !source_generated.is_file() || !layout.is_file() {
        return Err("Make did not emit the resident recipe".into());
    }
    fs::write(
        proof.join("src/lib.rs"),
        r#"pub mod resident;
pub mod source_recipe;
pub mod layout;
#[test]
fn independently_emitted_layout_is_bound_to_the_same_checked_source() {
    let resident = resident::resident();
    assert_eq!(source_recipe::spec().source, resident.source);
    let pinned = layout::layout(resident.source).unwrap();
    assert_eq!(pinned.records, resident.layout.records);
    assert_eq!(pinned.counts, resident.layout.counts);
    let mut wrong_source = resident.source;
    wrong_source.expanded_plot_digest[0] ^= 1;
    assert!(layout::layout(wrong_source).is_err());
}
"#,
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
            .args(["test", "--lib", "--locked", "--offline"])
            .env("CARGO_TARGET_DIR", root.join("target/environment-runtime")),
    )?;
    let command_proof = root.join("target/environment-native-command-proof");
    execute(
        Command::new("rustc")
            .current_dir(root)
            .args(["--edition", "2021", "--test"])
            .arg("targets/esp32/proof/tm1637/src/native_command.rs")
            .arg("-o")
            .arg(&command_proof),
    )?;
    execute(&mut Command::new(command_proof))
}
