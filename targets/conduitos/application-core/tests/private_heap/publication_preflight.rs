//! Hostile publication syntax is refused before typed allocation in real State.
use super::*;

pub(super) fn run(transfer: &mut [u8]) {
    assert!(ACTIVE.load(Ordering::SeqCst));
    assert_eq!(LIVE.load(Ordering::Relaxed), 0);
    for case in 0..4 {
        let (wire, expected) = on_caller(|| {
            let (json, expected) = match case {
                0 => (
                    format!(
                        r#"{{"show":{{"planned_mask":{{"plan":{}0{}}}}}}}"#,
                        "[".repeat(129),
                        "]".repeat(129)
                    ),
                    data::Refusal::Bounds,
                ),
                1 | 2 => {
                    let keys = if case == 1 {
                        ["show", "planned_mask", "plan"]
                    } else {
                        [r"\u0073how", r"planned_\u006dask", r"pl\u0061n"]
                    };
                    // Quotes make the exact Plan value one byte over 256 KiB.
                    let value = "x".repeat(256 * 1024 - 1);
                    (
                        format!(
                            r#"{{"{}":{{"{}":{{"{}":"{}"}}}}}}"#,
                            keys[0], keys[1], keys[2], value
                        ),
                        data::Refusal::Bounds,
                    )
                }
                _ => (r#"{"show":"\uD800"}"#.to_string(), data::Refusal::Malformed),
            };
            let mut plain = b"CAPP\x01".to_vec();
            plain.extend_from_slice(json.as_bytes());
            let mut wire = b"CAPZ\x01\0\0\0".to_vec();
            wire.extend_from_slice(&(plain.len() as u32).to_le_bytes());
            wire.extend_from_slice(&miniz_oxide::deflate::compress_to_vec_zlib(&plain, 6));
            assert!(wire.len() <= conduitos_application_core::MAX_BLOB);
            (wire, expected)
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
                        bytes: chunk.len() as u32,
                    },
                    transfer
                ),
                Ok(Response::Ready)
            );
        }
        let allocations = ALLOCATIONS.load(Ordering::Relaxed);
        assert_eq!(state.execute(Request::Preflight, transfer), Err(expected));
        // Rejection may free the admitted workspace, but cannot allocate a
        // decoded Face, Plan, string scratch or a dynamic parser stack.
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
