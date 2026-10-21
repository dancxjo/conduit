//! Root TCP realization for a guest-owned canonical asynchronous TLS connection.
//! Every operation resumes one retained future; Root grants only bounded cipher IO.
mod transport;
use crate::virtio_tls::VirtioTlsError;
use conduitos_tls_core::{
    Operation,
    wire::{Request, Response},
};
use embedded_io::{Error, ErrorKind, ErrorType, Read, Write};
use transport::Service;
const WINDOW: usize = 16 * 1024;
const RECEIVE: usize = 16_640;
const TRANSMIT: usize = 4096;
const POLLS: usize = 4096;
const CIPHER_BUDGET: usize = 1024 * 1024;
// These are transport engineering ceilings, not measured latency guarantees.
const _: () = assert!(WINDOW == conduitos_tls_core::memory::TRANSFER_BYTES);
const _: () = assert!(RECEIVE == conduitos_tls_core::RECEIVE_BYTES);
const _: () = assert!(TRANSMIT == conduitos_tls_core::TRANSMIT_BYTES);
const _: () = assert!(RECEIVE == crate::virtio_tls::TLS_RECEIVE_BUFFER_BYTES);
const _: () = assert!(TRANSMIT == crate::virtio_tls::TLS_TRANSMIT_BUFFER_BYTES);
#[derive(Clone, Copy, Debug)]
pub(super) enum Failure {
    Protocol,
    Authentication,
    Handshake,
    Write,
    Read,
    Close,
    Io(ErrorKind),
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct IoError(ErrorKind);
impl core::fmt::Display for IoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("isolated TLS operation failed")
    }
}
impl core::error::Error for IoError {}
impl Error for IoError {
    fn kind(&self) -> ErrorKind {
        self.0
    }
}
pub(crate) struct Tls<S: Read + Write> {
    stream: S,
    service: Service,
    operation: u64,
    event: u64,
    failure: Option<Failure>,
    deadline: Option<crate::arch::CandidateDeadline>,
}
impl<S: Read + Write> Tls<S> {
    pub fn open(
        stream: S,
        mut seed: [u8; 32],
        host: &str,
        certificate: &[u8],
        deadline: Option<crate::arch::CandidateDeadline>,
    ) -> Result<Self, VirtioTlsError> {
        if host.is_empty() || host.len() > 64 || certificate.is_empty() || certificate.len() > 1024
        {
            seed.fill(0);
            return Err(VirtioTlsError::InvalidDescriptor);
        }
        let service = Service::install().map_err(|_| VirtioTlsError::Handshake)?;
        let mut tls = Self {
            stream,
            service,
            operation: 0,
            event: 0,
            failure: None,
            deadline,
        };
        let mut descriptor = [0; 32 + 64 + 1024];
        descriptor[..32].copy_from_slice(&seed);
        seed.fill(0);
        descriptor[32..32 + host.len()].copy_from_slice(host.as_bytes());
        descriptor[32 + host.len()..32 + host.len() + certificate.len()]
            .copy_from_slice(certificate);
        let initialized = (|| {
            tls.service
                .input(&descriptor[..32 + host.len() + certificate.len()])?;
            if tls.service.request(Request::Init {
                host_bytes: host.len() as u32,
                cert_bytes: certificate.len() as u32,
            })? != Response::Ready
            {
                return Err(Failure::Protocol);
            }
            tls.run(Operation::Open, &[], &mut [])?;
            Ok(())
        })();
        descriptor.fill(0);
        if let Err(error) = initialized {
            tls.service.abort();
            return Err(match error {
                Failure::Authentication => VirtioTlsError::Authentication,
                Failure::Io(ErrorKind::TimedOut) => VirtioTlsError::HandshakeTimeout,
                _ => VirtioTlsError::Handshake,
            });
        }
        Ok(tls)
    }
    fn run(&mut self, kind: Operation, input: &[u8], output: &mut [u8]) -> Result<usize, Failure> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        let result = self.pump(kind, input, output);
        if let Err(error) = result {
            self.failure = Some(error);
            self.service.abort();
        }
        result
    }
    fn pump(&mut self, kind: Operation, input: &[u8], output: &mut [u8]) -> Result<usize, Failure> {
        self.operation = self.operation.checked_add(1).ok_or(Failure::Protocol)?;
        let operation = self.operation;
        if input.len() > WINDOW || output.len() > WINDOW {
            return Err(Failure::Protocol);
        }
        let bytes = if kind == Operation::Read {
            output.len()
        } else {
            input.len()
        };
        if !input.is_empty() {
            self.service.input(input)?;
        }
        if self.service.request(Request::Start {
            operation,
            kind,
            bytes: bytes as u32,
        })? != (Response::Started { operation })
        {
            return Err(Failure::Protocol);
        }
        let mut cipher_bytes = 0;
        let mut buffer = [0; WINDOW];
        for _ in 0..POLLS {
            if self
                .deadline
                .is_some_and(|deadline| deadline.elapsed_millis().is_none())
            {
                return Err(Failure::Io(ErrorKind::TimedOut));
            }
            match self.service.request(Request::Poll { operation })? {
                Response::NeedRead {
                    operation: returned,
                    event,
                    maximum,
                } => {
                    self.admit_event(operation, returned, event)?;
                    let maximum = maximum as usize;
                    if maximum == 0 || maximum > RECEIVE {
                        return Err(Failure::Protocol);
                    }
                    let maximum = maximum.min(WINDOW).min(CIPHER_BUDGET - cipher_bytes);
                    if maximum == 0 {
                        return Err(Failure::Protocol);
                    }
                    let count = self
                        .stream
                        .read(&mut buffer[..maximum])
                        .map_err(|e| Failure::Io(e.kind()))?;
                    if count == 0 || count > maximum {
                        return Err(Failure::Io(ErrorKind::ConnectionAborted));
                    }
                    cipher_bytes += count;
                    self.service.input(&buffer[..count])?;
                    if self.service.request(Request::Provide {
                        operation,
                        event,
                        bytes: count as u32,
                    })? != Response::Ready
                    {
                        return Err(Failure::Protocol);
                    }
                }
                Response::NeedWrite {
                    operation: returned,
                    event,
                    bytes,
                } => {
                    self.admit_event(operation, returned, event)?;
                    let bytes = bytes as usize;
                    if bytes == 0 || bytes > TRANSMIT || bytes > CIPHER_BUDGET - cipher_bytes {
                        return Err(Failure::Protocol);
                    }
                    self.download(operation, event, bytes, &mut buffer[..bytes], false)?;
                    self.stream
                        .write_all(&buffer[..bytes])
                        .map_err(|e| Failure::Io(e.kind()))?;
                    cipher_bytes += bytes;
                    if self.service.request(Request::Ack { operation, event })? != Response::Ready {
                        return Err(Failure::Protocol);
                    }
                }
                Response::NeedFlush {
                    operation: returned,
                    event,
                } => {
                    self.admit_event(operation, returned, event)?;
                    self.stream.flush().map_err(|e| Failure::Io(e.kind()))?;
                    if self.service.request(Request::Ack { operation, event })? != Response::Ready {
                        return Err(Failure::Protocol);
                    }
                }
                Response::Done {
                    operation: returned,
                    kind: returned_kind,
                    bytes,
                } => {
                    if returned != operation || returned_kind != kind {
                        return Err(Failure::Protocol);
                    }
                    let bytes = bytes as usize;
                    match kind {
                        Operation::Read => {
                            if bytes > output.len() {
                                return Err(Failure::Protocol);
                            }
                            self.download(operation, 0, bytes, &mut output[..bytes], true)?;
                        }
                        Operation::Write if bytes > 0 && bytes <= input.len() => {}
                        Operation::Open | Operation::Flush | Operation::Close if bytes == 0 => {}
                        _ => return Err(Failure::Protocol),
                    }
                    return Ok(bytes);
                }
                Response::Failed {
                    operation: returned,
                    error,
                } => {
                    if returned != operation {
                        return Err(Failure::Protocol);
                    }
                    return Err(classify(kind, error));
                }
                _ => return Err(Failure::Protocol),
            }
        }
        Err(Failure::Protocol)
    }
    fn admit_event(&mut self, expected: u64, returned: u64, event: u64) -> Result<(), Failure> {
        if returned != expected || event == 0 || event <= self.event {
            return Err(Failure::Protocol);
        }
        self.event = event;
        Ok(())
    }
    fn download(
        &mut self,
        operation: u64,
        event: u64,
        bytes: usize,
        output: &mut [u8],
        plaintext: bool,
    ) -> Result<(), Failure> {
        let mut offset = 0;
        while offset < bytes {
            let maximum = (bytes - offset).min(WINDOW) as u32;
            let request = if plaintext {
                Request::Plaintext {
                    operation,
                    offset: offset as u32,
                    maximum,
                }
            } else {
                Request::Take {
                    operation,
                    event,
                    offset: offset as u32,
                    maximum,
                }
            };
            let Response::Chunk {
                operation: returned,
                event: returned_event,
                offset: at,
                bytes: count,
                total,
            } = self.service.request(request)?
            else {
                return Err(Failure::Protocol);
            };
            if returned != operation
                || returned_event != event
                || at as usize != offset
                || total as usize != bytes
                || count == 0
                || count > maximum
            {
                return Err(Failure::Protocol);
            }
            let end = offset + count as usize;
            self.service
                .output(output.get_mut(offset..end).ok_or(Failure::Protocol)?)?;
            offset = end;
        }
        Ok(())
    }
    pub fn close(mut self) -> Result<S, (S, IoError)> {
        let result = self.run(Operation::Close, &[], &mut []);
        self.service.abort();
        match result {
            Ok(_) => Ok(self.stream),
            Err(error) => Err((self.stream, io_error(error))),
        }
    }
}
fn io_error(failure: Failure) -> IoError {
    IoError(match failure {
        Failure::Io(kind) => kind,
        _ => ErrorKind::Other,
    })
}
fn classify(kind: Operation, error: conduitos_tls_core::Failure) -> Failure {
    use conduitos_tls_core::Failure as E;
    match (kind, error) {
        (Operation::Open, E::Authentication) => Failure::Authentication,
        (Operation::Open, E::Handshake) => Failure::Handshake,
        (Operation::Write | Operation::Flush, E::Write) => Failure::Write,
        (Operation::Read, E::Read) => Failure::Read,
        (Operation::Close, E::Close) => Failure::Close,
        _ => Failure::Protocol,
    }
}
impl<S: Read + Write> ErrorType for Tls<S> {
    type Error = IoError;
}
impl<S: Read + Write> Read for Tls<S> {
    fn read(&mut self, output: &mut [u8]) -> Result<usize, IoError> {
        if output.is_empty() {
            return Ok(0);
        }
        let maximum = output.len().min(WINDOW);
        self.run(Operation::Read, &[], &mut output[..maximum])
            .map_err(io_error)
    }
}
impl<S: Read + Write> Write for Tls<S> {
    fn write(&mut self, input: &[u8]) -> Result<usize, IoError> {
        if input.is_empty() {
            return Ok(0);
        }
        self.run(Operation::Write, &input[..input.len().min(WINDOW)], &mut [])
            .map_err(io_error)
    }
    fn flush(&mut self) -> Result<(), IoError> {
        self.run(Operation::Flush, &[], &mut [])
            .map(|_| ())
            .map_err(io_error)
    }
}
