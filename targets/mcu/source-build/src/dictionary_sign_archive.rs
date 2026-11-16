//! Checked optional archive construction, not initialized memory or admission.
//! Actual target ABI backing, active-log overlap and execution custody must still
//! pass the original preparation/admission gates before any archive is used.
pub use conduit_host_make::dictionary_sign_archive::{
    initialization_contract, BASE_KIND, IMPLEMENTATION,
};
use conduit_host_make::{
    dictionary_sign_archive::DeclaredDictionarySignArchiveBounds, CheckedHostConfiguration,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedDictionarySignArchiveConstruction(DeclaredDictionarySignArchiveBounds);
impl core::ops::Deref for CheckedDictionarySignArchiveConstruction {
    type Target = DeclaredDictionarySignArchiveBounds;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl CheckedDictionarySignArchiveConstruction {
    /// Fixed cfg for an explicitly selected checked construction. The caller
    /// controls Cargo emission; this does not install or allocate an archive.
    pub fn cfg_selection(&self) -> &'static str {
        "dictionary_sign_archive"
    }
    /// Target-compiled constants, never host ABI size estimates. Physical usize
    /// literals must be representable on the actual target or compilation fails.
    pub fn rust_module(&self) -> String {
        format!(
            "pub const DICTIONARY_ARCHIVE_EVENTS: usize = {};\n\
             pub const DICTIONARY_ARCHIVE_TEMPLATES: usize = {};\n\
             pub const DICTIONARY_ARCHIVE_SEGMENTS: usize = {};\n\
             pub const DICTIONARY_ARCHIVE_REQUEST_LANES: usize = {};\n\
             pub const DICTIONARY_ARCHIVE_REMOTE_STREAMS: usize = {};\n\
             pub const DICTIONARY_ARCHIVE_LIMITS: conduit_kernel::live_sign_archive::ArchiveLimits = conduit_kernel::live_sign_archive::ArchiveLimits {{ retained_events: {}, retained_bytes: {}, archive_backing_bytes: {}, transfer_overlap_bytes: {} }};\n",
            self.template_events, self.template_count, self.segment_runs,
            self.request_lanes, self.remote_streams, self.maximum_retained_events,
            self.maximum_canonical_bytes, self.maximum_archive_backing_bytes,
            self.maximum_transfer_overlap_bytes,
        )
    }
    pub fn from_configuration(checked: &CheckedHostConfiguration) -> Result<Option<Self>, String> {
        if !checked
            .configuration()
            .bases
            .iter()
            .any(|b| b.kind == BASE_KIND)
        {
            return Ok(None);
        }
        let mut selected = checked
            .resolved_bases()
            .iter()
            .filter(|(kind, _)| kind == BASE_KIND);
        if selected
            .next()
            .is_none_or(|(_, implementation)| implementation != IMPLEMENTATION)
            || selected.next().is_some()
        {
            return Err("requires exact selected dictionary-prefix archive implementation".into());
        }
        Ok(
            DeclaredDictionarySignArchiveBounds::from_configuration(checked.configuration())?
                .map(Self),
        )
    }
}
#[cfg(test)]
mod tests;
