//! Complete real graphs survive the private cut; no public graph byte changes.
use super::*;
use crate::data::{Initial, Workspace, encode_command, transport_codec};
use alloc::vec;
#[derive(Deserialize)]
enum Fixture {
    Init {
        applications: [Option<Initial>; crate::SLOTS],
    },
}

#[test]
fn complete_graph_preparation_matches_public_json_and_independent_cbor() {
    let Fixture::Init { applications } =
        super::super::decode(include_bytes!("../../../../tests/fixtures/startup.catz")).unwrap();
    let Some(Initial::Patchbay { targets, .. }) = &applications[0] else {
        panic!("real targets")
    };
    for (index, target) in targets.iter().enumerate() {
        let Some(value) = target else { continue };
        let command = Command::PrepareTarget {
            slot: 0,
            target: index as u8,
            value: value.clone(),
        };
        let expected = super::super::encode(&command).unwrap();
        let wire = encode_command(&command).unwrap();
        let decoded = Workspace::new().unwrap().decode_command(&wire).unwrap();
        assert_eq!(super::super::encode(&decoded).unwrap(), expected);
        let mut backing = vec![0; super::super::MAX_DECODED_JSON_BYTES];
        let bytes = super::super::decoded_json(&wire, &mut backing).unwrap();
        let mut remaining = bytes.strip_prefix(MAGIC).unwrap();
        let Owned::PrepareTarget {
            slot,
            target,
            value: reference,
        } = ciborium::de::from_reader(&mut remaining).unwrap()
        else {
            panic!("target")
        };
        assert!(remaining.is_empty());
        assert_eq!(slot, 0);
        assert_eq!(target, index as u8);
        assert_eq!(reference.graph, value.graph);
        assert_eq!(reference.plan, value.plan);
        assert_eq!(reference.body, value.body);
        let legacy = super::super::encode_with(&command, transport_codec::encode_to_slice).unwrap();
        assert!(Workspace::new().unwrap().decode_command(&legacy).is_err());
    }
}

#[test]
fn graph_carrier_cannot_smuggle_an_unrelated_command_or_old_version() {
    let mut wire = MAGIC.to_vec();
    ciborium::ser::into_writer(
        &Command::Apply {
            slot: 0,
            input: vec![0],
        },
        &mut wire,
    )
    .unwrap();
    assert!(preflight(&wire).is_ok());
    assert!(decode_preflighted(&wire).is_err());
    wire[4] = 0;
    assert!(Workspace::new().unwrap().decode_command(&wire).is_err());
}

#[test]
#[ignore = "private retained CI input attribution, not native budget acceptance"]
fn retained_prepare_target_phase_attribution() {
    extern crate std;
    let path = std::env::var_os("CONDUIT_PREPARE_TARGET_CAPTURE").unwrap();
    let wire = std::fs::read(std::path::PathBuf::from(path).join("command.bin")).unwrap();
    assert_eq!(wire.len(), 9291);
    let mut workspace = Workspace::new().unwrap();
    for _ in 0..3 {
        let start = std::time::Instant::now();
        let length = workspace.preflight_command(&wire).unwrap();
        let preflight = start.elapsed();
        assert_eq!(length, 164655);
        let start = std::time::Instant::now();
        let command = workspace.decode_preflighted_command(length).unwrap();
        let decode = start.elapsed();
        assert_eq!(encode_command(&command).unwrap(), wire, "exact complete command wire remains unchanged");
        let Command::PrepareTarget {
            slot,
            target,
            value,
        } = command
        else {
            panic!("target")
        };
        assert_eq!((slot, target), (2, 3));
        assert_eq!(
            value.plan.as_ref().unwrap().as_str(),
            "65f2a3cf560c587fba263ebe699211d24fa359b871d68015e8d6f8008237f3dd"
        );
        std::println!(
            "ACTUAL_TARGET preflight_ns={} typed_decode_ns={} native_budget_proof=false",
            preflight.as_nanos(),
            decode.as_nanos()
        );
    }
}

#[test]
#[ignore = "conversion benchmark of exact captured canonical bytes; no admission claim"]
fn retained_target_byte_conversion_attribution() {
    extern crate std;
    struct Bulk(alloc::vec::Vec<u8>);
    impl<'de> Deserialize<'de> for Bulk {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct Bytes;
            impl<'de> serde::de::Visitor<'de> for Bytes {
                type Value = Bulk;
                fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                    f.write_str("bytes")
                }
                fn visit_bytes<E: serde::de::Error>(self, bytes: &[u8]) -> Result<Bulk, E> {
                    Ok(Bulk(bytes.to_vec()))
                }
                fn visit_seq<A: serde::de::SeqAccess<'de>>(
                    self,
                    mut seq: A,
                ) -> Result<Bulk, A::Error> {
                    let mut bytes = alloc::vec::Vec::with_capacity(seq.size_hint().unwrap_or(0));
                    while let Some(byte) = seq.next_element()? {
                        bytes.push(byte);
                    }
                    Ok(Bulk(bytes))
                }
            }
            deserializer.deserialize_byte_buf(Bytes)
        }
    }
    let path = std::env::var_os("CONDUIT_PREPARE_TARGET_CAPTURE").unwrap();
    let wire = std::fs::read(std::path::PathBuf::from(path).join("command.bin")).unwrap();
    let Command::PrepareTarget { value, .. } =
        Workspace::new().unwrap().decode_command(&wire).unwrap()
    else {
        panic!("target")
    };
    #[derive(Serialize)]
    struct Input<'a> {
        canonical_value: &'a [u8],
    }
    #[derive(Deserialize)]
    struct Original {
        canonical_value: alloc::vec::Vec<u8>,
    }
    #[derive(Deserialize)]
    struct Copied {
        canonical_value: Bulk,
    }
    for control in &value.graph.gears[1].controls {
        let conduit_core::ConfigurationValue::Structured(value) = &control.value else {
            continue;
        };
        let mut packed = alloc::vec![0; super::super::MAX_DECODED_JSON_BYTES];
        let length = super::super::finite_cbor::encode(
            &Input {
                canonical_value: value.canonical_value(),
            },
            &mut packed,
        )
        .unwrap();
        for _ in 0..3 {
            let start = std::time::Instant::now();
            let original: Original =
                super::super::finite_cbor::decode_preflighted(&packed[..length]).unwrap();
            let sequence = start.elapsed();
            let start = std::time::Instant::now();
            let bulk: Copied =
                super::super::finite_cbor::decode_preflighted(&packed[..length]).unwrap();
            let copy = start.elapsed();
            assert_eq!(original.canonical_value, value.canonical_value());
            assert_eq!(bulk.canonical_value.0, original.canonical_value);
            std::println!(
                "ACTUAL_BYTE_CONVERSION control={} bytes={} sequence_ns={} bulk_ns={} native_budget_proof=false",
                control.key,
                original.canonical_value.len(),
                sequence.as_nanos(),
                copy.as_nanos()
            );
        }
    }
}
