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
                "/../../examples/signal-demo.conduit"
            ),
            "--execution-fixture",
            "two-std-line",
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
        stdout.contains("two-std Line complete values=16 pressure_retries=1"),
        "{stdout}"
    );
    let artifact = std::fs::read_to_string(&report).unwrap();
    assert!(artifact.contains("product/std-source"));
    assert!(artifact.contains("product/std-sink"));
    assert!(artifact.contains("product/two-std/websocket-line"));
    std::fs::remove_file(report).unwrap();
}
