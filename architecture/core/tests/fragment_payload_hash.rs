//! Fingerprints must not duplicate immutable computation/configuration payloads.
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "common/sealed_state.rs"]
mod fixture;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core::{
    compute_fragment_id, kind_id, ConfigurationEntry as Entry, ConfigurationValue as Value,
    PlanFragment, StructuredConfigurationValue, StructuredInfoType, StructuredInfoValue,
};
fn fragment(size: usize) -> PlanFragment {
    let mut fragment = fixture::fragment();
    let value_type = StructuredInfoType::leaf(kind_id("fixture/blob")).unwrap();
    let profile = value_type.profile().unwrap().value_kind().clone();
    let structured = StructuredInfoValue::leaf(value_type, vec![0xa5; size]).unwrap();
    fragment.placements[0].configuration = vec![
        Entry {
            key: "before".into(),
            value: Value::Bool(true),
        },
        Entry {
            key: "text".into(),
            value: Value::Text("\0é🌡".repeat(size)),
        },
        Entry {
            key: "between".into(),
            value: Value::U64(0x8000_0000_0000_0001),
        },
        Entry {
            key: "structured".into(),
            value: Value::Structured(
                StructuredConfigurationValue::new(profile, structured.canonical_bytes().unwrap())
                    .unwrap(),
            ),
        },
        Entry {
            key: "after".into(),
            value: Value::Text("after-payload".into()),
        },
        Entry {
            key: "negative".into(),
            value: Value::I64(-37),
        },
    ];
    fragment
}
#[test]
fn immutable_payload_lengths_preserve_the_historical_fingerprint() {
    // Frozen from the historical buffered encoder, before direct payload hashing.
    let cases = [
        (
            0,
            "537e695912087ae8a2a339d464c83b9eb451df461754afe59793cd5bfbf31aa0",
        ),
        (
            1,
            "85a444409959e6343f0b5948c9107221616472dbfd8d772c4f98a6ddde909292",
        ),
        (
            15,
            "b433ed41a18a78f02259c31cb6909311c7ec8bdf9a86ba07809ebd5ac759f11c",
        ),
        (
            511,
            "83e97ef7e2bd2b5384234c468c51bc0e87d4351c448880113e1d87a2983e9bac",
        ),
        (
            4096,
            "9d5dbaf626281cd4eaf15b104eed447c001a60e4b0390f916a94c35b02b76ba8",
        ),
        (
            32768,
            "da865bb97a0de241d50fea44d5502dd4ad9992fcb10c6907981ba293dc35e88f",
        ),
    ];
    let plan_ids = [
        "06581783418f0734f00816dc4e64d3c6cd9e11e7f26e262473270f9d2519e51e",
        "d2b6b9eb2ca60ba5b96f8a0373ac97597eb57080be3ac64a23ec42b2c7da70f4",
        "4aa4b5b1c36101c7cf0952f27cc4a18a816b86332ed2218111c660f178fe6664",
        "ad7d225ccf9bd2dde47483da5b568c868d9bae857821fdf7015fbfbcf86cf3f2",
        "2c7c7a10bba91b90ce4ff8f0b4ce7483a6d13be82008ea1e34a7e908350cb024",
        "3feb54ffdc40ae28928c68d264ca482acc885e9e6c301416371bcc8adc8d8c50",
    ];
    for ((size, expected), plan_id) in cases.into_iter().zip(plan_ids) {
        let fragment = fragment(size);
        let id = compute_fragment_id(&fragment);
        assert_eq!(id.as_str(), expected, "payload size {size}");
        assert_eq!(fixture::seal(fragment.clone()).plan_id.as_str(), plan_id);
    }
}

#[test]
fn fingerprint_scratch_does_not_grow_with_immutable_payloads() {
    for size in [0, 1, 15, 511, 4096, 32768] {
        let fragment = fragment(size);
        let (_, memory) = allocation_probe::observe(|| compute_fragment_id(&fragment));
        assert!(
            memory.peak_bytes <= 2048,
            "payload size {size} copied into scratch: {memory:?}"
        );
    }
}

#[test]
fn equal_length_payload_changes_still_change_identity() {
    let original = fragment(4096);
    let expected = compute_fragment_id(&original);
    let mut changed = original.clone();
    let Value::Text(text) = &mut changed.placements[0].configuration[1].value else {
        unreachable!()
    };
    text.replace_range(7 * 1234..7 * 1234 + 1, "x");
    assert_ne!(compute_fragment_id(&changed), expected);

    let mut changed = original;
    let Value::Structured(value) = &changed.placements[0].configuration[3].value else {
        unreachable!()
    };
    let mut bytes = value.canonical_value().to_vec();
    *bytes.last_mut().unwrap() ^= 1;
    let replacement = StructuredConfigurationValue::new(value.profile().clone(), bytes).unwrap();
    changed.placements[0].configuration[3].value = Value::Structured(replacement);
    assert_ne!(compute_fragment_id(&changed), expected);
}

#[test]
fn callable_front_keeps_its_independently_encoded_fingerprint() {
    use conduit_core::{
        CheckedFront, FrontStartupParameter, PortDescriptor, PortDirection, PortTemporal,
    };
    let port = |id: &str, direction, temporal, abnormal_kind| PortDescriptor {
        port_id: conduit_core::port_id(id),
        value_kind: kind_id("fixture/byte@1"),
        direction,
        temporal,
        abnormal_kind,
    };
    let front = CheckedFront::new(
        vec![FrontStartupParameter {
            name: "literal é".into(),
            value_type: kind_id("fixture/source@1"),
            has_default: true,
        }],
        vec![port(
            "input",
            PortDirection::Input,
            PortTemporal::Value,
            Some(kind_id("fixture/error@1")),
        )],
        vec![port(
            "output",
            PortDirection::Output,
            PortTemporal::Flow { closes: true },
            None,
        )],
        Some((
            conduit_core::port_id("input"),
            conduit_core::port_id("output"),
        )),
    );
    // SHA-256 of the historical 155-byte length-prefixed encoding, computed
    // independently from the field specification rather than this encoder.
    assert_eq!(
        conduit_core::compute_checked_front_fingerprint(&front),
        "bdc96ef1e3938d5ac8784150172228785f2cf0c7f006c169b10c4e8b9c5190ee"
    );
}
