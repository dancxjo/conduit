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
fn transmitter_bearing_flash_requires_attachment_qualification() {
    assert!(validate_attachment_requirement(true, None).is_err());
    assert!(validate_attachment_requirement(true, Some(Path::new("qualification.json"))).is_ok());
    assert!(validate_attachment_requirement(false, None).is_ok());
}

#[test]
fn upload_port_is_one_exact_kernel_acm_endpoint() {
    validate_upload_port(Path::new("/dev/ttyACM0")).unwrap();
    validate_upload_port(Path::new("/dev/ttyACM17")).unwrap();
    for refused in [
        "/dev/serial/by-id/device",
        "/dev/ttyUSB0",
        "/dev/ttyACM",
        "/tmp/ttyACM0",
        "/dev/ttyACMzero",
    ] {
        assert!(validate_upload_port(Path::new(refused)).is_err());
    }
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
    assert!(validate_sizes(1, MAX_SRAM_BYTES + 1).is_err());
    validate_sizes(MAX_FLASH_BYTES, MAX_SRAM_BYTES).unwrap();
}

#[test]
fn standalone_sources_are_refused_inside_the_arduino_sketch() {
    for source in ["test.c", "test.cc", "test.cpp", "test.cxx"] {
        assert!(source_replaces_arduino_entry(Path::new(source)));
    }
    for allowed in ["promicro_brainstem.ino", "protocol.h", "README.md"] {
        assert!(!source_replaces_arduino_entry(Path::new(allowed)));
    }
}
