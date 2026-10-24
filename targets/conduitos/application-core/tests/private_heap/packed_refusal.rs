//! Hostile V2 packed fields fail in real State under the exact active grant.
use super::*;
pub(super) fn run(transfer: &mut [u8]) {
    assert!(ACTIVE.load(Ordering::SeqCst));
    assert_eq!(LIVE.load(Ordering::Relaxed), 0);
    for case in 0..4 {
        let wire = on_caller(|| {
            let (key, value) = match case {
                0 => (
                    "canonical_value".to_string(),
                    format!(
                        "~b64:{}",
                        "A".repeat(
                            (conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES + 1).div_ceil(3) * 4
                        )
                    ),
                ),
                1 => ("canonical_value".to_string(), "~b64:AB==".to_string()),
                2 => ("canonical_value".to_string(), "\\u007eb64:AA==".to_string()),
                _ => (
                    "canonical_\\u0076alue".to_string(),
                    "~b64:AA\\u003d=".to_string(),
                ),
            };
            let json = format!(r#"[2,"CAPSEQ2",{{"{key}":"{value}"}}]"#);
            let mut wire = b"CAPZ\x01\0\0\0".to_vec();
            wire.extend_from_slice(&(json.len() as u32).to_le_bytes());
            wire.extend_from_slice(&miniz_oxide::deflate::compress_to_vec_zlib(
                json.as_bytes(),
                6,
            ));
            assert!(wire.len() <= conduitos_application_core::MAX_BLOB);
            wire
        });
        let mut state = State::default();
        assert_eq!(
            state.execute(
                Request::Begin {
                    bytes: wire.len() as u32
                },
                transfer
            ),
            Ok(Response::Ready)
        );
        for (index, chunk) in wire.chunks(transfer.len()).enumerate() {
            transfer[..chunk.len()].copy_from_slice(chunk);
            assert_eq!(
                state.execute(
                    Request::Write {
                        offset: (index * transfer.len()) as u32,
                        bytes: chunk.len() as u32
                    },
                    transfer
                ),
                Ok(Response::Ready)
            );
        }
        let allocations = ALLOCATIONS.load(Ordering::Relaxed);
        assert_eq!(
            state.execute(Request::Preflight, transfer),
            Err(data::Refusal::Malformed)
        );
        // Incoming/workspace/compressor cleanup may deallocate; no Serde scratch,
        // decoded Vec or other allocation may occur during rejecting preflight.
        assert_eq!(ALLOCATIONS.load(Ordering::Relaxed), allocations);
        assert_eq!(LIVE.load(Ordering::Relaxed), 0);
        assert_eq!(
            state.execute(Request::Commit, transfer),
            Err(data::Refusal::Order)
        );
        assert_eq!(
            state.execute(Request::Begin { bytes: 1 }, transfer),
            Err(data::Refusal::Order)
        );
        drop(state);
        assert_eq!(LIVE.load(Ordering::Relaxed), 0);
        assert!(!unsafe { private_heap::exhausted() });
        drop(wire);
    }
}
