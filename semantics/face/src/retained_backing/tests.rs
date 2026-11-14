use super::*;
use alloc::{string::String, vec};

fn empty() -> Face {
    Face {
        identity: FaceContentId("owned-face".into()),
        revision: 1,
        basis: FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        interaction_context: FaceInteractionContext::general(),
        subjects: vec![],
        relationships: vec![],
        composition: vec![],
        properties: vec![],
        text: vec![],
        actions: vec![],
        disclosures: vec![],
        temporal_references: vec![],
        temporal_facts: vec![],
    }
}
#[test]
fn spare_vector_and_string_capacity_are_charged_without_changing_content() {
    let mut face = empty();
    let before = face.retained_backing_bytes().unwrap();
    face.text.reserve_exact(9);
    assert_eq!(
        face.retained_backing_bytes(),
        Some(before + face.text.capacity() * size_of::<FaceText>())
    );
    let mut text = String::with_capacity(257);
    text.push('x');
    let payload = text.capacity();
    face.text.push(FaceText {
        subject: String::new(),
        text,
    });
    assert_eq!(
        face.retained_backing_bytes(),
        Some(before + face.text.capacity() * size_of::<FaceText>() + payload)
    );
    let identity = face.identity.clone();
    let capacity_before = face.retained_backing_bytes().unwrap();
    let old = face.text[0].text.capacity();
    face.text[0].text.reserve_exact(1000);
    assert_eq!(
        face.retained_backing_bytes(),
        Some(capacity_before + face.text[0].text.capacity() - old)
    );
    assert_eq!(face.identity, identity);
    assert_eq!(face.text[0].text, "x");
}
#[test]
fn nested_typed_contract_and_temporal_clock_capacity_are_not_wire_lengths() {
    let mut face = empty();
    let base = face.retained_backing_bytes().unwrap();
    let mut bytes = Vec::with_capacity(317);
    bytes.push(1);
    let contract = conduit_core::CheckedValueContract {
        value_kind: conduit_core::kind_id("value/bytes"),
        maximum_bytes: 512,
        constraints: Vec::with_capacity(11),
    };
    let nested = bytes.capacity() + contract.retained_backing_bytes().unwrap();
    face.properties.push(FaceProperty {
        subject: String::new(),
        name: String::new(),
        value: FacePropertyValue::TypedValue { contract, bytes },
    });
    assert_eq!(
        face.retained_backing_bytes(),
        Some(base + face.properties.capacity() * size_of::<FaceProperty>() + nested)
    );
    let mut clock = String::with_capacity(409);
    clock.push('c');
    let clock_bytes = clock.capacity();
    let previous = face.retained_backing_bytes().unwrap();
    face.temporal_references.push(TemporalReference {
        identity: String::new(),
        instant: TemporalInstant {
            ticks: 1,
            scale: TemporalScale::Milliseconds,
            clock_basis: clock,
            resolution_ticks: 1,
            uncertainty_ticks: 0,
        },
    });
    assert_eq!(
        face.retained_backing_bytes(),
        Some(
            previous
                + face.temporal_references.capacity() * size_of::<TemporalReference>()
                + clock_bytes
        )
    );
}
