//! Root broker for the independently linked canonical Crèche draft reducer.
//! The broker owns transport correlation and bounded transient bytes, never a
//! second mutable BirthDraft or authority to create a Body/start a Play.
mod draft;
mod errors;
use crate::{
    arch::ServiceDomain,
    protected_region::{DomainBackend, DomainCost, DomainRefusal, DomainReturn},
    service_memory::{ServiceImage, ServiceProfile},
};
use alloc::vec::Vec;
use conduitos_creche_core::wire::{
    self, Envelope, InputKind, OutputKind, Refusal, Request, Response,
};
pub use draft::CrecheDraft;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportError {
    Backend(DomainRefusal),
    Protocol,
    Terminated,
    Exhausted,
    Bounds,
    Allocation,
    Refused(Refusal),
}

pub struct CrecheService {
    domain: Option<ServiceDomain>,
    instance: u64,
    next_request: u64,
    final_cost: DomainCost,
}

impl CrecheService {
    pub fn install_embedded() -> Result<Self, TransportError> {
        #[cfg(conduitos_creche_image)]
        {
            let bytes = include_bytes!(concat!(env!("OUT_DIR"), "/creche.elf"));
            let image = ServiceImage::for_profile(bytes, ServiceProfile::Draft)
                .map_err(|_| TransportError::Protocol)?;
            Self::install(&image)
        }
        #[cfg(not(conduitos_creche_image))]
        {
            Err(TransportError::Backend(DomainRefusal::Unsupported))
        }
    }

    pub fn install(image: &ServiceImage<'_>) -> Result<Self, TransportError> {
        if image.profile() != ServiceProfile::Draft {
            return Err(TransportError::Bounds);
        }
        let domain = ServiceDomain::install(image).map_err(TransportError::Backend)?;
        Ok(Self {
            instance: domain.instance(),
            final_cost: domain.cost(),
            domain: Some(domain),
            next_request: 1,
        })
    }

    pub fn instance(&self) -> u64 {
        self.instance
    }

    pub fn cost(&self) -> DomainCost {
        self.domain
            .as_ref()
            .map_or(self.final_cost, DomainBackend::cost)
    }

    /// Terminal release never retries, reinitializes, or changes another slot.
    pub fn quarantine(&mut self) {
        if let Some(mut domain) = self.domain.take() {
            domain.quarantine();
            self.final_cost = domain.cost();
            drop(domain);
        }
    }
}

impl CrecheService {
    pub fn is_live(&self) -> bool {
        self.domain.is_some()
    }

    fn request(&mut self, request: Request<'_>) -> Result<Response, TransportError> {
        let result = self.exchange(request);
        if result.is_err() {
            self.quarantine();
        }
        match result? {
            Response::Refused(error) => {
                if matches!(
                    error,
                    Refusal::Bounds
                        | Refusal::Malformed
                        | Refusal::Order
                        | Refusal::StaleInstance
                        | Refusal::StaleRequest
                        | Refusal::Capacity
                ) {
                    self.quarantine();
                }
                Err(TransportError::Refused(error))
            }
            response => Ok(response),
        }
    }

    fn exchange(&mut self, request: Request<'_>) -> Result<Response, TransportError> {
        let envelope = Envelope {
            instance: self.instance,
            request: self.next_request,
        };
        self.next_request = self
            .next_request
            .checked_add(1)
            .ok_or(TransportError::Exhausted)?;
        let domain = self.domain.as_mut().ok_or(TransportError::Terminated)?;
        let exchange = domain.exchange().map_err(TransportError::Backend)?;
        exchange.fill(0);
        wire::encode_request(envelope, request, exchange).map_err(|_| TransportError::Protocol)?;
        if domain.enter(1).map_err(TransportError::Backend)? != DomainReturn::Yielded {
            return Err(TransportError::Terminated);
        }
        let exchange = domain.exchange().map_err(TransportError::Backend)?;
        let length = u32::from_le_bytes(
            exchange
                .get(8..12)
                .ok_or(TransportError::Protocol)?
                .try_into()
                .map_err(|_| TransportError::Protocol)?,
        ) as usize;
        let (returned, response) =
            wire::decode_response(exchange.get(..length).ok_or(TransportError::Protocol)?)
                .map_err(|_| TransportError::Protocol)?;
        if returned != envelope || !response_matches(request, response) {
            return Err(TransportError::Protocol);
        }
        Ok(response)
    }

