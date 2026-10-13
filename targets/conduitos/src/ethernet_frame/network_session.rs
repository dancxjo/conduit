//! Root-approved retained use of the existing stack over the exact frame queues.
//! No USB, class, routing discovery, implicit permission, or new scheduler.
use super::{
    device::EthernetFrameDevice,
    queues::{EthernetFrameQueues, FrameRefusal},
};
use crate::virtio_tcp::{
    TcpExchange, TcpExchangeOptions, VirtioTcpEndpoint, VirtioTcpError, VirtioTcpReceipt,
};
use conduit_core::{BootId, HostId};
use smoltcp::{iface::SocketStorage, time::Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EthernetNetworkScope {
    pub mac: [u8; 6],
    pub endpoint: VirtioTcpEndpoint,
    pub maximum_polls: u32,
}

/// Move-only Root possession assertion. Descriptive grants cannot construct it.
/// The independent selected frame Base additionally needs its cryptographic lease.
pub struct EthernetNetworkApproval {
    host_id: HostId,
    boot_id: BootId,
    scope: EthernetNetworkScope,
}
impl EthernetNetworkApproval {
    /// # Safety
    /// The trusted Root has actual administrator approval to configure exactly
    /// this MAC/local address/prefix/gateway and connect exactly this remote
    /// address/port with these finite bounds. Root retains that approval throughout
    /// Play and must Stop it on revocation. USB discovery, source declarations,
    /// descriptive AuthorityGrants, planning and matching fields grant none of it.
    pub unsafe fn admit(
        host_id: HostId,
        boot_id: BootId,
        scope: EthernetNetworkScope,
    ) -> Result<Self, EthernetNetworkRefusal> {
        let endpoint = scope.endpoint;
        if host_id.as_str().is_empty()
            || boot_id.as_str().is_empty()
            || scope.mac == [0; 6]
            || scope.mac[0] & 1 != 0
            || scope.maximum_polls == 0
            || endpoint.guest_address == [0; 4]
            || endpoint.gateway == [0; 4]
            || endpoint.remote_address == [0; 4]
            || endpoint.remote_port == 0
            || endpoint.local_port == 0
            || !(1..=32).contains(&endpoint.prefix_length)
        {
            return Err(EthernetNetworkRefusal::Approval);
        }
        Ok(Self {
            host_id,
            boot_id,
            scope,
        })
    }
    pub fn host_id(&self) -> &HostId {
        &self.host_id
    }
    pub fn boot_id(&self) -> &BootId {
        &self.boot_id
    }
    pub const fn scope(&self) -> EthernetNetworkScope {
        self.scope
    }
}

/// All borrowed storage is fixed and installed before Play. Socket storage must
/// be exactly one vacant existing smoltcp socket; the existing adapter verifies it.
pub struct EthernetNetworkStorage<'a> {
    pub request: &'a [u8],
    pub response: &'a mut [u8],
    pub expected_response_bytes: usize,
    pub receive: &'a mut [u8],
    pub transmit: &'a mut [u8],
    pub sockets: &'a mut [SocketStorage<'a>],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EthernetNetworkRefusal {
    Approval,
    Queue(FrameRefusal),
    Tcp(VirtioTcpError),
}

pub struct PreparedEthernetNetwork<'a> {
    approval: EthernetNetworkApproval,
    device: EthernetFrameDevice<'a>,
    exchange: TcpExchange<'a>,
    terminal: Option<Result<VirtioTcpReceipt, EthernetNetworkRefusal>>,
}
impl<'a> PreparedEthernetNetwork<'a> {
    pub fn prepare(
        queues: &'a EthernetFrameQueues,
        approval: EthernetNetworkApproval,
        storage: EthernetNetworkStorage<'a>,
        random_seed: u64,
        timestamp: Instant,
    ) -> Result<Self, EthernetNetworkRefusal> {
        if approval.scope.mac != queues.mac() {
            return Err(EthernetNetworkRefusal::Approval);
        }
        queues
            .transmit_ready(queues.generation())
            .map_err(EthernetNetworkRefusal::Queue)?;
        let mut device = EthernetFrameDevice::prepare(queues);
        let exchange = TcpExchange::prepare(
            &mut device,
            TcpExchangeOptions {
                random_seed,
                mac: approval.scope.mac,
                endpoint: approval.scope.endpoint,
                maximum_polls: approval.scope.maximum_polls,
                timestamp,
            },
            storage.request,
            storage.response,
            storage.expected_response_bytes,
            storage.receive,
            storage.transmit,
            storage.sockets,
        )
        .map_err(EthernetNetworkRefusal::Tcp)?;
        if let Some(error) = device.take_error() {
            return Err(EthernetNetworkRefusal::Queue(error));
        }
        Ok(Self {
            approval,
            device,
            exchange,
            terminal: None,
        })
    }
    /// One existing bounded stack step, invoked by the ordinary Play lifecycle.
    /// The caller supplies its real admitted clock and interleaves frame Calls;
    /// this adapter never drives another kernel, waits, retries or allocates.
    pub fn poll_one(
        &mut self,
        timestamp: Instant,
    ) -> Result<Option<VirtioTcpReceipt>, EthernetNetworkRefusal> {
        if let Some(result) = self.terminal {
            return result.map(Some);
        }
        let result = self
            .exchange
            .poll_step(&mut self.device, timestamp)
            .map_err(EthernetNetworkRefusal::Tcp);
        let result = match self.device.take_error() {
            Some(error) => Err(EthernetNetworkRefusal::Queue(error)),
            None => result,
        };
        match result {
            Ok(Some(receipt)) => self.terminal = Some(Ok(receipt)),
            Err(error) => {
                self.exchange.cancel();
                self.terminal = Some(Err(error));
            }
            Ok(None) => {}
        }
        result
    }
    pub const fn scope(&self) -> EthernetNetworkScope {
        self.approval.scope
    }
    pub fn received(&self) -> &[u8] {
        self.exchange.received()
    }
    pub fn polls(&self) -> u32 {
        self.exchange.polls()
    }
    pub fn cancel(&mut self) {
        if self.terminal.is_none() {
            self.exchange.cancel();
            self.terminal = Some(Err(EthernetNetworkRefusal::Tcp(VirtioTcpError::Cancelled)));
        }
    }
}
#[cfg(test)]
mod tests;
