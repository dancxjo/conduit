//! One owning pinned operation. It owns the connection while borrowing it across
//! await, so Rust's async state machine supplies pinning without a self-pointer.
use crate::{
    CERTIFICATE_BYTES, Failure, Operation, Refusal, buffer, mailbox::Mailbox, resources::Resources,
    rng::ErasedChaCha,
};
use alloc::{boxed::Box, vec::Vec};
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};
use embedded_tls::{
    Aes128GcmSha256, Certificate, CryptoProvider, MaxFragmentLength, NoClock, TlsConfig,
    TlsConnection, TlsContext, TlsError, TlsVerifier, pki::CertVerifier,
};
type Connection = TlsConnection<'static, Mailbox, Aes128GcmSha256>;
struct Provider {
    rng: ErasedChaCha,
    verifier: CertVerifier<'static, Aes128GcmSha256, NoClock, CERTIFICATE_BYTES>,
}
impl CryptoProvider for Provider {
    type CipherSuite = Aes128GcmSha256;
    type Signature = [u8; 0];
    fn rng(&mut self) -> impl embedded_tls::CryptoRngCore {
        &mut self.rng
    }
    fn verifier(&mut self) -> Result<&mut impl TlsVerifier<Aes128GcmSha256>, TlsError> {
        Ok(&mut self.verifier)
    }
}
pub(crate) struct Finished {
    pub connection: Option<Box<Connection>>,
    pub result: Result<Vec<u8>, TlsError>,
    pub written: usize,
}
type Pending = Pin<Box<dyn Future<Output = Finished>>>;
pub(crate) struct Session {
    // Explicit Drop below ends the future's connection ownership first, then
    // drops any idle connection, before Resources can erase its borrowed buffers.
    pending: Option<Pending>,
    connection: Option<Box<Connection>>,
    _resources: Resources,
    host: &'static str,
    cert: &'static [u8],
    seed: Option<[u8; 32]>,
    pub mailbox: Mailbox,
    pub active: Option<(u64, Operation)>,
    pub next_operation: u64,
    opened: bool,
    closed: bool,
}
impl Session {
    pub fn new(seed: [u8; 32], host: &str, cert: &[u8]) -> Result<Self, Refusal> {
        let mut resources = Resources::new(host, cert)?;
        // SAFETY: Session privately owns Resources and never accesses its raw
        // allocations after this one loan. No returned reference escapes Session
        // or its owning operation future. Drop explicitly enforces loan teardown.
        let loans = unsafe { resources.loan_once()? };
        let mailbox = Mailbox::default();
        let connection = Box::new(Connection::new(mailbox.clone(), loans.rx, loans.tx));
        Ok(Self {
            pending: None,
            connection: Some(connection),
            _resources: resources,
            host: loans.host,
            cert: loans.cert,
            seed: Some(seed),
            mailbox,
            active: None,
            next_operation: 1,
            opened: false,
            closed: false,
        })
    }
    pub fn start(
        &mut self,
        operation: u64,
        kind: Operation,
        input: &[u8],
        maximum: usize,
    ) -> Result<(), Refusal> {
        if self.active.is_some()
            || self.closed
            || operation != self.next_operation
            || (kind == Operation::Open) == self.opened
        {
            return Err(Refusal::Order);
        }
        self.next_operation = self
            .next_operation
            .checked_add(1)
            .ok_or(Refusal::Capacity)?;
        let connection = self.connection.take().ok_or(Refusal::Order)?;
        let mut bus = self.mailbox.0.borrow_mut();
        if bus.grant.is_some() {
            return Err(Refusal::Order);
        }
        bus.operation = operation;
        drop(bus);
        let host = self.host;
        let cert = self.cert;
        // Every operation future owns its Box<Connection>, not a borrow of a
        // field in Session. Pin<Box<Future>> is never moved internally or repolled
        // after Ready, and exclusively retains all compiler-created await borrows.
        self.pending = Some(match kind {
            Operation::Open => {
                let seed = self.seed.take().ok_or(Refusal::Order)?;
                Box::pin(async move {
                    let mut connection = connection;
                    let config = TlsConfig::new()
                        .with_server_name(host)
                        .with_max_fragment_length(MaxFragmentLength::Bits11);
                    let provider = Provider {
                        rng: ErasedChaCha::new(seed),
                        verifier: CertVerifier::new(Certificate::X509(cert)),
                    };
                    let result = connection
                        .open(TlsContext::new(&config, provider))
                        .await
                        .map(|_| Vec::new());
                    Finished {
                        connection: Some(connection),
                        result,
                        written: 0,
                    }
                }) as Pending
            }
            Operation::Write => {
                let mut bytes = buffer(input.len())?;
                bytes.copy_from_slice(input);
                Box::pin(async move {
                    let mut connection = connection;
                    match connection.write(&bytes).await {
                        Ok(written) => Finished {
                            connection: Some(connection),
                            result: Ok(Vec::new()),
                            written,
                        },
                        Err(error) => Finished {
                            connection: Some(connection),
                            result: Err(error),
                            written: 0,
                        },
                    }
                })
            }
            Operation::Read => {
                let mut bytes = buffer(maximum)?;
                Box::pin(async move {
                    let mut connection = connection;
                    let result = match connection.read(&mut bytes).await {
                        Ok(length) => {
                            bytes.truncate(length);
                            Ok(bytes)
                        }
                        Err(error) => Err(error),
                    };
                    Finished {
                        connection: Some(connection),
                        result,
                        written: 0,
                    }
                })
            }
            Operation::Flush => Box::pin(async move {
                let mut connection = connection;
                let result = connection.flush().await.map(|_| Vec::new());
                Finished {
                    connection: Some(connection),
                    result,
                    written: 0,
                }
            }),
            Operation::Close => Box::pin(async move {
                let result = (*connection)
                    .close()
                    .await
                    .map(|_| Vec::new())
                    .map_err(|(_, error)| error);
                Finished {
                    connection: None,
                    result,
                    written: 0,
                }
            }),
        });
        self.active = Some((operation, kind));
        Ok(())
    }
    pub fn poll(
        &mut self,
        operation: u64,
    ) -> Result<Option<Result<(Operation, usize, Vec<u8>), Failure>>, Refusal> {
        let (current, kind) = self.active.ok_or(Refusal::Order)?;
        if current != operation {
            return Err(Refusal::Order);
        }
        let mut context = Context::from_waker(Waker::noop());
        let finished = match self
            .pending
            .as_mut()
            .ok_or(Refusal::Order)?
            .as_mut()
            .poll(&mut context)
        {
            Poll::Pending => return Ok(None),
            Poll::Ready(finished) => finished,
        };
        // Drop the completed future before making its returned connection
        // available to another operation; no await borrow survives this boundary.
        drop(self.pending.take());
        self.active = None;
        self.connection = finished.connection;
        match finished.result {
            Ok(bytes) => {
                if kind == Operation::Open {
                    self.opened = true;
                }
                if kind == Operation::Close {
                    self.closed = true;
                }
                Ok(Some(Ok((kind, finished.written, bytes))))
            }
            Err(error) => {
                // No operation is replayed/restarted after a TLS error. Root may
                // still retire TCP and preserve prior completed-operation truth.
                self.closed = true;
                drop(self.connection.take());
                self.mailbox.0.borrow_mut().grant = None;
                Ok(Some(Err(classify(kind, error))))
            }
        }
    }
}
fn classify(kind: Operation, error: TlsError) -> Failure {
    if kind == Operation::Open {
        return match error {
            TlsError::InvalidCertificate
            | TlsError::InvalidCertificateEntry
            | TlsError::InvalidSignature
            | TlsError::InvalidSignatureScheme => Failure::Authentication,
            _ => Failure::Handshake,
        };
    }
    match kind {
        Operation::Write | Operation::Flush => Failure::Write,
        Operation::Read => Failure::Read,
        Operation::Close => Failure::Close,
        Operation::Open => Failure::Handshake,
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        drop(self.pending.take());
        drop(self.connection.take());
        if let Some(seed) = &mut self.seed {
            for byte in seed {
                unsafe { core::ptr::write_volatile(byte, 0) };
            }
        }
        // resources drops after this body, once connection and future loans ended.
    }
}
