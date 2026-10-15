//! Bounded QMP input actions synchronized to complete ordinary-product records.

use std::{fs, os::unix::net::UnixStream, path::Path, process::Child, thread, time::Duration};

use serde_json::Value;

use super::{hid_qmp, journey_records, qmp, ConduitosError};

// The exhaustive x86 gate intentionally runs up to four proof VMs together.
// Keep every observation finite, but allow a runnable guest one baseline
// five-second window per contending VM before declaring its exact record absent.
const RECORD_WAIT_TIMEOUT: Duration = Duration::from_secs(20);

pub(super) fn key_pair(
    qmp: &mut UnixStream,
    reader: &mut qmp::Reader,
    key: &str,
    label: &'static str,
) -> Result<(), ConduitosError> {
    hid_qmp::send_named_keys(qmp, reader, &[key], true, label)?;
    hid_qmp::send_named_keys(qmp, reader, &[key], false, label)
}

/// QMP acceptance acknowledges the host command, not guest consumption.
/// Hold the planning key until its complete semantic record is observed;
/// release is still attempted on a refused observation, without replay.
/// The journey owner retains a failed guest for diagnostics and then reaps it.
pub(super) fn key_until_status(
    qmp: &mut UnixStream,
    reader: &mut qmp::Reader,
    serial: &Path,
    child: &mut Child,
    key: &str,
    status: &str,
) -> Result<(), ConduitosError> {
    held_key(
        |down| hid_qmp::send_named_keys(qmp, reader, &[key], down, "journey-lifecycle"),
        || wait_status_retaining_guest(serial, child, status),
    )
}
fn held_key(
    mut send: impl FnMut(bool) -> Result<(), ConduitosError>,
    observe: impl FnOnce() -> Result<(), ConduitosError>,
) -> Result<(), ConduitosError> {
    send(true)?;
    let observed = observe();
    let released = send(false);
    observed.and(released)
}

/// Birth the selected zero-Body Crèche plan through the ordinary keyboard Face.
/// Keep the separate journey boots on the same authored interaction sequence.
pub(super) fn birth_from_creche(
    qmp: &mut UnixStream,
    reader: &mut qmp::Reader,
) -> Result<(), ConduitosError> {
    // The initial Face focuses the name argument. Three reverse controls in
    // the current semantic Face lead to Birth without traversing plot choices.
    for _ in 0..3 {
        hid_qmp::send_named_keys(qmp, reader, &["shift", "tab"], true, "creche-focus-birth")?;
        hid_qmp::send_named_keys(qmp, reader, &["shift", "tab"], false, "creche-focus-birth")?;
    }
    key_pair(qmp, reader, "ret", "creche-birth")
}

pub(super) fn relative_motion(
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    x: i64,
    y: i64,
    label: &'static str,
) -> Result<(), ConduitosError> {
    if !(-127..=127).contains(&x) || !(-127..=127).contains(&y) {
        return Err(ConduitosError::refusal(
            "product-journey-pointer-motion-bound",
            "relative motion must fit one boot-pointer report",
        ));
    }
    let command = serde_json::json!({"execute":"input-send-event","arguments":{"events":[
        {"type":"rel","data":{"axis":"x","value":x}},
        {"type":"rel","data":{"axis":"y","value":y}}
    ]}});
    qmp::request(
        stream,
        reader,
        &serde_json::to_vec(&command).expect("QMP motion is serializable"),
        label,
    )
}

pub(super) fn primary_button(
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    down: bool,
    label: &'static str,
) -> Result<(), ConduitosError> {
    let command = serde_json::json!({"execute":"input-send-event","arguments":{"events":[
        {"type":"btn","data":{"down":down,"button":"left"}}
    ]}});
    qmp::request(
        stream,
        reader,
        &serde_json::to_vec(&command).expect("QMP button is serializable"),
        label,
    )
}

pub(super) fn wait_status(
    serial: &Path,
    child: &mut Child,
    status: &str,
) -> Result<(), ConduitosError> {
    wait_status_inner(serial, child, status, true)
}

