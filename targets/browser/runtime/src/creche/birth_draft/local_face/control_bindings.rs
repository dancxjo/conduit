//! Finite renderer references admitted by the portable C1/C2 contracts.
//! Labels, values, membership and actions remain solely in the canonical Face.
use conduit_face::{
    prepare_face_collection, prepare_face_text_entry, ChoiceMultiplicity, Face, FaceRole,
};
use serde::Serialize;

#[derive(Serialize)]
pub(super) struct ControlBindings {
    face_identity: String,
    face_revision: String,
    text_entries: Vec<String>,
    collections: Vec<CollectionBinding>,
    unsupported: Vec<UnsupportedBinding>,
}
#[derive(Serialize)]
struct CollectionBinding {
    subject: String,
    mode: &'static str,
}
#[derive(Serialize)]
struct UnsupportedBinding {
    subject: String,
    contract: &'static str,
    reason: String,
}
impl ControlBindings {
    pub(super) fn prepare(face: &Face) -> Self {
        let mut bindings = Self {
            face_identity: face.identity.as_str().into(),
            face_revision: face.revision.to_string(),
            text_entries: Vec::new(),
            collections: Vec::new(),
            unsupported: Vec::new(),
        };
        for subject in &face.subjects {
            match subject.role {
                FaceRole::TextEntry => match prepare_face_text_entry(face, &subject.identity, 64) {
                    Ok(_) => bindings.text_entries.push(subject.identity.clone()),
                    Err(reason) => bindings.unsupported.push(UnsupportedBinding {
                        subject: subject.identity.clone(),
                        contract: "text-entry",
                        reason: format!("{reason:?}"),
                    }),
                },
                FaceRole::Collection => {
                    match prepare_face_collection(face, &subject.identity, 64) {
                        Ok(collection) => bindings.collections.push(CollectionBinding {
                            subject: subject.identity.clone(),
                            mode: match collection.multiplicity() {
                                None => "information",
                                Some(ChoiceMultiplicity::Exclusive) => "exclusive",
                                Some(ChoiceMultiplicity::Independent) => "independent",
                            },
                        }),
                        Err(reason) => bindings.unsupported.push(UnsupportedBinding {
                            subject: subject.identity.clone(),
                            contract: "collection",
                            reason: format!("{reason:?}"),
                        }),
                    }
                }
                _ => {}
            }
        }
        bindings
    }
}
