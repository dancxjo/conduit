//! Preserve the existing model signature's Source refinements at the Fore.
use alloc::{format, string::String};
use conduit_plot::{
    check_syntax_document, parse_syntax_document, rust_binding::NativeRustBinding, StartupCatalog,
};

pub(crate) fn install_signature_type(startup: &mut StartupCatalog) -> Result<(), String> {
    let mut owner = StartupCatalog::new();
    owner.insert_value_kind_alias(
        "ResourceRef",
        conduit_core::kind_id(conduit_core::RESOURCE_REFERENCE_INFO_ID),
    )?;
    for (name, ty) in [
        ("TensorValue", conduit_data::TensorValue::semantic_type()),
        (
            "SampledSignal",
            conduit_data::SampledSignal::semantic_type(),
        ),
        (
            "TensorElement",
            conduit_data::TensorElement::semantic_type(),
        ),
        (
            "TensorAxisRole",
            conduit_data::TensorAxisRole::semantic_type(),
        ),
    ] {
        owner.insert_structured_type(
            name,
            ty.map_err(|e| format!("model dependency Type: {e:?}"))?,
        )?;
    }
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("../types.conduit")),
        &owner,
    )
    .map_err(|e| format!("{}: {}", e.code, e.message))?;
    let ty = checked
        .native_types
        .iter()
        .find(|t| t.name == "ModelSignature")
        .ok_or("missing model signature Type")?;
    if ty.value_type
        != crate::ModelSignature::semantic_type().map_err(|e| format!("signature Type: {e:?}"))?
    {
        return Err("model signature binding differs from its owner Source".into());
    }
    startup.insert_checked_native_type("ModelSignature", ty)
}
