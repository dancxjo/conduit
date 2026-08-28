//! Exact identity retained by remote Cord lifecycle Signs.

use crate::{CordId, RemoteEndpointId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteCordDirection {
    Egress,
    Ingress,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RemoteLifecycleIdentity {
    pub endpoint: RemoteEndpointId,
    pub cord: CordId,
    pub direction: RemoteCordDirection,
    pub sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RemoteLifecycleSign {
    pub event_sequence: u32,
    pub identity: RemoteLifecycleIdentity,
}

pub fn remote_sign_storage_bytes(item_capacity: u16) -> Option<u32> {
    usize::from(item_capacity)
        .checked_mul(core::mem::size_of::<RemoteLifecycleSign>())
        .and_then(|bytes| u32::try_from(bytes).ok())
}
