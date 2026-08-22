use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::time::{Duration, Instant};

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
        || identity["usb_serial"] != "nw-capstone"
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
            "usb_serial": "nw-capstone",
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
}
