//! Source inventory, query scope and an informational review collection.
use super::*;

impl Projection {
    pub(super) fn collections(&mut self, draft: &BirthDraft) {
        let inventory = format!("{}/initial-plots", self.root);
        // The migration view uses a status when the source query has no matches.
        // Preserve the actual empty collection and its source-owned query scope.
        if let Some(subject) = self.subjects.iter_mut().find(|s| s.identity == inventory) {
            subject.role = FaceRole::Collection;
            subject.name = "Plots to include".into();
        }
        let visible = self
            .relationships
            .iter()
            .filter(|r| {
                r.source == inventory
                    && r.kind == FaceRelationshipKind::Contains
                    && self
                        .subjects
                        .iter()
                        .any(|s| s.identity == r.target && s.role == FaceRole::Plot)
            })
            .count();
        let selected = draft
            .choices()
            .iter()
            .filter(|choice| choice.selected)
            .count();
        self.properties.extend([
            FaceProperty {
                subject: inventory.clone(),
                name: "item-count".into(),
                value: FacePropertyValue::Count(visible as u64),
            },
            FaceProperty {
                subject: inventory.clone(),
                name: "inventory-count".into(),
                value: FacePropertyValue::Count(draft.choices().len() as u64),
            },
            FaceProperty {
                subject: inventory.clone(),
                name: "selected-count".into(),
                value: FacePropertyValue::Count(selected as u64),
            },
            FaceProperty {
                subject: inventory.clone(),
                name: "choice-multiplicity".into(),
                value: FacePropertyValue::Text("independent".into()),
            },
        ]);
        let search = format!("{}/plot-search", self.root);
        if self
            .subjects
            .iter()
            .any(|subject| subject.identity == search)
        {
            self.properties.push(FaceProperty {
                subject: inventory.clone(),
                name: "query-source".into(),
                value: FacePropertyValue::Identity(search),
            });
        } else if !draft.search().is_empty() {
            self.properties.push(FaceProperty {
                subject: inventory.clone(),
                name: "source-query".into(),
                value: FacePropertyValue::TypedValue {
                    contract: CheckedValueContract::new(kind_id(UTF8_TEXT_VALUE_KIND), 128, vec![])
                        .expect("bounded UTF-8 source query"),
                    bytes: draft.search().as_bytes().to_vec(),
                },
            });
        }
        for (order, choice) in draft.choices().iter().enumerate() {
            let digest = crate::plot_choice_identity::plot_choice_identity(&choice.plot);
            let subject = format!("{inventory}/{digest}");
            if self.subjects.iter().any(|s| s.identity == subject) {
                self.properties.push(FaceProperty {
                    subject,
                    name: "order".into(),
                    value: FacePropertyValue::Count(order as u64),
                });
            }
        }
        let review = format!("{}/reviewed-plots", self.root);
        self.subjects.push(FaceSubject {
            identity: review.clone(),
            role: FaceRole::Collection,
            name: "Selected starting Plots".into(),
        });
        self.relationships.push(contains(&self.article, &review));
        self.disclosures.push(FaceDisclosure {
            subject: review.clone(),
            level: FaceDisclosureLevel::SelectedDetail,
        });
        self.properties.push(FaceProperty {
            subject: review.clone(),
            name: "item-count".into(),
            value: FacePropertyValue::Count(selected as u64),
        });
        for (order, choice) in draft
            .choices()
            .iter()
            .enumerate()
            .filter(|(_, choice)| choice.selected)
        {
            let digest = crate::plot_choice_identity::plot_choice_identity(&choice.plot);
            let identity = format!("{review}/{digest}");
            self.subjects.push(FaceSubject {
                identity: identity.clone(),
                role: FaceRole::Item,
                name: choice.title.clone(),
            });
            self.relationships.push(contains(&review, &identity));
            self.disclosures.push(FaceDisclosure {
                subject: identity.clone(),
                level: FaceDisclosureLevel::SelectedDetail,
            });
            self.properties.extend([
                FaceProperty {
                    subject: identity.clone(),
                    name: "checked-plot-id".into(),
                    value: FacePropertyValue::Identity(choice.plot.checked_plot_id.as_str().into()),
                },
                FaceProperty {
                    subject: identity,
                    name: "order".into(),
                    value: FacePropertyValue::Count(order as u64),
                },
            ]);
        }
    }
}
