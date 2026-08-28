use std::collections::BTreeMap;

use conduit_form::{
    parse_syntax_document, ConstructionRole, ExpressionSyntax, StructuredExpressionField,
};
use serde::de::DeserializeOwned;
use serde_json::{Map, Number, Value};

use crate::{BodyBindingTarget, BodyDescription, BodyDescriptionDiagnostic, BodyHostDescription};

pub fn parse_body_description_conduit(
    source: &str,
) -> Result<BodyDescription, BodyDescriptionDiagnostic> {
    let document = parse_syntax_document(source);
    if let Some(diagnostic) = document.diagnostics.first() {
        return Err(BodyDescriptionDiagnostic::Decode {
            detail: format!(
                "{} at {}:{}",
                diagnostic.message, diagnostic.span.line, diagnostic.span.column
            ),
        });
    }
    if !document.forms.is_empty() {
        return Err(decode_error(
            "construction source must not contain Form definitions",
        ));
    }
    if document.constructions.len() != 1 {
        return Err(decode_error(
            "construction source must contain exactly one document",
        ));
    }
    let construction = document
        .constructions
        .into_iter()
        .next()
        .expect("one construction was required");
    if construction.role != ConstructionRole::Body {
        return Err(decode_error("expected a body document"));
    }
    let declarations = construction.declarations.iter().fold(
        BTreeMap::<&str, Vec<&ExpressionSyntax>>::new(),
        |mut values, declaration| {
            values
                .entry(declaration.name.text.as_str())
                .or_default()
                .push(&declaration.value.syntax);
            values
        },
    );
    let schema = one_required::<u32>(&declarations, "schema").map_err(decode_error)?;
    let id = one_required::<String>(&declarations, "id").map_err(decode_error)?;
    let hosts = repeated::<BodyHostDescription>(&declarations, "host").map_err(decode_error)?;
    reject_unknown(&declarations, &["schema", "id", "host"]).map_err(decode_error)?;
    Ok(BodyDescription {
        schema,
        name: construction.name.text,
        body: BodyBindingTarget { id },
        hosts,
    })
}

fn one_required<T: DeserializeOwned>(
    declarations: &BTreeMap<&str, Vec<&ExpressionSyntax>>,
    name: &str,
) -> Result<T, String> {
    let Some(values) = declarations.get(name) else {
        return Err(format!("missing required '{name}' declaration"));
    };
    let [value] = values.as_slice() else {
        return Err(format!("duplicate conflicting '{name}' declaration"));
    };
    decode(value).map_err(|detail| format!("invalid '{name}' declaration: {detail}"))
}

fn repeated<T: DeserializeOwned>(
    declarations: &BTreeMap<&str, Vec<&ExpressionSyntax>>,
    name: &str,
) -> Result<Vec<T>, String> {
    declarations
        .get(name)
        .into_iter()
        .flat_map(|values| values.iter())
        .map(|value| {
            decode(value).map_err(|detail| format!("invalid '{name}' declaration: {detail}"))
        })
        .collect()
}

fn reject_unknown(
    declarations: &BTreeMap<&str, Vec<&ExpressionSyntax>>,
    allowed: &[&str],
) -> Result<(), String> {
    if let Some(name) = declarations.keys().find(|name| !allowed.contains(name)) {
        return Err(format!("unknown construction declaration '{name}'"));
    }
    Ok(())
}

fn decode<T: DeserializeOwned>(syntax: &ExpressionSyntax) -> Result<T, String> {
    serde_json::from_value(value(syntax)?).map_err(|error| error.to_string())
}

fn value(syntax: &ExpressionSyntax) -> Result<Value, String> {
    match syntax {
        ExpressionSyntax::Atomic(atom) => atomic(&atom.text),
        ExpressionSyntax::Collection { values, .. } => values
            .iter()
            .map(value)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        ExpressionSyntax::Record { fields, .. } => record(fields),
        ExpressionSyntax::Variant { tag, .. } => Err(format!(
            "variant '{}' is not a construction value",
            tag.text
        )),
    }
}

fn record(fields: &[StructuredExpressionField]) -> Result<Value, String> {
    let mut object = Map::new();
    for field in fields {
        if object.contains_key(&field.name.text) {
            return Err(format!("duplicate record field '{}'", field.name.text));
        }
        object.insert(field.name.text.clone(), value(&field.value)?);
    }
    Ok(Value::Object(object))
}

fn atomic(text: &str) -> Result<Value, String> {
    if text.starts_with('"') {
        return serde_json::from_str::<String>(text)
            .map(Value::String)
            .map_err(|error| error.to_string());
    }
    if text.starts_with('\'') && text.ends_with('\'') && text.len() >= 2 {
        return Ok(Value::String(text[1..text.len() - 1].to_string()));
    }
    match text {
        "true" => return Ok(Value::Bool(true)),
        "false" => return Ok(Value::Bool(false)),
        _ => {}
    }
    if let Ok(value) = text.parse::<u64>() {
        return Ok(Value::Number(Number::from(value)));
    }
    Err(format!(
        "'{text}' must be a quoted string, boolean, or unsigned integer"
    ))
}

fn decode_error(detail: impl Into<String>) -> BodyDescriptionDiagnostic {
    BodyDescriptionDiagnostic::Decode {
        detail: detail.into(),
    }
}
