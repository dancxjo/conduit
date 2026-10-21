//! One TLS instance with explicit operation and external IO grants.
use crate::{
    CERTIFICATE_BYTES, HOST_BYTES, Operation, Refusal, buffer,
    mailbox::Grant,
    memory::TRANSFER_BYTES,
    session::Session,
    wire::{Request, Response},
};
use alloc::vec::Vec;
#[derive(Default)]
pub struct State {
    session: Option<Session>,
    initialized: bool,
    plaintext: Option<(u64, Vec<u8>, usize)>,
}
/// Raw initialization transfer: independent cryptographic seed32, approved host
/// UTF-8, then approved pinned DER. Descriptor carries the two explicit lengths.
pub fn encode_initialize(seed: [u8; 32], host: &str, cert: &[u8]) -> Result<Vec<u8>, Refusal> {
    if host.is_empty()
        || host.len() > HOST_BYTES
        || cert.is_empty()
        || cert.len() > CERTIFICATE_BYTES
    {
        return Err(Refusal::Bounds);
    }
    let mut value = buffer(32 + host.len() + cert.len())?;
    value[..32].copy_from_slice(&seed);
    value[32..32 + host.len()].copy_from_slice(host.as_bytes());
    value[32 + host.len()..].copy_from_slice(cert);
    Ok(value)
}
impl State {
    pub fn execute(&mut self, request: Request, transfer: &mut [u8]) -> Result<Response, Refusal> {
        if transfer.len() != TRANSFER_BYTES {
            return Err(Refusal::Bounds);
        }
        match request {
            Request::Init {
                host_bytes,
                cert_bytes,
            } => {
                if self.initialized {
                    return Err(Refusal::Order);
                }
                self.initialized = true;
                let (h, c) = (host_bytes as usize, cert_bytes as usize);
                if h == 0 || h > HOST_BYTES || c == 0 || c > CERTIFICATE_BYTES {
                    return Err(Refusal::Bounds);
                }
                let seed = transfer[..32].try_into().map_err(|_| Refusal::Malformed)?;
                let host =
                    core::str::from_utf8(&transfer[32..32 + h]).map_err(|_| Refusal::Malformed)?;
                self.session = Some(Session::new(seed, host, &transfer[32 + h..32 + h + c])?);
                transfer.fill(0);
                Ok(Response::Ready)
            }
            Request::Start {
                operation,
                kind,
                bytes,
            } => {
                if self.plaintext.is_some() {
                    return Err(Refusal::Order);
                }
                let maximum = bytes as usize;
                if match kind {
                    Operation::Read | Operation::Write => maximum == 0 || maximum > TRANSFER_BYTES,
                    _ => maximum != 0,
                } {
                    return Err(Refusal::Bounds);
                }
                let input = if kind == Operation::Write {
                    &transfer[..maximum]
                } else {
                    &[]
                };
                self.session
                    .as_mut()
                    .ok_or(Refusal::Order)?
                    .start(operation, kind, input, maximum)?;
                transfer.fill(0);
                Ok(Response::Started { operation })
            }
            Request::Poll { operation } => self.poll(operation),
            Request::Provide {
                operation,
                event,
                bytes,
            } => {
                let session = self.current(operation)?;
                let mut bus = session.mailbox.0.borrow_mut();
                let Some(Grant::Read {
                    event: expected,
                    maximum,
                    data,
                }) = &mut bus.grant
                else {
                    return Err(Refusal::Order);
                };
                if *expected != event
                    || data.is_some()
                    || bytes == 0
                    || bytes as usize > TRANSFER_BYTES
                    || bytes as usize > *maximum
                {
                    return Err(Refusal::Order);
                }
                let mut value = buffer(bytes as usize)?;
                value.copy_from_slice(&transfer[..bytes as usize]);
                *data = Some(value);
                transfer.fill(0);
                Ok(Response::Ready)
            }
            Request::Take {
                operation,
                event,
                offset,
                maximum,
            } => {
                let session = self.current(operation)?;
                let mut bus = session.mailbox.0.borrow_mut();
                let Some(Grant::Write {
                    event: expected,
                    data,
                    read,
                    acked,
                }) = &mut bus.grant
                else {
                    return Err(Refusal::Order);
                };
                if *expected != event
                    || *acked
                    || offset as usize != *read
                    || maximum == 0
                    || maximum as usize > TRANSFER_BYTES
                    || *read >= data.len()
                {
                    return Err(Refusal::Order);
                }
                let count = (maximum as usize).min(data.len() - *read);
                transfer.fill(0);
                transfer[..count].copy_from_slice(&data[*read..*read + count]);
                *read += count;
                Ok(Response::Chunk {
                    operation,
                    event,
                    offset,
                    bytes: count as u32,
                    total: data.len() as u32,
                })
            }
            Request::Ack { operation, event } => {
                let session = self.current(operation)?;
                let mut bus = session.mailbox.0.borrow_mut();
                match &mut bus.grant {
                    Some(Grant::Write {
                        event: expected,
                        data,
                        read,
                        acked,
                    }) if *expected == event && !*acked && *read == data.len() => *acked = true,
                    Some(Grant::Flush {
                        event: expected,
                        acked,
                    }) if *expected == event && !*acked => *acked = true,
                    _ => return Err(Refusal::Order),
                }
                Ok(Response::Ready)
            }
            Request::Plaintext {
                operation,
                offset,
                maximum,
            } => {
                let (expected, data, read) = self.plaintext.as_mut().ok_or(Refusal::Order)?;
                if *expected != operation
                    || offset as usize != *read
                    || maximum == 0
                    || maximum as usize > TRANSFER_BYTES
                    || *read >= data.len()
                {
                    return Err(Refusal::Order);
                }
                let count = (maximum as usize).min(data.len() - *read);
                let total = data.len();
                transfer.fill(0);
                transfer[..count].copy_from_slice(&data[*read..*read + count]);
                *read += count;
                if *read == total {
                    self.plaintext = None;
                }
                Ok(Response::Chunk {
                    operation,
                    event: 0,
                    offset,
                    bytes: count as u32,
                    total: total as u32,
                })
            }
        }
    }
    fn current(&mut self, operation: u64) -> Result<&mut Session, Refusal> {
        let session = self.session.as_mut().ok_or(Refusal::Order)?;
        if !session
            .active
            .is_some_and(|(expected, _)| expected == operation)
        {
            return Err(Refusal::Order);
        }
        Ok(session)
    }
    fn poll(&mut self, operation: u64) -> Result<Response, Refusal> {
        let session = self.current(operation)?;
        {
            let bus = session.mailbox.0.borrow();
            if matches!(
                &bus.grant,
                Some(
                    Grant::Read { data: None, .. }
                        | Grant::Write { acked: false, .. }
                        | Grant::Flush { acked: false, .. }
                )
            ) {
                return Err(Refusal::Order);
            }
        }
        if let Some(result) = session.poll(operation)? {
            return match result {
                Ok((kind, written, bytes)) => {
                    if session.mailbox.0.borrow().grant.is_some() {
                        return Err(Refusal::Order);
                    }
                    let length = if kind == Operation::Read {
                        bytes.len()
                    } else {
                        written
                    };
                    if kind == Operation::Read && !bytes.is_empty() {
                        self.plaintext = Some((operation, bytes, 0));
                    }
                    Ok(Response::Done {
                        operation,
                        kind,
                        bytes: length as u32,
                    })
                }
                Err(error) => Ok(Response::Failed { operation, error }),
            };
        }
        let bus = session.mailbox.0.borrow();
        if bus.operation != operation {
            return Err(Refusal::Order);
        }
        match &bus.grant {
            Some(Grant::Read {
                event,
                maximum,
                data: None,
            }) => Ok(Response::NeedRead {
                operation,
                event: *event,
                maximum: *maximum as u32,
            }),
            Some(Grant::Write {
                event,
                data,
                acked: false,
                ..
            }) => Ok(Response::NeedWrite {
                operation,
                event: *event,
                bytes: data.len() as u32,
            }),
            Some(Grant::Flush {
                event,
                acked: false,
            }) => Ok(Response::NeedFlush {
                operation,
                event: *event,
            }),
            _ => Err(Refusal::Order),
        }
    }
}