    fn input(&mut self, kind: InputKind, bytes: &[u8]) -> Result<Response, TransportError> {
        let result = self.write_input(kind, bytes);
        // Once Begin succeeds, local failures cannot leave a reusable partial transaction.
        if result
            .as_ref()
            .err()
            .is_some_and(|e| !matches!(e, TransportError::Refused(_)))
        {
            self.quarantine();
        }
        result
    }
    fn write_input(&mut self, kind: InputKind, bytes: &[u8]) -> Result<Response, TransportError> {
        let total = u32::try_from(bytes.len()).map_err(|_| TransportError::Bounds)?;
        if total == 0 || total > wire::MAX_BLOB_BYTES {
            return Err(TransportError::Bounds);
        }
        self.request(Request::Begin { kind, total })?;
        let mut offset = 0;
        for chunk in bytes.chunks(wire::TRANSFER_BYTES as usize) {
            let window = self
                .domain
                .as_mut()
                .ok_or(TransportError::Terminated)?
                .transfer_window()
                .map_err(TransportError::Backend)?;
            window.fill(0);
            window
                .get_mut(..chunk.len())
                .ok_or(TransportError::Protocol)?
                .copy_from_slice(chunk);
            self.request(Request::Write {
                offset,
                window_offset: 0,
                bytes: chunk.len() as u32,
            })?;
            offset += chunk.len() as u32;
        }
        self.request(Request::Commit)
    }

    fn blob(
        &mut self,
        response: Response,
        expected: OutputKind,
        revision: u32,
    ) -> Result<Vec<u8>, TransportError> {
        let result = self.read_blob(response, expected, revision);
        if result.is_err() {
            self.quarantine();
        }
        result
    }
    fn read_blob(
        &mut self,
        response: Response,
        expected: OutputKind,
        revision: u32,
    ) -> Result<Vec<u8>, TransportError> {
        let Response::Blob {
            kind,
            revision: returned,
            bytes: total,
        } = response
        else {
            return Err(TransportError::Protocol);
        };
        if kind != expected || returned != revision || total == 0 || total > wire::MAX_BLOB_BYTES {
            return Err(TransportError::Protocol);
        }
        let mut result = Vec::new();
        result
            .try_reserve_exact(total as usize)
            .map_err(|_| TransportError::Allocation)?;
        let mut offset = 0;
        while offset < total {
            let maximum = (total - offset).min(wire::TRANSFER_BYTES);
            let Response::Chunk {
                offset: at,
                bytes,
                total: reported,
                complete,
            } = self.request(Request::Read { offset, maximum })?
            else {
                return Err(TransportError::Protocol);
            };
            if at != offset
                || bytes == 0
                || bytes > maximum
                || reported != total
                || complete != (offset + bytes == total)
            {
                return Err(TransportError::Protocol);
            }
            let window = self
                .domain
                .as_ref()
                .ok_or(TransportError::Terminated)?
                .output()
                .map_err(TransportError::Backend)?;
            result.extend_from_slice(
                window
                    .get(..bytes as usize)
                    .ok_or(TransportError::Protocol)?,
            );
            offset += bytes;
        }
        Ok(result)
    }
}
fn response_matches(request: Request<'_>, response: Response) -> bool {
    match (request, response) {
        (_, Response::Refused(_)) => true,
        (
            Request::Begin { .. } | Request::Write { .. } | Request::AllowIdle,
            Response::Ready { .. },
        ) => true,
        (
            Request::Commit | Request::Apply { .. },
            Response::Ready { .. } | Response::Blob { .. },
        ) => true,
        (
            Request::Snapshot { .. },
            Response::Blob {
                kind: OutputKind::Snapshot,
                ..
            },
        ) => true,
        (
            Request::Selection { .. },
            Response::Blob {
                kind: OutputKind::Selection,
                ..
            },
        ) => true,
        (Request::Read { .. }, Response::Chunk { .. }) => true,
        (
            Request::NativeKey { .. },
            Response::Blob {
                kind: OutputKind::Native,
                ..
            },
        ) => true,
        _ => false,
    }
}
