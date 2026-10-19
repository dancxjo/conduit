//! Source-owned atomic field values, finite membership and edit action contracts.
use super::*;
use conduit_core::ValueConstraint;

impl Projection {
    pub(super) fn field(
        &mut self,
        subject: String,
        field: &FormField,
    ) -> Result<(), BirthFaceRefusal> {
        self.subjects.push(FaceSubject {
            identity: subject.clone(),
            role: FaceRole::TextEntry,
            name: field.label.clone(),
        });
        self.relationships.push(contains(&self.article, &subject));
        let input_details = format!("{subject}/input-details");
        self.subjects.push(FaceSubject {
            identity: input_details.clone(),
            role: FaceRole::Info,
            name: format!("{} input details", field.label),
        });
        self.relationships.push(contains(&subject, &input_details));
        self.disclosures.push(FaceDisclosure {
            subject: input_details.clone(),
            level: FaceDisclosureLevel::SelectedDetail,
        });
        self.words(&subject, field.help.clone());
        self.words(
            &input_details,
            format!(
                "Current value: {}.",
                if field.value.is_empty() {
                    "(empty)"
                } else {
                    &field.value
                }
            ),
        );
        if let Some(error) = &field.error {
            self.words(&subject, format!("Error: {error}"));
        }
        let mut argument =
            FaceActionArgument::text("value".into(), field.label.clone(), 0, field.value_capacity)
                .map_err(|_| BirthFaceRefusal::InvalidProjection)?;
        match &field.kind {
            FieldKind::Text => {}
            FieldKind::NamedSelect { options } => {
                let mut members: Vec<Vec<u8>> = options
                    .iter()
                    .map(|option| option.identity.as_bytes().to_vec())
                    .collect();
                members.sort();
                argument
                    .contract
                    .constraints
                    .push(ValueConstraint::CanonicalMembership {
                        members,
                        negated: false,
                    });
                self.finite_options(&subject, &input_details, field, options, &argument.contract)?;
            }
            _ => return Err(BirthFaceRefusal::UnsupportedMechanism),
        }
        argument
            .contract
            .validate(field.value.as_bytes())
            .map_err(|_| BirthFaceRefusal::InvalidProjection)?;
        // The same Source value supplies readable fallback and exact
        // input state. A Mask need not interpret the fallback prose.
        self.properties.extend([
            FaceProperty {
                subject: subject.clone(),
                name: "current-value".into(),
                value: FacePropertyValue::TypedValue {
                    contract: argument.contract.clone(),
                    bytes: field.value.as_bytes().to_vec(),
                },
            },
            FaceProperty {
                subject: subject.clone(),
                name: "value-change-action".into(),
                value: FacePropertyValue::Identity(field.input_action.identity.clone()),
            },
        ]);
        self.action(&subject, &field.input_action, vec![argument]);
        Ok(())
    }

    fn finite_options(
        &mut self,
        subject: &str,
        details: &str,
        field: &FormField,
        options: &[SelectOption],
        contract: &CheckedValueContract,
    ) -> Result<(), BirthFaceRefusal> {
        let collection = self
            .subjects
            .iter_mut()
            .find(|s| s.identity == details)
            .ok_or(BirthFaceRefusal::InvalidProjection)?;
        collection.role = FaceRole::Collection;
        collection.name = format!("{} choices", field.label);
        self.properties.extend([
            FaceProperty {
                subject: details.into(),
                name: "item-count".into(),
                value: FacePropertyValue::Count(options.len() as u64),
            },
            FaceProperty {
                subject: details.into(),
                name: "choice-multiplicity".into(),
                value: FacePropertyValue::Text("exclusive".into()),
            },
            FaceProperty {
                subject: details.into(),
                name: "minimum-selected-count".into(),
                value: FacePropertyValue::Count(1),
            },
            FaceProperty {
                subject: details.into(),
                name: "maximum-selected-count".into(),
                value: FacePropertyValue::Count(1),
            },
            FaceProperty {
                subject: details.into(),
                name: "selection-action".into(),
                value: FacePropertyValue::Identity(field.input_action.identity.clone()),
            },
            FaceProperty {
                subject: details.into(),
                name: "value-source".into(),
                value: FacePropertyValue::Identity(subject.into()),
            },
        ]);
        // The parent action alone owns membership. Each item carries its UTF-8
        // value without repeating the complete enumeration in every item.
        let value_contract = CheckedValueContract::new(
            kind_id(UTF8_TEXT_VALUE_KIND),
            contract.maximum_bytes,
            vec![],
        )
        .map_err(|_| BirthFaceRefusal::InvalidProjection)?;
        for (order, option) in options.iter().enumerate() {
            contract
                .validate(option.identity.as_bytes())
                .map_err(|_| BirthFaceRefusal::InvalidProjection)?;
            let digest = Sha256::digest(option.identity.as_bytes());
            let identity = format!("{details}/{digest:x}");
            self.subjects.push(FaceSubject {
                identity: identity.clone(),
                role: FaceRole::Item,
                name: option.label.clone(),
            });
            self.relationships.push(contains(details, &identity));
            self.disclosures.push(FaceDisclosure {
                subject: identity.clone(),
                level: FaceDisclosureLevel::SelectedDetail,
            });
            self.properties.extend([
                FaceProperty {
                    subject: identity.clone(),
                    name: "order".into(),
                    value: FacePropertyValue::Count(order as u64),
                },
                FaceProperty {
                    subject: identity.clone(),
                    name: "selected".into(),
                    value: FacePropertyValue::Flag(option.identity == field.value),
                },
                FaceProperty {
                    subject: identity,
                    name: "choice-value".into(),
                    value: FacePropertyValue::TypedValue {
                        contract: value_contract.clone(),
                        bytes: option.identity.as_bytes().to_vec(),
                    },
                },
            ]);
            self.words(
                details,
                format!(
                    "Choice: {}. Value: {}. {}.",
                    option.label,
                    option.identity,
                    if option.identity == field.value {
                        "Selected"
                    } else {
                        "Not selected"
                    }
                ),
            );
        }
        Ok(())
    }
}
