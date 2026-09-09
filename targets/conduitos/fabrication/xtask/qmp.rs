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

pub(super) struct Reader {
    inner: BufReader<UnixStream>,
    transcript: Option<std::fs::File>,
    trace_bytes: usize,
    next_request: u16,
}
impl Reader {
    pub(super) fn new(stream: UnixStream) -> Self {
        Self {
            inner: BufReader::new(stream),
            transcript: None,
            trace_bytes: 0,
            next_request: 0,
        }
    }
    fn record(&mut self, direction: &str, bytes: &[u8]) -> Result<(), ConduitosError> {
        let Some(file) = &mut self.transcript else {
            return Ok(());
        };
        let entry =
            serde_json::json!({"direction":direction,"message":String::from_utf8_lossy(bytes)})
                .to_string();
        if self.trace_bytes + entry.len() + 1 > 1024 * 1024 {
            return Err(ConduitosError::refusal(
                "qemu-qmp-transcript-bound",
                "transcript exceeds 1 MiB",
            ));
        }
        writeln!(file, "{entry}").map_err(|error| {
            ConduitosError::refusal("qemu-qmp-transcript-io", error.to_string())
        })?;
        self.trace_bytes += entry.len() + 1;
        Ok(())
    }
}
impl std::ops::Deref for Reader {
    type Target = BufReader<UnixStream>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl std::ops::DerefMut for Reader {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

pub(super) fn connect(
    socket: &Path,
    child: &mut Child,
) -> Result<(UnixStream, Reader), ConduitosError> {
    connect_traced(socket, child, None)
}

pub(super) fn connect_traced(
    socket: &Path,
    child: &mut Child,
    transcript: Option<&Path>,
) -> Result<(UnixStream, Reader), ConduitosError> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut qmp = loop {
        match UnixStream::connect(socket) {
            Ok(stream) => break stream,
            Err(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ConduitosError::refusal(
                    "qemu-qmp-unavailable",
                    error.to_string(),
                ));
            }
        }
    };
    qmp.set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| ConduitosError::refusal("qemu-qmp-failed", error.to_string()))?;
    let mut reader = Reader::new(
        qmp.try_clone()
            .map_err(|error| ConduitosError::refusal("qemu-qmp-failed", error.to_string()))?,
    );
    if let Some(path) = transcript {
        reader.transcript = Some(std::fs::File::create(path).map_err(|error| {
            ConduitosError::refusal("qemu-qmp-transcript-io", error.to_string())
        })?);
    }
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
    reader: &mut Reader,
    command: &[u8],
    id: &str,
) -> Result<(), ConduitosError> {
    request_value(stream, reader, command, id).map(|_| ())
}

pub(super) fn request_value(
    stream: &mut UnixStream,
    reader: &mut Reader,
    command: &[u8],
    id: &str,
) -> Result<serde_json::Value, ConduitosError> {
    if reader.next_request >= 1024 {
        return Err(ConduitosError::refusal(
            "qemu-qmp-command-bound",
            "connection exhausted its 1024 admitted command IDs",
        ));
    }
    let id = format!("{}:{id}", reader.next_request);
    reader.next_request += 1;
    let mut command: serde_json::Value = serde_json::from_slice(command)
        .map_err(|error| ConduitosError::refusal("qemu-qmp-invalid-command", error.to_string()))?;
    let object = command
        .as_object_mut()
        .ok_or_else(|| ConduitosError::refusal("qemu-qmp-invalid-command", "expected object"))?;
    object.insert("id".into(), id.clone().into());
    let mut encoded = serde_json::to_vec(&command)
        .map_err(|error| ConduitosError::refusal("qemu-qmp-invalid-command", error.to_string()))?;
    if encoded.len() > MAXIMUM_MESSAGE_BYTES {
        return Err(ConduitosError::refusal(
            "qemu-qmp-message-bound",
            "command too large",
        ));
    }
    reader.record("command", &encoded)?;
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
        if response.get("id").and_then(serde_json::Value::as_str) != Some(id.as_str()) {
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
    reader: &mut Reader,
    deadline: Instant,
) -> Result<serde_json::Value, ConduitosError> {
    let bytes = read_message_bytes(&mut reader.inner, deadline, |reader, remaining| {
        reader.get_ref().set_read_timeout(Some(remaining))
    })?;
    reader.record("response", &bytes)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| ConduitosError::refusal("qemu-qmp-malformed-response", error.to_string()))
}

