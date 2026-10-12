//! Private stable-box projection for ordinary retained queue/stack ownership.
//! No leaked allocation, global queue, escaping static reference or USB DMA.
use super::{
    contract::EthernetFrameContract, installation::*, network_session::*,
    queues::EthernetFrameQueues,
};
use alloc::boxed::Box;
use conduit_core::{BaseCapabilityHandle, BaseCapabilityTable, BaseOperationClaim};
use conduit_kernel::{HostCallId, NodeId, RequestId};
use core::ptr::NonNull;
use smoltcp::{iface::SocketStorage, time::Instant};
const TCP_BYTES: usize = 1024;

pub struct OwnedEthernetNetworkInput<'a> {
    pub request: &'a [u8],
    pub maximum_response_bytes: u16,
    pub expected_response_bytes: u16,
}
struct OwnedStorage(NonNull<Storage>);
impl OwnedStorage {
    fn pointer(&self) -> *mut Storage {
        self.0.as_ptr()
    }
}
impl Drop for OwnedStorage {
    fn drop(&mut self) {
        // All private Ready/session references have already been destroyed.
        // SocketSet borrows storage and can leave sockets in its slots; clear
        // those borrowed buffer references before recreating exclusive Box ownership.
        unsafe {
            (*self.pointer()).sockets = [SocketStorage::EMPTY];
            drop(Box::from_raw(self.pointer()));
        }
    }
}
struct Storage {
    queues: EthernetFrameQueues,
    request: [u8; TCP_BYTES],
    response: [u8; TCP_BYTES],
    receive: [u8; TCP_BYTES],
    transmit: [u8; TCP_BYTES],
    sockets: [SocketStorage<'static>; 1],
    request_bytes: usize,
    response_bytes: usize,
    expected_response_bytes: usize,
}
impl Storage {
    fn prepare(
        identity: &EthernetFrameNativeIdentity,
        approval: &EthernetNetworkApproval,
        input: OwnedEthernetNetworkInput<'_>,
    ) -> Result<OwnedStorage, EthernetFrameNativeRefusal> {
        if input.request.is_empty()
            || input.request.len() > TCP_BYTES
            || input.maximum_response_bytes == 0
            || usize::from(input.maximum_response_bytes) > TCP_BYTES
            || input.expected_response_bytes == 0
            || input.expected_response_bytes > input.maximum_response_bytes
        {
            return Err(EthernetFrameNativeRefusal::Network(
                EthernetNetworkRefusal::Approval,
            ));
        }
        let queues =
            EthernetFrameQueues::prepare(identity.provider_generation, approval.scope().mac)
                .map_err(|e| {
                    EthernetFrameNativeRefusal::Network(EthernetNetworkRefusal::Queue(e))
                })?;
        let mut storage = Box::new(Self {
            queues,
            request: [0; TCP_BYTES],
            response: [0; TCP_BYTES],
            receive: [0; TCP_BYTES],
            transmit: [0; TCP_BYTES],
            sockets: [SocketStorage::EMPTY],
            request_bytes: input.request.len(),
            response_bytes: input.maximum_response_bytes.into(),
            expected_response_bytes: input.expected_response_bytes.into(),
        });
        storage.request[..input.request.len()].copy_from_slice(input.request);
        Ok(OwnedStorage(
            NonNull::new(Box::into_raw(storage)).expect("owned allocation"),
        ))
    }
    /// # Safety
    /// The allocation remains owned at this exact stable address while returned
    /// references exist. Only the private container may retain them; no storage
    /// field may be accessed except through its disjoint session projection.
    /// The session is destroyed before any field or allocation is destroyed.
    unsafe fn network(pointer: *mut Self) -> EthernetNetworkStorage<'static> {
        // SAFETY: caller retains this stable Box. Distinct fields never alias;
        // lengths were admitted and cannot change while the session exists.
        unsafe {
            EthernetNetworkStorage {
                request: core::slice::from_raw_parts(
                    core::ptr::addr_of!((*pointer).request).cast::<u8>(),
                    (*pointer).request_bytes,
                ),
                response: core::slice::from_raw_parts_mut(
                    core::ptr::addr_of_mut!((*pointer).response).cast::<u8>(),
                    (*pointer).response_bytes,
                ),
                expected_response_bytes: (*pointer).expected_response_bytes,
                receive: &mut (*pointer).receive,
                transmit: &mut (*pointer).transmit,
                sockets: &mut (*pointer).sockets,
            }
        }
    }
}

