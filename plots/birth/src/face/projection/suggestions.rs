//! Pure source composition of atomic text actions and bounded suggestions.
use super::*;

impl Projection {
    pub(super) fn suggestions(&mut self, draft: &BirthDraft) {
        let naming = format!("{}/name-system", self.root);
        self.suggestion_binding(&naming, &format!("{naming}/input-details"), "fixed");
        let field = format!("{}/body-name", self.root);
        let collection = format!("{}/name-suggestions", self.root);
        self.suggestion_binding(&field, &collection, "free");
        self.subjects.push(FaceSubject {
            identity: collection.clone(),
            role: FaceRole::Collection,
            name: "Suggested Body names".into(),
        });
        self.relationships.push(contains(&field, &collection));
        self.disclosures.push(FaceDisclosure {
            subject: collection.clone(),
            level: FaceDisclosureLevel::SelectedDetail,
        });
        self.properties.push(FaceProperty {
            subject: collection.clone(),
            name: "item-count".into(),
            value: FacePropertyValue::Count(1),
        });
        // Reuse the current source-owned candidate. Reading it never calls
        // suggest(), advances variation or changes the committed friendly name.
        let name = &draft.suggestion().name;
        let digest = Sha256::digest(name.as_bytes());
        let item = format!("{collection}/{digest:x}");
        self.subjects.push(FaceSubject {
            identity: item.clone(),
            role: FaceRole::Item,
            name: name.clone(),
        });
        self.relationships.push(contains(&collection, &item));
        self.disclosures.push(FaceDisclosure {
            subject: item.clone(),
            level: FaceDisclosureLevel::SelectedDetail,
        });
        self.properties.extend([
            FaceProperty {
                subject: item.clone(),
                name: "order".into(),
                value: FacePropertyValue::Count(0),
            },
            FaceProperty {
                subject: item,
                name: "choice-value".into(),
                value: FacePropertyValue::TypedValue {
                    contract: CheckedValueContract::new(kind_id(UTF8_TEXT_VALUE_KIND), 64, vec![])
                        .expect("bounded source name suggestion"),
                    bytes: name.as_bytes().to_vec(),
                },
            },
        ]);
    }

    fn suggestion_binding(&mut self, field: &str, collection: &str, policy: &str) {
        self.properties.extend([
            FaceProperty {
                subject: field.into(),
                name: "suggestion-source".into(),
                value: FacePropertyValue::Identity(collection.into()),
            },
            FaceProperty {
                subject: field.into(),
                name: "suggestion-policy".into(),
                value: FacePropertyValue::Text(policy.into()),
            },
        ]);
    }
}