/// Leave the guest available for the journey owner’s failure capture and cleanup.
pub(super) fn wait_status_retaining_guest(
    serial: &Path,
    child: &mut Child,
    status: &str,
) -> Result<(), ConduitosError> {
    wait_status_inner(serial, child, status, false)
}

fn wait_status_inner(
    serial: &Path,
    child: &mut Child,
    status: &str,
    stop_on_timeout: bool,
) -> Result<(), ConduitosError> {
    wait_for_record_inner(
        serial,
        child,
        "product-journey-stage-timeout",
        status,
        stop_on_timeout,
        |text| {
            journey_records::decode(text).map(|records| {
                records
                    .iter()
                    .any(|record| record.get("status").and_then(Value::as_str) == Some(status))
            })
        },
    )
}

pub(super) fn wait_for_record(
    serial: &Path,
    child: &mut Child,
    reason: &'static str,
    status: &str,
    observed: impl FnMut(&str) -> Result<bool, ConduitosError>,
) -> Result<(), ConduitosError> {
    wait_for_record_inner(serial, child, reason, status, true, observed)
}

fn wait_for_record_inner(
    serial: &Path,
    child: &mut Child,
    reason: &'static str,
    status: &str,
    stop_on_timeout: bool,
    mut observed: impl FnMut(&str) -> Result<bool, ConduitosError>,
) -> Result<(), ConduitosError> {
    let timeout = match status {
        "born-lulled" | "awake" | "planned" => hid_qmp::COLD_PREPARATION_WAIT_TIMEOUT,
        _ => RECORD_WAIT_TIMEOUT,
    };
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let bytes = fs::read(serial).map_err(|error| {
            ConduitosError::refusal("product-journey-serial-unavailable", error.to_string())
        })?;
        if observed(complete_records(&bytes)?)? {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            if !stop_on_timeout {
                return Err(ConduitosError::refusal(
                    reason,
                    format!("no complete guest record for {status}"),
                ));
            }
            return hid_qmp::stop(
                child,
                reason,
                format!("no complete guest record for {status}"),
            );
        }
        thread::sleep(Duration::from_millis(1));
    }
}

