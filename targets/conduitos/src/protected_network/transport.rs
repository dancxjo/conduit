//! Exact Network lease correlation and bounded raw input/output transactions.
use crate::{
    arch::ServiceDomain,
    protected_region::{DomainBackend, DomainReturn},
    service_memory::{ServiceImage, ServiceProfile},
};
use conduitos_network_core::{
    memory::TRANSFER_BYTES,
    wire::{self, Envelope, InputKind, OutputKind, Request, Response},
    Refusal, WebSocketError as E, MAX_BLOB,
};
pub(super) struct Service {
    domain: Option<ServiceDomain>,
    instance: u64,
    sequence: u64,
    error: Option<E>,
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
        })
    }
    pub fn abort(&mut self, error: E) {
        self.error = Some(error);
        if let Some(mut domain) = self.domain.take() {
            domain.quarantine();
            drop(domain);
        }
    }
    pub fn live(&self) -> Result<(), E> {
        if let Some(error) = self.error {
            return Err(error);
        }
        if self.domain.is_none() {
            return Err(E::UnexpectedFrame);
        }
        Ok(())
    }
    pub fn request(&mut self, request: Request) -> Result<Response, E> {
        let result = self.exchange(request);
        if let Err(error) = result {
            self.abort(error);
        }
        result
    }
    fn exchange(&mut self, request: Request) -> Result<Response, E> {
        self.live()?;
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
            return Err(map_refusal(refusal));
        }
        Ok(response)
    }
    pub fn input(&mut self, kind: InputKind, bytes: &[u8]) -> Result<Response, E> {
        let result = self.input_inner(kind, bytes);
        if let Err(error) = result {
            self.abort(error);
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
        self.request(Request::Commit)
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
