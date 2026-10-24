//! Actual public fixed-storage request lifecycle; no detached request identity.
use conduit_web::{HttpBody, HttpMethod, HttpRequest, HttpScheme, HttpTarget, HttpTransactionId};
use conduitos::http::*;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
thread_local! {
    static TRACK: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}
struct Allocator;
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        TRACK.with(|track| {
            if track.get() {
                ALLOCATIONS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        TRACK.with(|track| {
            if track.get() {
                ALLOCATIONS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
struct Provider {
    calls: u32,
    failure: Option<NetworkFailure>,
    wire: &'static [u8],
    oversized: bool,
}
impl HttpNetworkBase for Provider {
    fn exchange(&mut self, _: &[u8], response: &mut [u8]) -> Result<usize, NetworkFailure> {
        self.calls += 1;
        if let Some(error) = self.failure {
            return Err(error);
        }
        if self.oversized {
            return Ok(response.len() + 1);
        }
        response[..self.wire.len()].copy_from_slice(self.wire);
        Ok(self.wire.len())
    }
}
#[test]
fn affine_requests_retire_without_allocation_retry_or_partial_output() {
    let encoded = conduit_web::encode_request(&HttpRequest {
        transaction_id: HttpTransactionId::new(44).unwrap(),
        method: HttpMethod::Post,
        target: HttpTarget::new("192.0.2.1:80".into(), "/same".into(), HttpScheme::Http).unwrap(),
        headers: conduit_web::http_headers([]).unwrap(),
        body: HttpBody::inline(b"same"),
    })
    .unwrap();
    let mut client = NativeHttpClient::prepare();
    let mut sibling = NativeHttpClient::prepare();
    let mut output = FixedHttpOutput::new();
    let mut provider = Provider {
        calls: 0,
        failure: None,
        wire: b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nok",
        oversized: false,
    };
    client
        .exchange(&encoded, true, &mut provider, &mut output)
        .unwrap();
    let original = output.as_bytes().to_vec();
    TRACK.with(|track| track.set(true));
    client.begin(&encoded, true).unwrap().cancel().unwrap();
    assert_eq!(client.sign_count(), 5, "one begin and one retirement");
    drop(client.begin(&encoded, true).unwrap());
    assert_eq!(client.sign_count(), 7, "Drop retires exactly once");
    // Equal transaction and bytes can be reused without reusing a request guard.
    for _ in 0..2 {
        client
            .exchange(&encoded, true, &mut provider, &mut output)
            .unwrap();
    }
    assert_eq!(provider.calls, 3);
    for (failure, expected) in [
        (NetworkFailure::Connect, HttpClientFailure::Connect),
        (NetworkFailure::BaseLost, HttpClientFailure::BaseLost),
        (
            NetworkFailure::ProviderLost,
            HttpClientFailure::ProviderLost,
        ),
    ] {
        provider.failure = Some(failure);
        assert_eq!(
            client.exchange(&encoded, true, &mut provider, &mut output),
            Err(expected)
        );
        assert_eq!(output.as_bytes(), original);
        client.begin(&encoded, true).unwrap().cancel().unwrap();
    }
    provider.failure = None;
    provider.wire = b"bad";
    assert_eq!(
        client.exchange(&encoded, true, &mut provider, &mut output),
        Err(HttpClientFailure::MalformedResponse)
    );
    assert_eq!(output.as_bytes(), original);
    provider.oversized = true;
    assert_eq!(
        client.exchange(&encoded, true, &mut provider, &mut output),
        Err(HttpClientFailure::ResponseBodyOverflow)
    );
    assert_eq!(output.as_bytes(), original);
    assert_eq!(
        provider.calls, 8,
        "every attempted exchange calls provider once"
    );
    let guard = client.begin(&encoded, true).unwrap();
    assert!(!guard.request_bytes().is_empty());
    core::mem::forget(guard);
    assert!(matches!(
        client.begin(&encoded, true),
        Err(HttpClientFailure::Pressure)
    ));
    assert_eq!(output.as_bytes(), original);
    assert_eq!(provider.calls, 8);
    sibling.begin(&encoded, true).unwrap().cancel().unwrap();
    TRACK.with(|track| track.set(false));
    ALLOCATIONS.with(|count| assert_eq!(count.get(), 0));
}
