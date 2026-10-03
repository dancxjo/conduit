//! Rust name projection; semantic identities never change with escaping.
use super::*;

pub(in crate::rust_binding) fn rust_pascal_identifier(
    value: &str,
) -> Result<String, RustBindingGenerationError> {
    rust_identifier(value, true)
}

pub(in crate::rust_binding) fn rust_snake_identifier(
    value: &str,
) -> Result<String, RustBindingGenerationError> {
    rust_identifier(value, false)
}

pub(super) fn rust_screaming_identifier(value: &str) -> Result<String, RustBindingGenerationError> {
    rust_identifier(value, false).map(|value| {
        value
            .strip_prefix("r#")
            .unwrap_or(&value)
            .to_ascii_uppercase()
    })
}

fn rust_identifier(value: &str, pascal: bool) -> Result<String, RustBindingGenerationError> {
    let mut output = String::new();
    for part in value.split(|character: char| !character.is_ascii_alphanumeric()) {
        if part.is_empty() {
            continue;
        }
        if pascal {
            let mut characters = part.chars();
            if let Some(first) = characters.next() {
                output.push(first.to_ascii_uppercase());
                output.extend(characters);
            }
        } else {
            if !output.is_empty() {
                output.push('_');
            }
            output.push_str(&part.to_ascii_lowercase());
        }
    }
    if output.is_empty()
        || output.as_bytes()[0].is_ascii_digit()
        || output == "Self"
        || (!pascal && matches!(output.as_str(), "self" | "super" | "crate"))
    {
        return Err(RustBindingGenerationError::InvalidRustIdentifier(
            value.into(),
        ));
    }
    if !pascal && RUST_KEYWORDS.contains(&output.as_str()) {
        output.insert_str(0, "r#");
    }
    Ok(output)
}

pub(super) fn byte_literals(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(", ")
}

const RUST_KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for",
    "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where",
    "while", "async", "await", "dyn", "abstract", "become", "box", "do", "final", "macro",
    "override", "priv", "typeof", "unsized", "virtual", "yield", "try", "gen",
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reserved_words_escape_without_changing_semantic_spelling() {
        assert_eq!(rust_snake_identifier("final").unwrap(), "r#final");
        assert_eq!(rust_snake_identifier("type").unwrap(), "r#type");
        assert_eq!(rust_screaming_identifier("final").unwrap(), "FINAL");
        assert_eq!(rust_pascal_identifier("final").unwrap(), "Final");
        assert!(rust_snake_identifier("self").is_err());
        assert!(rust_pascal_identifier("self").is_err());
    }
}
