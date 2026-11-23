//! Authored observation Types, including complete original image owner custody.
use alloc::{format, string::String};
use conduit_plot::{
    check_syntax_document, parse_syntax_document, CheckedSyntaxDocument, StartupCatalog,
};

pub const AMBIENT_DISPLAY_TYPES_SOURCE: &str = include_str!("../../types.conduit");

/// Allocating preparation only. No observation or effect is created.
pub fn prepare_ambient_display_types() -> Result<CheckedSyntaxDocument, String> {
    let image_owner = conduit_face::image_resource_source_catalog()?;
    check_syntax_document(
        &parse_syntax_document(AMBIENT_DISPLAY_TYPES_SOURCE),
        &image_owner,
    )
    .map_err(|error| format!("ambient display Types: {error:?}"))
}

/// Install exact checked Types/laws without granting their external effects.
pub fn install_ambient_display_types(startup: &mut StartupCatalog) -> Result<(), String> {
    let checked = prepare_ambient_display_types()?;
    let mut staged = startup.clone();
    for value_type in &checked.native_types {
        staged.insert_checked_native_type_from_document(
            &value_type.name,
            &checked,
            &value_type.identity,
        )?;
    }
    *startup = staged;
    Ok(())
}
