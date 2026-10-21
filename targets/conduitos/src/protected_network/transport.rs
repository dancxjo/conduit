//! Exact Network lease correlation and bounded raw input/output transactions.
use crate::{
    arch::ServiceDomain,
    protected_region::{DomainBackend, DomainReturn},
    service_memory::{ServiceImage, ServiceProfile},
};
use conduitos_network_core::{
    MAX_BLOB, Refusal, WebSocketError as E,
    memory::TRANSFER_BYTES,
    wire::{self, Envelope, InputKind, OutputKind, Request, Response},
};
pub(super) struct Service {
    domain: Option<ServiceDomain>,
    instance: u64,
    sequence: u64,
    error: Option<E>,
    // An operation-matched canonical codec refusal fences ordinary work, but
    // permits the enclosing adapter's one explicit Close on the same instance.
    codec_refusal: Option<E>,
}
impl Service {
    pub fn install() -> Result<Self, E> {
        #[cfg(conduitos_network_image)]
        let bytes: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/network.elf"));
        #[cfg(not(conduitos_network_image))]
        let bytes: &[u8] = &[];
        let image = ServiceImage::for_profile(bytes, ServiceProfile::Network)
            .map_err(|_| E::InvalidDescriptor)?;
        let domain = ServiceDomain::install(&image).map_err(|_| E::InvalidDescriptor)?;
        Ok(Self {
            instance: domain.instance(),
            domain: Some(domain),
            sequence: 1,
            error: None,
            codec_refusal: None,
        })
    }
    pub fn abort(&mut self, error: E) {
        self.error = Some(error);
        self.codec_refusal = None;
        if let Some(mut domain) = self.domain.take() {
            domain.quarantine();
            drop(domain);
        }
    }
    pub fn live(&self) -> Result<(), E> {
        if let Some(error) = self.codec_refusal {
            return Err(error);
        }
        self.transport_live()
    }
    fn transport_live(&self) -> Result<(), E> {
        if let Some(error) = self.error {
            return Err(error);
        }
        if self.domain.is_none() {
            return Err(E::UnexpectedFrame);
        }
        Ok(())
    }
    pub fn codec_refused(&self) -> bool {
        self.codec_refusal.is_some()
    }
    pub fn abort_unless_codec(&mut self, error: E) {
        if self.codec_refusal != Some(error) {
            self.abort(error);
        }
    }
    pub fn request(&mut self, request: Request) -> Result<Response, E> {
        self.request_for(request, None)
    }
    fn request_for(&mut self, request: Request, commit: Option<InputKind>) -> Result<Response, E> {
        // Only explicit Close can proceed after a canonical codec refusal.
        // No malformed response, hardware fault or IO failure reaches this path.
        if !matches!(request, Request::Close) {
            self.live()?;
        }
        self.codec_refusal = None;
        let result = self.exchange(request, commit);
        if let Err(error) = result {
            self.abort_unless_codec(error);
        }
        result
    }
    fn exchange(&mut self, request: Request, commit: Option<InputKind>) -> Result<Response, E> {
        self.transport_live()?;
        let envelope = Envelope {
            instance: self.instance,
            sequence: self.sequence,
        };
        self.sequence = self.sequence.checked_add(1).ok_or(E::UnexpectedFrame)?;
        let domain = self.domain.as_mut().ok_or(E::UnexpectedFrame)?;
        let exchange = domain.exchange().map_err(|_| E::UnexpectedFrame)?;
        exchange.fill(0);
        wire::encode_request(envelope, request, exchange).map_err(map_refusal)?;
        if domain.enter(1).map_err(|_| E::UnexpectedFrame)? != DomainReturn::Yielded {
            return Err(E::UnexpectedFrame);
        }
        let exchange = domain.exchange().map_err(|_| E::UnexpectedFrame)?;
        let (returned, response) =
            wire::decode_response(exchange.get(..wire::BYTES).ok_or(E::UnexpectedFrame)?)
                .map_err(map_refusal)?;
        if returned != envelope {
            return Err(E::UnexpectedFrame);
        }
        if let Response::Refused(refusal) = response {
            if let Refusal::WebSocket(error) = refusal {
                if expected_codec_refusal(request, commit, error) {
                    self.codec_refusal = Some(error);
                    return Err(error);
                }
            }
            return Err(E::UnexpectedFrame);
        }
        Ok(response)
    }
    pub fn input(&mut self, kind: InputKind, bytes: &[u8]) -> Result<Response, E> {
        let result = self.input_inner(kind, bytes);
        if let Err(error) = result {
            self.abort_unless_codec(error);
        }
        result
    }
    fn input_inner(&mut self, kind: InputKind, bytes: &[u8]) -> Result<Response, E> {
        if bytes.is_empty() || bytes.len() > MAX_BLOB {
            return Err(E::RequestTooLarge);
        }
        if self.request(Request::Begin {
            kind,
            bytes: bytes.len() as u32,
        })? != Response::Ready
        {
            return Err(E::UnexpectedFrame);
        }
        let mut offset = 0;
        for chunk in bytes.chunks(TRANSFER_BYTES) {
            let window = self
                .domain
                .as_mut()
                .ok_or(E::UnexpectedFrame)?
                .transfer_window()
                .map_err(|_| E::UnexpectedFrame)?;
            window.fill(0);
            window
                .get_mut(..chunk.len())
                .ok_or(E::UnexpectedFrame)?
                .copy_from_slice(chunk);
            if self.request(Request::Write {
                offset,
                bytes: chunk.len() as u32,
            })? != Response::Ready
            {
                return Err(E::UnexpectedFrame);
            }
            offset += chunk.len() as u32;
        }
        self.request_for(Request::Commit, Some(kind))
    }
    pub fn blob(
        &mut self,
        response: Response,
        kind: OutputKind,
        output: &mut [u8],
    ) -> Result<usize, E> {
        let result = self.read_blob(response, kind, output);
        if let Err(error) = result {
            self.abort(error);
        }
        result
    }
    fn read_blob(
        &mut self,
        response: Response,
        kind: OutputKind,
        output: &mut [u8],
    ) -> Result<usize, E> {
        let Response::Blob {
            kind: returned,
            bytes,
            ..
        } = response
        else {
            return Err(E::UnexpectedFrame);
        };
        if returned != kind || bytes as usize > MAX_BLOB || bytes as usize > output.len() {
            return Err(E::ResponseTooLarge);
        }
        let mut offset = 0;
        while offset < bytes {
            let maximum = (bytes - offset).min(TRANSFER_BYTES as u32);
            let Response::Chunk {
                offset: at,
                bytes: count,
                total,
            } = self.request(Request::Read { offset, maximum })?
            else {
                return Err(E::UnexpectedFrame);
            };
            if at != offset || total != bytes || count == 0 || count > maximum {
                return Err(E::UnexpectedFrame);
            }
            let source = self
                .domain
                .as_ref()
                .ok_or(E::UnexpectedFrame)?
                .output()
                .map_err(|_| E::UnexpectedFrame)?;
            let end = offset.checked_add(count).ok_or(E::UnexpectedFrame)?;
            output
                .get_mut(offset as usize..end as usize)
                .ok_or(E::UnexpectedFrame)?
                .copy_from_slice(source.get(..count as usize).ok_or(E::UnexpectedFrame)?);
            offset = end;
        }
        Ok(bytes as usize)
    }
}
fn map_refusal(refusal: Refusal) -> E {
    match refusal {
        Refusal::WebSocket(error) => error,
        Refusal::Bounds | Refusal::Malformed | Refusal::Order | Refusal::Capacity => {
            E::UnexpectedFrame
        }
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.abort(E::Close);
    }
}

// IO errors are Root-owned and never accepted as guest codec refusals. The
// allowlist is the exact set returned by the canonical Framer operation.
fn expected_codec_refusal(request: Request, commit: Option<InputKind>, error: E) -> bool {
    match (request, commit, error) {
        (Request::Commit, Some(InputKind::Initialize), E::InvalidDescriptor) => true,
        (
            Request::Commit,
            Some(InputKind::Handshake),
            E::HandshakeAuthentication | E::ResponseTooLarge,
        ) => true,
        (Request::Commit, Some(InputKind::SendBinary), E::RequestTooLarge | E::FrameWrite) => true,
        (Request::Commit, Some(InputKind::Receive), E::ResponseTooLarge) => true,
        (
            Request::StartReceive { .. } | Request::StepReceive,
            None,
            E::ResponseTooLarge | E::UnexpectedFrame,
        ) => true,
        (Request::Close, None, E::Close) => true,
        _ => false,
    }
}