/// The borrowed Ready is private; its apparent static projection cannot escape.
/// Moving this container moves only owning pointers, never their fixed storage.
pub struct ReadyOwnedEthernetFrameBase {
    ready: Option<ReadyEthernetFrameBase<'static>>,
    storage: OwnedStorage,
}
impl ReadyOwnedEthernetFrameBase {
    /// # Safety
    /// Root possesses the exact independently approved network Base/resources.
    /// Approval describes actual administrator permission, not discovery/grants.
    pub unsafe fn prepare(
        identity: EthernetFrameNativeIdentity,
        approval: EthernetNetworkApproval,
        input: OwnedEthernetNetworkInput<'_>,
    ) -> Result<Self, EthernetFrameNativeRefusal> {
        let storage = Storage::prepare(&identity, &approval, input)?;
        let pointer = storage.pointer();
        // SAFETY: private Ready retains the stable allocation; neither queues
        // nor its static lifetime can escape. Ready is dropped before storage.
        let ready = unsafe { ReadyEthernetFrameBase::new(identity, &(*pointer).queues, approval) }?;
        Ok(Self {
            ready: Some(ready),
            storage,
        })
    }
    pub fn identity(&self) -> &EthernetFrameNativeIdentity {
        self.ready.as_ref().expect("retained Ready").identity()
    }
    pub fn scope(&self) -> EthernetNetworkScope {
        self.ready.as_ref().expect("retained Ready").scope()
    }
    pub fn resource_bytes(&self) -> u64 {
        core::mem::size_of::<Storage>() as u64
    }
    pub fn append_to_advertisement(
        &self,
        advertisement: &mut conduit_core::HostAdvertisement,
        contract: &EthernetFrameContract,
    ) -> Result<(), EthernetFrameNativeRefusal> {
        self.ready
            .as_ref()
            .expect("retained Ready")
            .append_to_advertisement(advertisement, contract)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn bind_selected(
        mut self,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &EthernetFrameContract,
        selection: EthernetFrameSelection<'_>,
        random_seed: u64,
        timestamp: Instant,
    ) -> Result<OwnedNativeEthernetFrameHost, EthernetFrameNativeRefusal> {
        let ready = self.ready.take().expect("retained Ready");
        let pointer = self.storage.pointer();
        // SAFETY: private host retains all projections and is destroyed before
        // this allocation. Ready's shared queue and mutable TCP fields disjoint.
        let network = unsafe { Storage::network(pointer) };
        let host = ready.bind_selected(
            table,
            handle,
            claim,
            contract,
            selection,
            network,
            random_seed,
            timestamp,
        )?;
        Ok(OwnedNativeEthernetFrameHost {
            host: Some(host),
            storage: self.storage,
        })
    }
}

/// Owner of every queue/socket/octet allocation, with no public inner host.
/// USB transfers copy canonical frame bytes into separately owned native DMA;
/// this container never owns or retires USB DMA/controller resources.
pub struct OwnedNativeEthernetFrameHost {
    host: Option<NativeEthernetFrameHost<'static>>,
    storage: OwnedStorage,
}
impl OwnedNativeEthernetFrameHost {
    pub fn invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
    ) -> Result<&[u8], EthernetFrameNativeRefusal> {
        self.host
            .as_mut()
            .expect("retained host")
            .invoke(node, call, request, input)
    }
    pub fn poll_network(
        &mut self,
        timestamp: Instant,
    ) -> Result<Option<crate::virtio_tcp::VirtioTcpReceipt>, EthernetFrameNativeRefusal> {
        self.host
            .as_mut()
            .expect("retained host")
            .poll_network(timestamp)
    }
    pub fn received(&self) -> &[u8] {
        self.host.as_ref().expect("retained host").received()
    }
    pub fn polls(&self) -> u32 {
        self.host.as_ref().expect("retained host").polls()
    }
    pub fn scope(&self) -> EthernetNetworkScope {
        self.host.as_ref().expect("retained host").scope()
    }
    /// Failure retains host, leases and all owned storage for Root to inspect
    /// and retry retirement; it grants no Lull. Root must separately stop USB,
    /// retaining the aggregate Play and native DMA on unacknowledged USB stop.
    pub fn cancel(&mut self) -> Result<(), EthernetFrameNativeRefusal> {
        self.host.as_mut().expect("retained host").cancel()
    }
    pub fn resource_bytes(&self) -> u64 {
        core::mem::size_of::<Storage>() as u64
    }
}
impl Drop for OwnedNativeEthernetFrameHost {
    fn drop(&mut self) {
        // Memory cleanup is not acknowledged physical Stop or a Lull proof.
        // Abort the local socket, revoke where possible, then destroy all private
        // references before freeing queues or any socket/octet backing storage.
        if let Some(mut host) = self.host.take() {
            let _ = host.cancel();
            drop(host);
        }
        // SAFETY: the inner host has been destroyed, so no queue borrow escapes.
        unsafe {
            (*self.storage.pointer()).queues.cancel();
        }
    }
}
#[cfg(test)]
mod tests;
