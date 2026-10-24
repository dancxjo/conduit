#![cfg(feature = "face-projection")]
use conduit_core::{
    self as c, PreparedStructuredComposer as Composer, StructuredInfoType as Type,
    StructuredInfoTypeShape as Shape,
};
use conduit_environment_runtime::{
    publication::{PublicationChannel as Channel, PublicationContract, PublicationSnapshot},
    publication_face::PreparedPublicationFace,
};
use conduit_face::{FaceContributionBasis, FacePropertyValue as Value};
fn basis() -> FaceContributionBasis {
    FaceContributionBasis {
        checked_plot_id: "checked/fixture".into(),
        plan_id: "plan/fixture".into(),
        active_play_id: "play/fixture".into(),
        required_interaction_context: None,
    }
}
fn payload<'a>(ty: &'a Type, tag: &str) -> &'a Type {
    let Shape::Variant { cases, .. } = ty.shape() else {
        panic!("variant expected")
    };
    cases
        .iter()
        .find(|case| case.tag() == tag)
        .unwrap()
        .payload_type()
}
fn empty(ty: &Type) -> Vec<u8> {
    let mut encoder = Composer::new(ty, 4096).unwrap();
    encoder.leaf(&[]).unwrap().to_vec()
}
#[test]
fn physical_quantities_survive_projection_and_freshness_is_explicit() {
    let bytes = include_bytes!(
        "../../evidence/2026-10-23-native-offer-withdrawal/physical-observation-1.bin"
    );
    let mut projector = PreparedPublicationFace::prepare().unwrap();
    for fresh in [true, false] {
        let face = projector
            .project(
                PublicationSnapshot {
                    channel: Channel::Reading,
                    sequence: 1,
                    canonical: bytes,
                },
                fresh,
                basis(),
            )
            .unwrap();
        assert_eq!(face.basis, basis());
        assert!(face
            .properties
            .iter()
            .any(|p| p.name == "fresh" && p.value == Value::Flag(fresh)));
        for name in ["temperature", "relative-humidity"] {
            let source = c::validate_canonical_structured_value(bytes)
                .unwrap()
                .variant_payload("observation")
                .unwrap()
                .unwrap();
            let (field, kind) = if name == "temperature" {
                ("temperature", c::TEMPERATURE_INFO_ID)
            } else {
                ("humidity", c::RATIO_INFO_ID)
            };
            let expected = source
                .record_field(field)
                .unwrap()
                .unwrap()
                .primitive_bytes(kind)
                .unwrap();
            let property = face.properties.iter().find(|p| p.name == name).unwrap();
            let Value::TypedValue { bytes, .. } = &property.value else {
                panic!("quantity expected")
            };
            assert_eq!(bytes, expected);
        }
    }
    assert!(projector
        .project(
            PublicationSnapshot {
                channel: Channel::ProviderFailure,
                sequence: 2,
                canonical: bytes
            },
            true,
            basis()
        )
        .is_err());
}
#[test]
fn failures_remain_distinct_and_do_not_retain_previous_quantities() {
    let mut projector = PreparedPublicationFace::prepare().unwrap();
    let contract = PublicationContract::prepare(Channel::Reading).unwrap();
    let failure_type = payload(contract.value_type(), "unavailable");
    for tag in [
        "short-frame",
        "malformed-length",
        "checksum",
        "invalid-payload",
    ] {
        let mut leaf_encoder = Composer::new(payload(failure_type, tag), 4096).unwrap();
        let leaf = leaf_encoder
            .leaf(if matches!(tag, "short-frame" | "malformed-length") {
                &[0]
            } else {
                &[]
            })
            .unwrap()
            .to_vec();
        let mut failure = Composer::new(failure_type, 4096).unwrap();
        failure
            .variant(tag, c::validate_canonical_structured_value(&leaf).unwrap())
            .unwrap();
        let mut result = Composer::new(contract.value_type(), 4096).unwrap();
        let bytes = result
            .variant(
                "unavailable",
                c::validate_canonical_structured_value(failure.encoded()).unwrap(),
            )
            .unwrap();
        check_failure(&mut projector, Channel::Reading, bytes, tag);
    }
    for (channel, reason) in [
        (Channel::ProviderFailure, "provider-failed"),
        (Channel::GeometryRefusal, "geometry-refused"),
    ] {
        let bytes = empty(PublicationContract::prepare(channel).unwrap().value_type());
        check_failure(&mut projector, channel, &bytes, reason);
    }
}
fn check_failure(
    projector: &mut PreparedPublicationFace,
    channel: Channel,
    bytes: &[u8],
    reason: &str,
) {
    let face = projector
        .project(
            PublicationSnapshot {
                channel,
                sequence: 2,
                canonical: bytes,
            },
            true,
            basis(),
        )
        .unwrap();
    assert!(face
        .properties
        .iter()
        .any(|p| p.name == "sensor-failure" && p.value == Value::Text(reason.into())));
    assert!(face
        .properties
        .iter()
        .any(|p| p.name == "fresh" && p.value == Value::Flag(false)));
    assert!(!face
        .properties
        .iter()
        .any(|p| matches!(p.value, Value::TypedValue { .. })));
}
