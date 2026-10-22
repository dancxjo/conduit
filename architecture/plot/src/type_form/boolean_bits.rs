//! A representation of a finite Boolean record, independent of its meaning.
use super::*;

pub(super) fn check(
    declaration: &TypeFormSyntax,
    value_type: &CheckedNativeType,
) -> Result<CheckedTypeForm, SyntaxCheckDiagnostic> {
    let refuse = |message: &str| diagnostic(declaration.span, message.into());
    let StructuredInfoTypeShape::Record { fields, .. } = value_type.value_type.shape() else {
        return Err(refuse("u8 bits requires a Boolean record Type"));
    };
    if fields.is_empty() || fields.len() > 8 || !value_type.invariants.is_empty()
        || !value_type.value_contracts.is_empty()
        || fields.iter().any(|field| !matches!(field.value_type().shape(), StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == "value/bool"))
    {
        return Err(refuse("u8 bits requires 1–8 unconstrained Boolean fields"));
    }
    let order = if declaration.mappings.is_empty() {
        fields.iter().map(|field| field.name()).collect::<Vec<_>>()
    } else {
        declaration
            .mappings
            .iter()
            .map(|mapping| mapping.variant.text.as_str())
            .collect()
    };
    let names = order.iter().copied().collect::<BTreeSet<_>>();
    if order.len() != fields.len()
        || names.len() != fields.len()
        || fields.iter().any(|field| !names.contains(field.name()))
    {
        return Err(refuse("u8 bits must map each Boolean field exactly once"));
    }
    let mappings = order
        .iter()
        .enumerate()
        .map(|(bit, field)| CheckedTypeFormMapping {
            variant: (*field).into(),
            discriminant: 1 << bit,
        })
        .collect::<Vec<_>>();
    let mask = mappings
        .iter()
        .fold(0, |mask, field| mask | field.discriminant);
    let mut identity = b"conduit.form.boolean-bits@1\0".to_vec();
    push(&mut identity, declaration.name.text.as_bytes());
    push(&mut identity, value_type.identity.as_str().as_bytes());
    for mapping in &mappings {
        push(&mut identity, mapping.variant.as_bytes());
        identity.push(mapping.discriminant);
    }
    Ok(CheckedTypeForm {
        name: declaration.name.text.clone(),
        compatibility_id: format!("form/sha256:{:x}", Sha256::digest(identity)),
        value_type_name: value_type.name.clone(),
        value_type: value_type.identity.clone(),
        storage: CheckedTypeFormStorage::BooleanBits(mask),
        mappings,
        invalid_refusal: CheckedTypeFormRefusal::ReservedBits,
        exact_bytes: 1,
        maximum_bytes: 1,
        maximum_decode_steps: fields.len() as u16 + 2,
    })
}
