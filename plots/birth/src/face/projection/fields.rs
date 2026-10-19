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
                for option in options {
                    self.words(
                        &input_details,
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
}
