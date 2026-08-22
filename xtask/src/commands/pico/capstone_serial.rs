use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use conduit_std_host::usb_cdc::NativePathCdcLine;

use super::doctor::repo_root;
use super::firmware::identity_manifest_path;
use super::{PicoArgs, PicoResult};

pub(super) fn verify(args: &PicoArgs) -> PicoResult<()> {
    let port = resolve_port(args.port.as_deref())?;
    println!(
        "==> pico verify: reading Pete capstone qualification from {}",
        port.display()
    );
    let file = std::fs::OpenOptions::new().read(true).open(&port)?;
    conduit_std_host::usb_cdc::configure_cdc_port(&file, 0, 50)?;
    let mut reader = BufReader::new(file);
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut records = Vec::new();
    while Instant::now() < deadline
        && !records
            .iter()
            .any(|record: &serde_json::Value| record["schema"] == "conduit.pete/capstone-ready@1")
    {
        let mut line = String::new();
        if reader.read_line(&mut line)? > 0 {
            let record = serde_json::from_str::<serde_json::Value>(line.trim())?;
            print!("{line}");
            records.push(record);
        }
    }
    let identity: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        identity_manifest_path(&repo_root()),
    )?)?;
    validate_records(&records, &identity)?;
    drop(reader);
    let expected_build = identity["firmware_build_id"]
        .as_str()
        .ok_or("capstone image identity missing firmware_build_id")?;
    let diagnostics = collect_uart_diagnostics(&port, expected_build, 8)?;
    validate_diagnostics(&diagnostics, expected_build)?;
    println!("==> pico verify: Pete capstone qualification complete");
    Ok(())
}

fn validate_records(records: &[serde_json::Value], identity: &serde_json::Value) -> PicoResult<()> {
    let expected_schemas = [
        "conduit.pete/capstone-boot@1",
        "conduit.pete/capstone-disposition@1",
        "conduit.pete/imu-probe@1",
        "conduit.pete/capstone-ready@1",
    ];
    if records.len() != expected_schemas.len() {
        return Err(format!(
            "capstone qualification requires exactly {} records, got {}",
            expected_schemas.len(),
            records.len()
        )
        .into());
    }
    for (record, expected) in records.iter().zip(expected_schemas) {
        if record["schema"].as_str() != Some(expected) {
            return Err(format!(
                "expected capstone record {expected}, got {}",
                record["schema"]
            )
            .into());
        }
    }

    if identity["schema"] != "conduit.pete/capstone-image@1"
        || identity["firmware_mode"] != "pete-capstone"
        || identity["usb_serial"] != "pete-capstone"
        || identity["robot_control_capable"] != true
        || identity["form"] != "pete-capstone"
        || identity["kernel"] != "conduit-kernel"
        || identity["oi_exposed"] != false
    {
        return Err("built capstone identity does not describe the physical Play image".into());
    }
    let expected_build = identity["firmware_build_id"]
        .as_str()
        .ok_or("capstone image identity missing firmware_build_id; rebuild the image")?;
    if records[0]["build_id"].as_str() != Some(expected_build) {
        return Err("running capstone firmware does not match the current built image".into());
    }
    let disposition = &records[1];
    if disposition["translator_oe"] != "high"
        || disposition["power_toggle"] != "low"
        || disposition["create_uart"] != "supervised_57600_8n1"
        || disposition["watchdog"]["timeout_ms"] != 2_000
        || disposition["watchdog"]["feed_interval_ms"] != 250
        || disposition["charging_indicator"]["gpio"] != 20
        || disposition["charging_indicator"]["active_high"] != true
        || !matches!(
            disposition["charging_indicator"]["level"].as_str(),
            Some("low" | "high")
        )
    {
        return Err("capstone disposition does not preserve the supervised carrier state".into());
    }
    let imu = &records[2];
    if imu["success"] != true || !matches!(imu["address"].as_u64(), Some(104 | 105)) {
        return Err("IMU probe did not report a supported physical MPU-6050".into());
    }
    let acceleration = imu["accel_mm_s2"]
        .as_array()
        .filter(|values| values.len() == 3)
        .ok_or("IMU probe missing three-axis acceleration")?;
    let magnitude_squared = acceleration.iter().try_fold(0_i64, |sum, value| {
        let axis = value
            .as_i64()
            .ok_or("IMU acceleration axis is not an integer")?;
        Ok::<i64, Box<dyn std::error::Error>>(sum.saturating_add(axis.saturating_mul(axis)))
    })?;
    if magnitude_squared < 1_000_i64.pow(2) {
        return Err("IMU acceleration is below the physical gravity-reference floor".into());
    }
    let terminal = &records[3];
    if terminal["qualification_complete"] != true
        || terminal["robot_control_ready"] != true
        || terminal["create_link_fresh"] != true
        || terminal["create_packets"]
            .as_u64()
            .is_none_or(|value| value == 0)
        || terminal["ready_cue_command_sent"] != true
        || terminal["form"] != "pete-capstone"
        || terminal["kernel"] != "conduit-kernel"
        || terminal["oi_exposed"] != false
    {
        return Err(
            "capstone ready receipt does not establish the intended control boundary".into(),
        );
    }
    Ok(())
}

