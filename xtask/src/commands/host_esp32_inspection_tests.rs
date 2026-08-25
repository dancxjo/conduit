use std::path::Path;

use super::host_esp32_inspection::{parse_flash_facts, parse_rom_facts, run, Esp32SocClass};
use crate::cli::GlobalOpts;

const CHIP: &str = r#"
Connected to ESP32:
Chip type:          ESP32-D0WD-V3 (revision v3.1)
Features:           Wi-Fi, BT, Dual Core + LP Core, 240MHz, Vref calibration in eFuse, Coding Scheme None
Crystal frequency:  40MHz
MAC:                24:dc:c3:9a:0a:44
"#;

const FLASH: &str = r#"
Flash Memory Information:
Manufacturer: 5e
Device: 4016
Detected flash size: 4MB
Flash voltage set by a strapping pin: 3.3V
"#;

const S3_FLASH: &str = r#"
Flash Memory Information:
Manufacturer: 5e
Device: 4018
Detected flash size: 16MB
Flash type set in eFuse: quad (4 data lines)
Flash voltage set by eFuse: 3.3V
"#;

const C3_EMBEDDED_FLASH: &str = r#"
Flash Memory Information:
Manufacturer: 20
Device: 4016
Detected flash size: 4MB
"#;

const C3_CHIP: &str = r#"
Connected to ESP32-C3:
Chip type:          ESP32-C3 (QFN32) (revision v0.4)
Features:           Wi-Fi, BLE, Single Core, 160MHz
Crystal frequency:  40MHz
MAC:                84:f7:03:00:00:01
"#;

const S3_CHIP: &str = r#"
Connected to ESP32-S3:
Chip type:          ESP32-S3 (QFN56) (revision v0.2)
Features:           Wi-Fi, BLE, Dual Core, 240MHz
Crystal frequency:  40MHz
MAC:                7c:df:a1:00:00:02
"#;

#[test]
fn exact_classic_rom_and_flash_transcripts_parse() {
    let rom = parse_rom_facts(CHIP, Esp32SocClass::ClassicEsp32).unwrap();
    assert_eq!(rom.chip, "ESP32-D0WD-V3");
    assert_eq!(rom.revision, "v3.1");
    assert_eq!(rom.crystal_mhz, 40);
    assert_eq!(rom.mac, "24:dc:c3:9a:0a:44");
    let flash = parse_flash_facts(FLASH).unwrap();
    assert_eq!(flash.manufacturer_id, "0x5e");
    assert_eq!(flash.device_id, "0x4016");
    assert_eq!(flash.detected_bytes, 4 * 1024 * 1024);
    assert_eq!(flash.voltage.as_deref(), Some("3.3V"));
}

#[test]
fn exact_s3_efuse_flash_transcript_parses_without_classic_wording() {
    let flash = parse_flash_facts(S3_FLASH).unwrap();
    assert_eq!(flash.manufacturer_id, "0x5e");
    assert_eq!(flash.device_id, "0x4018");
    assert_eq!(flash.detected_bytes, 16 * 1024 * 1024);
    assert_eq!(flash.voltage.as_deref(), Some("3.3V"));
}

#[test]
fn exact_c3_embedded_flash_preserves_absent_voltage() {
    let flash = parse_flash_facts(C3_EMBEDDED_FLASH).unwrap();
    assert_eq!(flash.manufacturer_id, "0x20");
    assert_eq!(flash.device_id, "0x4016");
    assert_eq!(flash.detected_bytes, 4 * 1024 * 1024);
    assert_eq!(flash.voltage, None);
}

#[test]
fn expected_soc_class_accepts_only_its_observed_soc_family() {
    let c3 = parse_rom_facts(C3_CHIP, Esp32SocClass::Esp32C3).unwrap();
    assert_eq!(c3.chip, "ESP32-C3 (QFN32)");
    let s3 = parse_rom_facts(S3_CHIP, Esp32SocClass::Esp32S3).unwrap();
    assert_eq!(s3.chip, "ESP32-S3 (QFN56)");

    for (source, requested) in [
        (CHIP, Esp32SocClass::Esp32C3),
        (C3_CHIP, Esp32SocClass::ClassicEsp32),
        (C3_CHIP, Esp32SocClass::Esp32S3),
        (S3_CHIP, Esp32SocClass::Esp32C3),
    ] {
        let error = parse_rom_facts(source, requested).unwrap_err().to_string();
        assert!(error.contains("SoC-class mismatch"));
    }
}

#[test]
fn malformed_or_different_hardware_refuses_before_evidence() {
    for hostile in [
        CHIP.replace("24:dc:c3:9a:0a:44", "not-a-mac"),
        CHIP.replace("40MHz", "unknown"),
    ] {
        assert!(parse_rom_facts(&hostile, Esp32SocClass::ClassicEsp32).is_err());
    }
    for hostile in [
        FLASH.replace("4MB", "4GB"),
        FLASH.replace("Manufacturer: 5e", "Manufacturer: nope"),
        FLASH.replace("Device: 4016", "Device:"),
        format!("{FLASH}\nFlash voltage set by eFuse: 3.3V"),
    ] {
        assert!(parse_flash_facts(&hostile).is_err());
    }
}

#[test]
fn dry_run_performs_no_device_probe_and_live_mode_refuses_absence() {
    let missing = Path::new("/conduit-test-device-that-does-not-exist");
    let output = Path::new("/conduit-test-output-that-must-not-exist");
    let dry = GlobalOpts {
        dry_run: true,
        quiet: true,
        json: false,
        locked: false,
    };
    run(
        missing,
        Esp32SocClass::ClassicEsp32,
        "fixture-board",
        "fixture-module",
        "unmarked",
        output,
        &dry,
    )
    .unwrap();
    assert!(!output.exists());

    let live = GlobalOpts::default();
    let error = run(
        missing,
        Esp32SocClass::ClassicEsp32,
        "fixture-board",
        "fixture-module",
        "unmarked",
        output,
        &live,
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("serial path does not exist"));

    assert!(run(
        missing,
        Esp32SocClass::ClassicEsp32,
        "",
        "fixture",
        "unmarked",
        output,
        &dry
    )
    .is_err());
}
