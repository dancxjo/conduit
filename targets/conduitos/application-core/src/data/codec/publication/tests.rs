extern crate std;
use super::*;
use crate::data::{Workspace, encode_command, transport_codec};
use alloc::{vec, vec::Vec};
use conduit_face::{FaceBasis, FaceRole, FaceSubject, MaskShow};
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
    assert!(conduit_core::verify_plan(&show.planned_mask.plan));
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
    std::println!("publication_direct_fixture wire_bytes={}", bytes.len());
}

#[test]
fn old_private_publication_refuses_and_other_command_bytes_stay_exact() {
    let command = fixture();
    let mut old_version = encode_command(&command).unwrap();
    assert!(old_version.starts_with(MAGIC));
    old_version[MAGIC.len() - 1] = 1;
    assert!(
        Workspace::new()
            .unwrap()
            .decode_command(&old_version)
            .is_err()
    );
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
    let mut value: ciborium::Value = ciborium::de::from_reader(&bytes[MAGIC.len()..]).unwrap();
    value.as_map_mut().unwrap().push((
        ciborium::Value::Text("slot".into()),
        ciborium::Value::Integer(0.into()),
    ));
    let mut duplicate = MAGIC.to_vec();
    ciborium::ser::into_writer(&value, &mut duplicate).unwrap();
    assert_eq!(super::preflight(&duplicate), Ok(()));
    assert!(super::decode_preflighted(&duplicate).is_err());
    let result: Result<Owned, _> = ciborium::de::from_reader(&duplicate[MAGIC.len()..]);
    assert!(alloc::format!("{:?}", result.err().unwrap()).contains("duplicate field"));
}

#[test]
fn json_and_binary_preserve_complete_integer_ranges() {
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
    let length = transport_codec::encode_plain(&value, &mut storage).unwrap();
    assert_eq!(transport_codec::preflight_plain(&storage[..length]), Ok(()));
    let actual: Numeric = serde_json::from_slice(&storage[..length]).unwrap();
    assert_eq!(actual, value);
    let mut binary = Vec::new();
    ciborium::ser::into_writer(&value, &mut binary).unwrap();
    assert_eq!(
        super::preflight::validate(&binary, MAX_PLAN_CBOR_BYTES),
        Ok(())
    );
    let actual: Numeric = ciborium::de::from_reader(binary.as_slice()).unwrap();
    assert_eq!(actual, value);
}

#[test]
fn direct_publication_still_invokes_checked_structured_configuration_constructor() {
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

fn field_mut<'a>(value: &'a mut ciborium::Value, name: &str) -> &'a mut ciborium::Value {
    &mut value
        .as_map_mut()
        .unwrap()
        .iter_mut()
        .find(|(key, _)| key.as_text() == Some(name))
        .unwrap()
        .1
}

#[test]
fn exact_expanded_plan_boundary_survives_outer_compression() {
    let command = fixture();
    let Command::PatchbayPublish { show, .. } = &command else {
        unreachable!()
    };
    let original = encode_command(&command).unwrap();
    let value: ciborium::Value = ciborium::de::from_reader(&original[MAGIC.len()..]).unwrap();
    for extra in [0, 1] {
        let mut value = value.clone();
        let plan = field_mut(
            field_mut(field_mut(&mut value, "show"), "planned_mask"),
            "plan",
        );
        plan.as_map_mut().unwrap().push((
            ciborium::Value::Text("padding".into()),
            ciborium::Value::Text("".into()),
        ));
        let mut encoded = Vec::new();
        ciborium::ser::into_writer(&*plan, &mut encoded).unwrap();
        let mut padding = MAX_PLAN_CBOR_BYTES - encoded.len() - 4 + extra;
        loop {
            *field_mut(plan, "padding") = ciborium::Value::Text(" ".repeat(padding));
            encoded.clear();
            ciborium::ser::into_writer(&*plan, &mut encoded).unwrap();
            let wanted = MAX_PLAN_CBOR_BYTES + extra;
            if encoded.len() == wanted {
                break;
            }
            padding = padding + wanted - encoded.len();
        }
        let mut wire = MAGIC.to_vec();
        ciborium::ser::into_writer(&value, &mut wire).unwrap();
        let compressed = super::super::compress_json(&wire).unwrap();
        assert!(compressed.len() <= crate::MAX_BLOB);
        let decoded = Workspace::new().unwrap().decode_command(&compressed);
        if extra == 0 {
            let Command::PatchbayPublish { show: actual, .. } = decoded.unwrap() else {
                unreachable!()
            };
            assert_eq!(actual, *show);
        } else {
            assert_eq!(decoded.err(), Some(Refusal::Bounds));
        }
    }
}
