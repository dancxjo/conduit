use std::process::Command;

#[test]
fn bootsel_alias_reports_only_the_requested_operation_without_external_tools() {
    for entrance in ["pico", "pico-local"] {
        let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
            .args([
                "--dry-run",
                "make",
                entrance,
                "bootsel",
                "--usb-remote",
                "--link-port",
                "/nonexistent/pico-link",
                "--port",
                "/nonexistent/pico-port",
            ])
            .env("PATH", "")
            .output()
            .expect("run xtask without external tools");
        assert!(output.status.success(), "{entrance}: {output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "==> pico bootsel (dry-run): request exact current firmware over CDC 0\n"
        );
        assert!(output.stderr.is_empty(), "{entrance}: {output:?}");
    }
}
