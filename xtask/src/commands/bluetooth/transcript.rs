use std::{
    io::{ErrorKind, Read},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::SyncSender,
        Arc,
    },
    time::{Duration, Instant},
};

const MAXIMUM_TRANSCRIPT_LINES: usize = 256;
const MAXIMUM_TRANSCRIPT_LINE_BYTES: usize = 2_048;

pub(super) fn capture(
    mut file: std::fs::File,
    terminal_marker: &'static str,
    stop: Arc<AtomicBool>,
    boot_sender: Option<SyncSender<String>>,
) -> Result<Vec<String>, String> {
    let deadline = Instant::now() + Duration::from_secs(180);
    let mut lines = Vec::with_capacity(MAXIMUM_TRANSCRIPT_LINES);
    let mut line = Vec::with_capacity(MAXIMUM_TRANSCRIPT_LINE_BYTES);
    let mut input = [0_u8; 256];
    let mut boot_sender = boot_sender;
    loop {
        match file.read(&mut input) {
            Ok(0) => {}
            Ok(count) => {
                for byte in &input[..count] {
                    if *byte == b'\n' {
                        let text = std::str::from_utf8(&line)
                            .map_err(|_| "serial transcript contained non-UTF-8 bytes")?
                            .trim();
                        if !text.is_empty() {
                            if lines.len() == MAXIMUM_TRANSCRIPT_LINES {
                                return Err(
                                    "serial transcript exceeded the admitted line count".into()
                                );
                            }
                            let complete = text.starts_with(terminal_marker);
                            if let Some(sender) = boot_sender.as_ref() {
                                if let Some(boot) = runtime_boot_id(text) {
                                    let _ = sender.send(boot);
                                    boot_sender = None;
                                }
                            }
                            lines.push(text.to_owned());
                            if complete {
                                return Ok(lines);
                            }
                        }
                        line.clear();
                    } else if line.len() == MAXIMUM_TRANSCRIPT_LINE_BYTES {
                        return Err(
                            "serial transcript line exceeded the admitted byte count".into()
                        );
                    } else {
                        line.push(*byte);
                    }
                }
            }
            Err(error) if matches!(error.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) => {}
            Err(error) => return Err(format!("serial capture failed: {error}")),
        }
        if Instant::now() >= deadline {
            return Err("timed out waiting for Bluetooth terminal transcript".into());
        }
        if stop.load(Ordering::Acquire) {
            return Ok(lines);
        }
    }
}

fn runtime_boot_id(line: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(line).ok()?["runtime_boot_id"]
        .as_str()
        .map(str::to_owned)
}

pub(super) fn verify_esp32(
    lines: &[String],
    loss: bool,
    expected_host: &str,
    expected_boot: &str,
    expected_address: &str,
) -> Result<(), String> {
    require_matching_identity(
        lines,
        "CONDUIT_ESP32_BOOT ",
        expected_host,
        expected_boot,
        None,
    )?;
    require_matching_identity(
        lines,
        "CONDUIT_ESP32_HOST ",
        expected_host,
        expected_boot,
        Some(expected_address),
    )?;

    let connected = unique_position(lines, "CONDUIT_ESP32_BLE_CONNECTED")?;
    let paired = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.starts_with("CONDUIT_ESP32_BLE_PAIRED "))
        .map(|(index, line)| {
            if field(line, "security") != Some("Encrypted") {
                Err("ESP32 pairing receipt did not prove encrypted security".into())
            } else {
                Ok(index)
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    if paired.len() != 1 || paired[0] <= connected {
        return Err("expected one encrypted ESP32 pairing after connection".into());
    }
    let paired = paired[0];

    if loss {
        if lines.iter().any(|line| {
            line.starts_with("CONDUIT_ESP32_PRESENT ")
                || line.starts_with("CONDUIT_ESP32_LINE_COMPLETE ")
        }) {
            return Err("transport-loss proof contained completed ESP32 traffic".into());
        }
        let lost = unique_position(lines, "CONDUIT_ESP32_BLE_LOST")?;
        return if lost > paired {
            Ok(())
        } else {
            Err("ESP32 transport loss preceded encrypted pairing".into())
        };
    }

    let presentations = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.starts_with("CONDUIT_ESP32_PRESENT "))
        .collect::<Vec<_>>();
    if presentations.len() != 16 {
        return Err(format!(
            "expected 16 exact ESP32 presentations, found {}",
            presentations.len()
        ));
    }
    let mut previous = paired;
    for (expected_sequence, (position, line)) in presentations.into_iter().enumerate() {
        let sequence = expected_sequence.to_string();
        let level = if expected_sequence % 2 == 0 {
            "false"
        } else {
            "true"
        };
        if position <= previous
            || field(line, "sequence") != Some(sequence.as_str())
            || field(line, "level") != Some(level)
        {
            return Err(format!(
                "ESP32 presentation {expected_sequence} was missing, duplicated, or out of order"
            ));
        }
        previous = position;
    }
    let complete = unique_position(lines, "CONDUIT_ESP32_LINE_COMPLETE final-sequence=16")?;
    if complete <= previous {
        return Err("ESP32 completion preceded its final presentation".into());
    }
    if lines.iter().any(|line| line == "CONDUIT_ESP32_BLE_LOST") {
        return Err("completion proof also claimed transport loss".into());
    }
    Ok(())
}

fn require_matching_identity(
    lines: &[String],
    prefix: &str,
    expected_host: &str,
    expected_boot: &str,
    expected_address: Option<&str>,
) -> Result<(), String> {
    let identities = lines
        .iter()
        .filter(|line| line.starts_with(prefix))
        .collect::<Vec<_>>();
    if identities.is_empty() {
        return Err(format!("missing exact {} receipt", prefix.trim()));
    }
    for identity in identities {
        if field(identity, "host") != Some(expected_host)
            || field(identity, "boot") != Some(expected_boot)
            || expected_address.is_some_and(|address| field(identity, "address") != Some(address))
        {
            return Err(format!(
                "{} receipt disagreed with the selected peer",
                prefix.trim()
            ));
        }
    }
    Ok(())
}

fn unique_position(lines: &[String], exact: &str) -> Result<usize, String> {
    let positions = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| (line == exact).then_some(index))
        .collect::<Vec<_>>();
    if positions.len() == 1 {
        Ok(positions[0])
    } else {
        Err(format!("expected one exact `{exact}` marker"))
    }
}

