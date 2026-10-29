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
