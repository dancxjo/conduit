use super::*;
use crate::{
    data::Command,
    state::State,
    wire::{Request, Response},
};
use alloc::{string::String, vec};

#[test]
fn small_raw_bytes_and_exact_wire_boundary_remain_unchanged() {
    assert_eq!(encode(&"hello").unwrap(), b"\"hello\"");
    let value = "a".repeat(MAX_BLOB - 2);
    let bytes = encode(&value).unwrap();
    assert_eq!(bytes.len(), MAX_BLOB);
    assert_eq!(decode::<String>(&bytes).unwrap(), value);
    assert!(!bytes.starts_with(MAGIC));
}

#[test]
fn compressed_values_keep_every_byte_through_the_decoded_ceiling() {
    for length in [MAX_BLOB, MAX_DECODED_JSON_BYTES - 2] {
        let value = "a".repeat(length);
        let bytes = encode(&value).unwrap();
        assert!(bytes.starts_with(MAGIC));
        assert!(bytes.len() <= MAX_BLOB);
        assert_eq!(decode::<String>(&bytes).unwrap(), value);
    }
    assert_eq!(
        encode(&"a".repeat(MAX_DECODED_JSON_BYTES)),
        Err(Refusal::Capacity)
    );
}

#[test]
fn framing_checksum_trailing_bytes_and_expansion_claims_refuse() {
    let bytes = encode(&"a".repeat(2 * MAX_BLOB)).unwrap();
    for index in 0..MAGIC.len() {
        let mut altered = bytes.clone();
        altered[index] ^= 1;
        assert!(decode::<String>(&altered).is_err());
    }
    for length in [0, 1, HEADER - 1, HEADER, bytes.len() / 2, bytes.len() - 1] {
        assert!(decode::<String>(&bytes[..length]).is_err());
    }
    let mut altered = bytes.clone();
    *altered.last_mut().unwrap() ^= 1;
    assert_eq!(decode::<String>(&altered), Err(Refusal::Malformed));
    let mut altered = bytes.clone();
    altered.push(0);
    assert_eq!(decode::<String>(&altered), Err(Refusal::Malformed));
    for length in [
        0,
        1,
        2 * MAX_BLOB,
        2 * MAX_BLOB + 3,
        MAX_DECODED_JSON_BYTES + 1,
    ] {
        let mut altered = bytes.clone();
        altered[MAGIC.len()..HEADER].copy_from_slice(&(length as u32).to_le_bytes());
        assert!(decode::<String>(&altered).is_err());
    }
    assert_eq!(
        decode::<String>(&vec![0; MAX_BLOB + 1]),
        Err(Refusal::Bounds)
    );
}

#[test]
fn incompressible_data_cannot_expand_the_wire_limit() {
    let mut random = 1u32;
    let value: String = (0..4 * MAX_BLOB)
        .map(|_| {
            random ^= random << 13;
            random ^= random >> 17;
            random ^= random << 5;
            char::from(32 + (random % 95) as u8)
        })
        .collect();
    assert_eq!(encode(&value), Err(Refusal::Capacity));
}

fn commit(state: &mut State, bytes: &[u8]) -> Result<Response, Refusal> {
    let mut transfer = vec![0; crate::memory::TRANSFER_BYTES];
    assert_eq!(
        state.execute(
            Request::Begin {
                bytes: bytes.len() as u32
            },
            &mut transfer
        )?,
        Response::Ready
    );
    for (index, chunk) in bytes.chunks(transfer.len()).enumerate() {
        transfer[..chunk.len()].copy_from_slice(chunk);
        assert_eq!(
            state.execute(
                Request::Write {
                    offset: (index * transfer.len()) as u32,
                    bytes: chunk.len() as u32
                },
                &mut transfer
            ),
            Ok(Response::Ready)
        );
    }
    state.execute(Request::Commit, &mut transfer)
}

#[test]
fn malformed_compressed_commit_cannot_partially_initialize_models() {
    let mut state = State::default();
    let mut invalid = encode(&"a".repeat(2 * MAX_BLOB)).unwrap();
    *invalid.last_mut().unwrap() ^= 1;
    assert_eq!(commit(&mut state, &invalid), Err(Refusal::Malformed));
    let init = encode_command(&Command::PrepareInit {
        manifest: crate::data::InitManifest {
            slots: [None; crate::SLOTS],
        },
    })
    .unwrap();
    assert_eq!(commit(&mut state, &init), Err(Refusal::Order));
    let mut state = State::default();
    assert!(matches!(
        commit(&mut state, &init),
        Ok(Response::Blob { .. })
    ));
    assert_eq!(commit(&mut state, &init), Err(Refusal::Order));
}

