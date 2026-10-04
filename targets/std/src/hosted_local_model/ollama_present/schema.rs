//! Provider-side grammar guidance for the finite wording proposal.
//! Exact Face grounding is still checked by `GenerativePresenterRequest`.

use conduit_presentation::GenerativePresenterInput;
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
                        "items": { "oneOf": [
                            clause("text", &["subject", "value"]),
                            clause("property", &["subject", "name", "value"]),
                            clause("action", &["identity", "name"]),
                        ] },
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

fn clause(kind: &str, fields: &[&str]) -> Value {
    let mut properties = json!({
        "kind": { "type": "string", "const": kind },
        "index": { "type": "integer", "minimum": 0 },
        "style": { "type": "string", "enum": ["direct", "guided"] },
    });
    let mut required = vec!["kind", "index", "style"];
    for field in fields {
        properties[*field] = json!({ "type": "string" });
        required.push(field);
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

    #[test]
    fn schema_binds_source_identity_and_requires_tagged_claims() {
        let request = super::super::proof_request().unwrap();
        let format = wording_format(&request.semantic_data, &[]);
        assert_eq!(
            format["properties"]["proposal"]["properties"]["source_presentation_identity"]["const"],
            request.semantic_data.source_presentation_identity,
        );
        assert_eq!(
            format["properties"]["proposal"]["properties"]["source_presentation_revision"]["const"],
            request.semantic_data.source_presentation_revision,
        );
        let variants = &format["properties"]["proposal"]["properties"]["clauses"]["items"]["oneOf"];
        assert_eq!(
            format["properties"]["proposal"]["properties"]["clauses"]["maxItems"],
            1,
        );
        assert_eq!(variants.as_array().unwrap().len(), 3);
        assert!(variants[0]["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|field| field == "style"));
    }
}
