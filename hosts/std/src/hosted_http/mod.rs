//! Finite hosted realization primitives for the portable HTTP semantic waist.
//!
//! This module owns hosted HTTP/1.1 mechanism only. Planning, authority, and
//! kernel correlation remain outside it.

mod wire;

use conduit_std_catalog::{
    HttpExchangeFailure, HttpRequest, HttpResponse, HttpServerResponseRefusal,
    HttpServerTransactions, HttpTransactionId, HTTP_MAXIMUM_IN_FLIGHT,
};
use std::io::Write;
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::time::Duration;

const IO_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug)]
pub struct HostedHttpClient {
    maximum_in_flight: u16,
    active: u16,
}

impl HostedHttpClient {
    pub fn new(maximum_in_flight: u16) -> Result<Self, HttpExchangeFailure> {
        if maximum_in_flight == 0 || maximum_in_flight > HTTP_MAXIMUM_IN_FLIGHT {
            return Err(HttpExchangeFailure::Capacity);
        }
        Ok(Self {
            maximum_in_flight,
            active: 0,
        })
    }

    pub fn exchange(&mut self, request: &HttpRequest) -> Result<HttpResponse, HttpExchangeFailure> {
        request
            .validate()
            .map_err(|_| HttpExchangeFailure::RequestOverflow)?;
        if self.active == self.maximum_in_flight {
            return Err(HttpExchangeFailure::Capacity);
        }
        if request.target.scheme != "http" {
            return Err(HttpExchangeFailure::Tls);
        }
        self.active += 1;
        let result = exchange(request);
        self.active -= 1;
        result
    }
}

fn exchange(request: &HttpRequest) -> Result<HttpResponse, HttpExchangeFailure> {
    let address = resolve_authority(&request.target.authority)?;
    let mut stream = TcpStream::connect_timeout(&address, IO_TIMEOUT)
        .map_err(|_| HttpExchangeFailure::Connect)?;
    stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .and_then(|()| stream.set_write_timeout(Some(IO_TIMEOUT)))
        .map_err(|_| HttpExchangeFailure::ProviderLost)?;
    let encoded = wire::encode_request(request)?;
    stream
        .write_all(&encoded)
        .map_err(|_| HttpExchangeFailure::ProviderLost)?;
    stream
        .flush()
        .map_err(|_| HttpExchangeFailure::ProviderLost)?;
    wire::read_response(&mut stream, request.transaction_id)
}

fn resolve_authority(authority: &str) -> Result<SocketAddr, HttpExchangeFailure> {
    authority
        .to_socket_addrs()
        .map_err(|_| HttpExchangeFailure::NameResolution)?
        .next()
        .ok_or(HttpExchangeFailure::RouteUnavailable)
}

#[derive(Debug)]
pub struct HostedHttpListener {
    listener: TcpListener,
    transactions: HttpServerTransactions,
    next_transaction: u64,
    pending: Vec<PendingConnection>,
}

#[derive(Debug)]
struct PendingConnection {
    transaction_id: HttpTransactionId,
    stream: TcpStream,
}

