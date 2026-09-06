use super::*;

fn gate() -> PhysicalGate {
    PhysicalGate {
        create_stopped: true,
        attended: true,
        wheels_clear: true,
    }
}

#[test]
fn flash_refuses_before_any_device_work_without_all_physical_gates() {
    let mut missing = gate();
    missing.wheels_clear = false;
    let error = validate_flash_request(
        Path::new("/dev/serial/by-id/usb-SparkFun_SparkFun_Pro_Micro-if00"),
        &"a".repeat(64),
        missing,
    )
    .unwrap_err();
    assert!(error.to_string().contains("--wheels-clear"));
}

#[test]
fn flash_refuses_alias_or_wrong_device_and_malformed_digest() {
    assert!(validate_flash_request(Path::new("/dev/ttyACM0"), &"a".repeat(64), gate()).is_err());
    assert!(validate_flash_request(
        Path::new("/dev/serial/by-id/usb-other"),
        &"a".repeat(64),
        gate()
    )
    .is_err());
    assert!(validate_flash_request(
        Path::new("/dev/serial/by-id/usb-SparkFun_SparkFun_Pro_Micro-if00"),
        "not-a-digest",
        gate()
    )
    .is_err());
}

#[test]
fn parses_exact_build_metrics_and_enforces_both_capacities() {
    let report = "Sketch uses 4,508 bytes (15%).\nGlobal variables use 376 bytes (14%).";
    assert_eq!(metric(report, "Sketch uses ", " bytes").unwrap(), 4508);
    assert_eq!(
        metric(report, "Global variables use ", " bytes").unwrap(),
        376
    );
    assert!(validate_sizes(MAX_FLASH_BYTES + 1, 1).is_err());
    assert!(validate_sizes(1, MAX_STATIC_SRAM_BYTES + 1).is_err());
    validate_sizes(MAX_FLASH_BYTES, MAX_STATIC_SRAM_BYTES).unwrap();
    assert_eq!(
        metric("Program:     304 bytes", "Program:", "bytes").unwrap(),
        304
    );
    assert_eq!(
        metric("Data:          1 bytes", "Data:", "bytes").unwrap(),
        1
    );
}

#[test]
fn child_output_wait_is_bounded() {
    let completed = Command::new("sh")
        .args(["-c", "exit 0"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    assert!(
        wait_for_child_output(completed, Duration::from_secs(1), "test")
            .unwrap()
            .status
            .success()
    );

    let stuck = Command::new("sleep")
        .arg("60")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let error = wait_for_child_output(stuck, Duration::from_millis(20), "test").unwrap_err();
    assert!(error.to_string().contains("timed out after 20 ms"));
}
