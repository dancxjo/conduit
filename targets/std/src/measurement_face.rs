//! Portable chart meaning from the existing typed plot series, not a renderer.
//! Host-side Face preparation allocates. The caller must admit that preparation
//! and supply its inspecting Play's real basis; labels grant no producer trust.
use conduit_core::{kind_id, CheckedValueContract};
use conduit_data::{
    encode_measurement_plot_series, MeasurementPlotSeries, MAXIMUM_MEASUREMENT_PLOT_SERIES_BYTES,
    MEASUREMENT_PLOT_FACE_KIND, MEASUREMENT_PLOT_SERIES_INFO_ID,
};
use conduit_face::{
    FaceContributionBasis, FaceFragment, FaceProperty, FacePropertyValue, FaceRole, FaceSubject,
    FaceText, MAX_FACE_ID_BYTES,
};

pub const MAXIMUM_CHART_SERIES: usize = 4;

pub struct MeasurementFaceSeries<'a> {
    /// Human name from authored metadata; never an authenticated Host identity.
    pub label: &'a str,
    pub series: &'a MeasurementPlotSeries,
}

#[derive(Debug, PartialEq, Eq)]
pub enum MeasurementFaceRefusal {
    SeriesCount,
    InvalidName,
    DuplicateSeries,
    InvalidSeries,
    Fragment,
}

/// A single chart subject can carry several independently prepared series. The
/// shared contract normalizes sample INDEX and values, not wall-clock time or
/// physical units. Those meanings are stated explicitly for every Mask. Original
/// producer/clock evidence belongs in ordinary surrounding Face subjects; this
/// helper does not infer it from names or rebase old producer Plays as current.
pub fn measurement_plot_fragment(
    subject: &str,
    title: &str,
    series: &[MeasurementFaceSeries<'_>],
    basis: FaceContributionBasis,
) -> Result<FaceFragment, MeasurementFaceRefusal> {
    use MeasurementFaceRefusal as E;
    if series.is_empty() || series.len() > MAXIMUM_CHART_SERIES {
        return Err(E::SeriesCount);
    }
    if !name(subject) || !name(title) || series.iter().any(|s| !name(s.label)) {
        return Err(E::InvalidName);
    }
    let mut properties = Vec::with_capacity(series.len());
    for (index, item) in series.iter().enumerate() {
        if series[..index]
            .iter()
            .any(|previous| previous.label == item.label)
        {
            return Err(E::DuplicateSeries);
        }
        let bytes = encode_measurement_plot_series(item.series).map_err(|_| E::InvalidSeries)?;
        let contract = CheckedValueContract::new(
            kind_id(MEASUREMENT_PLOT_SERIES_INFO_ID),
            MAXIMUM_MEASUREMENT_PLOT_SERIES_BYTES as u32,
            vec![],
        )
        .map_err(|_| E::InvalidSeries)?;
        contract.validate(&bytes).map_err(|_| E::InvalidSeries)?;
        properties.push(FaceProperty {
            subject: subject.into(),
            name: item.label.into(),
            value: FacePropertyValue::TypedValue { contract, bytes },
        });
    }
    let fragment = FaceFragment {
        basis,
        subjects: vec![FaceSubject {
            identity: subject.into(),
            role: FaceRole::Semantic(kind_id(MEASUREMENT_PLOT_FACE_KIND)),
            name: title.into(),
        }],
        relationships: vec![],
        composition: vec![],
        properties,
        text: vec![FaceText {
            subject: subject.into(),
            text: "Horizontal: sample index; vertical: normalized value (0–1).".into(),
        }],
        actions: vec![],
        disclosures: vec![],
        temporal_references: vec![],
        temporal_facts: vec![],
    };
    fragment.validate_bounds().map_err(|_| E::Fragment)?;
    Ok(fragment)
}

fn name(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_FACE_ID_BYTES
        && !value.chars().any(char::is_control)
}

#[cfg(test)]
#[path = "measurement_face/tests.rs"]
mod tests;
