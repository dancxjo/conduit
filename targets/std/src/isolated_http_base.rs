//! Exact-endpoint HTTP client Base with an OS-confined provider process.
//!
//! The trusted bootstrap opens one admitted TCP connection and transfers only
//! that connected descriptor. The provider cannot create sockets or access the
//! filesystem after installing Landlock and seccomp.

use conduit_core::{BaseOperationClaim, CapabilityIssueRequest};
use serde::{Deserialize, Serialize};

mod adversarial;
mod host;
mod provider;

pub use adversarial::{run_adversarial_http_proof, AdversarialHttpReport};
pub use host::{
    isolated_http_client_offer, IsolatedHttpBaseConfig, IsolatedHttpBaseInspection,
    IsolatedHttpExchangeReceipt, IsolatedHttpHost,
};
pub use provider::provider_main;

pub const ISOLATED_HTTP_IMPLEMENTATION: &str = "std/isolated-http-client-http1@1";
pub const ISOLATED_HTTP_PROFILE: &str = "std/http1-exact-endpoint-process-isolated@1";
pub const ISOLATED_HTTP_ARTIFACT: &str = "conduit-base-http/exact-endpoint-http1@1";
pub const HTTP_CLIENT_OPERATION: &str = "conduit.host/http-client-exchange@1";
pub const HTTP_CLIENT_RESOURCE: &str = "conduit.resource/network/http-client@1";
pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_FRAME_BYTES: usize = 65_536;
pub const CONNECTED_SOCKET_FD: i32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpBootstrap {
    pub issue: CapabilityIssueRequest,
    pub expected_authority: String,
    pub expected_endpoint: std::net::SocketAddr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum RequestFrame {
    Bootstrap {
        protocol_version: u16,
        bootstrap: Box<HttpBootstrap>,
    },
    Exchange {
        claim: Box<BaseOperationClaim>,
        request: Vec<u8>,
    },
    Revoke,
    ProbeRawSocket,
    ProbeListener,
    ProbeProcessSpawn,
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum ResponseFrame {
    Ready {
        protocol_version: u16,
        enforcement: String,
        environment_entries: u16,
        descriptor_ceiling: u16,
        peer: std::net::SocketAddr,
    },
    Completed {
        response: Vec<u8>,
    },
    Refused {
        reason: String,
    },
    Revoked,
    ProbeDenied {
        probe: String,
        os_error: i32,
    },
    Stopped,
}
