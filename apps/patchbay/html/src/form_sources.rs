//! Host-owned loading of explicitly admitted canonical Form sources.

use conduit_core::SignId;
use patchbay_model::{FormCandidate, MAX_FORM_SOURCE_BYTES, MAX_FRONT_DOOR_FORMS};
use std::collections::BTreeSet;
use std::path::PathBuf;

pub const MAX_ADDITIONAL_FORMS: usize = MAX_FRONT_DOOR_FORMS - 1;
pub const MAX_FORM_LABEL_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormSource {
    pub label: String,
    pub path: PathBuf,
}

impl FormSource {
    pub fn new(label: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            label: label.into(),
            path: path.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormSourceError {
    TooManySources,
    InvalidLabel,
    NonCanonicalExtension(PathBuf),
    MissingOrNonFile(PathBuf),
    SourceTooLarge(PathBuf),
    NonUtf8Path(PathBuf),
    ReadFailed(PathBuf),
    InvalidSource { path: PathBuf, detail: String },
    DuplicateForm(PathBuf),
}

impl std::fmt::Display for FormSourceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Form source registration failed: {self:?}")
    }
}

impl std::error::Error for FormSourceError {}

pub fn load_form_sources(sources: &[FormSource]) -> Result<Vec<FormCandidate>, FormSourceError> {
    if sources.len() > MAX_ADDITIONAL_FORMS {
        return Err(FormSourceError::TooManySources);
    }
    let mut identities = BTreeSet::new();
    let mut candidates = Vec::with_capacity(sources.len());
    for (index, source) in sources.iter().enumerate() {
        validate_source(source)?;
        let metadata = std::fs::metadata(&source.path)
            .map_err(|_| FormSourceError::MissingOrNonFile(source.path.clone()))?;
        if !metadata.is_file() {
            return Err(FormSourceError::MissingOrNonFile(source.path.clone()));
        }
        if metadata.len() > MAX_FORM_SOURCE_BYTES as u64 {
            return Err(FormSourceError::SourceTooLarge(source.path.clone()));
        }
        let source_name = source
            .path
            .to_str()
            .ok_or_else(|| FormSourceError::NonUtf8Path(source.path.clone()))?;
        let bytes = std::fs::read(&source.path)
            .map_err(|_| FormSourceError::ReadFailed(source.path.clone()))?;
        let text = String::from_utf8(bytes).map_err(|_| FormSourceError::InvalidSource {
            path: source.path.clone(),
            detail: "source is not valid UTF-8".into(),
        })?;
        let candidate = FormCandidate::from_source(
            &source.label,
            source_name,
            text,
            format!("explicit repository Form {}", source.path.display()),
            SignId::from(format!("patchbay-html/form-source/{}", index + 1)),
            index as u64 + 2,
        )
        .map_err(|detail| FormSourceError::InvalidSource {
            path: source.path.clone(),
            detail,
        })?;
        if !identities.insert(candidate.checked_form_id.as_str().to_owned()) {
            return Err(FormSourceError::DuplicateForm(source.path.clone()));
        }
        candidates.push(candidate);
    }
    Ok(candidates)
}

fn validate_source(source: &FormSource) -> Result<(), FormSourceError> {
    if source.label.is_empty()
        || source.label.len() > MAX_FORM_LABEL_BYTES
        || source.label.chars().any(char::is_control)
    {
        return Err(FormSourceError::InvalidLabel);
    }
    if source.path.extension().and_then(std::ffi::OsStr::to_str) != Some("conduit") {
        return Err(FormSourceError::NonCanonicalExtension(source.path.clone()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_source(name: &str, source: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("conduit-form-source-{nonce}"));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join(name);
        std::fs::write(&path, source).unwrap();
        path
    }

    #[test]
    fn explicit_canonical_sources_are_checked_in_order() {
        let alpha = temporary_source(
            "alpha.conduit",
            include_str!("../../../../examples/hello.conduit"),
        );
        let beta = temporary_source(
            "beta.conduit",
            include_str!("../../../../examples/greet.conduit"),
        );
        let candidates = load_form_sources(&[
            FormSource::new("Alpha", &alpha),
            FormSource::new("Beta", &beta),
        ])
        .unwrap();
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].label, "Alpha");
        assert_eq!(candidates[1].label, "Beta");
        assert_ne!(candidates[0].checked_form_id, candidates[1].checked_form_id);
    }

    #[test]
    fn missing_noncanonical_duplicate_and_excess_sources_fail_closed() {
        let canonical = temporary_source(
            "same.conduit",
            include_str!("../../../../examples/hello.conduit"),
        );
        let noncanonical = temporary_source("old.form", "not a canonical source");
        assert!(matches!(
            load_form_sources(&[FormSource::new("Missing", "missing.conduit")]),
            Err(FormSourceError::MissingOrNonFile(_))
        ));
        assert!(matches!(
            load_form_sources(&[FormSource::new("Old", noncanonical)]),
            Err(FormSourceError::NonCanonicalExtension(_))
        ));
        assert!(matches!(
            load_form_sources(&[
                FormSource::new("Same A", &canonical),
                FormSource::new("Same B", &canonical),
            ]),
            Err(FormSourceError::DuplicateForm(_))
        ));
        let excessive = (0..=MAX_ADDITIONAL_FORMS)
            .map(|index| FormSource::new(format!("Form {index}"), &canonical))
            .collect::<Vec<_>>();
        assert!(matches!(
            load_form_sources(&excessive),
            Err(FormSourceError::TooManySources)
        ));
    }
}
