//! Public Make preparation accepts explicit offline and radio-enabled profiles.
//! Dry runs never compile firmware, initialize equipment, or write artifacts.
use std::{path::Path, process::Command};

#[test]
fn public_rp2040_build_dry_run_accepts_optional_checked_radio() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for profile in ["pico-w-house-yellow", "pico-w-house-green"] {
        let output = std::env::temp_dir().join(format!(
            "conduit-optional-radio-dry-run-{}-{profile}",
            std::process::id()
        ));
        assert!(!output.exists());
        let result = Command::new(env!("CARGO_BIN_EXE_xtask"))
            .current_dir(&root)
            .args(["--dry-run", "make", "host", "build"])
            .arg(format!("targets/rp2040/profiles/{profile}.host.conduit"))
            .arg("--output")
            .arg(&output)
            .output()
            .expect("public Make dry run");
        assert!(
            result.status.success(),
            "{profile}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stdout).contains("would BUILD"));
        assert!(!output.exists(), "dry run created artifacts");
    }
}
