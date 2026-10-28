//! Hostile publication syntax is refused before typed allocation in real State.
use super::*;

pub(super) fn run(transfer: &mut [u8]) {
    assert!(ACTIVE.load(Ordering::SeqCst));
    assert_eq!(LIVE.load(Ordering::Relaxed), 0);
    for selected in 0..10 {
        let graph = selected >= 6;
        let case = if graph {
            [0, 3, 4, 5][selected - 6]
        } else {
            selected
        };
        let (wire, expected) = on_caller(|| {
            fn map(key: &str, value: &[u8], wide: bool) -> Vec<u8> {
                let mut bytes = vec![0xa1];
                if wide {
                    bytes.extend_from_slice(&[0x78, key.len() as u8]);
                    bytes.extend_from_slice(key.as_bytes());
                } else {
                    ciborium::ser::into_writer(key, &mut bytes).unwrap();
                }
                bytes.extend_from_slice(value);
                bytes
            }
            let (value, expected) = match case {
                0 => {
                    let mut value = vec![0x81; 129];
                    value.push(0);
                    (value, data::Refusal::Bounds)
                }
                1 | 2 => {
                    let mut value = vec![0x7a];
                    value.extend_from_slice(&(256u32 * 1024).to_be_bytes());
                    value.extend(core::iter::repeat_n(b'x', 256 * 1024));
                    (value, data::Refusal::Bounds)
                }
                3 => (vec![0x61, 0xff], data::Refusal::Malformed),
                4 => (
                    vec![0x9b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
                    data::Refusal::Malformed,
                ),
                _ => (vec![0x9f, 0xff], data::Refusal::Malformed),
            };
            let encoded = if case <= 2 {
                map(
                    "show",
                    &map("planned_mask", &map("plan", &value, case == 2), case == 2),
                    case == 2,
                )
            } else {
                map("payload", &value, false)
            };
            let mut plain = if graph {
                b"CAPG\x01".to_vec()
            } else {
                b"CAPP\x02".to_vec()
            };
            plain.extend_from_slice(&encoded);
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
