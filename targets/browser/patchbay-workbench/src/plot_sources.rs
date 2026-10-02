//! host-owned loading of explicitly admitted canonical Plot sources.

use conduit_core::SignId;
use conduit_patchbay_workbench::{PlotCandidate, MAX_FRONT_DOOR_PLOTS, MAX_PLOT_SOURCE_BYTES};
use std::collections::BTreeSet;
use std::path::PathBuf;

pub const MAX_ADDITIONAL_PLOTS: usize = MAX_FRONT_DOOR_PLOTS - 1;
pub const MAX_PLOT_LABEL_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlotSource {
    pub label: String,
    pub path: PathBuf,
}

impl PlotSource {
    pub fn new(label: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            label: label.into(),
            path: path.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlotSourceError {
    TooManySources,
    InvalidLabel,
    NonCanonicalExtension(PathBuf),
    MissingOrNonFile(PathBuf),
    SourceTooLarge(PathBuf),
    NonUtf8Path(PathBuf),
    ReadFailed(PathBuf),
    InvalidSource { path: PathBuf, detail: String },
    DuplicatePlot(PathBuf),
}

impl std::fmt::Display for PlotSourceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Plot source registration failed: {self:?}")
    }
}

impl std::error::Error for PlotSourceError {}

pub fn load_plot_sources(sources: &[PlotSource]) -> Result<Vec<PlotCandidate>, PlotSourceError> {
    if sources.len() > MAX_ADDITIONAL_PLOTS {
        return Err(PlotSourceError::TooManySources);
    }
    let mut identities = BTreeSet::new();
    let mut candidates = Vec::with_capacity(sources.len());
    for (index, source) in sources.iter().enumerate() {
        validate_source(source)?;
        let metadata = std::fs::metadata(&source.path)
            .map_err(|_| PlotSourceError::MissingOrNonFile(source.path.clone()))?;
        if !metadata.is_file() {
            return Err(PlotSourceError::MissingOrNonFile(source.path.clone()));
        }
        if metadata.len() > MAX_PLOT_SOURCE_BYTES as u64 {
            return Err(PlotSourceError::SourceTooLarge(source.path.clone()));
        }
        let source_name = source
            .path
            .to_str()
            .ok_or_else(|| PlotSourceError::NonUtf8Path(source.path.clone()))?;
        let bytes = std::fs::read(&source.path)
            .map_err(|_| PlotSourceError::ReadFailed(source.path.clone()))?;
        let text = String::from_utf8(bytes).map_err(|_| PlotSourceError::InvalidSource {
            path: source.path.clone(),
            detail: "source is not valid UTF-8".into(),
        })?;
        let provenance = format!("explicit repository Plot {}", source.path.display());
        let evidence_sign = SignId::from(format!("patchbay-html/plot-source/{}", index + 1));
        let candidate = PlotCandidate::from_source_plot(
            &source.label,
            source_name,
            text.clone(),
            &source.label,
            &provenance,
            evidence_sign.clone(),
            index as u64 + 2,
        )
        .or_else(|_| {
            PlotCandidate::from_source(
                &source.label,
                source_name,
                text,
                provenance,
                evidence_sign,
                index as u64 + 2,
            )
        })
        .map_err(|detail| PlotSourceError::InvalidSource {
            path: source.path.clone(),
            detail,
        })?;
        if !identities.insert(candidate.checked_plot_id.as_str().to_owned()) {
            return Err(PlotSourceError::DuplicatePlot(source.path.clone()));
        }
        candidates.push(candidate);
    }
    Ok(candidates)
}

fn validate_source(source: &PlotSource) -> Result<(), PlotSourceError> {
    if source.label.is_empty()
        || source.label.len() > MAX_PLOT_LABEL_BYTES
        || source.label.chars().any(char::is_control)
    {
        return Err(PlotSourceError::InvalidLabel);
    }
    if source.path.extension().and_then(std::ffi::OsStr::to_str) != Some("conduit") {
        return Err(PlotSourceError::NonCanonicalExtension(source.path.clone()));
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
        let directory = std::env::temp_dir().join(format!("conduit-plot-source-{nonce}"));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join(name);
        std::fs::write(&path, source).unwrap();
        path
    }

    #[test]
    fn explicit_canonical_sources_are_checked_in_order() {
        let alpha = temporary_source(
            "alpha.conduit",
            include_str!("../../../../plots/hello/main.conduit"),
        );
        let beta = temporary_source(
            "beta.conduit",
            include_str!("../../../../plots/greet/main.conduit"),
        );
        let candidates = load_plot_sources(&[
            PlotSource::new("Alpha", &alpha),
            PlotSource::new("Beta", &beta),
        ])
        .unwrap();
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].label, "Alpha");
        assert_eq!(candidates[1].label, "Beta");
        assert_ne!(candidates[0].checked_plot_id, candidates[1].checked_plot_id);
    }

    #[test]
    fn an_exact_label_selects_a_named_plot_from_a_multi_plot_source() {
        let path = temporary_source(
            "multiple.conduit",
            include_str!("../../../../plots/desk-telegraph/main.conduit"),
        );
        let named = load_plot_sources(&[PlotSource::new("desk_telegraph", &path)]).unwrap();
        let default = load_plot_sources(&[PlotSource::new("Display label", path)]).unwrap();
        assert_ne!(named[0].checked_plot_id, default[0].checked_plot_id);
    }

    #[test]
    fn missing_noncanonical_duplicate_and_excess_sources_fail_closed() {
        let canonical = temporary_source(
            "same.conduit",
            include_str!("../../../../plots/hello/main.conduit"),
        );
        let noncanonical = temporary_source("old.plot", "not a canonical source");
        assert!(matches!(
            load_plot_sources(&[PlotSource::new("Missing", "missing.conduit")]),
            Err(PlotSourceError::MissingOrNonFile(_))
        ));
        assert!(matches!(
            load_plot_sources(&[PlotSource::new("Old", noncanonical)]),
            Err(PlotSourceError::NonCanonicalExtension(_))
        ));
        assert!(matches!(
            load_plot_sources(&[
                PlotSource::new("Same A", &canonical),
                PlotSource::new("Same B", &canonical),
            ]),
            Err(PlotSourceError::DuplicatePlot(_))
        ));
        let excessive = (0..=MAX_ADDITIONAL_PLOTS)
            .map(|index| PlotSource::new(format!("Plot {index}"), &canonical))
            .collect::<Vec<_>>();
        assert!(matches!(
            load_plot_sources(&excessive),
            Err(PlotSourceError::TooManySources)
        ));
    }
}