impl HostedHttpListener {
    pub fn bind_loopback() -> Result<Self, HttpServerResponseRefusal> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .map_err(|_| HttpServerResponseRefusal::AuthorityDenied)?;
        Ok(Self {
            listener,
            transactions: HttpServerTransactions::new(),
            next_transaction: 0,
            pending: Vec::with_capacity(HTTP_MAXIMUM_IN_FLIGHT as usize),
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr, HttpServerResponseRefusal> {
        self.listener
            .local_addr()
            .map_err(|_| HttpServerResponseRefusal::ListenerLost)
    }

    pub fn accept_request(&mut self) -> Result<HttpRequest, HttpServerResponseRefusal> {
        if self.pending.len() == HTTP_MAXIMUM_IN_FLIGHT as usize {
            return Err(HttpServerResponseRefusal::Capacity);
        }
        let (mut stream, _) = self
            .listener
            .accept()
            .map_err(|_| HttpServerResponseRefusal::ListenerLost)?;
        stream
            .set_read_timeout(Some(IO_TIMEOUT))
            .and_then(|()| stream.set_write_timeout(Some(IO_TIMEOUT)))
            .map_err(|_| HttpServerResponseRefusal::ListenerLost)?;
        let transaction_id = HttpTransactionId(self.next_transaction);
        self.next_transaction = self
            .next_transaction
            .checked_add(1)
            .ok_or(HttpServerResponseRefusal::Capacity)?;
        let request =
            wire::read_request(&mut stream, transaction_id).map_err(map_server_wire_failure)?;
        self.transactions.admit_request(transaction_id)?;
        self.pending.push(PendingConnection {
            transaction_id,
            stream,
        });
        Ok(request)
    }

    pub fn send_response(
        &mut self,
        response: &HttpResponse,
    ) -> Result<(), HttpServerResponseRefusal> {
        response.validate().map_err(|error| match error {
            conduit_std_catalog::HttpContractError::ResponseBodyOverflow => {
                HttpServerResponseRefusal::ResponseBodyOverflow
            }
            _ => HttpServerResponseRefusal::ResponseHeaderOverflow,
        })?;
        self.transactions.accept_response(response.transaction_id)?;
        let index = self
            .pending
            .iter()
            .position(|entry| entry.transaction_id == response.transaction_id)
            .ok_or(HttpServerResponseRefusal::UnknownTransaction)?;
        let mut connection = self.pending.swap_remove(index);
        let encoded = wire::encode_response(response).map_err(map_server_wire_failure)?;
        connection
            .stream
            .write_all(&encoded)
            .and_then(|()| connection.stream.flush())
            .map_err(|_| HttpServerResponseRefusal::ListenerLost)
    }

    pub fn cancel(
        &mut self,
        transaction_id: HttpTransactionId,
    ) -> Result<(), HttpServerResponseRefusal> {
        self.transactions.cancel(transaction_id)?;
        if let Some(index) = self
            .pending
            .iter()
            .position(|entry| entry.transaction_id == transaction_id)
        {
            self.pending.swap_remove(index);
        }
        Ok(())
    }
}

fn map_server_wire_failure(failure: HttpExchangeFailure) -> HttpServerResponseRefusal {
    match failure {
        HttpExchangeFailure::RequestOverflow => HttpServerResponseRefusal::Capacity,
        HttpExchangeFailure::ResponseHeaderOverflow => {
            HttpServerResponseRefusal::ResponseHeaderOverflow
        }
        HttpExchangeFailure::ResponseBodyOverflow => {
            HttpServerResponseRefusal::ResponseBodyOverflow
        }
        HttpExchangeFailure::Cancelled => HttpServerResponseRefusal::Cancelled,
        _ => HttpServerResponseRefusal::ListenerLost,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_std_catalog::{HttpHeader, HttpMethod, HttpTarget};
    use std::thread;

    #[test]
    fn real_loopback_preserves_status_body_and_correlation() {
        let mut listener = HostedHttpListener::bind_loopback().unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let request = listener.accept_request().unwrap();
            assert_eq!(request.method, HttpMethod::Post);
            assert_eq!(request.body.as_inline(), Some(b"bounded".as_slice()));
            listener
                .send_response(&HttpResponse {
                    transaction_id: request.transaction_id,
                    status: 503,
                    headers: vec![HttpHeader {
                        name: "content-type".into(),
                        value: b"text/plain".to_vec(),
                    }],
                    body: conduit_std_catalog::HttpBody::inline(b"still HTTP data".to_vec()),
                })
                .unwrap();
        });
        let mut client = HostedHttpClient::new(1).unwrap();
        let response = client
            .exchange(&HttpRequest {
                transaction_id: HttpTransactionId(77),
                method: HttpMethod::Post,
                target: HttpTarget {
                    scheme: "http".into(),
                    authority: address.to_string(),
                    path_and_query: "/fixture".into(),
                },
                headers: Vec::new(),
                body: conduit_std_catalog::HttpBody::inline(b"bounded".to_vec()),
            })
            .unwrap();
        server.join().unwrap();
        assert_eq!(response.transaction_id, HttpTransactionId(77));
        assert_eq!(response.status, 503);
        assert_eq!(
            response.body.as_inline(),
            Some(b"still HTTP data".as_slice())
        );
    }

    #[test]
    fn unsupported_tls_and_unreachable_provider_are_distinct() {
        let mut client = HostedHttpClient::new(1).unwrap();
        let request = |scheme: &str, authority: String| HttpRequest {
            transaction_id: HttpTransactionId(1),
            method: HttpMethod::Get,
            target: HttpTarget {
                scheme: scheme.into(),
                authority,
                path_and_query: "/".into(),
            },
            headers: Vec::new(),
            body: conduit_std_catalog::HttpBody::inline(Vec::new()),
        };
        assert_eq!(
            client.exchange(&request("https", "example.test:443".into())),
            Err(HttpExchangeFailure::Tls)
        );
        let unavailable = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let authority = unavailable.local_addr().unwrap().to_string();
        drop(unavailable);
        assert_eq!(
            client.exchange(&request("http", authority)),
            Err(HttpExchangeFailure::Connect)
        );
        assert_eq!(
            client.exchange(&request("http", "not a socket authority".into())),
            Err(HttpExchangeFailure::NameResolution)
        );
    }

    #[test]
    fn listener_refuses_more_than_the_admitted_pending_capacity() {
        let mut listener = HostedHttpListener::bind_loopback().unwrap();
        let address = listener.local_addr().unwrap();
        let mut clients = Vec::new();
        for _ in 0..HTTP_MAXIMUM_IN_FLIGHT {
            let mut stream = TcpStream::connect(address).unwrap();
            stream
                .write_all(
                    b"GET / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .unwrap();
            clients.push(stream);
            listener.accept_request().unwrap();
        }

        assert_eq!(
            listener.accept_request(),
            Err(HttpServerResponseRefusal::Capacity)
        );
        drop(clients);
    }
}
