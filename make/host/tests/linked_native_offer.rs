//! Compile the shared Make emitter's real microphone descriptions. This proves
//! metadata parity only, not initialized ALSA equipment or capture authority.
use conduit_host_make::linked_material::write_offer;
use std::{fmt::Write, fs, path::Path, process::Command};

#[test]
fn linked_microphone_clip_and_flow_preserve_complete_native_offers() {
    let offers = [
        conduit_std_offers::microphone_clip_offer(),
        conduit_std_offers::converted_microphone_clip_offer(),
        conduit_std_offers::microphone_flow_offer(false),
        conduit_std_offers::microphone_flow_offer(true),
    ];
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.join("../..").canonicalize().unwrap();
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    let directory = target.join("linked-native-offer-conformance");
    fs::create_dir_all(directory.join("src")).unwrap();
    fs::write(
        directory.join("Cargo.toml"),
        format!(
            "[package]\nname=\"linked-native-offer-conformance\"\nversion=\"0.0.0\"\nedition=\"2021\"\n[workspace]\n[dependencies]\nconduit-core={{path={:?}}}\nserde_json=\"1\"\n",
            root.join("architecture/core")
        ),
    )
    .unwrap();
    let mut generated = String::from("extern crate alloc; use conduit_core as c;\n");
    for (index, offer) in offers.iter().enumerate() {
        // Real installed-provider requirements must remain in emitted metadata.
        assert_eq!(offer.host_calls.len(), 1);
        assert_eq!(offer.resource_requirements.len(), 1);
        assert_eq!(offer.authority_requirements.len(), 1);
        write_offer(&mut generated, &format!("offer_{index}"), offer).unwrap();
    }
    // Runtime reconstructs typed fields directly. JSON is used only below as an
    // independent complete public-wire equality oracle, never for construction.
    assert!(!generated.contains("serde_json::"));
    generated.push_str("fn main() {\n");
    for (index, offer) in offers.iter().enumerate() {
        let expected = serde_json::to_vec(offer).unwrap();
        writeln!(
            generated,
            "let actual = offer_{index}().expect(\"linked complete native offer\");\nassert_eq!(serde_json::to_vec(&actual).unwrap().as_slice(), &{expected:?});"
        )
        .unwrap();
        writeln!(
            generated,
            "assert_eq!(actual.host_calls.len(), 1); assert_eq!(actual.resource_requirements.len(), 1); assert_eq!(actual.authority_requirements.len(), 1);"
        )
        .unwrap();
        println!(
            "linked native offer {}: public_bytes={} defaults={} laws={} realization_properties={}",
            offer.implementation.implementation_id.as_str(),
            expected.len(),
            offer.semantic_contract.configuration.len(),
            offer.semantic_contract.laws.len(),
            offer.realization_properties.len()
        );
    }
    generated.push_str("}\n");
    fs::write(directory.join("src/main.rs"), generated).unwrap();
    let output = Command::new("cargo")
        .args(["+stable", "run", "--offline", "--quiet", "--manifest-path"])
        .arg(directory.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", directory.join("target"))
        .env("CARGO_INCREMENTAL", "0")
        .env("CARGO_PROFILE_DEV_DEBUG", "0")
        .output()
        .unwrap();
    fs::write(directory.join("proof.stdout"), &output.stdout).unwrap();
    fs::write(directory.join("proof.stderr"), &output.stderr).unwrap();
    assert!(
        output.status.success(),
        "generated real microphone offers failed compilation/parity: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