// The guest may be midway through a UTF-8 scalar or JSON record when a live
// file read ends. Only completed records can satisfy an observation. Keep
// strict UTF-8 validation for that prefix; never replace malformed bytes.
pub(super) fn complete_records(bytes: &[u8]) -> Result<&str, ConduitosError> {
    let end = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |index| index + 1);
    std::str::from_utf8(&bytes[..end]).map_err(|error| {
        ConduitosError::refusal("product-journey-serial-unavailable", error.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};

    #[test]
    fn live_utf8_records_are_observed_only_after_their_newline() {
        let record = "{\"friendly_name\":\"Lương Quốc Lâm\",\"probe\":\"é中🦀\"}\n";
        let mut bytes = b"READY\n".to_vec();
        for byte in record.as_bytes() {
            assert_eq!(complete_records(&bytes).unwrap(), "READY\n");
            bytes.push(*byte);
        }
        assert_eq!(
            complete_records(&bytes).unwrap(),
            format!("READY\n{record}")
        );
        assert!(std::str::from_utf8(&"Lương".as_bytes()[..2]).is_err());
        assert_eq!(complete_records(&"Lương".as_bytes()[..2]).unwrap(), "");
    }

    #[test]
    fn malformed_completed_records_still_refuse() {
        assert!(complete_records(b"READY\n\xff\n").is_err());
        assert_eq!(complete_records(b"READY\npartial").unwrap(), "READY\n");
        assert_eq!(complete_records(b"READY").unwrap(), "");
    }

    #[test]
    fn failed_journey_keeps_guest_available_until_owner_cleanup() {
        let serial = std::env::temp_dir().join(format!(
            "conduit-journey-timeout-test-{}.log",
            std::process::id()
        ));
        fs::write(&serial, b"").unwrap();
        let mut guest = std::process::Command::new("sleep")
            .arg("60")
            .spawn()
            .unwrap();
        let result = wait_status_retaining_guest(&serial, &mut guest, "stopped");
        let guest_state = guest.try_wait();
        let _ = guest.kill();
        let reaped = guest.wait();
        let _ = fs::remove_file(serial);
        assert_eq!(result.unwrap_err().reason, "product-journey-stage-timeout");
        assert!(guest_state.unwrap().is_none());
        assert!(reaped.is_ok());
    }

    fn acknowledged(action: impl FnOnce(&mut UnixStream, &mut qmp::Reader)) -> Value {
        let (mut client, mut server) = UnixStream::pair().unwrap();
        let worker = std::thread::spawn(move || {
            let mut line = String::new();
            BufReader::new(server.try_clone().unwrap())
                .read_line(&mut line)
                .unwrap();
            let command: Value = serde_json::from_str(&line).unwrap();
            writeln!(
                server,
                "{}",
                serde_json::json!({"return":{},"id":command["id"]})
            )
            .unwrap();
            command
        });
        let mut reader = qmp::Reader::new(client.try_clone().unwrap());
        action(&mut client, &mut reader);
        worker.join().unwrap()
    }

    #[test]
    fn relative_motion_and_button_are_typed_and_acknowledged() {
        let motion = acknowledged(|stream, reader| {
            relative_motion(stream, reader, -7, 11, "motion").unwrap()
        });
        assert_eq!(motion["id"], "0:motion");
        assert_eq!(motion["arguments"]["events"][0]["type"], "rel");
        assert_eq!(motion["arguments"]["events"][0]["data"]["value"], -7);
        let (mut invalid_stream, _) = UnixStream::pair().unwrap();
        let (reader_stream, _) = UnixStream::pair().unwrap();
        let mut invalid_reader = qmp::Reader::new(reader_stream);
        assert_eq!(
            relative_motion(&mut invalid_stream, &mut invalid_reader, 128, 0, "overflow",)
                .unwrap_err()
                .reason,
            "product-journey-pointer-motion-bound"
        );
        let button =
            acknowledged(|stream, reader| primary_button(stream, reader, true, "press").unwrap());
        assert_eq!(button["id"], "0:press");
        assert_eq!(button["arguments"]["events"][0]["data"]["button"], "left");
        assert_eq!(button["arguments"]["events"][0]["data"]["down"], true);
    }
    #[test]
    fn planning_release_follows_guest_record_not_qmp_acceptance() {
        let events = std::cell::RefCell::new(Vec::new());
        held_key(
            |down| {
                events.borrow_mut().push(if down { "down" } else { "up" });
                Ok(())
            },
            || {
                assert_eq!(*events.borrow(), ["down"]);
                events.borrow_mut().push("guest-planned");
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(*events.borrow(), ["down", "guest-planned", "up"]);
    }
    #[test]
    fn refused_guest_record_releases_once_and_preserves_original_failure() {
        let events = std::cell::RefCell::new(Vec::new());
        let error = held_key(
            |down| {
                events.borrow_mut().push(down);
                if down {
                    Ok(())
                } else {
                    Err(ConduitosError::refusal("release-failed", "QMP stopped"))
                }
            },
            || {
                Err(ConduitosError::refusal(
                    "product-journey-stage-timeout",
                    "no planned record",
                ))
            },
        )
        .unwrap_err();
        assert_eq!(error.reason, "product-journey-stage-timeout");
        assert_eq!(*events.borrow(), [true, false]);
    }
    #[test]
    fn failed_press_never_waits_for_an_unissued_action() {
        let observed = std::cell::Cell::new(false);
        let error = held_key(
            |_| Err(ConduitosError::refusal("press-failed", "QMP unavailable")),
            || {
                observed.set(true);
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(error.reason, "press-failed");
        assert!(!observed.get());
    }
}
