//! Test-local compiler fixture; no restored make library or execution entrance.
use conduit_core as c;
use conduit_plot::{ExpandedAuthoringPlot, ProfileCatalog, StartupCatalog};
pub const MAXIMUM_SEED_BYTES: u32 = 1024;
pub struct Compiled {
    pub plot: ExpandedAuthoringPlot,
    pub value_type: c::StructuredInfoType,
}
pub fn checkpoint(reference: &c::BoundedResourceRef) -> Result<Compiled, String> {
    reference
        .validate()
        .map_err(|why| format!("catalogue reference: {why:?}"))?;
    if reference.content_profile.as_str() != "text/json-utf8@1"
        || reference.access_class.as_str() != "resource/snapshot@1"
        || reference.extent.items != Some(1)
        || !(1..=4096).contains(&reference.extent.bytes)
        || reference.lifetime.expires_at.is_some()
    {
        return Err("catalogue snapshot profile".into());
    }
    let hex = reference
        .encode()
        .map_err(|why| format!("catalogue reference: {why:?}"))?
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    compile(
        &include_str!("../../../profiles/catalog-checkpoint.conduit").replace("@REFERENCE@", &hex),
        true,
    )
}
pub fn read_only() -> Result<Compiled, String> {
    compile(
        include_str!("../../../profiles/catalog-seed-read.conduit"),
        false,
    )
}
fn compile(source: &str, checkpoint: bool) -> Result<Compiled, String> {
    let mut startup = StartupCatalog::new();
    startup.insert_value_kind_alias("ResourceRef", c::kind_id(c::RESOURCE_REFERENCE_INFO_ID))?;
    let mut profiles = ProfileCatalog::new();
    if checkpoint {
        startup.insert_value_kind_alias("JsonText", c::kind_id("text/json-utf8@1"))?;
        conduit_semantic_catalog::install_resource_snapshot_catalogs(&mut startup, &mut profiles)?;
        let publish = conduit_semantic_catalog::resource_snapshot_semantic_contract(true);
        startup.insert_fore(publish.kind_id.as_str(), publish.checked_front())?;
    }
    startup.insert(conduit_plot::KindSignature {
        kind: "state/latest".into(),
        startup_parameters: vec![],
    })?;
    let syntax = conduit_plot::parse_syntax_document(source);
    let checked = conduit_plot::check_syntax_document(&syntax, &startup)
        .map_err(|why| format!("catalogue Source: {why:?}"))?;
    let value_type = c::optional_info_type(
        c::StructuredInfoType::leaf(c::kind_id(c::RESOURCE_REFERENCE_INFO_ID))
            .map_err(|why| format!("reference Type: {why:?}"))?,
    )
    .map_err(|why| format!("optional Type: {why:?}"))?;
    let plot =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "recorder-catalog", &profiles)
            .map_err(|why| format!("catalogue expansion: {why:?}"))?;
    Ok(Compiled { plot, value_type })
}
