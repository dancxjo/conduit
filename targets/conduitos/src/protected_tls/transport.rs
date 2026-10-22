//! Exact correlation and finite byte grants for the independent TLS lease.
use super::Failure;
use crate::{
    arch::ServiceDomain,
    protected_region::{DomainBackend, DomainReturn},
    service_memory::{ServiceImage, ServiceProfile},
};
use conduitos_tls_core::wire::{self, Envelope, Request, Response};
pub(super) struct Service {
    domain: Option<ServiceDomain>,
    instance: u64,
    sequence: u64,
}
impl Service {
    pub fn install() -> Result<Self, Failure> {
        #[cfg(conduitos_tls_image)]
        let bytes: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tls.elf"));
        #[cfg(not(conduitos_tls_image))]
        let bytes: &[u8] = &[];
        let image =
            ServiceImage::for_profile(bytes, ServiceProfile::Tls).map_err(|_| Failure::Protocol)?;
        let domain = ServiceDomain::install(&image).map_err(|_| Failure::Protocol)?;
        Ok(Self {
            instance: domain.instance(),
            domain: Some(domain),
            sequence: 1,
        })
    }
    pub fn abort(&mut self) {
        if let Some(mut domain) = self.domain.take() {
            domain.quarantine();
            drop(domain);
        }
    }
    pub fn request(&mut self, request: Request) -> Result<Response, Failure> {
        let result = self.exchange(request);
        if result.is_err() {
            self.abort();
        }
        result
    }
    fn exchange(&mut self, request: Request) -> Result<Response, Failure> {
        let envelope = Envelope {
            instance: self.instance,
            sequence: self.sequence,
        };
        self.sequence = self.sequence.checked_add(1).ok_or(Failure::Protocol)?;
        let domain = self.domain.as_mut().ok_or(Failure::Protocol)?;
        let exchange = domain.exchange().map_err(|_| Failure::Protocol)?;
        exchange.fill(0);
        wire::encode_request(envelope, request, exchange).map_err(|_| Failure::Protocol)?;
        if domain.enter(1).map_err(|_| Failure::Protocol)? != DomainReturn::Yielded {
            return Err(Failure::Protocol);
        }
        let exchange = domain.exchange().map_err(|_| Failure::Protocol)?;
        let (returned, response) =
            wire::decode_response(exchange.get(..wire::BYTES).ok_or(Failure::Protocol)?)
                .map_err(|_| Failure::Protocol)?;
        if returned != envelope || matches!(response, Response::Refused(_)) {
            return Err(Failure::Protocol);
        }
        Ok(response)
    }
    pub fn input(&mut self, bytes: &[u8]) -> Result<(), Failure> {
        let window = self
            .domain
            .as_mut()
            .ok_or(Failure::Protocol)?
            .transfer_window()
            .map_err(|_| Failure::Protocol)?;
        window.fill(0);
        window
            .get_mut(..bytes.len())
            .ok_or(Failure::Protocol)?
            .copy_from_slice(bytes);
        Ok(())
    }
    pub fn output(&self, bytes: &mut [u8]) -> Result<(), Failure> {
        let window = self
            .domain
            .as_ref()
            .ok_or(Failure::Protocol)?
            .output()
            .map_err(|_| Failure::Protocol)?;
        bytes.copy_from_slice(window.get(..bytes.len()).ok_or(Failure::Protocol)?);
        Ok(())
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.abort();
    }
}