fn field<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    line.split_ascii_whitespace().find_map(|token| {
        let (field, value) = token.split_once('=')?;
        (field == name).then_some(value)
    })
}

#[cfg(test)]
mod tests {
    use super::{runtime_boot_id, verify_esp32};

    const HOST: &str = "esp32/24dcc39a0a44";
    const BOOT: &str = "00112233445566778899aabbccddeeff";
    const ADDRESS: &str = "45:0A:9A:C3:DC:E6";

    fn completion() -> Vec<String> {
        let mut lines = vec![
            format!("CONDUIT_ESP32_BOOT schema=conduit.host/esp32-boot@1 host={HOST} boot={BOOT}"),
            format!("CONDUIT_ESP32_HOST schema=conduit.host/esp32-advertisement@1 host={HOST} boot={BOOT} address={ADDRESS}"),
            "CONDUIT_ESP32_BLE_CONNECTED".into(),
            "CONDUIT_ESP32_BLE_PAIRED security=Encrypted retained-boot-bond=true".into(),
        ];
        lines.extend((0..16).map(|sequence| {
            format!(
                "CONDUIT_ESP32_PRESENT sequence={sequence} level={}",
                sequence % 2 == 1
            )
        }));
        lines.push("CONDUIT_ESP32_LINE_COMPLETE final-sequence=16".into());
        lines
    }

    #[test]
    fn completion_requires_identity_encryption_exact_traffic_and_terminal() {
        let lines = completion();
        assert!(verify_esp32(&lines, false, HOST, BOOT, ADDRESS).is_ok());

        let mut wrong_level = lines.clone();
        *wrong_level
            .iter_mut()
            .find(|line| line.starts_with("CONDUIT_ESP32_PRESENT sequence=4 "))
            .unwrap() = "CONDUIT_ESP32_PRESENT sequence=4 level=true".into();
        assert!(verify_esp32(&wrong_level, false, HOST, BOOT, ADDRESS).is_err());

        assert!(verify_esp32(&lines, false, "esp32/wrong", BOOT, ADDRESS).is_err());
        assert!(verify_esp32(&lines, false, HOST, "wrong-boot", ADDRESS).is_err());
    }

    #[test]
    fn loss_is_distinct_and_follows_encrypted_pairing() {
        let lines = vec![
            format!("CONDUIT_ESP32_BOOT host={HOST} boot={BOOT}"),
            format!("CONDUIT_ESP32_HOST host={HOST} boot={BOOT} address={ADDRESS}"),
            "CONDUIT_ESP32_BLE_CONNECTED".into(),
            "CONDUIT_ESP32_BLE_PAIRED security=Encrypted retained-boot-bond=true".into(),
            "CONDUIT_ESP32_BLE_LOST".into(),
        ];
        assert!(verify_esp32(&lines, true, HOST, BOOT, ADDRESS).is_ok());

        let mut completed = lines;
        completed.insert(4, "CONDUIT_ESP32_PRESENT sequence=0 level=false".into());
        assert!(verify_esp32(&completed, true, HOST, BOOT, ADDRESS).is_err());
    }

    #[test]
    fn runtime_boot_identity_is_taken_only_from_a_structured_receipt() {
        assert_eq!(
            runtime_boot_id(r#"{"runtime_boot_id":"runtime/boot/1"}"#).as_deref(),
            Some("runtime/boot/1")
        );
        assert_eq!(runtime_boot_id("runtime_boot_id=ambient-text"), None);
    }
}
