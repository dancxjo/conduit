//! Checked meaning for Root custody of the last acknowledged catalog reference.
//! Storage residence, current authority and real Body installation stay external.
use conduit_core as c;

pub const MAXIMUM_CATALOG_SEED_BYTES: u32 = 1024;
pub struct RecordingCatalogSeedSource {
    pub plot: conduit_plot::ExpandedAuthoringPlot,
    pub value_type: c::StructuredInfoType,
}
/// Prepared recorder-domain input and its checked publication-to-seed Source.
/// This is not a publication receipt: Root must still admit the listed chunk
/// commits and independently retain the selected storage and authority.
pub struct PreparedRecordingCatalogCheckpoint {
    source: RecordingCatalogSeedSource,
    catalog: conduit_environment_runtime::recording::PreparedRecordingCatalog,
    reference: c::BoundedResourceRef,
}
impl PreparedRecordingCatalogCheckpoint {
    pub fn prepare(
        reference: c::BoundedResourceRef,
        catalog: conduit_environment_runtime::recording::PreparedRecordingCatalog,
    ) -> Result<Self, String> {
        if reference.extent.bytes != catalog.content().len() as u64 {
            return Err("catalog reference extent does not match prepared content".into());
        }
        let source = RecordingCatalogSeedSource::compile_checkpoint(&reference)?;
        Ok(Self {
            source,
            catalog,
            reference,
        })
    }
    pub fn source(&self) -> &RecordingCatalogSeedSource {
        &self.source
    }
    pub fn content(&self) -> &str {
        self.catalog.content()
    }
    pub fn reference(&self) -> &c::BoundedResourceRef {
        &self.reference
    }
    pub fn required_records(&self) -> usize {
        self.catalog.required_records()
    }
    pub fn required_content_bytes(&self) -> u64 {
        self.catalog.required_content_bytes()
    }
}
impl RecordingCatalogSeedSource {
    pub fn compile() -> Result<Self, String> {
        Self::compile_source(include_str!("../../profiles/catalog-seed.conduit"), false)
    }
    /// Recover the same Body/State without supplying a new value.
    pub fn compile_read_only() -> Result<Self, String> {
        Self::compile_source(
            include_str!("../../profiles/catalog-seed-read.conduit"),
            false,
        )
    }
    /// The returned publication value, never a supplied acknowledgement flag,
    /// supplies this view's seed update. Root admits prior chunk publications.
    pub fn compile_checkpoint(reference: &c::BoundedResourceRef) -> Result<Self, String> {
        reference
            .validate()
            .map_err(|error| format!("catalog reference: {error:?}"))?;
        if reference.content_profile.as_str() != "text/json-utf8@1"
            || reference.access_class.as_str() != "resource/snapshot@1"
            || reference.extent.items != Some(1)
            || reference.extent.bytes == 0
            || reference.extent.bytes > 4096
            || reference.lifetime.expires_at.is_some()
        {
            return Err("catalog snapshot profile".into());
        }
        let hex: String = reference
            .encode()
            .map_err(|error| format!("catalog reference: {error:?}"))?
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let source =
            include_str!("../../profiles/catalog-checkpoint.conduit").replace("@REFERENCE@", &hex);
        Self::compile_source(&source, true)
    }
    fn compile_source(source: &str, checkpoint: bool) -> Result<Self, String> {
        let mut startup = conduit_plot::StartupCatalog::new();
        startup
            .insert_value_kind_alias("ResourceRef", c::kind_id(c::RESOURCE_REFERENCE_INFO_ID))?;
        let mut profiles = conduit_plot::ProfileCatalog::new();
        if checkpoint {
            startup.insert_value_kind_alias("JsonText", c::kind_id("text/json-utf8@1"))?;
            conduit_semantic_catalog::install_resource_snapshot_catalogs(
                &mut startup,
                &mut profiles,
            )?;
            let publish = conduit_semantic_catalog::resource_snapshot_semantic_contract(true);
            startup.insert_fore(publish.kind_id.as_str(), publish.checked_front())?;
        }
        let syntax = conduit_plot::parse_syntax_document(source);
        startup.insert(conduit_plot::KindSignature {
            kind: "state/latest".into(),
            startup_parameters: vec![],
        })?;
        let checked = conduit_plot::check_syntax_document(&syntax, &startup)
            .map_err(|error| format!("catalog seed Source: {error:?}"))?;
        let value_type = c::optional_info_type(
            c::StructuredInfoType::leaf(c::kind_id(c::RESOURCE_REFERENCE_INFO_ID))
                .map_err(|error| format!("catalog reference type: {error:?}"))?,
        )
        .map_err(|error| format!("catalog seed type: {error:?}"))?;
        let plot = conduit_plot::expand_canonical_plot_for_authoring(
            &checked,
            "recorder-catalog",
            &profiles,
        )
        .map_err(|error| format!("catalog seed expansion: {error:?}"))?;
        Ok(Self { plot, value_type })
    }
}
