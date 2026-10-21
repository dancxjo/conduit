//! Finite observational inputs for guest Home layout; no lifecycle authority.
use crate::{data::NativeScope, wire::Refusal};
use alloc::{string::String, vec::Vec};
use conduit_face::{FaceProperty, FaceText};
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plot {
    pub title: String,
    pub plot: conduit_body::ResidentPlot,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Extent {
    pub width: u16,
    pub height: u16,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub scope: NativeScope,
    pub plots: Vec<Plot>,
    pub heading: String,
    pub birth_available: bool,
    pub idle_status: String,
    pub friendly_name: Option<String>,
    pub wake_id: Option<String>,
    pub plan_id: Option<String>,
    pub play_id: Option<String>,
    pub line_count: u8,
    pub root_refusal: Option<String>,
    pub extent: Option<Extent>,
}
impl Request {
    pub fn validate(&self) -> Result<(), Refusal> {
        self.scope.validate()?;
        let titles: Vec<String> = self.plots.iter().map(|plot| plot.title.clone()).collect();
        super::data::validate_titles(&titles)?;
        if self.heading.len() > 2048
            || self.idle_status.len() > 2048
            || self.line_count > 1
            || self
                .extent
                .as_ref()
                .is_some_and(|extent| extent.width == 0 || extent.height == 0)
        {
            return Err(Refusal::Bounds);
        }
        for text in [
            &self.friendly_name,
            &self.wake_id,
            &self.plan_id,
            &self.play_id,
            &self.root_refusal,
        ]
        .into_iter()
        .flatten()
        {
            if text.len() > 2048 {
                return Err(Refusal::Bounds);
            }
        }
        for plot in &self.plots {
            for identity in [
                plot.plot.source_document_id.as_str(),
                plot.plot.checked_plot_id.as_str(),
            ] {
                if identity.len() > 2048 {
                    return Err(Refusal::Bounds);
                }
            }
        }
        conduit_body::BodyWorkset::from_plots(self.plots.iter().map(|plot| plot.plot.clone()))
            .map_err(|_| Refusal::Bounds)?;
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Content {
    pub scope: NativeScope,
    pub request_digest: [u8; 32],
    pub properties: Vec<FaceProperty>,
    pub text: Vec<FaceText>,
}
pub fn digest(bytes: &[u8]) -> [u8; 32] {
    conduit_core::semantic_digest("conduitos/home-native-presentation@1", bytes)
}
