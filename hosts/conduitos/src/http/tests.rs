use super::*;
use conduit_std_catalog::{
    HttpMethod, HttpRequest, HttpResponse, HttpTarget, HttpTransactionId, decode_response,
    encode_request,
};

struct LocalEndpoint {
    response: &'static [u8],
    failure: Option<NetworkFailure>,
    seen: [u8; 512],
    seen_len: usize,
}

impl HttpNetworkBase for LocalEndpoint {
    fn exchange(&mut self, request: &[u8], response: &mut [u8]) -> Result<usize, NetworkFailure> {
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        self.seen_len = request.len();
        self.seen[..request.len()].copy_from_slice(request);
        response[..self.response.len()].copy_from_slice(self.response);
        Ok(self.response.len())
    }
}

fn request(scheme: &str, authority: &str) -> alloc::vec::Vec<u8> {
    encode_request(&HttpRequest {
        transaction_id: HttpTransactionId(44),
        method: HttpMethod::Post,
        target: HttpTarget {
            scheme: scheme.into(),
            authority: authority.into(),
            path_and_query: "/v1/check?q=1".into(),
        },
        headers: alloc::vec![conduit_std_catalog::HttpHeader {
            name: "x-test".into(),
            value: b"yes".to_vec(),
        }],
        body: b"hello".to_vec(),
    })
    .unwrap()
}

fn endpoint(response: &'static [u8]) -> LocalEndpoint {
    LocalEndpoint {
        response,
        failure: None,
        seen: [0; 512],
        seen_len: 0,
    }
}

#[test]
fn exact_offer_is_finite_and_requires_narrow_authority() {
    let offer = offer();
    assert_eq!(
        offer.kind_id.as_str(),
        conduit_std_catalog::HTTP_CLIENT_KIND
    );
    assert_eq!(offer.host_operations.len(), 1);
    assert_eq!(
        offer.resource_requirements[0].class_id.as_str(),
        RESOURCE_CLASS
    );
    assert_eq!(
        offer.authority_requirements[0].contract_id.as_str(),
        AUTHORITY
    );
    assert_eq!(offer.host_operations[0].maximum_in_flight, 1);
    assert_eq!(PACKET_BUFFERS, 4);
    assert_eq!(SOCKET_SLOTS, 1);
    assert_eq!(TIMER_SLOTS, 2);
    assert_eq!(SIGN_ITEMS, 32);
}

#[test]
fn request_encoding_and_response_parsing_preserve_status_as_data() {
    let mut client = NativeHttpClient::prepare();
    let mut output = FixedHttpOutput::new();
    let mut endpoint =
        endpoint(b"HTTP/1.1 500 Internal\r\ncontent-length: 4\r\nx-one: a\r\n\r\noops");
    client
        .exchange(
            &request("http", "192.0.2.1:8080"),
            true,
            &mut endpoint,
            &mut output,
        )
        .unwrap();
    assert_eq!(
        &endpoint.seen[..endpoint.seen_len],
        b"POST /v1/check?q=1 HTTP/1.1\r\nHost: 192.0.2.1:8080\r\nx-test: yes\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello"
    );
    let response = decode_response(output.as_bytes()).unwrap();
    assert_eq!(response.transaction_id, HttpTransactionId(44));
    assert_eq!(response.status, 500);
    assert_eq!(response.body, b"oops");
    assert_eq!(client.sign_count(), 3);
}

#[test]
fn unsupported_name_tls_and_missing_authority_refuse_before_network_use() {
    for (encoded, refusal) in [
        (
            request("http", "api.example.test:80"),
            HttpClientFailure::NameResolutionUnsupported,
        ),
        (
            request("https", "192.0.2.1:443"),
            HttpClientFailure::TlsUnsupported,
        ),
    ] {
        let mut client = NativeHttpClient::prepare();
        let mut output = FixedHttpOutput::new();
        let mut endpoint = endpoint(b"");
        assert_eq!(
            client.exchange(&encoded, true, &mut endpoint, &mut output),
            Err(refusal)
        );
        assert_eq!(endpoint.seen_len, 0);
    }
    assert_eq!(
        NativeHttpClient::prepare().begin(&request("http", "192.0.2.1:80"), false),
        Err(HttpClientFailure::AuthorityDenied)
    );
}

#[test]
fn pressure_cancellation_and_stale_completion_are_distinct() {
    let mut client = NativeHttpClient::prepare();
    let encoded = request("http", "192.0.2.1:80");
    let ticket = client.begin(&encoded, true).unwrap();
    assert_eq!(
        client.begin(&encoded, true),
        Err(HttpClientFailure::Pressure)
    );
    client.cancel().unwrap();
    assert_eq!(
        client.request_bytes(ticket),
        Err(HttpClientFailure::StaleCompletion)
    );
    assert_eq!(
        client.complete(ticket, 0, &mut FixedHttpOutput::new()),
        Err(HttpClientFailure::StaleCompletion)
    );
}

#[test]
fn base_connect_provider_close_and_overflow_failures_do_not_become_statuses() {
    for (failure, expected) in [
        (NetworkFailure::Connect, HttpClientFailure::Connect),
        (NetworkFailure::BaseLost, HttpClientFailure::BaseLost),
        (
            NetworkFailure::ProviderLost,
            HttpClientFailure::ProviderLost,
        ),
    ] {
        let mut endpoint = endpoint(b"");
        endpoint.failure = Some(failure);
        assert_eq!(
            NativeHttpClient::prepare().exchange(
                &request("http", "192.0.2.1:80"),
                true,
                &mut endpoint,
                &mut FixedHttpOutput::new(),
            ),
            Err(expected)
        );
    }
    let mut closed = endpoint(b"HTTP/1.1 200 OK\r\ncontent-length: 4\r\n\r\nno");
    assert_eq!(
        NativeHttpClient::prepare().exchange(
            &request("http", "192.0.2.1:80"),
            true,
            &mut closed,
            &mut FixedHttpOutput::new(),
        ),
        Err(HttpClientFailure::ProviderLost)
    );

    let oversized = alloc::format!(
        "HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n{}",
        conduit_std_catalog::HTTP_MAXIMUM_RESPONSE_BODY_BYTES + 1,
        "x".repeat(conduit_std_catalog::HTTP_MAXIMUM_RESPONSE_BODY_BYTES + 1)
    );
    let leaked: &'static [u8] = alloc::boxed::Box::leak(oversized.into_bytes().into_boxed_slice());
    let mut endpoint = endpoint(leaked);
    assert_eq!(
        NativeHttpClient::prepare().exchange(
            &request("http", "192.0.2.1:80"),
            true,
            &mut endpoint,
            &mut FixedHttpOutput::new(),
        ),
        Err(HttpClientFailure::ResponseBodyOverflow)
    );
}

#[test]
fn semantic_response_matches_the_shared_http_contract() {
    let response = HttpResponse {
        transaction_id: HttpTransactionId(44),
        status: 204,
        headers: alloc::vec::Vec::new(),
        body: alloc::vec::Vec::new(),
    };
    assert!(conduit_std_catalog::encode_response(&response).is_ok());
}
