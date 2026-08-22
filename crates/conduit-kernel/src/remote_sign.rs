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
