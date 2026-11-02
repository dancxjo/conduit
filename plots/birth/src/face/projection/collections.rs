//! Complete source inventory and read-only review of selected Source/check pairs.
use super::*;

impl Projection {
    fn count(&mut self, subject: &str, name: &str, value: usize) {
        let value = FacePropertyValue::Count(value as u64);
        if let Some(property) = self
            .properties
            .iter_mut()
            .find(|p| p.subject == subject && p.name == name)
        {
            property.value = value;
        } else {
            self.properties.push(FaceProperty {
                subject: subject.into(),
                name: name.into(),
                value,
            });
        }
    }
    pub(super) fn collections(&mut self, draft: &BirthDraft) -> Result<(), BirthFaceRefusal> {
        let inventory = format!("{}/initial-plots", self.root);
        let subject = self
            .subjects
            .iter_mut()
            .find(|s| s.identity == inventory)
            .ok_or(BirthFaceRefusal::InvalidProjection)?;
        // A source query with no matches is still an empty collection, not missing meaning.
        subject.role = FaceRole::Collection;
        subject.name = "Plots to include".into();
        let visible = self
            .relationships
            .iter()
            .filter(|r| r.source == inventory && r.kind == FaceRelationshipKind::Contains)
            .count();
        self.count(&inventory, "item-count", visible);
        self.count(&inventory, "inventory-count", draft.choices().len());
        let selected = draft.choices().iter().filter(|c| c.selected).count();
        self.count(&inventory, "selected-count", selected);
        if !self
            .properties
            .iter()
            .any(|p| p.subject == inventory && p.name == "choice-multiplicity")
        {
            self.properties.push(FaceProperty {
                subject: inventory.clone(),
                name: "choice-multiplicity".into(),
                value: FacePropertyValue::Text("independent".into()),
            });
        }
        let search = format!("{}/plot-search", self.root);
        if self.subjects.iter().any(|s| s.identity == search) {
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
                        .map_err(|_| BirthFaceRefusal::InvalidProjection)?,
                    bytes: draft.search().as_bytes().to_vec(),
                },
            });
        }
        if visible == 0 {
            self.words(&inventory, "No Plots match your search.".into());
        }
        let review = format!("{}/reviewed-plots", self.root);
        self.subjects.push(FaceSubject {
            identity: review.clone(),
            role: FaceRole::Collection,
            name: "Selected starting Plots".into(),
        });
        self.relationships.push(contains(&self.article, &review));
        let level = if self.selected_review_in_primary {
            FaceDisclosureLevel::Primary
        } else {
            FaceDisclosureLevel::SelectedDetail
        };
        self.disclosures.push(FaceDisclosure {
            subject: review.clone(),
            level,
        });
        self.count(&review, "item-count", selected);
        let mut previous = None;
        for (order, choice) in draft
            .choices()
            .iter()
            .enumerate()
            .filter(|(_, c)| c.selected)
        {
            // Keep every bit of the current authoritative Source/check-pair digest.
            let identity = format!(
                "{}/review",
                crate::plot_choice_identity::plot_choice_identity(&choice.plot)
            );
            self.subjects.push(FaceSubject {
                identity: identity.clone(),
                role: FaceRole::Item,
                name: choice.title.clone(),
            });
            self.relationships.push(contains(&review, &identity));
            if let Some(source) = previous.replace(identity.clone()) {
                self.relationships.push(FaceRelationship {
                    source,
                    target: identity.clone(),
                    kind: FaceRelationshipKind::Semantic(kind_id("document/precedes")),
                });
            }
            self.disclosures.push(FaceDisclosure {
                subject: identity.clone(),
                level,
            });
            for (name, value) in [
                (
                    "checked-plot-id",
                    FacePropertyValue::Identity(choice.plot.checked_plot_id.as_str().into()),
                ),
                ("order", FacePropertyValue::Count(order as u64)),
            ] {
                self.properties.push(FaceProperty {
                    subject: identity.clone(),
                    name: name.into(),
                    value,
                });
            }
        }
        Ok(())
    }
}
