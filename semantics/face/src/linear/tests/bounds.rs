use super::*;
use conduit_core::{kind_id, CheckedValueContract};

#[test]
fn complete_valid_face_refuses_expanded_linear_bytes_without_truncation() {
    let mut face = exact_face();
    let contract =
        CheckedValueContract::new(kind_id("value/text"), crate::MAX_FACE_VALUE_BYTES, vec![])
            .unwrap();
    // The complete canonical Face fits its existing 512 KiB semantic ceiling.
    // Its exact numeric-byte observation exceeds the independent 2 MiB ceiling.
    face.properties = (0..430)
        .map(|index| FaceProperty {
            subject: "subject/play".into(),
            name: format!("value/{index}"),
            value: FacePropertyValue::TypedValue {
                contract: contract.clone(),
                bytes: vec![b'z'; crate::MAX_FACE_VALUE_BYTES as usize],
            },
        })
        .collect();
    let face = Face::new(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
    )
    .unwrap();
    assert!(face.content_bytes() <= crate::MAX_FACE_TOTAL_BYTES);
    assert_eq!(
        render_linear_face(&face),
        Err(LinearFaceError::TooManyBytes)
    );
}

#[test]
fn complete_valid_face_refuses_excess_linear_records_without_truncation() {
    let mut basis = exact_face().basis;
    basis.sign_ids = (0..crate::MAX_FACE_SIGNS)
        .map(|index| SignId::from(format!("sign/{index:04}")))
        .collect();
    let subjects: Vec<_> = (0..crate::MAX_FACE_SUBJECTS)
        .map(|index| FaceSubject {
            identity: format!("s/{index}"),
            role: FaceRole::Info,
            name: "Observed".into(),
        })
        .collect();
    // Ordinary distinct, acyclic relationships need no invented source meaning.
    let mut relationships = Vec::new();
    for source in 0..3 {
        for target in source + 1..crate::MAX_FACE_SUBJECTS {
            if relationships.len() == crate::MAX_FACE_RELATIONSHIPS {
                break;
            }
            relationships.push(FaceRelationship {
                source: format!("s/{source}"),
                target: format!("s/{target}"),
                kind: FaceRelationshipKind::Contains,
            });
        }
    }
    let properties = (0..crate::MAX_FACE_PROPERTIES)
        .map(|index| FaceProperty {
            subject: "s/0".into(),
            name: format!("p/{index}"),
            value: FacePropertyValue::Count(0),
        })
        .collect();
    let text = (0..crate::MAX_FACE_TEXT_ITEMS)
        .map(|_| FaceText {
            subject: "s/0".into(),
            text: "Observed".into(),
        })
        .collect();
    let disclosures = subjects
        .iter()
        .map(|subject| crate::FaceDisclosure {
            subject: subject.identity.clone(),
            level: crate::FaceDisclosureLevel::Primary,
        })
        .collect();
    let face = Face::new_with_semantics(
        1,
        basis,
        subjects,
        relationships,
        properties,
        text,
        vec![],
        disclosures,
    )
    .unwrap();
    assert!(face.content_bytes() <= crate::MAX_FACE_TOTAL_BYTES);
    assert_eq!(
        render_linear_face(&face),
        Err(LinearFaceError::TooManyLines)
    );
}
