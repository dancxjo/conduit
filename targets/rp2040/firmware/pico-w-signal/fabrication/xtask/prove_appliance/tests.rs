use super::*;
use std::collections::VecDeque;
use std::io::{BufReader, Cursor};

#[test]
fn physical_sign_contract_is_exact_and_bounded() {
    assert_eq!(EXPECTED_SIGNS.len(), 8);
    assert!(
        EXPECTED_SIGNS.len()
            <= conduit_rp2040_network_realization::MAXIMUM_APPLIANCE_SIGNS as usize
    );
    assert_eq!(EXPECTED_SIGNS[0], "ap-ready");
    assert_eq!(EXPECTED_SIGNS[7], "terminal");
    assert!(!EXPECTED_SIGNS.contains(&"success"));
}

#[test]
fn lease_parser_accepts_only_reviewed_pool() {
    assert_eq!(parse_ipv4("192.168.4.2").unwrap(), [192, 168, 4, 2]);
    assert!(parse_ipv4("192.168.4").is_err());
}

#[test]
fn physical_proof_refuses_an_active_wifi_connection() {
    assert!(require_dedicated_wifi_interface("wlan-proof", None).is_ok());
    let error = require_dedicated_wifi_interface("wlo1", Some("remote-access"))
        .unwrap_err()
        .to_string();
    assert!(error.contains("refusing to move active Wi-Fi interface `wlo1`"));
    assert!(error.contains("disconnected dedicated client interface"));
}

fn transcript(mut mutate: impl FnMut(usize, &mut serde_json::Value)) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (index, kind) in EXPECTED_SIGNS.into_iter().enumerate() {
        let mut sign = serde_json::json!({
            "schema": "conduit.pico-appliance/sign@1",
            "firmware_build_id": "build/1",
            "profile": conduit_rp2040_network_realization::PICO_APPLIANCE_PROFILE,
            "host_id": "pico/appliance-hello",
            "runtime_boot_id": "runtime/boot/1",
            "sequence": index + 1,
            "sign_id": format!("pico/appliance/sign:runtime/boot/1:{:02}", index + 1),
            "kind": kind,
        });
        if matches!(index, 1 | 2) {
            sign["address"] = "192.168.4.2".into();
        }
        mutate(index, &mut sign);
        bytes.extend_from_slice(serde_json::to_string(&sign).unwrap().as_bytes());
        bytes.push(b'\n');
    }
    bytes
}

#[test]
fn transcript_requires_exact_identity_order_lease_and_terminal() {
    let valid = transcript(|_, _| {});
    let (boot, signs) = verify_signs(Cursor::new(valid), "build/1", Some("192.168.4.2")).unwrap();
    assert_eq!(boot, "runtime/boot/1");
    assert_eq!(signs.len(), EXPECTED_SIGNS.len());

    let wrong_kind = transcript(|index, sign| {
        if index == 4 {
            sign["kind"] = "http-response".into();
        }
    });
    assert!(verify_signs(Cursor::new(wrong_kind), "build/1", Some("192.168.4.2")).is_err());

    let stale_build = transcript(|index, sign| {
        if index == 6 {
            sign["firmware_build_id"] = "stale".into();
        }
    });
    assert!(verify_signs(Cursor::new(stale_build), "build/1", Some("192.168.4.2")).is_err());

    let wrong_lease = transcript(|_, _| {});
    assert!(verify_signs(Cursor::new(wrong_lease), "build/1", Some("192.168.4.3")).is_err());
}

struct TimeoutSplitReader {
    chunks: VecDeque<Vec<u8>>,
}

impl Read for TimeoutSplitReader {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        let Some(mut chunk) = self.chunks.pop_front() else {
            return Ok(0);
        };
        let count = output.len().min(chunk.len());
        output[..count].copy_from_slice(&chunk[..count]);
        if count < chunk.len() {
            chunk.drain(..count);
            self.chunks.push_front(chunk);
        }
        Ok(count)
    }
}

#[test]
fn serial_timeout_does_not_promote_a_partial_json_fragment() {
    let mut reader = BufReader::new(TimeoutSplitReader {
        chunks: VecDeque::from([
            br#"{"schema":"conduit""#.to_vec(),
            Vec::new(),
            b"}\n".to_vec(),
        ]),
    });
    let mut line = String::new();
    assert!(!read_complete_bounded_line(&mut reader, &mut line, "test").unwrap());
    assert_eq!(line, r#"{"schema":"conduit""#);
    assert!(read_complete_bounded_line(&mut reader, &mut line, "test").unwrap());
    assert_eq!(line, "{\"schema\":\"conduit\"}\n");
}
