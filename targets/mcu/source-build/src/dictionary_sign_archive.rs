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
