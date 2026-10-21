//! Exact finite exchange/window custody for one independent TCP lease.
use crate::{
    arch::ServiceDomain,
    protected_region::{DomainBackend, DomainReturn},
    service_memory::{ServiceImage, ServiceProfile},
    virtio_tcp::VirtioTcpError,
};
use conduitos_tcp_core::{
    Refusal,
    wire::{self, Envelope, Request, Response},
};
pub(super) struct Service {
    domain: Option<ServiceDomain>,
    instance: u64,
    sequence: u64,
}
impl Service {
    pub fn install() -> Result<Self, VirtioTcpError> {
        #[cfg(conduitos_tcp_image)]
        let bytes: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tcp.elf"));
        #[cfg(not(conduitos_tcp_image))]
        let bytes: &[u8] = &[];
        let image = ServiceImage::for_profile(bytes, ServiceProfile::Tcp)
            .map_err(|_| VirtioTcpError::ConnectRefused)?;
        let domain = ServiceDomain::install(&image).map_err(|_| VirtioTcpError::ConnectRefused)?;
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
    pub fn request(&mut self, request: Request) -> Result<Response, VirtioTcpError> {
        let result = self.exchange(request);
        if result.is_err() {
            self.abort();
        }
        result
    }
    fn exchange(&mut self, request: Request) -> Result<Response, VirtioTcpError> {
        let envelope = Envelope {
            instance: self.instance,
            sequence: self.sequence,
        };
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(VirtioTcpError::ReceiveRefused)?;
        let domain = self.domain.as_mut().ok_or(VirtioTcpError::ReceiveRefused)?;
        let exchange = domain
            .exchange()
            .map_err(|_| VirtioTcpError::ReceiveRefused)?;
        exchange.fill(0);
        wire::encode_request(envelope, request, exchange)
            .map_err(|_| VirtioTcpError::ReceiveRefused)?;
        if domain
            .enter(1)
            .map_err(|_| VirtioTcpError::ReceiveRefused)?
            != DomainReturn::Yielded
        {
            return Err(VirtioTcpError::ReceiveRefused);
        }
        let exchange = domain
            .exchange()
            .map_err(|_| VirtioTcpError::ReceiveRefused)?;
        let (returned, response) = wire::decode_response(
            exchange
                .get(..wire::BYTES)
                .ok_or(VirtioTcpError::ReceiveRefused)?,
        )
        .map_err(|_| VirtioTcpError::ReceiveRefused)?;
        if returned != envelope {
            return Err(VirtioTcpError::ReceiveRefused);
        }
        if let Response::Refused(refusal) = response {
            return Err(match refusal {
                Refusal::Connect => VirtioTcpError::ConnectRefused,
                Refusal::Send => VirtioTcpError::SendRefused,
                Refusal::Receive => VirtioTcpError::ReceiveRefused,
                Refusal::Closed => VirtioTcpError::RemoteClosed,
                Refusal::Clock => VirtioTcpError::ClockRegressed,
                _ => VirtioTcpError::ReceiveRefused,
            });
        }
        let matches = matches!(
            (request, response),
            (
                Request::Init | Request::Close | Request::Abort | Request::AckFrame { .. },
                Response::Ready
            ) | (
                Request::Poll { .. } | Request::Status,
                Response::Status { .. }
            ) | (Request::TakeFrame { .. }, Response::Frame { .. })
                | (Request::Read { .. }, Response::Read { .. })
                | (Request::Write { .. }, Response::Written { .. })
        );
        if !matches {
            return Err(VirtioTcpError::ReceiveRefused);
        }
        Ok(response)
    }
    pub fn input(&mut self, bytes: &[u8]) -> Result<(), VirtioTcpError> {
        let window = self
            .domain
            .as_mut()
            .ok_or(VirtioTcpError::ReceiveRefused)?
            .transfer_window()
            .map_err(|_| VirtioTcpError::ReceiveRefused)?;
        window.fill(0);
        window
            .get_mut(..bytes.len())
            .ok_or(VirtioTcpError::RequestTooLarge)?
            .copy_from_slice(bytes);
        Ok(())
    }
    pub fn output(&self, bytes: &mut [u8]) -> Result<(), VirtioTcpError> {
        let window = self
            .domain
            .as_ref()
            .ok_or(VirtioTcpError::ReceiveRefused)?
            .output()
            .map_err(|_| VirtioTcpError::ReceiveRefused)?;
        bytes.copy_from_slice(
            window
                .get(..bytes.len())
                .ok_or(VirtioTcpError::ResponseTooLarge)?,
        );
        Ok(())
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.abort();
    }
}