fn collect_uart_diagnostics(
    port: &std::path::Path,
    expected_build: &str,
    trials: usize,
) -> PicoResult<Vec<serde_json::Value>> {
    let request = format!("CONDUIT_UART_DIAGNOSTIC@1:{expected_build}");
    let mut records = Vec::with_capacity(trials);
    for trial in 0..trials {
        let mut line = NativePathCdcLine::open(port, 1024)?;
        std::thread::sleep(Duration::from_millis(250));
        line.send_raw_stream_frame(request.as_bytes(), Duration::from_secs(2))?;
        let mut raw = [0_u8; 1024];
        let response = line.receive_raw_stream_frame(&mut raw, Duration::from_secs(2))?;
        let record: serde_json::Value = serde_json::from_slice(response)?;
        println!("{}", serde_json::to_string(&record)?);
        records.push(record);
        drop(line);
        if trial + 1 < trials {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    Ok(records)
}

fn validate_diagnostics(diagnostics: &[serde_json::Value], expected_build: &str) -> PicoResult<()> {
    if diagnostics.len() != 8 {
        return Err("UART diagnosis requires exactly eight separate CDC-open receipts".into());
    }
    let mut prior_counters: Option<[u64; 11]> = None;
    let mut expected_window_start = None;
    let mut prior_window_end = None;
    for diagnostic in diagnostics {
        if diagnostic["schema"] != "conduit.pete/uart-diagnostic@1" {
            return Err("UART diagnostic response has the wrong schema".into());
        }
        let window_start = diagnostic["window_start_ms"]
            .as_u64()
            .ok_or("UART diagnostic window start is missing")?;
        let window_end = diagnostic["window_end_ms"]
            .as_u64()
            .ok_or("UART diagnostic window end is missing")?;
        if expected_window_start
            .replace(window_start)
            .is_some_and(|start| start != window_start)
        {
            return Err("UART diagnostic window changed across CDC reopen".into());
        }
        if prior_window_end
            .replace(window_end)
            .is_some_and(|end| window_end < end)
        {
            return Err("UART diagnostic window end moved backward across CDC reopen".into());
        }
        if diagnostic["build_id"].as_str() != Some(expected_build)
            || window_end < window_start
            || diagnostic["oe_sequence"] != "low_during_uart_init_then_high_after_rx_pullup"
            || diagnostic["uart"]["controller"] != 0
            || diagnostic["uart"]["tx_gpio"] != 0
            || diagnostic["uart"]["rx_gpio"] != 1
            || diagnostic["uart"]["baud"] != 57_600
            || diagnostic["uart"]["data_bits"] != 8
            || diagnostic["uart"]["stop_bits"] != 1
            || diagnostic["uart"]["parity"] != "none"
        {
            return Err(
                "UART diagnostic receipt has the wrong build, profile, pins, or OE sequence".into(),
            );
        }
        let mut counters = [0_u64; 11];
        for (index, field) in [
            "rx_bytes",
            "tx_bytes",
            "valid_frames",
            "corrupt_frames",
            "resync_discarded_bytes",
            "timeouts",
        ]
        .into_iter()
        .enumerate()
        {
            let Some(value) = diagnostic[field].as_u64() else {
                return Err(
                    format!("UART diagnostic field {field} is not a finite counter").into(),
                );
            };
            counters[index] = value;
        }
        for (index, field) in ["overrun", "break", "parity", "framing", "other"]
            .into_iter()
            .enumerate()
        {
            let Some(value) = diagnostic["errors"][field].as_u64() else {
                return Err(
                    format!("UART diagnostic error field {field} is not a finite counter").into(),
                );
            };
            counters[6 + index] = value;
        }
        let first_byte_ms = diagnostic["first_byte_after_boot_ms"]
            .as_i64()
            .filter(|value| *value >= -1)
            .ok_or("UART diagnostic first-byte timing is not -1 or a nonnegative integer")?;
        if (counters[0] == 0) != (first_byte_ms == -1) {
            return Err("UART diagnostic first-byte timing disagrees with RX byte count".into());
        }
        let observed_len = diagnostic["last_corrupt_frame"]["observed_len"]
            .as_u64()
            .ok_or("UART diagnostic last-frame length is missing")?;
        let observed_hex = diagnostic["last_corrupt_frame"]["hex"]
            .as_str()
            .ok_or("UART diagnostic last-frame hex is missing")?;
        if observed_len > 30 || observed_hex.len() != observed_len as usize * 2 {
            return Err(
                "UART diagnostic last-frame sample exceeds its bound or has inconsistent hex"
                    .into(),
            );
        }
        if diagnostic["last_corrupt_frame"]["present"] != (counters[3] != 0) {
            return Err("UART diagnostic corrupt-frame presence disagrees with its counter".into());
        }
        if let Some(prior) = prior_counters {
            if counters
                .iter()
                .zip(prior)
                .any(|(current, prior)| *current < prior)
            {
                return Err("UART diagnostic counters moved backward across CDC reopen".into());
            }
        }
        prior_counters = Some(counters);
    }
    Ok(())
}

pub(super) fn resolve_port(explicit: Option<&str>) -> PicoResult<PathBuf> {
    let port = if let Some(port) = explicit {
        PathBuf::from(port)
    } else {
        let directory = std::fs::read_dir("/dev/serial/by-id")?;
        let candidates = directory
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .contains("Pico_W_Pete_Capstone")
            })
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        if candidates.len() != 1 {
            return Err(format!(
                "expected exactly one Pete capstone CDC port, found {}",
                candidates.len()
            )
            .into());
        }
        candidates[0].clone()
    };
    Ok(port)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> serde_json::Value {
        serde_json::json!({
            "schema": "conduit.pete/capstone-image@1",
            "firmware_build_id": "capstone-build",
            "firmware_mode": "pete-capstone",
            "usb_serial": "pete-capstone",
            "robot_control_capable": true,
            "form": "pete-capstone",
            "kernel": "conduit-kernel",
            "oi_exposed": false
        })
    }

    fn records(acceleration: [i64; 3]) -> Vec<serde_json::Value> {
        vec![
            serde_json::json!({
                "schema": "conduit.pete/capstone-boot@1",
                "build_id": "capstone-build"
            }),
            serde_json::json!({
                "schema": "conduit.pete/capstone-disposition@1",
                "translator_oe": "high",
                "power_toggle": "low",
                "create_uart": "supervised_57600_8n1",
                "charging_indicator": {"gpio": 20, "active_high": true, "level": "low"},
                "watchdog": {"timeout_ms": 2000, "feed_interval_ms": 250}
            }),
            serde_json::json!({
                "schema": "conduit.pete/imu-probe@1",
                "success": true,
                "address": 104,
                "accel_mm_s2": acceleration,
                "gyro_milliradians_s": [0, 0, 0]
            }),
            serde_json::json!({
                "schema": "conduit.pete/capstone-ready@1",
                "qualification_complete": true,
                "robot_control_ready": true,
                "create_link_fresh": true,
                "create_packets": 3,
                "ready_cue_command_sent": true,
                "form": "pete-capstone",
                "kernel": "conduit-kernel",
                "oi_exposed": false
            }),
        ]
    }

    fn diagnostic(rx_bytes: u64) -> serde_json::Value {
        serde_json::json!({
            "schema": "conduit.pete/uart-diagnostic@1",
            "build_id": "capstone-build",
            "window_start_ms": 10,
            "window_end_ms": 100,
            "oe_sequence": "low_during_uart_init_then_high_after_rx_pullup",
            "uart": {"controller": 0, "tx_gpio": 0, "rx_gpio": 1, "baud": 57600, "data_bits": 8, "stop_bits": 1, "parity": "none"},
            "rx_bytes": rx_bytes,
            "tx_bytes": 12,
            "valid_frames": 3,
            "corrupt_frames": 0,
            "resync_discarded_bytes": 0,
            "timeouts": 0,
            "errors": {"overrun": 0, "break": 0, "parity": 0, "framing": 0, "other": 0},
            "first_byte_after_boot_ms": 25,
            "last_corrupt_frame": {"present": false, "packet_id": 0, "observed_len": 0, "hex": ""}
        })
    }

    #[test]
    fn accepts_build_bound_physical_sample() {
        validate_records(&records([0, 0, 9_807]), &identity())
            .expect("valid capstone qualification");
    }

    #[test]
    fn rejects_all_zero_imu_sample() {
        assert!(validate_records(&records([0, 0, 0]), &identity()).is_err());
    }

    #[test]
    fn rejects_firmware_from_another_build() {
        let mut records = records([0, 0, 9_807]);
        records[0]["build_id"] = "older-build".into();
        assert!(validate_records(&records, &identity()).is_err());
    }

    #[test]
    fn accepts_eight_monotonic_reopen_receipts() {
        let diagnostics = (0..8).map(|step| diagnostic(90 + step)).collect::<Vec<_>>();
        validate_diagnostics(&diagnostics, "capstone-build").expect("bounded diagnostics");
    }

    #[test]
    fn rejects_counter_rollback_across_reopen() {
        let mut diagnostics = vec![diagnostic(90); 8];
        diagnostics[4]["rx_bytes"] = 89.into();
        assert!(validate_diagnostics(&diagnostics, "capstone-build").is_err());
    }

    #[test]
    fn rejects_unbounded_corrupt_frame_sample() {
        let mut diagnostics = vec![diagnostic(90); 8];
        diagnostics[0]["corrupt_frames"] = 1.into();
        diagnostics[0]["last_corrupt_frame"] = serde_json::json!({
            "present": true,
            "packet_id": 35,
            "observed_len": 31,
            "hex": "00".repeat(31)
        });
        assert!(validate_diagnostics(&diagnostics, "capstone-build").is_err());
    }

    #[test]
    fn rejects_window_end_rollback_across_reopen() {
        let mut diagnostics = vec![diagnostic(90); 8];
        diagnostics[4]["window_end_ms"] = 99.into();
        diagnostics[3]["window_end_ms"] = 100.into();
        assert!(validate_diagnostics(&diagnostics, "capstone-build").is_err());
    }

    #[test]
    fn rejects_first_byte_timing_that_disagrees_with_rx_count() {
        let mut diagnostics = vec![diagnostic(90); 8];
        diagnostics[0]["first_byte_after_boot_ms"] = (-1).into();
        assert!(validate_diagnostics(&diagnostics, "capstone-build").is_err());
    }
}
