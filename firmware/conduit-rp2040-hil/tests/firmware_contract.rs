use conduit_embedded::{
    EmbeddedEventKind, HIL_PROTOCOL_VERSION, HilEventFrame, HilRunHeader, HilRunStatus, RunControl,
    RunIdentity, RunStatus, execute_static_plan,
};
use conduit_rp2040_hil::{
    CONDUIT_REVISION, FIRMWARE_IDENTITY, FULL_PLAN_HASH, GENERIC_RP2040_BOARD_PROFILE,
    ReferenceHost, ReferenceStorage, drivers, plan, profile, with_capability_report,
};
use sha2::{Digest, Sha256};

const FIXTURE: &str = include_str!("../../../conformance/c5/rp2040-firmware-hil.json");

#[test]
fn rp2040_link_contract_places_boot2_before_the_application() {
    let memory = include_str!("../memory.x");
    assert!(memory.contains(".boot2 ORIGIN(BOOT2)"));
    assert!(memory.contains("KEEP(*(.boot2))"));
    assert!(memory.contains("INSERT BEFORE .text"));

    let build = include_str!("../build.rs");
    assert!(build.contains(r#"if target == "thumbv6m-none-eabi""#));
    assert!(build.contains(r#"cargo:rustc-link-arg=-Tlink.x"#));
}

#[test]
fn linked_firmware_path_matches_the_physical_hil_oracle() {
    let repository_revision = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(repository_revision.status.success());
    assert_eq!(
        CONDUIT_REVISION,
        std::str::from_utf8(&repository_revision.stdout)
            .unwrap()
            .trim(),
        "generated firmware plan must name the exact Conduit revision"
    );
    assert_eq!(
        FIRMWARE_IDENTITY.as_bytes(),
        &current_firmware_identity(),
        "embedded firmware identity must cover the current build inputs"
    );
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(
        fixture["cases"][0]["id"],
        "linked-firmware-stream-matches-hil-oracle"
    );

    let selected = profile();
    let static_plan = plan();
    let run = RunIdentity {
        boot_id: [4; 16],
        run_sequence: 9,
    };
    let mut storage = ReferenceStorage::new();
    let mut host = ReferenceHost { indicator: false };
    let summary = execute_static_plan(
        &static_plan,
        &selected,
        &mut storage,
        &mut drivers(),
        &mut host,
        run,
        RunControl {
            maximum_decisions: 64,
            cancellation_at_decision: None,
            initial_tick: 0,
        },
    )
    .unwrap();
    assert_eq!(summary.status, RunStatus::Succeeded);
    assert!(host.indicator);

    let header = HilRunHeader {
        protocol_version: HIL_PROTOCOL_VERSION,
        nonce: [3; 16],
        plan_hash: FULL_PLAN_HASH,
        firmware_identity: FIRMWARE_IDENTITY,
        capability_report_hash: with_capability_report(0, |report| {
            assert_eq!(
                report.current_constraints[0],
                GENERIC_RP2040_BOARD_PROFILE.semantic_hash
            );
            assert_eq!(report.current_constraints[1], selected.identity);
            assert_eq!(report.current_constraints[2], FIRMWARE_IDENTITY);
            assert!(report.capabilities.is_empty());
            assert_eq!(
                report.supported_targets,
                &[conduit_core::Id("thumbv6m-none-eabi")]
            );
            assert_eq!(
                report.supported_abis,
                &[conduit_core::Id("conduit-static-step")]
            );
            report.identity
        }),
        run,
        status: HilRunStatus::Succeeded,
        decisions: summary.decisions,
        evidence_records: summary.evidence_records,
    };
    let mut header_bytes = [0; HilRunHeader::ENCODED_BYTES];
    header.encode(&mut header_bytes);
    assert_eq!(HilRunHeader::decode(&header_bytes).unwrap(), header);

    let mut accepted = [[0_u8; 4]; 2];
    let mut accepted_lengths = [0_usize; 2];
    let mut accepted_count = 0;
    let mut pressure_entered = false;
    let mut pressure_cleared = false;
    let mut succeeded = false;
    for event in storage.events() {
        let frame = HilEventFrame {
            nonce: header.nonce,
            event: *event,
        };
        let mut bytes = [0; HilEventFrame::ENCODED_BYTES];
        frame.encode(&mut bytes).unwrap();
        let decoded = HilEventFrame::decode(&bytes).unwrap();
        assert_eq!(decoded, frame);
        match decoded.event.kind {
            EmbeddedEventKind::ValueAccepted => {
                let value = decoded.event.value.unwrap();
                accepted[accepted_count][..value.as_slice().len()]
                    .copy_from_slice(value.as_slice());
                accepted_lengths[accepted_count] = value.as_slice().len();
                accepted_count += 1;
            }
            EmbeddedEventKind::PressureEntered => pressure_entered = true,
            EmbeddedEventKind::PressureCleared => pressure_cleared = true,
            EmbeddedEventKind::RunSucceeded => succeeded = true,
            _ => {}
        }
    }
    assert_eq!(accepted_count, 2);
    assert_eq!(&accepted[0][..accepted_lengths[0]], &42_u32.to_be_bytes());
    assert_eq!(&accepted[1][..accepted_lengths[1]], &[1]);
    assert!(pressure_entered);
    assert!(pressure_cleared);
    assert!(succeeded);
    let actual = serde_json::json!({
        "same_firmware_path": true,
        "exact_rp2040_plan_hash": static_plan.full_plan_hash == FULL_PLAN_HASH,
        "build_input_identity": true,
        "fresh_capability_report_identity": header.capability_report_hash.as_bytes() != &[0; 32],
        "values": ["0000002a", "01"],
        "pressure_entered": pressure_entered,
        "pressure_cleared": pressure_cleared,
        "terminal": "succeeded"
    });
    assert_eq!(actual, fixture["cases"][0]["expected"]);
}

fn current_firmware_identity() -> [u8; 32] {
    const INPUTS: [&str; 14] = [
        "../../Cargo.lock",
        "../../Cargo.toml",
        "../../crates/conduit-core/Cargo.toml",
        "../../crates/conduit-core/src",
        "../../crates/conduit-embedded/Cargo.toml",
        "../../crates/conduit-embedded/src/lib.rs",
        "../../crates/conduit-embedded-build/Cargo.toml",
        "../../crates/conduit-embedded-build/src",
        "Cargo.toml",
        "build.rs",
        "memory.x",
        "src/lib.rs",
        "src/main.rs",
        "src/reference_plan.rs",
    ];
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut digest = Sha256::new();
    for relative in INPUTS {
        let source = root.join(relative);
        if source.is_dir() {
            let mut files = Vec::new();
            collect_files(&source, &source, &mut files);
            files.sort();
            for path in files {
                let label = format!("{relative}/{}", path.display());
                hash_file(&mut digest, &label, &source.join(path));
            }
        } else {
            hash_file(&mut digest, relative, &source);
        }
    }
    hash_bytes(
        &mut digest,
        "generated-embedded-plan",
        include_bytes!(concat!(env!("OUT_DIR"), "/embedded_plan.rs")),
    );
    hash_bytes(
        &mut digest,
        "cargo-target",
        env!("CONDUIT_FIRMWARE_TARGET").as_bytes(),
    );
    hash_bytes(
        &mut digest,
        "cargo-profile",
        env!("CONDUIT_FIRMWARE_PROFILE").as_bytes(),
    );
    let rustc_version = std::process::Command::new("rustc")
        .arg("-vV")
        .output()
        .unwrap();
    assert!(rustc_version.status.success());
    hash_bytes(&mut digest, "rustc-version", &rustc_version.stdout);
    digest.finalize().into()
}

fn collect_files(
    root: &std::path::Path,
    directory: &std::path::Path,
    files: &mut Vec<std::path::PathBuf>,
) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_files(root, &path, files);
        } else {
            files.push(path.strip_prefix(root).unwrap().to_owned());
        }
    }
}

fn hash_file(digest: &mut Sha256, label: &str, path: &std::path::Path) {
    let bytes = std::fs::read(path).unwrap();
    hash_bytes(digest, label, &bytes);
}

fn hash_bytes(digest: &mut Sha256, label: &str, bytes: &[u8]) {
    digest.update(label.as_bytes());
    digest.update([0]);
    digest.update(u64::try_from(bytes.len()).unwrap().to_be_bytes());
    digest.update(bytes);
}
