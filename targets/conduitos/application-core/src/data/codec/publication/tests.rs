extern crate std;
use super::*;
use crate::data::{Workspace, encode_command, transport_codec};
use alloc::{vec, vec::Vec};
use conduit_face::{FaceBasis, FaceRole, FaceSubject};
#[rustfmt::skip]
#[path = "../../../../../../../semantics/face/tests/common/mod.rs"]
mod mask_fixture;

fn fixture() -> Command {
    let body = serde_json::from_str("\"body/publication-codec\"").unwrap();
    let basis = FaceContributionBasis {
        checked_plot_id: "checked/publication".into(),
        plan_id: "plan/publication".into(),
        active_play_id: "play/publication".into(),
        required_interaction_context: None,
    };
    let face = Face::new_with_semantics(
        1,
        FaceBasis {
            body_id: Some(body),
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![FaceSubject {
            identity: "document/publication".into(),
            role: FaceRole::Document,
            name: "Publication".into(),
        }],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let show = mask_fixture::available_mask_show(&face);
    Command::PatchbayPublish {
        slot: 0,
        body: face.basis.body_id.clone().unwrap(),
        basis,
        face,
        show,
    }
}

#[test]
fn complete_plan_round_trip_preserves_public_json_and_canonical_validation() {
    let command = fixture();
    let Command::PatchbayPublish { face, show, .. } = &command else {
        unreachable!()
    };
    show.validate(face).unwrap();
    conduit_core::verify_plan(&show.planned_mask.plan).unwrap();
    let public = serde_json::to_vec(&command).unwrap();
    let public_face = serde_json::to_vec(face).unwrap();
    let public_show = serde_json::to_vec(show).unwrap();
    assert_eq!(serde_json::from_slice::<Face>(&public_face).unwrap(), *face);
    assert_eq!(
        serde_json::from_slice::<MaskShow>(&public_show).unwrap(),
        *show
    );
    assert!(
        std::str::from_utf8(&public)
            .unwrap()
            .contains("mask-plan-deflate-json@1")
    );
    let bytes = encode_command(&command).unwrap();
    assert!(bytes.starts_with(MAGIC));
    assert!(!bytes.windows(8).any(|v| v == b"zlib_hex"));
    let mut workspace = Workspace::new().unwrap();
    let decoded = workspace.decode_command(&bytes).unwrap();
    let Command::PatchbayPublish {
        face: actual,
        show: actual_show,
        ..
    } = &decoded
    else {
        unreachable!()
    };
    assert_eq!(actual, face);
    assert_eq!(actual_show, show);
    assert_eq!(serde_json::to_vec(&decoded).unwrap(), public);
    actual_show.validate(actual).unwrap();
    std::println!("publication_cbor_fixture wire_bytes={}", bytes.len());
}

#[test]
fn old_private_publication_refuses_and_other_command_bytes_stay_exact() {
    let command = fixture();
    let legacy = super::super::encode_with(&command, transport_codec::encode_to_slice).unwrap();
    assert!(Workspace::new().unwrap().decode_command(&legacy).is_err());
    let input = encode_command(&Command::PatchbayConsume {
        slot: 3,
        input: vec![7, 9],
    })
    .unwrap();
    assert_eq!(
        input,
        br#"[2,"CAPSEQ2",{"PatchbayConsume":{"slot":3,"input":[2,7,9]}}]"#
    );
    assert!(
        matches!(Workspace::new().unwrap().decode_command(&input).unwrap(),
        Command::PatchbayConsume {slot: 3, input} if input == [7,9])
    );
}

#[test]
fn private_tampering_still_reaches_complete_plan_refusal() {
    for implementation in [false, true] {
        let mut command = fixture();
        let Command::PatchbayPublish { show, .. } = &mut command else {
            unreachable!()
        };
        if implementation {
            show.planned_mask.plan.fragments[0].placements[0].implementation_id =
                "foreign/implementation".into();
        } else {
            show.planned_mask.plan.plan_id = "foreign/plan".into();
        }
        let decoded = Workspace::new()
            .unwrap()
            .decode_command(&encode_command(&command).unwrap())
            .unwrap();
        let Command::PatchbayPublish { face, show, .. } = decoded else {
            unreachable!()
        };
        assert!(show.validate(&face).is_err());
    }
}

#[test]
fn malformed_prefix_duplicate_fields_and_trailing_values_refuse() {
    let bytes = encode_command(&fixture()).unwrap();
    for n in [0, 1, 4, 5, bytes.len() / 2, bytes.len() - 1] {
        assert!(
            Workspace::new()
                .unwrap()
                .decode_command(&bytes[..n])
                .is_err()
        );
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(Workspace::new().unwrap().decode_command(&trailing).is_err());
    // Preserve every required field and append a duplicate: failure cannot be
    // explained by an incomplete fixture or by malformed CBOR framing.
    let mut duplicate = bytes.clone();
    assert_eq!(duplicate[MAGIC.len()], 0xa5);
    duplicate[MAGIC.len()] = 0xa6;
    duplicate.extend_from_slice(&[0x64, b's', b'l', b'o', b't', 0]);
    assert_eq!(super::preflight(&duplicate), Ok(()));
    assert!(super::decode_preflighted(&duplicate).is_err());
    let result: Result<Owned, _> =
        ciborium::de::from_reader_with_recursion_limit(&duplicate[MAGIC.len()..], 64);
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("duplicate slot admitted"),
    };
    assert!(alloc::format!("{error:?}").contains("duplicate field"));
}

#[test]
fn binary_native_integer_ranges_and_bignums_preserve_typed_values() {
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Numeric {
        u: u64,
        i: i64,
        wide_u: u128,
        wide_i: i128,
        bytes: [u8; 4],
    }
    let value = Numeric {
        u: u64::MAX,
        i: i64::MIN,
        wide_u: u128::MAX,
        wide_i: i128::MIN,
        bytes: [0, 23, 24, 255],
    };
    let mut storage = [0u8; 512];
    let mut remaining = &mut storage[..];
    ciborium::ser::into_writer(&value, &mut remaining).unwrap();
    let length = 512 - remaining.len();
    assert_eq!(super::preflight::validate(&storage[..length]), Ok(()));
    let actual: Numeric =
        ciborium::de::from_reader_with_recursion_limit(&storage[..length], 64).unwrap();
    assert_eq!(actual, value);
}

#[test]
fn binary_publication_still_invokes_checked_structured_configuration_constructor() {
    use conduit_core::{
        ConfigurationEntry, ConfigurationValue, StructuredConfigurationValue, StructuredInfoType,
        StructuredInfoValue, TEXT_INFO_ID, kind_id,
    };
    let mut command = fixture();
    let ty = StructuredInfoType::leaf(kind_id(TEXT_INFO_ID)).unwrap();
    let profile = ty.profile().unwrap().value_kind().clone();
    let value = StructuredInfoValue::leaf(ty, b"publication constructor fixture".to_vec()).unwrap();
    let structured =
        StructuredConfigurationValue::new(profile.clone(), value.canonical_bytes().unwrap())
            .unwrap();
    let Command::PatchbayPublish { show, .. } = &mut command else {
        unreachable!()
    };
    show.planned_mask.plan.fragments[0].placements[0]
        .configuration
        .push(ConfigurationEntry {
            key: "publication-constructor-fixture".into(),
            value: ConfigurationValue::Structured(structured),
        });
    let mut bytes = encode_command(&command).unwrap();
    assert!(Workspace::new().unwrap().decode_command(&bytes).is_ok());
    let offsets: Vec<_> = bytes
        .windows(profile.as_str().len())
        .enumerate()
        .filter_map(|(i, v)| (v == profile.as_str().as_bytes()).then_some(i))
        .collect();
    assert_eq!(offsets.len(), 1);
    bytes[offsets[0]] = b'x';
    assert_eq!(
        Workspace::new().unwrap().decode_command(&bytes).err(),
        Some(Refusal::Malformed)
    );
}
