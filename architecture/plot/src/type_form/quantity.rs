//! Quantity representation selection does not change semantic Type identity.
use super::*;
pub(super) fn check(
    declaration: &TypeFormSyntax,
    catalog: &crate::StartupCatalog,
) -> Result<CheckedTypeForm, SyntaxCheckDiagnostic> {
    let failure = |message: &str| diagnostic(declaration.span, message.into());
    let parameters = declaration
        .quantity
        .as_ref()
        .ok_or_else(|| failure("missing quantity Form parameters"))?;
    if !declaration.mappings.is_empty() {
        return Err(failure("quantity Form cannot contain variant mappings"));
    }
    let role = catalog
        .physical
        .quantities
        .get(&declaration.value_type.text)
        .ok_or_else(|| failure("quantity Form requires a declared quantity Type"))?;
    let unit = catalog
        .physical
        .units
        .get(&parameters.unit.text)
        .copied()
        .map(conduit_core::Unit::from_definition)
        .ok_or_else(|| failure("Form requires an exact Unit declared in this Source catalogue"))?;
    if unit.family().identity() != role.family.identity() || !unit.admits_role(role.role) {
        return Err(failure(
            "Form Unit has a different family or role from its semantic Type",
        ));
    }
    let integer = match declaration.storage {
        TypeFormStorageSyntax::I16Le => conduit_core::QuantityFormInteger::I16Le,
        TypeFormStorageSyntax::U16Le => conduit_core::QuantityFormInteger::U16Le,
        _ => return Err(failure("unsupported quantity Form storage")),
    };
    let form = conduit_core::DecimalQuantityForm {
        integer,
        unit,
        exponent: parameters.exponent,
        role: role.role,
    };
    form.validate()
        .map_err(|_| failure("invalid quantity Form definition"))?;
    let mut digest = Sha256::new();
    digest.update(b"conduit.form.decimal-quantity/v1\0");
    for part in [
        declaration.name.text.as_bytes(),
        role.leaf_kind.as_str().as_bytes(),
        &unit.encode(),
        &parameters.exponent.to_le_bytes(),
        &[u8::from(
            integer == conduit_core::QuantityFormInteger::U16Le,
        )],
    ] {
        digest.update((part.len() as u64).to_le_bytes());
        digest.update(part);
    }
    let identity = format!("form/sha256:{:x}", digest.finalize());
    Ok(CheckedTypeForm {
        name: declaration.name.text.clone(),
        compatibility_id: identity,
        value_type_name: declaration.value_type.text.clone(),
        value_type: role.leaf_kind.clone(),
        storage: CheckedTypeFormStorage::DecimalQuantity(alloc::boxed::Box::new(form)),
        mappings: Vec::new(),
        invalid_refusal: CheckedTypeFormRefusal::InvalidQuantity,
        exact_bytes: 2,
        maximum_bytes: 2,
        maximum_decode_steps: 1024,
    })
}
