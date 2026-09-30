use crate::checked_syntax::{
    CheckedNativeType, CheckedRepresentation, CheckedRepresentationMapping,
    CheckedRepresentationRefusal, CheckedRepresentationStorage, SyntaxCheckDiagnostic,
};
use crate::prelude::*;
use crate::syntax::{
    RepresentationStorageSyntax, RepresentationSyntax, TypeDefinitionSyntax, TypeSyntax,
};
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{kind_id, StructuredInfoTypeShape};
use sha2::{Digest, Sha256};

pub(crate) fn check_representations(
    declarations: &[RepresentationSyntax],
    type_declarations: &[TypeSyntax],
    native_types: &[CheckedNativeType],
) -> Result<Vec<CheckedRepresentation>, SyntaxCheckDiagnostic> {
    let types = native_types
        .iter()
        .map(|value_type| (value_type.name.as_str(), value_type))
        .collect::<BTreeMap<_, _>>();
    let mut names = BTreeSet::new();
    let mut checked = Vec::with_capacity(declarations.len());
    for declaration in declarations {
        if !names.insert(declaration.name.text.as_str()) {
            return Err(diagnostic(
                declaration.name.span,
                alloc::format!(
                    "representation '{}' is declared more than once",
                    declaration.name.text
                ),
            ));
        }
        let value_type = types
            .get(declaration.value_type.text.as_str())
            .ok_or_else(|| {
                diagnostic(
                    declaration.value_type.span,
                    alloc::format!(
                        "representation '{}' names unknown source Type '{}'",
                        declaration.name.text,
                        declaration.value_type.text
                    ),
                )
            })?;
        let cases = match value_type.value_type.shape() {
            StructuredInfoTypeShape::Variant { cases, .. } => cases,
            _ => {
                return Err(diagnostic(
                    declaration.value_type.span,
                    "u8 representation requires a finite variant Type".into(),
                ));
            }
        };
        let source_cases = type_declarations
            .iter()
            .find(|candidate| candidate.name.text == declaration.value_type.text)
            .and_then(|candidate| match &candidate.definition {
                TypeDefinitionSyntax::Variant(cases) => Some(cases.as_slice()),
                _ => None,
            })
            .expect("checked local variant retains its source declaration");
        let mut authored = BTreeMap::new();
        for mapping in &declaration.mappings {
            if authored
                .insert(mapping.variant.text.as_str(), mapping)
                .is_some()
            {
                return Err(diagnostic(
                    mapping.variant.span,
                    alloc::format!(
                        "variant '{}' is mapped more than once",
                        mapping.variant.text
                    ),
                ));
            }
        }
        let unit = kind_id("value/unit");
        for case in cases {
            if !matches!(case.payload_type().shape(), StructuredInfoTypeShape::Leaf(kind) if kind == &unit)
            {
                return Err(diagnostic(
                    declaration.value_type.span,
                    alloc::format!(
                        "u8 representation cannot encode payload-bearing variant '{}'",
                        case.tag()
                    ),
                ));
            }
        }
        let ordered = if declaration.mappings.is_empty() {
            source_cases
                .iter()
                .map(|case| case.tag.text.as_str())
                .collect::<Vec<_>>()
        } else {
            for case in cases {
                if !authored.contains_key(case.tag()) {
                    return Err(diagnostic(
                        declaration.span,
                        alloc::format!("representation is missing variant '{}'", case.tag()),
                    ));
                }
            }
            if let Some((unknown, mapping)) = authored
                .iter()
                .find(|(variant, _)| !cases.iter().any(|case| case.tag() == **variant))
            {
                return Err(diagnostic(
                    mapping.variant.span,
                    alloc::format!("representation names unknown variant '{unknown}'"),
                ));
            }
            declaration
                .mappings
                .iter()
                .map(|mapping| mapping.variant.text.as_str())
                .collect()
        };
        let mappings = ordered
            .into_iter()
            .enumerate()
            .map(|(discriminant, variant)| {
                Ok(CheckedRepresentationMapping {
                    variant: variant.into(),
                    discriminant: u8::try_from(discriminant).map_err(|_| {
                        diagnostic(
                            declaration.span,
                            "u8 representation has too many variants".into(),
                        )
                    })?,
                })
            })
            .collect::<Result<Vec<_>, SyntaxCheckDiagnostic>>()?;
        let storage = match declaration.storage {
            RepresentationStorageSyntax::U8 => CheckedRepresentationStorage::U8,
        };
        let maximum_decode_steps = u16::try_from(mappings.len() + 1).map_err(|_| {
            diagnostic(
                declaration.span,
                "representation decode work exceeds bounds".into(),
            )
        })?;
        let invalid_refusal = CheckedRepresentationRefusal::InvalidTag;
        let compatibility_id = compatibility_id(
            &declaration.name.text,
            value_type.identity.as_str(),
            &mappings,
            invalid_refusal.as_str(),
        );
        checked.push(CheckedRepresentation {
            name: declaration.name.text.clone(),
            compatibility_id,
            value_type_name: value_type.name.clone(),
            value_type: value_type.identity.clone(),
            storage,
            mappings,
            invalid_refusal,
            exact_bytes: 1,
            maximum_bytes: 1,
            maximum_decode_steps,
        });
    }
    checked.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(checked)
}

fn compatibility_id(
    identity: &str,
    value_type: &str,
    mappings: &[CheckedRepresentationMapping],
    invalid_refusal: &str,
) -> String {
    let mut canonical = b"conduit.representation.u8@1\0".to_vec();
    push(&mut canonical, identity.as_bytes());
    push(&mut canonical, value_type.as_bytes());
    for mapping in mappings {
        push(&mut canonical, mapping.variant.as_bytes());
        canonical.push(mapping.discriminant);
    }
    push(&mut canonical, invalid_refusal.as_bytes());
    let digest = Sha256::digest(canonical);
    let mut suffix = String::with_capacity(64);
    for byte in digest {
        use core::fmt::Write;
        write!(&mut suffix, "{byte:02x}").expect("writing to String cannot fail");
    }
    alloc::format!("{identity}@{suffix}")
}

fn push(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
}

fn diagnostic(span: crate::Span, message: String) -> SyntaxCheckDiagnostic {
    SyntaxCheckDiagnostic {
        code: "CND-FRM-059",
        span,
        message,
    }
}
