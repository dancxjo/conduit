use super::*;
struct Transport {
    sends: usize,
    bytes: [u8; MAXIMUM_BYTES],
    length: usize,
    fail: bool,
}
impl MessageTransport for Transport {
    type Error = ();
    async fn receive<'a>(&mut self, _: &'a mut [u8]) -> Result<&'a [u8], ()> {
        panic!("terminal diagnostic never receives")
    }
    async fn send(&mut self, bytes: &[u8]) -> Result<(), ()> {
        self.sends += 1;
        self.bytes[..bytes.len()].copy_from_slice(bytes);
        self.length = bytes.len();
        if self.fail {
            Err(())
        } else {
            Ok(())
        }
    }
}
#[test]
fn one_bounded_send_reuses_response_and_never_retries_even_on_failure() {
    for fail in [false, true] {
        let mut transport = Transport {
            sends: 0,
            bytes: [0; MAXIMUM_BYTES],
            length: 0,
            fail,
        };
        let mut response = [0x55; 4096];
        let result = crate::tests::ready(send_once(
            &mut transport,
            &mut response,
            &Details {
                reason: "complete-source-preparation",
                detail: "exact refusal",
                heap_used_bytes: 123,
                heap_peak_bytes: Some(150),
                heap_limit_bytes: 118784,
            },
        ));
        assert_eq!(result.is_err(), fail);
        assert_eq!(transport.sends, 1);
        assert!(transport.length <= MAXIMUM_BYTES);
        assert!(response[MAXIMUM_BYTES..].iter().all(|byte| *byte == 0x55));
        let status: serde_json::Value =
            serde_json::from_slice(&transport.bytes[..transport.length]).unwrap();
        assert_eq!(status["phase"], "refused-before-play");
        assert_eq!(status["reason"], "complete-source-preparation");
        assert_eq!(status["detail"], "exact refusal");
        assert_eq!(status["heap_limit_bytes"], 118784);
    }
}
#[test]
fn escaped_oversized_detail_and_missing_buffer_use_one_fixed_fallback() {
    for bytes in [0, MAXIMUM_BYTES, 4096] {
        let mut transport = Transport {
            sends: 0,
            bytes: [0; MAXIMUM_BYTES],
            length: 0,
            fail: false,
        };
        let mut response = [0x55; 4096];
        // A small input can still exceed the cap after JSON escaping.
        let detail = "\0".repeat(192);
        crate::tests::ready(send_once(
            &mut transport,
            &mut response[..bytes],
            &Details {
                reason: "complete-source-preparation",
                detail: &detail,
                heap_used_bytes: 123,
                heap_peak_bytes: None,
                heap_limit_bytes: 118784,
            },
        ))
        .unwrap();
        assert_eq!(transport.sends, 1);
        assert_eq!(&transport.bytes[..transport.length], FALLBACK);
        assert!(response[MAXIMUM_BYTES..].iter().all(|byte| *byte == 0x55));
    }
}
