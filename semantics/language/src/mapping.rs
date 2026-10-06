//! Explicit finite mapping from external tags/model-private identities.
//! Exact equality is the only lookup law; no language detection or coverage.
use crate::{LanguageExternalIdentity, LanguageId, LanguageVariety};

pub const MAXIMUM_LANGUAGE_MAPPING_ROWS: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageSelection {
    language: LanguageId,
    variety: Option<LanguageVariety>,
}
impl LanguageSelection {
    pub fn new(
        language: LanguageId,
        variety: Option<LanguageVariety>,
    ) -> Result<Self, LanguageMappingRefusal> {
        if variety.as_ref().is_some_and(|v| v.language() != &language) {
            return Err(LanguageMappingRefusal::VarietyLanguage);
        }
        Ok(Self { language, variety })
    }
    pub fn language(&self) -> &LanguageId {
        &self.language
    }
    pub fn variety(&self) -> Option<&LanguageVariety> {
        self.variety.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageMappingRow {
    pub external: LanguageExternalIdentity,
    pub target: LanguageSelection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageMappingRefusal {
    Bound,
    Duplicate,
    Undeclared,
    VarietyLanguage,
}

pub struct LanguageMapping<'a> {
    rows: &'a [LanguageMappingRow],
}
impl<'a> LanguageMapping<'a> {
    pub fn new(rows: &'a [LanguageMappingRow]) -> Result<Self, LanguageMappingRefusal> {
        if rows.len() > MAXIMUM_LANGUAGE_MAPPING_ROWS {
            return Err(LanguageMappingRefusal::Bound);
        }
        for (i, row) in rows.iter().enumerate() {
            if rows[..i].iter().any(|prior| prior.external == row.external) {
                return Err(LanguageMappingRefusal::Duplicate);
            }
        }
        Ok(Self { rows })
    }
    pub fn resolve(
        &self,
        external: &LanguageExternalIdentity,
    ) -> Result<&'a LanguageSelection, LanguageMappingRefusal> {
        self.rows
            .iter()
            .find(|row| &row.external == external)
            .map(|row| &row.target)
            .ok_or(LanguageMappingRefusal::Undeclared)
    }
}