#[test]
fn nested_byte_fields_use_standard_json_sequences_even_in_small_raw_values() {
    #[derive(serde::Deserialize, Debug, PartialEq)]
    #[serde(transparent)]
    struct Bytes(Vec<u8>);
    impl serde::Serialize for Bytes {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.serialize_bytes(&self.0)
        }
    }
    #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
    struct Nested {
        rows: Vec<Option<Bytes>>,
    }
    let value = Nested {
        rows: vec![Some(Bytes(vec![0, 34, 77, 255])), None],
    };
    let bytes = encode(&value).unwrap();
    assert_eq!(bytes, serde_json::to_vec(&value).unwrap());
    assert_eq!(decode::<Nested>(&bytes).unwrap(), value);
}

#[test]
fn refused_uploads_are_terminal_and_never_expose_partial_models() {
    let valid = encode_command(&Command::PrepareInit {
        manifest: crate::data::InitManifest {
            slots: [None; crate::SLOTS],
        },
    })
    .unwrap();
    for case in 0..6 {
        let mut state = State::default();
        let mut transfer = vec![0; crate::memory::TRANSFER_BYTES];
        let result = if case == 0 {
            state.execute(
                Request::Begin {
                    bytes: (MAX_BLOB + 1) as u32,
                },
                &mut transfer,
            )
        } else {
            state
                .execute(
                    Request::Begin {
                        bytes: valid.len() as u32,
                    },
                    &mut transfer,
                )
                .unwrap();
            match case {
                1 => state.execute(
                    Request::Begin {
                        bytes: valid.len() as u32,
                    },
                    &mut transfer,
                ),
                2 => state.execute(Request::Commit, &mut transfer),
                3 => state.execute(
                    Request::Write {
                        offset: 1,
                        bytes: 1,
                    },
                    &mut transfer,
                ),
                4 => state.execute(
                    Request::Write {
                        offset: 0,
                        bytes: (valid.len() + 1) as u32,
                    },
                    &mut transfer,
                ),
                _ => {
                    transfer[0] = valid[0];
                    state
                        .execute(
                            Request::Write {
                                offset: 0,
                                bytes: 1,
                            },
                            &mut transfer,
                        )
                        .unwrap();
                    state.execute(
                        Request::Write {
                            offset: 0,
                            bytes: 1,
                        },
                        &mut transfer,
                    )
                }
            }
        };
        assert!(result.is_err(), "case {case}");
        assert_eq!(commit(&mut state, &valid), Err(Refusal::Order));
        assert_eq!(
            commit(
                &mut state,
                &encode_command(&Command::Graph { slot: 0 }).unwrap()
            ),
            Err(Refusal::Order)
        );
    }
}

#[test]
fn hostile_json_depth_strings_and_counts_refuse_before_initialization() {
    for input in [
        alloc::format!("{}0{}", "[".repeat(130), "]".repeat(130)),
        alloc::format!(
            "{{\"Init\":{{\"applications\":[{}]}}}}",
            "null,".repeat(1000)
        ),
        alloc::format!("\"{}", "x".repeat(MAX_BLOB - 2)),
    ] {
        let mut state = State::default();
        assert!(commit(&mut state, input.as_bytes()).is_err());
        assert_eq!(
            commit(
                &mut state,
                &encode_command(&Command::Graph { slot: 0 }).unwrap()
            ),
            Err(Refusal::Order)
        );
    }
}

#[test]
fn reusable_workspace_preserves_exact_wire_and_graph_custody_bytes() {
    let mut workspace = Workspace::new().unwrap();
    for length in [0, MAX_BLOB - 2, MAX_BLOB, 3 * MAX_BLOB] {
        let value = "x".repeat(length);
        assert_eq!(workspace.encode(&value).unwrap(), encode(&value).unwrap());
        assert_eq!(
            workspace
                .decode::<String>(&encode(&value).unwrap())
                .unwrap(),
            value
        );
    }
    // Only this retained test fixture recognizes the retired aggregate shape.
    #[derive(serde::Serialize, serde::Deserialize)]
    enum Fixture {
        Init {
            applications: [Option<crate::data::Initial>; 4],
        },
    }
    let original = include_bytes!("../../../tests/fixtures/startup.catz");
    let fixture: Fixture = decode(original).unwrap();
    assert_eq!(encode(&fixture).unwrap(), original);
    assert_eq!(workspace.encode(&fixture).unwrap(), original);
    let Fixture::Init { applications } = fixture;
    let Some(crate::data::Initial::Patchbay { targets, .. }) = &applications[0] else {
        panic!("fixture");
    };
    for target in targets.iter().flatten() {
        let expected = crate::graph_facts::digest(&target.graph).unwrap();
        let bytes = workspace.encode(&target.graph).unwrap();
        assert_eq!(bytes, encode(&target.graph).unwrap());
        assert_eq!(
            conduit_core::semantic_digest("conduitos/graph-face-facts@1", &bytes),
            expected
        );
    }
    for malformed in [b"CAPZ\x01\0\0\0".as_slice(), b"[[[[", b"\"unterminated"] {
        assert!(workspace.decode::<Command>(malformed).is_err());
    }
}
