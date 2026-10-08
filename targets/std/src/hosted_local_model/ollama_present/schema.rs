//! Provider-side grammar guidance for the finite wording proposal.
//! Exact Face grounding is still checked by `GenerativePresenterRequest`.

use conduit_presentation::GenerativePresenterInput;
use serde_json::{json, Value};

pub(in crate::hosted_local_model) fn wording_format(
    input: &GenerativePresenterInput,
    available_actions: &[String],
) -> Result<Value, String> {
    let outline = crate::spoken_face_mask::select_spoken_outline(&input.presentation)
        .map_err(|error| format!("select finite speech grammar: {error:?}"))?;
    let available_actions = available_actions
        .iter()
        .filter(|identity| outline.action_ids.contains(identity))
        .cloned()
        .collect::<Vec<_>>();
    let action_items = if available_actions.is_empty() {
        json!({ "type": "string" })
    } else {
        json!({ "type": "string", "enum": available_actions })
    };
    let mut clauses = Vec::new();
    for (index, text) in input
        .presentation
        .text
        .iter()
        .enumerate()
        .filter(|(index, _)| outline.text_indices.contains(&(*index as u32)))
    {
        clauses.push(clause(
            "text",
            index,
            &[("subject", &text.subject), ("value", &text.text)],
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
    if clauses.is_empty() {
        return Err("current Face has no renderable finite wording clause".into());
    }
    Ok(json!({
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
    }))
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
    #[test]
    fn schema_binds_source_identity_and_only_selected_tagged_claims() {
        let input = super::super::proof_request().unwrap().semantic_data;
        let outline = crate::spoken_face_mask::select_spoken_outline(&input.presentation).unwrap();
        let format = wording_format(&input, &["body.inspect".into()]).unwrap();
        let proposal = &format["properties"]["proposal"]["properties"];
        assert_eq!(
            proposal["source_presentation_identity"]["const"],
            input.source_presentation_identity
        );
        assert_eq!(
            proposal["source_presentation_revision"]["const"],
            input.source_presentation_revision
        );
        assert_eq!(proposal["clauses"]["maxItems"], 1);
        let variants = proposal["clauses"]["items"]["oneOf"].as_array().unwrap();
        assert_eq!(
            variants.len(),
            outline.text_indices.len() + outline.action_ids.len()
        );
        assert!(variants
            .iter()
            .all(|variant| variant["properties"]["kind"]["const"] != "property"));
        for variant in variants {
            assert!(variant["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|field| field == "style"));
            if variant["properties"]["kind"]["const"] == "text" {
                let index = variant["properties"]["index"]["const"].as_u64().unwrap() as usize;
                assert!(outline.text_indices.contains(&(index as u32)));
                assert_eq!(
                    variant["properties"]["value"]["const"],
                    input.presentation.text[index].text
                );
            }
        }
        let without_action = wording_format(&input, &[]).unwrap();
        assert_eq!(
            without_action["properties"]["proposal"]["properties"]["clauses"]["items"]["oneOf"]
                .as_array()
                .unwrap()
                .len(),
            outline.text_indices.len()
        );
        let mut invalid = input;
        invalid.presentation.revision += 1;
        assert!(wording_format(&invalid, &[]).is_err());
    }
}
