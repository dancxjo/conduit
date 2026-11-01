//! One recording publication's checked meaning, with sealed external Fore.
//! Reference selection creates no storage, initialized Back or authority.
use conduit_core as c;
pub struct RecordingPublicationSource {
    pub plot: conduit_plot::ExpandedAuthoringPlot,
}
impl RecordingPublicationSource {
    pub fn compile(reference: &c::BoundedResourceRef) -> Result<Self, String> {
        reference
            .validate()
            .map_err(|error| format!("reference: {error:?}"))?;
        if reference.content_profile.as_str() != "text/json-utf8@1"
            || reference.access_class.as_str() != "resource/snapshot@1"
            || reference.extent.items != Some(1)
            || reference.extent.bytes == 0
            || reference.extent.bytes > 4096
            || reference.lifetime.expires_at.is_some()
        {
            return Err("recording snapshot profile".into());
        }
        let mut startup = conduit_plot::StartupCatalog::new();
        startup
            .insert_value_kind_alias("ResourceRef", c::kind_id(c::RESOURCE_REFERENCE_INFO_ID))?;
        startup.insert_value_kind_alias("JsonText", c::kind_id("text/json-utf8@1"))?;
        let mut profiles = conduit_plot::ProfileCatalog::new();
        conduit_semantic_catalog::install_resource_snapshot_catalogs(&mut startup, &mut profiles)?;
        let kind = conduit_semantic_catalog::resource_snapshot_semantic_contract(true);
        startup.insert_fore(kind.kind_id.as_str(), kind.checked_front())?;
        let encoded: String = reference
            .encode()
            .map_err(|error| format!("reference: {error:?}"))?
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let source = include_str!("../../profiles/recording-publication.conduit")
            .replace("@REFERENCE@", &encoded);
        let checked = conduit_plot::check_syntax_document(
            &conduit_plot::parse_syntax_document(&source),
            &startup,
        )
        .map_err(|error| format!("recording publication Source: {error:?}"))?;
        let plot = conduit_plot::expand_canonical_plot_for_authoring(
            &checked,
            "recorder-publication",
            &profiles,
        )
        .map_err(|error| format!("recording publication expansion: {error:?}"))?;
        Ok(Self { plot })
    }
}
