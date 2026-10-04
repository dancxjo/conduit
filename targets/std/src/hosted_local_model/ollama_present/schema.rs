//! Provider-side grammar guidance for the finite wording proposal.
//! Exact Face grounding is still checked by `GenerativePresenterRequest`.

use conduit_presentation::{GenerativePresenterInput, PresentationPropertyValue};
use serde_json::{json, Value};

pub(in crate::hosted_local_model) fn wording_format(
    input: &GenerativePresenterInput,
    available_actions: &[String],
) -> Value {
    let action_items = if available_actions.is_empty() {
        json!({ "type": "string" })
    } else {
        json!({ "type": "string", "enum": available_actions })
    };
    let mut clauses = Vec::new();
    for (index, text) in input.presentation.text.iter().enumerate() {
        clauses.push(clause(
            "text",
            index,
            &[("subject", &text.subject), ("value", &text.text)],
        ));
    }
    for (index, property) in input.presentation.properties.iter().enumerate() {
        let value = match &property.value {
            PresentationPropertyValue::Text(value) | PresentationPropertyValue::Identity(value) => {
                value.clone()
            }
            PresentationPropertyValue::Count(value) => value.to_string(),
            PresentationPropertyValue::Signed(value) => value.to_string(),
            PresentationPropertyValue::Flag(value) => value.to_string(),
            _ => continue,
        };
        clauses.push(clause(
            "property",
            index,
            &[
                ("subject", &property.subject),
                ("name", &property.name),
                ("value", &value),
            ],
        ));
    }
    for (index, action) in input.presentation.actions.iter().enumerate() {
        if action.availability.is_available() && available_actions.contains(&action.identity) {
            clauses.push(clause(
                "action",
                index,
                &[("identity", &action.identity), ("name", &action.name)],
            ));
        }
    }
    json!({
        "type": "object",
        "properties": {
            "proposal": {
                "type": "object",
                "properties": {
                    "source_presentation_identity": {
                        "type": "string",
                        "const": input.source_presentation_identity,
                    },
                    "source_presentation_revision": {
                        "type": "integer",
                        "const": input.source_presentation_revision,
                    },
                    "clauses": {
                        "type": "array",
                        "items": { "oneOf": clauses },
                        "minItems": 1,
                        // The selected Host Back has a 256-token output budget.
                        // One claim leaves room for the exact source envelope.
                        "maxItems": 1,
                    },
                },
                "required": [
                    "source_presentation_identity",
                    "source_presentation_revision",
                    "clauses",
                ],
                "additionalProperties": false,
            },
            "suggested_action_identities": {
                "type": "array",
                "items": action_items,
                "maxItems": available_actions.len(),
            },
        },
        "required": ["proposal", "suggested_action_identities"],
        "additionalProperties": false,
    })
}

fn clause(kind: &str, index: usize, fields: &[(&str, &str)]) -> Value {
    let mut properties = json!({
        "kind": { "type": "string", "const": kind },
        "index": { "type": "integer", "const": index },
        "style": { "type": "string", "enum": ["direct", "guided"] },
    });
    let mut required = vec!["kind", "index", "style"];
    for (field, value) in fields {
        properties[*field] = json!({ "type": "string", "const": value });
        required.push(*field);
    }
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

#[cfg(test)]
mod tests {
    use super::wording_format;
    use conduit_presentation::{PresentationProperty, PresentationPropertyValue, PresentationText};

    #[test]
    fn schema_binds_source_identity_and_requires_tagged_claims() {
        let mut input = super::super::proof_request().unwrap().semantic_data;
        input.presentation.text.push(PresentationText {
            subject: "body/other".into(),
            text: "Another current fact.".into(),
        });
        input.presentation.properties.push(PresentationProperty {
            subject: "body/current".into(),
            name: "count".into(),
            value: PresentationPropertyValue::Count(2),
        });
        let format = wording_format(&input, &["body.inspect".into()]);
        assert_eq!(
            format["properties"]["proposal"]["properties"]["source_presentation_identity"]["const"],
            input.source_presentation_identity,
        );
        assert_eq!(
            format["properties"]["proposal"]["properties"]["source_presentation_revision"]["const"],
            input.source_presentation_revision,
        );
        let variants = &format["properties"]["proposal"]["properties"]["clauses"]["items"]["oneOf"];
        assert_eq!(
            format["properties"]["proposal"]["properties"]["clauses"]["maxItems"],
            1,
        );
        assert_eq!(variants.as_array().unwrap().len(), 4);
        assert!(variants[0]["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|field| field == "style"));
        assert_eq!(variants[0]["properties"]["index"]["const"], 0);
        assert_eq!(
            variants[0]["properties"]["subject"]["const"],
            "body/current"
        );
        assert_eq!(variants[0]["properties"]["value"]["const"], "I am awake.");
        assert_eq!(variants[1]["properties"]["index"]["const"], 1);
        assert_eq!(variants[1]["properties"]["subject"]["const"], "body/other");
        assert_eq!(
            variants[2]["properties"]["subject"]["const"],
            "body/current"
        );
        assert_eq!(variants[2]["properties"]["name"]["const"], "count");
        assert_eq!(variants[2]["properties"]["value"]["const"], "2");
        assert_eq!(
            variants[3]["properties"]["identity"]["const"],
            "body.inspect"
        );
        assert_eq!(variants[3]["properties"]["name"]["const"], "Inspect Body");
        let without_action = wording_format(&input, &[]);
        assert_eq!(
            without_action["properties"]["proposal"]["properties"]["clauses"]["items"]["oneOf"]
                .as_array()
                .unwrap()
                .len(),
            3,
        );
    }
}
