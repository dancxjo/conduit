//! A cached dispatcher must not silently execute a different checkout's suite.
#[cfg(unix)]
#[test]
fn changed_or_missing_suite_refuses_before_launching_cargo() {
    use std::{fs, os::unix::fs::PermissionsExt, process::Command};
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "conduit-focused-custody-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(root.join("tools/xtask/src/suites")).unwrap();
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _scratch = Scratch(root.clone());
    fs::write(root.join("Cargo.toml"), "[workspace]\nmembers=[]\n").unwrap();
    let marker = root.join("cargo-was-launched");
    let cargo = root.join("cargo");
    fs::write(&cargo, "#!/bin/sh\nprintf invoked > \"$CUSTODY_MARKER\"\n").unwrap();
    fs::set_permissions(&cargo, fs::Permissions::from_mode(0o755)).unwrap();
    for (suite, file, compiled) in [
        (
            "acoustic-flow",
            "acoustic_flow.rs",
            include_bytes!("../../xtask/src/suites/acoustic_flow.rs").as_slice(),
        ),
        (
            "acoustic-control",
            "acoustic_control.rs",
            include_bytes!("../../xtask/src/suites/acoustic_control.rs").as_slice(),
        ),
        (
            "angle-reference",
            "angle_reference.rs",
            include_bytes!("../../xtask/src/suites/angle_reference.rs").as_slice(),
        ),
        (
            "dsp-shared-compositions",
            "dsp_shared_compositions.rs",
            include_bytes!("../../xtask/src/suites/dsp_shared_compositions.rs").as_slice(),
        ),
        (
            "inverse-angle",
            "inverse_angle.rs",
            include_bytes!("../../xtask/src/suites/inverse_angle.rs").as_slice(),
        ),
    ] {
        let path = root.join("tools/xtask/src/suites").join(file);
        for missing in [false, true] {
            if missing {
                fs::remove_file(&path).unwrap();
            } else {
                let mut changed = compiled.to_vec();
                changed.extend_from_slice(b"\n// newer checkout suite\n");
                fs::write(&path, changed).unwrap();
            }
            let output = Command::new(env!("CARGO_BIN_EXE_conduit-xtask-dispatch"))
                .args(["check", suite])
                .current_dir(&root)
                .env("PATH", &root)
                .env("CUSTODY_MARKER", &marker)
                .output()
                .unwrap();
            assert!(!output.status.success(), "{suite}, missing={missing}");
            let error = String::from_utf8_lossy(&output.stderr);
            assert!(error.contains(file), "{error}");
            assert!(
                error.contains(if missing {
                    "cannot verify focused suite"
                } else {
                    "differs from this compiled dispatcher"
                }),
                "{error}"
            );
            assert!(!marker.exists(), "stale suite launched Cargo");
        }
    }
}
