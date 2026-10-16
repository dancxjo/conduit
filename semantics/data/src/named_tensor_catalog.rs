//! Import the owner's complete checked Type, including refinement laws.
use alloc::{format, string::String};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};

pub fn install_named_tensor_type(startup: &mut StartupCatalog) -> Result<(), String> {
    let ty = checked_named_tensor_type()?;
    startup.insert_checked_native_type("NamedTensorBundle", &ty)
}

pub(crate) fn checked_named_tensor_type() -> Result<conduit_plot::CheckedNativeType, String> {
    let mut owner = StartupCatalog::new();
    owner.insert_value_kind_alias(
        "ResourceRef",
        conduit_core::kind_id(conduit_core::RESOURCE_REFERENCE_INFO_ID),
    )?;
    owner.insert_structured_type(
        "TemporalInstant",
        conduit_time::NativeTemporalInstant::semantic_type()
            .map_err(|e| format!("temporal Type: {e:?}"))?,
    )?;
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("../types.conduit")),
        &owner,
    )
    .map_err(|e| format!("{}: {}", e.code, e.message))?;
    let ty = checked
        .native_types
        .iter()
        .find(|t| t.name == "NamedTensorBundle")
        .ok_or("missing named tensor Type")?;
    if ty.value_type
        != crate::NamedTensorBundle::semantic_type().map_err(|e| format!("tensor Type: {e:?}"))?
    {
        return Err("named tensor binding differs from its owner Source".into());
    }
    Ok(ty.clone())
}
