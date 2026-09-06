use std::process::Command;

#[test]
fn installed_run_reports_two_hosts_one_line_and_terminal_values() {
    let report = std::env::temp_dir().join(format!(
        "conduit-two-std-line-{}-{}.json",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let output = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args([
            "run",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../forms/signal-demo/main.conduit"
            ),
            "--body",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../proof/fixtures/bodies/std-line.body.conduit"
            ),
            "--placements",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../proof/fixtures/placements/std-body-line.placements"
            ),
            "--report",
        ])
        .arg(&report)
        .output()
        .expect("installed conduit binary runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("Body Line complete values=16 pressure_retries=1"),
        "{stdout}"
    );
    let artifact = std::fs::read_to_string(&report).unwrap();
    assert!(artifact.contains("body:std-line/host/clock"));
    assert!(artifact.contains("body:std-line/host/serial"));
    assert!(artifact.contains("body-line/body:std-line/host/clock/body:std-line/host/serial"));
    std::fs::remove_file(report).unwrap();
}
