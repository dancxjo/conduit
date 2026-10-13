//! Explicit retained artifact capacity selected before the installed Host Boot.
use crate::cli::InstalledSpeechOptions;
use conduit_std_host::hosted_wav_artifact::WavArtifactRetentionLimits;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct RetentionSelection {
    maximum_artifacts: usize,
    maximum_bytes: u64,
}
impl RetentionSelection {
    pub(super) fn limits(&self) -> Result<WavArtifactRetentionLimits, String> {
        WavArtifactRetentionLimits::new(self.maximum_artifacts, self.maximum_bytes)
    }
    pub(super) fn from_options(options: &InstalledSpeechOptions) -> Result<Option<Self>, String> {
        match (
            options.speech_artifact_maximum_artifacts,
            options.speech_artifact_maximum_bytes,
        ) {
            (None, None) => Ok(None),
            (Some(maximum_artifacts), Some(maximum_bytes))
                if (options.selected_speech || options.selected_artifact_speech)
                    && !options.without_selected_speech =>
            {
                let selection = Self {
                    maximum_artifacts,
                    maximum_bytes,
                };
                selection.limits()?;
                Ok(Some(selection))
            }
            _ => {
                Err("artifact retention needs both bounds and an explicit speech selection".into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retention_requires_explicit_selection_and_both_finite_bounds() {
        assert!(
            RetentionSelection::from_options(&InstalledSpeechOptions::default())
                .unwrap()
                .is_none()
        );
        let mut options = InstalledSpeechOptions {
            speech_artifact_maximum_artifacts: Some(128),
            speech_artifact_maximum_bytes: Some(800_000_000),
            ..Default::default()
        };
        assert!(RetentionSelection::from_options(&options).is_err());
        options.selected_artifact_speech = true;
        assert!(RetentionSelection::from_options(&options)
            .unwrap()
            .is_some());
        options.speech_artifact_maximum_artifacts = Some(1025);
        assert!(RetentionSelection::from_options(&options).is_err());
        options.speech_artifact_maximum_artifacts = Some(128);
        options.speech_artifact_maximum_bytes = Some(1_073_741_825);
        assert!(RetentionSelection::from_options(&options).is_err());
        options.speech_artifact_maximum_bytes = None;
        assert!(RetentionSelection::from_options(&options).is_err());
    }
    #[test]
    fn legacy_retention_defaults_and_explicit_capacity_roundtrip_without_provider_effect() {
        use super::super::{fixture_retained_selection, Selection};
        let original = fixture_retained_selection();
        let value = serde_json::to_value(&original).unwrap();
        assert!(value.get("artifact_retention").is_none());
        let mut restored: Selection = serde_json::from_value(value).unwrap();
        assert!(restored.artifact_retention.is_none());
        restored.artifact_retention = Some(
            serde_json::from_value(serde_json::json!({
                "maximum_artifacts":128,"maximum_bytes":800_000_000
            }))
            .unwrap(),
        );
        restored.validate().unwrap();
        let roundtrip: Selection =
            serde_json::from_value(serde_json::to_value(&restored).unwrap()).unwrap();
        assert_eq!(roundtrip, restored);
        let mut invalid = serde_json::to_value(restored).unwrap();
        invalid["artifact_retention"]["maximum_artifacts"] = serde_json::json!(1025);
        let invalid: Selection = serde_json::from_value(invalid).unwrap();
        assert!(invalid.validate().unwrap_err().contains("finite admission"));
    }
}
