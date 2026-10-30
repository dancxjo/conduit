//! Checked meaning for Root custody of the last acknowledged catalog reference.
//! Storage residence, current authority and real Body installation stay external.
use conduit_core as c;

pub const MAXIMUM_CATALOG_SEED_BYTES: u32 = 1024;
pub struct RecordingCatalogSeedSource {
    pub plot: conduit_plot::ExpandedAuthoringPlot,
    pub value_type: c::StructuredInfoType,
}
impl RecordingCatalogSeedSource {
    pub fn compile() -> Result<Self, String> {
        Self::compile_source(include_str!("../../profiles/catalog-seed.conduit"))
    }
    /// Recover the same Body/State without supplying a new value.
    pub fn compile_read_only() -> Result<Self, String> {
        Self::compile_source(include_str!("../../profiles/catalog-seed-read.conduit"))
    }
    fn compile_source(source: &str) -> Result<Self, String> {
        let mut startup = conduit_plot::StartupCatalog::new();
        startup
            .insert_value_kind_alias("ResourceRef", c::kind_id(c::RESOURCE_REFERENCE_INFO_ID))?;
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
        let profiles = conduit_plot::ProfileCatalog::new();
        let plot = conduit_plot::expand_canonical_plot_for_authoring(
            &checked,
            "recorder-catalog",
            &profiles,
        )
        .map_err(|error| format!("catalog seed expansion: {error:?}"))?;
        Ok(Self { plot, value_type })
    }
}
