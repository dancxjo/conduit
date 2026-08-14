//! Hosted HTTP platform-effect adapter. It does not own scheduling or policy.

use crate::hosted_http::{HostedHttpClient, HostedHttpListener};
use conduit_std_catalog::{
    decode_request, decode_response, encode_request, encode_response, HttpExchangeFailure,
    HttpServerResponseRefusal,
};

pub(super) struct InstalledHttpHost {
    pub(super) client: Option<HostedHttpClient>,
    pub(super) server: Option<HostedHttpListener>,
}

impl InstalledHttpHost {
    pub(super) fn prepare(fragment: &conduit_core::PlanFragment) -> Result<Self, String> {
        let client = fragment
            .placements
            .iter()
            .any(|placement| {
                placement.implementation_id.as_str() == super::http::CLIENT_IMPLEMENTATION
            })
            .then(|| HostedHttpClient::new(conduit_std_catalog::HTTP_MAXIMUM_IN_FLIGHT))
            .transpose()
            .map_err(|failure| format!("prepare hosted HTTP client: {failure:?}"))?;
        let server = fragment
            .placements
            .iter()
            .any(|placement| {
                placement.implementation_id.as_str() == super::http::SERVER_IMPLEMENTATION
            })
            .then(HostedHttpListener::bind_loopback)
            .transpose()
            .map_err(|failure| format!("prepare hosted HTTP listener: {failure:?}"))?;
        Ok(Self { client, server })
    }

    pub(super) fn listener_address(&self) -> Result<Option<std::net::SocketAddr>, String> {
        self.server
            .as_ref()
            .map(HostedHttpListener::local_addr)
            .transpose()
            .map_err(|failure| format!("inspect hosted HTTP listener: {failure:?}"))
    }

    pub(super) fn cancel(&mut self) {
        self.client = None;
        self.server = None;
    }

    pub(super) fn execute(
        &mut self,
        contract: &str,
        input: &[u8],
        output: &mut Vec<u8>,
    ) -> Result<(), HostedHttpFailure> {
        output.clear();
        match contract {
            super::http::CLIENT_OPERATION => {
                let request = decode_request(input).map_err(|_| {
                    HostedHttpFailure::Exchange(HttpExchangeFailure::RequestOverflow)
                })?;
                let response = self
                    .client
                    .as_mut()
                    .ok_or(HostedHttpFailure::Exchange(
                        HttpExchangeFailure::ProviderLost,
                    ))?
                    .exchange(&request)
                    .map_err(HostedHttpFailure::Exchange)?;
                output.extend_from_slice(&encode_response(&response).map_err(|_| {
                    HostedHttpFailure::Exchange(HttpExchangeFailure::ResponseBodyOverflow)
                })?);
                Ok(())
            }
            super::http::SERVER_ACCEPT_OPERATION => {
                let request = self
                    .server
                    .as_mut()
                    .ok_or(HostedHttpFailure::Server(
                        HttpServerResponseRefusal::ListenerLost,
                    ))?
                    .accept_request()
                    .map_err(HostedHttpFailure::Server)?;
                output.extend_from_slice(
                    &encode_request(&request).map_err(|_| {
                        HostedHttpFailure::Server(HttpServerResponseRefusal::Capacity)
                    })?,
                );
                Ok(())
            }
            super::http::SERVER_RESPOND_OPERATION => {
                let response = decode_response(input).map_err(|_| {
                    HostedHttpFailure::Server(HttpServerResponseRefusal::ResponseBodyOverflow)
                })?;
                self.server
                    .as_mut()
                    .ok_or(HostedHttpFailure::Server(
                        HttpServerResponseRefusal::ListenerLost,
                    ))?
                    .send_response(&response)
                    .map_err(HostedHttpFailure::Server)
            }
            _ => Err(HostedHttpFailure::UnsupportedContract),
        }
    }
}

#[derive(Debug)]
pub(super) enum HostedHttpFailure {
    Exchange(HttpExchangeFailure),
    Server(HttpServerResponseRefusal),
    UnsupportedContract,
}

impl HostedHttpFailure {
    pub(super) fn detail(&self) -> u16 {
        match self {
            Self::Exchange(value) => 100 + *value as u16,
            Self::Server(value) => 200 + *value as u16,
            Self::UnsupportedContract => 299,
        }
    }
}
