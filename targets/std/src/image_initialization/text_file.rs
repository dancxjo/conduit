//! Actual one-generation file ownership enters only during original construction.
use super::*;
use crate::kernel_preparation::StagedImageMemory;
use crate::selected_text_file::{InitialTextFile, TextFileInitialization};
use conduit_host_make::CheckedHostConfiguration;
impl StdHost {
    /// Initialize the selected timer and one exact durable Text generation on
    /// the original IMAGE counter. The real directory and independent grant
    /// are consumed; a declaration alone initializes no storage. This provides
    /// no disk-space reservation, Source authority, Body membership or Play.
    pub fn from_selected_timer_and_text_file_image(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &CheckedHostConfiguration,
        file: TextFileInitialization,
    ) -> Result<Self, String> {
        let memory = StagedImageMemory::verified(&config, image, checked, catalog)?;
        validate_checked_file_definition(checked, &file.definition)?;
        let file = InitialTextFile::prepare(&config, image, file)?;
        Self::from_selected_initial_bases(
            config.clone(),
            image,
            catalog,
            InitialBases {
                memory: Some(memory),
                text_file: Some(file),
                ..InitialBases::default()
            },
        )
    }
}

/// The IMAGE omits initialization values, so preserve their original checked
/// correspondence here instead of trusting runtime limits supplied by a caller.
pub(super) fn validate_checked_file_definition(
    checked: &CheckedHostConfiguration,
    d: &crate::NativeTextFileDefinition,
) -> Result<(), String> {
    let mut bases = checked
        .configuration()
        .bases
        .iter()
        .filter(|b| b.kind == conduit_std_offers::DURABLE_TEXT_BASE_KIND);
    let base = bases.next().ok_or("checked Host has no Text file Base")?;
    if bases.next().is_some()
        || !base.implementations.is_empty()
        || base.implementation.as_deref() != Some(d.base.implementation_id.as_str())
    {
        return Err("Text file owner differs from original checked Base selection".into());
    }
    let values = &base
        .initialization
        .as_ref()
        .ok_or("Text file lacks checked initialization")?
        .values;
    if values.len() != 3
        || [
            ("maximum_text_bytes", u64::from(d.maximum_text_bytes)),
            ("preparation_bytes", u64::from(d.preparation_bytes)),
            ("retained_bytes", u64::from(d.retained_bytes)),
        ]
        .iter()
        .any(|(key, value)| values.get(*key).and_then(serde_json::Value::as_u64) != Some(*value))
    {
        return Err("Text file limits differ from original checked initialization".into());
    }
    Ok(())
}