fn read_message_bytes<R: BufRead>(
    reader: &mut R,
    deadline: Instant,
    mut set_timeout: impl FnMut(&R, Duration) -> std::io::Result<()>,
) -> Result<Vec<u8>, ConduitosError> {
    let mut bytes = Vec::new();
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|duration| !duration.is_zero())
            .ok_or_else(|| {
                ConduitosError::refusal("qemu-qmp-timeout", "response deadline expired")
            })?;
        set_timeout(reader, remaining)
            .map_err(|error| ConduitosError::refusal("qemu-qmp-read-failed", error.to_string()))?;
        let available = match reader.fill_buf() {
            Ok(available) => available,
            // A signal interrupted the read, not the QMP command. Preserve the
            // partial response and the original deadline; never resend input.
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => {
                return Err(ConduitosError::refusal(
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) {
                        "qemu-qmp-timeout"
                    } else {
                        "qemu-qmp-read-failed"
                    },
                    error.to_string(),
                ));
            }
        };
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
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct InterruptedResponse {
        reads: std::collections::VecDeque<std::io::Result<&'static [u8]>>,
    }

    impl std::io::Read for InterruptedResponse {
        fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
            let bytes = self.reads.pop_front().unwrap_or(Ok(&[]))?;
            assert!(bytes.len() <= output.len());
            output[..bytes.len()].copy_from_slice(bytes);
            Ok(bytes.len())
        }
    }

    #[test]
    fn interrupted_reads_preserve_partial_response_and_do_not_reset_deadline() {
        use std::io::{Error, ErrorKind};
        let source = InterruptedResponse {
            reads: [
                Err(Error::from(ErrorKind::Interrupted)),
                Ok(b"{\"return\":".as_slice()),
                Err(Error::from(ErrorKind::Interrupted)),
                Ok(b"{},\"id\":\"0:proof\"}\n".as_slice()),
            ]
            .into(),
        };
        let mut reader = BufReader::new(source);
        let mut previous = Duration::from_secs(2);
        let bytes = read_message_bytes(&mut reader, Instant::now() + previous, |_, remaining| {
            assert!(remaining <= previous);
            previous = remaining;
            Ok(())
        })
        .unwrap();
        assert_eq!(bytes, b"{\"return\":{},\"id\":\"0:proof\"}\n");
        assert!(reader.get_ref().reads.is_empty());
    }

    #[test]
    fn interrupted_read_does_not_mask_the_following_failure_or_closed_response() {
        use std::io::{Error, ErrorKind};
        for (last, reason) in [
            (Err(Error::from(ErrorKind::TimedOut)), "qemu-qmp-timeout"),
            (
                Err(Error::from(ErrorKind::BrokenPipe)),
                "qemu-qmp-read-failed",
            ),
            (Ok(b"".as_slice()), "qemu-qmp-closed"),
        ] {
            let mut reader = BufReader::new(InterruptedResponse {
                reads: [Err(Error::from(ErrorKind::Interrupted)), last].into(),
            });
            assert_eq!(
                read_message_bytes(
                    &mut reader,
                    Instant::now() + Duration::from_secs(2),
                    |_, _| Ok(())
                )
                .unwrap_err()
                .reason,
                reason
            );
        }
    }

    fn reply(response: &[u8]) -> Result<(), ConduitosError> {
        let (mut client, mut server) = UnixStream::pair().unwrap();
        let response = response.to_vec();
        let worker = thread::spawn(move || {
            let mut reader = Reader::new(server.try_clone().unwrap());
            let mut command = String::new();
            reader.read_line(&mut command).unwrap();
            let command: serde_json::Value = serde_json::from_str(&command).unwrap();
            assert_eq!(command["id"], "0:proof");
            server.write_all(&response).unwrap();
        });
        let mut reader = Reader::new(client.try_clone().unwrap());
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
    fn expired_deadline_and_transcript_capacity_have_distinct_refusals() {
        let (_sender, stream) = UnixStream::pair().unwrap();
        let mut reader = Reader::new(stream);
        assert_eq!(
            read_message(&mut reader, Instant::now())
                .unwrap_err()
                .reason,
            "qemu-qmp-timeout"
        );
        reader.transcript = Some(
            std::fs::OpenOptions::new()
                .write(true)
                .open("/dev/null")
                .unwrap(),
        );
        reader.trace_bytes = 1024 * 1024;
        assert_eq!(
            reader.record("command", b"{}").unwrap_err().reason,
            "qemu-qmp-transcript-bound"
        );
    }

    #[test]
    fn repeated_action_labels_cannot_accept_a_duplicate_previous_reply() {
        let (mut client, mut server) = UnixStream::pair().unwrap();
        let worker = thread::spawn(move || {
            let mut reader = BufReader::new(server.try_clone().unwrap());
            for expected in ["0:repeat", "1:repeat"] {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let command: serde_json::Value = serde_json::from_str(&line).unwrap();
                assert_eq!(command["id"], expected);
                server
                    .write_all(b"{\"return\":{},\"id\":\"0:repeat\"}\n")
                    .unwrap();
            }
        });
        let mut reader = Reader::new(client.try_clone().unwrap());
        let command = br#"{"execute":"query-status"}"#;
        request(&mut client, &mut reader, command, "repeat").unwrap();
        assert_eq!(
            request(&mut client, &mut reader, command, "repeat")
                .unwrap_err()
                .reason,
            "qemu-qmp-response-id"
        );
        worker.join().unwrap();
    }

    #[test]
    fn interleaved_events_do_not_replace_matching_response() {
        reply(b"{\"event\":\"STOP\"}\n{\"return\":{},\"id\":\"0:proof\"}\n").unwrap();
    }

    #[test]
    fn stale_response_and_command_error_refuse() {
        assert!(reply(b"{\"return\":{},\"id\":\"old\"}\n").is_err());
        assert!(reply(b"{\"error\":{\"class\":\"GenericError\"},\"id\":\"0:proof\"}\n").is_err());
        assert!(reply(b"{\"id\":\"0:proof\"}\n").is_err());
    }

    #[test]
    fn event_budget_and_message_size_are_finite() {
        let events = b"{\"event\":\"STOP\"}\n".repeat(MAXIMUM_EVENTS + 1);
        let (mut sender, stream) = UnixStream::pair().unwrap();
        sender
            .write_all(&vec![b'x'; MAXIMUM_MESSAGE_BYTES + 1])
            .unwrap();
        assert!(read_message(
            &mut Reader::new(stream),
            Instant::now() + Duration::from_secs(1)
        )
        .is_err());
        // The parser accepts events as JSON; request owns the independent event-count bound.
        assert!(reply(&events).is_err());
    }
}
