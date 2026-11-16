//! Checked optional archive construction, not initialized memory or admission.
//! Actual target ABI backing, active-log overlap and execution custody must still
//! pass the original preparation/admission gates before any archive is used.
pub use conduit_host_make::sign_archive::{initialization_contract, BASE_KIND, IMPLEMENTATION};
use conduit_host_make::{sign_archive::DeclaredSignArchiveBounds, CheckedHostConfiguration};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedSignArchiveConstruction(DeclaredSignArchiveBounds);
impl core::ops::Deref for CheckedSignArchiveConstruction {
    type Target = DeclaredSignArchiveBounds;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl CheckedSignArchiveConstruction {
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
            return Err("requires exact selected affine-prefix archive implementation".into());
        }
        Ok(DeclaredSignArchiveBounds::from_configuration(checked.configuration())?.map(Self))
    }
}
#[cfg(test)]
mod tests;
