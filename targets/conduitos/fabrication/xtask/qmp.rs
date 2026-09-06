//! Bounded QMP transport shared by input, hotplug and display capture.
use super::ConduitosError;
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::Path,
    process::Child,
    thread,
    time::{Duration, Instant},
};
const MAXIMUM_MESSAGE_BYTES: usize = 16 * 1024;
const MAXIMUM_EVENTS: usize = 8;

pub(super) fn connect(
    socket: &Path,
    child: &mut Child,
) -> Result<(UnixStream, BufReader<UnixStream>), ConduitosError> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut qmp = loop {
        match UnixStream::connect(socket) {
            Ok(stream) => break stream,
            Err(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Err(error) => {
                return super::hid_qmp::stop(child, "qemu-qmp-unavailable", error.to_string())
            }
        }
    };
    qmp.set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| ConduitosError::refusal("qemu-qmp-failed", error.to_string()))?;
    let mut reader = BufReader::new(
        qmp.try_clone()
            .map_err(|error| ConduitosError::refusal("qemu-qmp-failed", error.to_string()))?,
    );
    let greeting = read_message(&mut reader, Instant::now() + Duration::from_secs(2))?;
    if !greeting
        .get("QMP")
        .is_some_and(serde_json::Value::is_object)
    {
        return Err(ConduitosError::refusal(
            "qemu-qmp-invalid-greeting",
            "missing structured QMP greeting",
        ));
    }
    request(
        &mut qmp,
        &mut reader,
        br#"{"execute":"qmp_capabilities"}"#,
        "capabilities",
    )?;
    Ok((qmp, reader))
}

pub(super) fn request(
    stream: &mut UnixStream,
    reader: &mut BufReader<UnixStream>,
    command: &[u8],
    id: &str,
) -> Result<(), ConduitosError> {
    request_value(stream, reader, command, id).map(|_| ())
}

pub(super) fn request_value(
    stream: &mut UnixStream,
    reader: &mut BufReader<UnixStream>,
    command: &[u8],
    id: &str,
) -> Result<serde_json::Value, ConduitosError> {
    let mut command: serde_json::Value = serde_json::from_slice(command)
        .map_err(|error| ConduitosError::refusal("qemu-qmp-invalid-command", error.to_string()))?;
    let object = command
        .as_object_mut()
        .ok_or_else(|| ConduitosError::refusal("qemu-qmp-invalid-command", "expected object"))?;
    object.insert("id".into(), id.into());
    let mut encoded = serde_json::to_vec(&command)
        .map_err(|error| ConduitosError::refusal("qemu-qmp-invalid-command", error.to_string()))?;
    if encoded.len() > MAXIMUM_MESSAGE_BYTES {
        return Err(ConduitosError::refusal(
            "qemu-qmp-message-bound",
            "command too large",
        ));
    }
    encoded.push(b'\n');
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .and_then(|()| stream.write_all(&encoded))
        .map_err(|error| ConduitosError::refusal("qemu-qmp-write-failed", error.to_string()))?;
    let deadline = Instant::now() + Duration::from_secs(2);
    for _ in 0..=MAXIMUM_EVENTS {
        let response = read_message(reader, deadline)?;
        if response.get("event").is_some() {
            continue;
        }
        if response.get("id").and_then(serde_json::Value::as_str) != Some(id) {
            return Err(ConduitosError::refusal(
                "qemu-qmp-response-id",
                "response does not match pending command",
            ));
        }
        if response.get("error").is_some() {
            return Err(ConduitosError::refusal(
                "qemu-qmp-response-error",
                response.to_string(),
            ));
        }
        if response.get("return").is_some() {
            return Ok(response["return"].clone());
        }
        return Err(ConduitosError::refusal(
            "qemu-qmp-malformed-response",
            "missing return or error",
        ));
    }
    Err(ConduitosError::refusal(
        "qemu-qmp-event-bound",
        "too many unrelated events",
    ))
}

fn read_message(
    reader: &mut BufReader<UnixStream>,
    deadline: Instant,
) -> Result<serde_json::Value, ConduitosError> {
    let mut bytes = Vec::new();
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|duration| !duration.is_zero())
            .ok_or_else(|| {
                ConduitosError::refusal("qemu-qmp-timeout", "response deadline expired")
            })?;
        reader
            .get_ref()
            .set_read_timeout(Some(remaining))
            .map_err(|error| ConduitosError::refusal("qemu-qmp-read-failed", error.to_string()))?;
        let available = reader.fill_buf().map_err(|error| {
            ConduitosError::refusal(
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) {
                    "qemu-qmp-timeout"
                } else {
                    "qemu-qmp-read-failed"
                },
                error.to_string(),
            )
        })?;
        if available.is_empty() {
            return Err(ConduitosError::refusal(
                "qemu-qmp-closed",
                "QMP closed before response",
            ));
        }
        let end = available.iter().position(|byte| *byte == b'\n');
        let count = end.map_or(available.len(), |index| index + 1);
        if bytes.len() + count > MAXIMUM_MESSAGE_BYTES {
            return Err(ConduitosError::refusal(
                "qemu-qmp-message-bound",
                "response too large",
            ));
        }
        bytes.extend_from_slice(&available[..count]);
        reader.consume(count);
        if end.is_some() {
            break;
        }
    }
    serde_json::from_slice(&bytes)
        .map_err(|error| ConduitosError::refusal("qemu-qmp-malformed-response", error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(response: &[u8]) -> Result<(), ConduitosError> {
        let (mut client, mut server) = UnixStream::pair().unwrap();
        let response = response.to_vec();
        let worker = thread::spawn(move || {
            let mut reader = BufReader::new(server.try_clone().unwrap());
            let mut command = String::new();
            reader.read_line(&mut command).unwrap();
            let command: serde_json::Value = serde_json::from_str(&command).unwrap();
            assert_eq!(command["id"], "proof");
            server.write_all(&response).unwrap();
        });
        let mut reader = BufReader::new(client.try_clone().unwrap());
        let result = request(
            &mut client,
            &mut reader,
            br#"{"execute":"query-status"}"#,
            "proof",
        );
        worker.join().unwrap();
        result
    }

    #[test]
    fn interleaved_events_do_not_replace_matching_response() {
        reply(b"{\"event\":\"STOP\"}\n{\"return\":{},\"id\":\"proof\"}\n").unwrap();
    }

    #[test]
    fn stale_response_and_command_error_refuse() {
        assert!(reply(b"{\"return\":{},\"id\":\"old\"}\n").is_err());
        assert!(reply(b"{\"error\":{\"class\":\"GenericError\"},\"id\":\"proof\"}\n").is_err());
        assert!(reply(b"{\"id\":\"proof\"}\n").is_err());
    }

    #[test]
    fn event_budget_and_message_size_are_finite() {
        let events = b"{\"event\":\"STOP\"}\n".repeat(MAXIMUM_EVENTS + 1);
        let (mut sender, stream) = UnixStream::pair().unwrap();
        sender
            .write_all(&vec![b'x'; MAXIMUM_MESSAGE_BYTES + 1])
            .unwrap();
        assert!(read_message(
            &mut BufReader::new(stream),
            Instant::now() + Duration::from_secs(1)
        )
        .is_err());
        // The parser accepts events as JSON; request owns the independent event-count bound.
        assert!(reply(&events).is_err());
    }
}
