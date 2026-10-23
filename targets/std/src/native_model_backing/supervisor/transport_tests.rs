use super::*;
use std::io;
struct Sink {
    bytes: Vec<u8>,
    chunk: usize,
    blocked: bool,
}
impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.blocked {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let count = self.chunk.min(bytes.len());
        self.bytes.extend_from_slice(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        panic!("fixed unbuffered transport must not flush")
    }
}
struct Source {
    bytes: [u8; 8],
    offset: usize,
    chunk: usize,
    blocked: bool,
}
impl Read for Source {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.blocked {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let count = out.len().min(self.chunk).min(8 - self.offset);
        out[..count].copy_from_slice(&self.bytes[self.offset..self.offset + count]);
        self.offset += count;
        Ok(count)
    }
}
fn pipes(sequence: u32, value: f32) -> (Sink, Source) {
    let mut bytes = [0; 8];
    bytes[..4].copy_from_slice(&sequence.to_le_bytes());
    bytes[4..].copy_from_slice(&value.to_bits().to_le_bytes());
    (
        Sink {
            bytes: Vec::new(),
            chunk: 1,
            blocked: false,
        },
        Source {
            bytes,
            offset: 0,
            chunk: 1,
            blocked: false,
        },
    )
}
#[test]
fn partial_io_and_blocked_ack_never_publish_early_or_duplicate() {
    let mut exchange = Exchange::new();
    exchange.begin(7, -0.0).unwrap();
    assert_eq!(exchange.begin(8, 1.0), Err(NativeBackingRefusal::Protocol));
    let (mut sink, mut source) = pipes(7, -0.0);
    sink.blocked = true;
    assert!(!exchange.poll(&mut sink, &mut source).unwrap());
    assert!(sink.bytes.is_empty());
    sink.blocked = false;
    for _ in 0..15 {
        assert!(!exchange.poll(&mut sink, &mut source).unwrap());
    }
    assert!(exchange.poll(&mut sink, &mut source).unwrap());
    assert_eq!(sink.bytes.len(), 9);
    sink.blocked = true;
    assert_eq!(exchange.take(&mut sink).unwrap(), None);
    sink.blocked = false;
    assert_eq!(
        exchange.take(&mut sink).unwrap().unwrap().to_bits(),
        (-0.0f32).to_bits()
    );
    assert_eq!(sink.bytes[9], b'C');
    assert_eq!(exchange.take(&mut sink).unwrap(), None);
    assert_eq!(sink.bytes.len(), 10);
}
#[test]
fn cancellation_after_ready_revokes_value_and_ack_and_cannot_restart() {
    let mut exchange = Exchange::new();
    exchange.begin(1, 5.0).unwrap();
    let (mut sink, mut source) = pipes(1, 5.0);
    sink.chunk = 9;
    source.chunk = 8;
    assert!(exchange.poll(&mut sink, &mut source).unwrap());
    exchange.revoke();
    exchange.revoke();
    assert_eq!(exchange.take(&mut sink).unwrap(), None);
    assert_eq!(
        exchange.poll(&mut sink, &mut source),
        Err(NativeBackingRefusal::Protocol)
    );
    assert_eq!(exchange.begin(2, 5.0), Err(NativeBackingRefusal::Protocol));
    assert_eq!(sink.bytes.len(), 9);
}
#[test]
fn wrong_sequence_invalid_value_and_eof_cannot_be_acknowledged() {
    for (sequence, value, expected) in [
        (2, 5.0, NativeBackingRefusal::Protocol),
        (1, 6.0, NativeBackingRefusal::InvalidOutput),
    ] {
        let mut exchange = Exchange::new();
        exchange.begin(1, 5.0).unwrap();
        let (mut sink, mut source) = pipes(sequence, value);
        sink.chunk = 9;
        source.chunk = 8;
        assert_eq!(exchange.poll(&mut sink, &mut source), Err(expected));
        assert_eq!(exchange.take(&mut sink).unwrap(), None);
        assert_eq!(sink.bytes.len(), 9);
    }
    let mut exchange = Exchange::new();
    exchange.begin(1, 5.0).unwrap();
    let (mut sink, mut source) = pipes(1, 5.0);
    sink.chunk = 9;
    source.offset = 8;
    assert_eq!(
        exchange.poll(&mut sink, &mut source),
        Err(NativeBackingRefusal::ProviderLost)
    );
    assert_eq!(exchange.take(&mut sink).unwrap(), None);
}

#[test]
fn actual_nonblocking_unix_pipes_keep_partial_reply_private_until_ack() {
    use std::os::unix::net::UnixStream;
    let (mut input, mut worker_input) = UnixStream::pair().unwrap();
    let (mut worker_output, mut output) = UnixStream::pair().unwrap();
    super::super::nonblocking(&input).unwrap();
    super::super::nonblocking(&output).unwrap();
    let mut exchange = Exchange::new();
    exchange.begin(17, 2.5).unwrap();
    assert!(!exchange.poll(&mut input, &mut output).unwrap());
    let mut request = [0; 9];
    worker_input.read_exact(&mut request).unwrap();
    assert_eq!(request[0], 1);
    assert_eq!(&request[1..5], &17u32.to_le_bytes());
    worker_output.write_all(&request[1..4]).unwrap();
    assert!(!exchange.poll(&mut input, &mut output).unwrap());
    assert_eq!(exchange.take(&mut input).unwrap(), None);
    worker_output.write_all(&request[4..]).unwrap();
    assert!(exchange.poll(&mut input, &mut output).unwrap());
    assert_eq!(exchange.take(&mut input).unwrap(), Some(2.5));
    let mut ack = [0];
    worker_input.read_exact(&mut ack).unwrap();
    assert_eq!(ack, *b"C");
}
