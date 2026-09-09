//! Bounded QMP input actions synchronized to complete ordinary-product records.

use std::{fs, os::unix::net::UnixStream, path::Path, process::Child, thread, time::Duration};

use serde_json::Value;

use super::{hid_qmp, journey_records, qmp, ConduitosError};

pub(super) fn key_pair(
    qmp: &mut UnixStream,
    reader: &mut qmp::Reader,
    key: &str,
    label: &'static str,
) -> Result<(), ConduitosError> {
    hid_qmp::send_named_keys(qmp, reader, &[key], true, label)?;
    hid_qmp::send_named_keys(qmp, reader, &[key], false, label)
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
    wait_for_record(
        serial,
        child,
        "product-journey-stage-timeout",
        status,
        |text| {
            journey_records::decode(text).map(|records| {
                records
                    .iter()
                    .any(|record| record.get("status").and_then(Value::as_str) == Some(status))
            })
        },
    )
}

pub(super) fn wait_tour_status(
    serial: &Path,
    child: &mut Child,
    status: &str,
) -> Result<(), ConduitosError> {
    wait_for_record(
        serial,
        child,
        "product-journey-tour-stage-timeout",
        status,
        |text| {
            journey_records::tour(text).map(|records| {
                records
                    .iter()
                    .any(|record| record.get("status").and_then(Value::as_str) == Some(status))
            })
        },
    )
}

pub(super) fn wait_pointer_status(
    serial: &Path,
    child: &mut Child,
    status: &str,
) -> Result<(), ConduitosError> {
    wait_for_record(
        serial,
        child,
        "product-journey-pointer-stage-timeout",
        status,
        |text| {
            journey_records::pointer(text).map(|records| {
                records
                    .iter()
                    .any(|record| record.get("status").and_then(Value::as_str) == Some(status))
            })
        },
    )
}

pub(super) fn wait_transient_status(
    serial: &Path,
    child: &mut Child,
    status: &str,
) -> Result<(), ConduitosError> {
    wait_for_record(
        serial,
        child,
        "product-journey-transient-stage-timeout",
        status,
        |text| {
            journey_records::transient(text).map(|records| {
                records
                    .iter()
                    .any(|record| record.get("status").and_then(Value::as_str) == Some(status))
            })
        },
    )
}

pub(super) fn wait_pointer_status_count(
    serial: &Path,
    child: &mut Child,
    status: &str,
    count: usize,
) -> Result<(), ConduitosError> {
    wait_for_record(
        serial,
        child,
        "product-journey-pointer-stage-timeout",
        status,
        |text| {
            journey_records::pointer(text).map(|records| {
                records
                    .iter()
                    .filter(|record| record.get("status").and_then(Value::as_str) == Some(status))
                    .count()
                    >= count
            })
        },
    )
}

pub(super) fn wait_for_record(
    serial: &Path,
    child: &mut Child,
    reason: &'static str,
    status: &str,
    mut observed: impl FnMut(&str) -> Result<bool, ConduitosError>,
) -> Result<(), ConduitosError> {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let text = fs::read_to_string(serial).map_err(|error| {
            ConduitosError::refusal("product-journey-serial-unavailable", error.to_string())
        })?;
        if observed(&text)? {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return hid_qmp::stop(
                child,
                reason,
                format!("no complete guest record for {status}"),
            );
        }
        thread::sleep(Duration::from_millis(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};

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
}
