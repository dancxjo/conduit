//! Public API lifetime regression: equal transaction/bytes do not confer custody.
use conduit_web::{HttpBody, HttpMethod, HttpRequest, HttpScheme, HttpTarget, HttpTransactionId};
use conduitos::http::{HttpClientFailure, NativeHttpClient};

#[test]
fn cancelled_client_ticket_cannot_name_an_equal_fresh_client_request() {
    let encoded = conduit_web::encode_request(&HttpRequest {
        transaction_id: HttpTransactionId::new(44).unwrap(),
        method: HttpMethod::Post,
        target: HttpTarget::new("192.0.2.1:80".into(), "/same".into(), HttpScheme::Http).unwrap(),
        headers: conduit_web::http_headers([]).unwrap(),
        body: HttpBody::inline(b"same"),
    })
    .unwrap();
    let mut retired = NativeHttpClient::prepare();
    let old = retired.begin(&encoded, true).unwrap();
    retired.cancel().unwrap();
    let mut current = NativeHttpClient::prepare();
    let fresh = current.begin(&encoded, true).unwrap();
    assert_eq!(
        current.request_bytes(old),
        Err(HttpClientFailure::StaleCompletion),
        "a cancelled owner's ticket must not regain custody through fresh client reuse"
    );
    assert!(!current.request_bytes(fresh).unwrap().is_empty());
}
