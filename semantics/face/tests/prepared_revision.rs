//! Exact portable Face revisions can be validated and resealed after allocation
//! admission. No renderer-specific or sensor-specific identity is introduced.
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core::{
    kind_id, CheckedValueContract, Quantity, QUANTITY_ENCODED_LEN, TEMPERATURE_INFO_ID,
};
use conduit_face::{
    Face, FaceBasis, FaceError, FaceProperty, FacePropertyValue, FaceRole, FaceSubject,
};

fn fixture() -> Face {
    Face::new(
        1,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some("source/environment".into()),
            checked_plot_id: Some("checked/environment".into()),
            expanded_plot_id: Some("expanded/environment".into()),
            plan_id: Some("plan/environment".into()),
            active_play_id: Some("play/environment".into()),
            sign_ids: vec![],
        },
        vec![FaceSubject {
            identity: "environment".into(),
            role: FaceRole::Info,
            name: "Room".into(),
        }],
        vec![],
        vec![FaceProperty {
            subject: "environment".into(),
            name: "temperature".into(),
            value: FacePropertyValue::TypedValue {
                contract: CheckedValueContract::new(
                    kind_id(TEMPERATURE_INFO_ID),
                    QUANTITY_ENCODED_LEN as u32,
                    vec![],
                )
                .unwrap(),
                bytes: Quantity::parse_plot_literal("19°C")
                    .unwrap()
                    .encode()
                    .to_vec(),
            },
        }],
        vec![],
    )
    .unwrap()
}
#[test]
fn prepared_revision_keeps_canonical_identity_without_allocating() {
    let mut face = fixture();
    let old = face.identity.clone();
    let next = Quantity::parse_plot_literal("20°C").unwrap().encode();
    let (result, allocation) = allocation_probe::observe(|| {
        face.validate()?;
        face.revision += 1;
        let FacePropertyValue::TypedValue { bytes, .. } = &mut face.properties[0].value else {
            unreachable!()
        };
        bytes.copy_from_slice(&next);
        assert_eq!(face.validate(), Err(FaceError::InvalidIdentity));
        face.reseal_prepared()?;
        face.validate()
    });
    result.unwrap();
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    assert_ne!(old, face.identity);
    // The ordinary constructor and prepared publication must produce the same
    // identity, including provenance, units, contracts and revision.
    let expected = Face::new(
        face.revision,
        face.basis.clone(),
        face.subjects.clone(),
        face.relationships.clone(),
        face.properties.clone(),
        face.text.clone(),
    )
    .unwrap();
    assert_eq!(face, expected);
}
#[test]
fn refused_content_does_not_receive_a_valid_identity() {
    let mut face = fixture();
    let old = face.identity.clone();
    face.properties[0].subject = "unknown".into();
    let (result, allocation) = allocation_probe::observe(|| face.reseal_prepared());
    assert_eq!(result, Err(FaceError::UnknownPropertySubject));
    assert_eq!(face.identity, old);
    assert!(face.validate().is_err());
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
}
