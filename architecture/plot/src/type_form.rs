use crate::checked_syntax::{
    CheckedNativeType, CheckedTypeForm, CheckedTypeFormMapping, CheckedTypeFormRefusal,
    CheckedTypeFormStorage, SyntaxCheckDiagnostic,
};
use crate::prelude::*;
use crate::syntax::{TypeDefinitionSyntax, TypeFormStorageSyntax, TypeFormSyntax, TypeSyntax};
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{kind_id, StructuredInfoTypeShape};
use sha2::{Digest, Sha256};

pub(crate) fn check_type_forms(
    declarations: &[TypeFormSyntax],
    type_declarations: &[TypeSyntax],
    native_types: &[CheckedNativeType],
) -> Result<Vec<CheckedTypeForm>, SyntaxCheckDiagnostic> {
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
                    "form '{}' is declared more than once",
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
                        "form '{}' names unknown source Type '{}'",
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
                    "u8 form requires a finite variant Type".into(),
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
        let unit = kind_id("value/empty");
        for case in cases {
            if !matches!(case.payload_type().shape(), StructuredInfoTypeShape::Leaf(kind) if kind == &unit)
            {
                return Err(diagnostic(
                    declaration.value_type.span,
                    alloc::format!(
                        "u8 form cannot encode payload-bearing variant '{}'",
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
                        alloc::format!("form is missing variant '{}'", case.tag()),
                    ));
                }
            }
            if let Some((unknown, mapping)) = authored
                .iter()
                .find(|(variant, _)| !cases.iter().any(|case| case.tag() == **variant))
            {
                return Err(diagnostic(
                    mapping.variant.span,
                    alloc::format!("form names unknown variant '{unknown}'"),
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
            .map(|(offset, variant)| {
                Ok(CheckedTypeFormMapping {
                    variant: variant.into(),
                    discriminant: declaration
                        .first_discriminant
                        .checked_add(u8::try_from(offset).map_err(|_| {
                            diagnostic(declaration.span, "u8 form has too many variants".into())
                        })?)
                        .ok_or_else(|| {
                            diagnostic(declaration.span, "u8 form iota exceeds 255".into())
                        })?,
                })
            })
            .collect::<Result<Vec<_>, SyntaxCheckDiagnostic>>()?;
        let storage = match declaration.storage {
            TypeFormStorageSyntax::U8 => CheckedTypeFormStorage::U8,
        };
        let maximum_decode_steps = u16::try_from(mappings.len() + 1)
            .map_err(|_| diagnostic(declaration.span, "form decode work exceeds bounds".into()))?;
        let invalid_refusal = CheckedTypeFormRefusal::InvalidTag;
        let compatibility_id = compatibility_id(
            &declaration.name.text,
            value_type.identity.as_str(),
            &mappings,
            invalid_refusal.as_str(),
        );
        checked.push(CheckedTypeForm {
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
    mappings: &[CheckedTypeFormMapping],
    invalid_refusal: &str,
) -> String {
    // Frozen compatibility domain: the language noun changed, not existing bytes or identities.
    let mut canonical = b"conduit.code.u8@1\0".to_vec();
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
