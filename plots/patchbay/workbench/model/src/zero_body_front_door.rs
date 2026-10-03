//! Truthful Patchbay WORLD state for one host that bears no Body.

use conduit_body::{Body, BodyMembership, MembershipProofId, Wake};
use conduit_core::{BootId, CheckedPlotId, HostId, SignId, SourceDocumentId};
use conduit_presentation::Presentation;

use crate::{LocalFrontDoor, PatchbayModel, PlotEditor};
use std::sync::Arc;

pub const MAX_FRONT_DOOR_BODY_CANDIDATES: usize = 16;
pub const MAX_FRONT_DOOR_PLOTS: usize = 16;
pub const MAX_FRONT_DOOR_REFUSAL_SIGNS: usize = 16;

#[derive(Clone)]
pub(super) struct FrontDoorRefusalSign {
    pub sign_id: SignId,
    pub code: String,
}

#[derive(Clone)]
pub struct BodyJoinCandidate {
    pub label: String,
    pub body: Body,
    pub wake: Wake,
    pub membership: BodyMembership,
    pub editor: PlotEditor,
    pub proof_id: MembershipProofId,
    pub evidence_sign: SignId,
    pub freshness_sequence: u64,
}

impl BodyJoinCandidate {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        label: impl Into<String>,
        body: Body,
        wake: Wake,
        membership: BodyMembership,
        editor: PlotEditor,
        proof_id: MembershipProofId,
        evidence_sign: SignId,
        freshness_sequence: u64,
    ) -> Result<Self, String> {
        body.validate().map_err(|error| error.to_string())?;
        wake.validate().map_err(|error| error.to_string())?;
        membership
            .validate()
            .map_err(|error| format!("{error:?}"))?;
        if body.body_id != wake.body_id
            || body.body_id != membership.body_id
            || body.workset.plots().iter().any(|resident| {
                editor.view().checked.source_document_id.as_ref()
                    != Some(&resident.source_document_id)
                    || !editor
                        .view()
                        .checked
                        .plots
                        .iter()
                        .any(|plot| plot.checked_plot_id == resident.checked_plot_id)
            })
        {
            return Err("Body join candidate identity chain is inconsistent".into());
        }
        let label = label.into();
        if label.is_empty() || freshness_sequence == 0 {
            return Err("Body join candidate label and freshness must be present".into());
        }
        Ok(Self {
            label,
            body,
            wake,
            membership,
            editor,
            proof_id,
            evidence_sign,
            freshness_sequence,
        })
    }
}

#[derive(Clone)]
pub struct PlotCandidate {
    pub label: String,
    pub source_document_id: SourceDocumentId,
    pub checked_plot_id: CheckedPlotId,
    pub source_name: String,
    pub source: String,
    pub provenance: String,
    pub evidence_sign: SignId,
    pub freshness_sequence: u64,
    pub(super) editor: PlotEditor,
}

impl PlotCandidate {
    pub fn from_source(
        label: impl Into<String>,
        source_name: impl Into<String>,
        source: impl Into<String>,
        provenance: impl Into<String>,
        evidence_sign: SignId,
        freshness_sequence: u64,
    ) -> Result<Self, String> {
        let label = label.into();
        let source_name = source_name.into();
        let source = source.into();
        let provenance = provenance.into();
        if label.is_empty() || provenance.is_empty() || freshness_sequence == 0 {
            return Err("Plot label, provenance, and freshness must be present".into());
        }
        let editor = PlotEditor::from_source(source_name.clone().into(), source.clone())
            .map_err(|error| error.to_string())?;
        let source_document_id = editor
            .view()
            .checked
            .source_document_id
            .clone()
            .ok_or_else(|| {
                editor
                    .view()
                    .checked
                    .diagnostics
                    .first()
                    .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
                    .unwrap_or_else(|| "Plot source is unchecked".into())
            })?;
        let checked_plot_id = editor
            .view()
            .checked
            .plots
            .first()
            .ok_or("Plot source contains no Plot")?
            .checked_plot_id
            .clone();
        Ok(Self {
            label,
            source_document_id,
            checked_plot_id,
            source_name,
            source,
            provenance,
            evidence_sign,
            freshness_sequence,
            editor,
        })
    }

    pub fn from_source_plot(
        label: impl Into<String>,
        source_name: impl Into<String>,
        source: impl Into<String>,
        plot_name: &str,
        provenance: impl Into<String>,
        evidence_sign: SignId,
        freshness_sequence: u64,
    ) -> Result<Self, String> {
        let mut candidate = Self::from_source(
            label,
            source_name,
            source,
            provenance,
            evidence_sign,
            freshness_sequence,
        )?;
        candidate
            .editor
            .open_back(plot_name)
            .map_err(|error| error.to_string())?;
        candidate.checked_plot_id = candidate
            .editor
            .view()
            .checked
            .plots
            .iter()
            .find(|plot| plot.name == plot_name)
            .ok_or_else(|| format!("Plot source contains no '{plot_name}' Plot"))?
            .checked_plot_id
            .clone();
        Ok(candidate)
    }

    pub fn select_checked_plot(&mut self, checked_plot_id: &CheckedPlotId) -> Result<(), String> {
        if !self
            .editor
            .view()
            .checked
            .plots
            .iter()
            .any(|plot| &plot.checked_plot_id == checked_plot_id)
        {
            return Err("checked plot is absent from its exact source document".into());
        }
        self.checked_plot_id = checked_plot_id.clone();
        Ok(())
    }

    pub(super) fn editor(&self) -> Result<PlotEditor, String> {
        Ok(self.editor.clone())
    }

    pub fn body_planning_plot(
        &self,
    ) -> Result<conduit_body_make::BodyPlanningPlot, conduit_body::BodyPlanningSessionError> {
        let view = self.editor.view();
        let name = view
            .checked
            .plots
            .iter()
            .find(|plot| plot.checked_plot_id == self.checked_plot_id)
            .map(|plot| plot.name.as_str())
            .ok_or_else(|| {
                conduit_body::BodyPlanningSessionError::InvalidPlot("checked plot is absent".into())
            })?;
        let expanded = self.editor.expand_plot(name).map_err(|error| {
            conduit_body::BodyPlanningSessionError::InvalidPlot(error.to_string())
        })?;
        conduit_body_make::BodyPlanningPlot::new(
            conduit_body::ResidentPlot::new(
                self.source_document_id.clone(),
                self.checked_plot_id.clone(),
            ),
            expanded,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenedFrontDoorSubject {
    Body {
        body_id: conduit_body::BodyId,
        observed_at: u64,
    },
    Plot {
        checked_plot_id: CheckedPlotId,
        observed_at: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZeroBodyFrontDoorProjection {
    pub presentation: Presentation,
    pub navigation: patchbay_application::PatchbayNavigationProjection,
}

#[derive(Clone)]
pub struct ZeroBodyFrontDoor {
    pub(super) adapter: Arc<dyn crate::PatchbayHostAdapter>,
    pub(super) model: PatchbayModel,
    pub(super) body_candidates: Vec<BodyJoinCandidate>,
    pub(super) plots: Vec<PlotCandidate>,
    pub(super) opened: Option<OpenedFrontDoorSubject>,
    pub(super) refusals: Vec<FrontDoorRefusalSign>,
    pub(super) revision: u64,
}

impl ZeroBodyFrontDoor {
    pub fn fresh(adapter: Arc<dyn crate::PatchbayHostAdapter>) -> Result<Self, String> {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let advertisement = adapter.advertisement(
            HostId::from(format!("patchbay-native/{nonce:x}")),
            BootId::from(format!("patchbay-boot/{nonce:x}")),
            conduit_core::OfferGeneration(1),
            crate::PatchbayHostProfile::Text,
        )?;
        Self::from_model(adapter, PatchbayModel::from_advertisement(advertisement))
    }

    pub fn with_identity(
        adapter: Arc<dyn crate::PatchbayHostAdapter>,
        host_id: HostId,
        boot_id: BootId,
    ) -> Result<Self, String> {
        let advertisement = adapter.advertisement(
            host_id,
            boot_id,
            conduit_core::OfferGeneration(1),
            crate::PatchbayHostProfile::Text,
        )?;
        Self::from_model(adapter, PatchbayModel::from_advertisement(advertisement))
    }

    pub fn from_model(
        adapter: Arc<dyn crate::PatchbayHostAdapter>,
        model: PatchbayModel,
    ) -> Result<Self, String> {
        let morse_network = PlotCandidate::from_source(
            "Morse Network",
            "plots/morse-network/main.conduit",
            include_str!("../../../../../plots/morse-network/main.conduit"),
            "reviewed plot inventory; opening is inert and BIRTH remains explicit",
            SignId::from("patchbay/front-door/plot-available"),
            1,
        )?;
        let clock = PlotCandidate::from_source(
            "Clock",
            "plots/clock/main.conduit",
            include_str!("../../../../../plots/clock/main.conduit"),
            "reviewed plot inventory; opening is inert and BIRTH remains explicit",
            SignId::from("patchbay/front-door/clock-available"),
            2,
        )?;
        Ok(Self {
            adapter,
            model,
            body_candidates: Vec::new(),
            plots: vec![morse_network, clock],
            opened: None,
            refusals: Vec::new(),
            revision: 1,
        })
    }

    pub fn advertisement(&self) -> &conduit_core::HostAdvertisement {
        self.model.advertisement()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn opened(&self) -> Option<&OpenedFrontDoorSubject> {
        self.opened.as_ref()
    }

    pub fn plot_ids(&self) -> Vec<CheckedPlotId> {
        self.plots
            .iter()
            .map(|plot| plot.checked_plot_id.clone())
            .collect()
    }

    pub fn record_refusal(&mut self, code: &str) -> Result<SignId, String> {
        if !matches!(code, "StalePresentation" | "StaleDiscovery" | "StalePlot") {
            return Err("unsupported front-door refusal code".into());
        }
        if self.refusals.len() == MAX_FRONT_DOOR_REFUSAL_SIGNS {
            return Err("front-door refusal Sign capacity exhausted".into());
        }
        let sign_id = SignId::from(format!(
            "patchbay/front-door/refused/{}/{code}",
            self.revision
        ));
        self.refusals.push(FrontDoorRefusalSign {
            sign_id: sign_id.clone(),
            code: code.into(),
        });
        self.advance()?;
        Ok(sign_id)
    }

    pub fn observe_body_candidate(&mut self, candidate: BodyJoinCandidate) -> Result<(), String> {
        if self.body_candidates.len() == MAX_FRONT_DOOR_BODY_CANDIDATES {
            return Err("front-door Body candidate capacity exhausted".into());
        }
        if self
            .body_candidates
            .iter()
            .any(|value| value.body.body_id == candidate.body.body_id)
        {
            return Err("front-door Body candidate is already observed".into());
        }
        self.body_candidates.push(candidate);
        self.advance()
    }

    pub fn add_plot(&mut self, plot: PlotCandidate) -> Result<(), String> {
        if self.plots.len() == MAX_FRONT_DOOR_PLOTS {
            return Err("front-door Plot capacity exhausted".into());
        }
        if self
            .plots
            .iter()
            .any(|value| value.checked_plot_id == plot.checked_plot_id)
        {
            return Err("front-door Plot is already available".into());
        }
        self.plots.push(plot);
        self.advance()
    }

    pub fn open_body(
        &mut self,
        body_id: &conduit_body::BodyId,
        revision: u64,
    ) -> Result<(), String> {
        self.require_revision(revision)?;
        let candidate = self
            .body_candidates
            .iter()
            .find(|candidate| &candidate.body.body_id == body_id)
            .ok_or("unknown Body candidate")?;
        self.opened = Some(OpenedFrontDoorSubject::Body {
            body_id: body_id.clone(),
            observed_at: candidate.freshness_sequence,
        });
        self.advance()
    }

    pub fn open_plot(
        &mut self,
        checked_plot_id: &CheckedPlotId,
        revision: u64,
    ) -> Result<(), String> {
        self.require_revision(revision)?;
        let plot = self
            .plots
            .iter()
            .find(|plot| &plot.checked_plot_id == checked_plot_id)
            .ok_or("unknown Plot")?;
        self.opened = Some(OpenedFrontDoorSubject::Plot {
            checked_plot_id: checked_plot_id.clone(),
            observed_at: plot.freshness_sequence,
        });
        self.advance()
    }

    pub fn open_subject(&mut self, subject: &str, revision: u64) -> Result<(), String> {
        if let Some(candidate) = self
            .body_candidates
            .iter()
            .find(|candidate| format!("body/{}", candidate.body.body_id.as_str()) == subject)
        {
            let body_id = candidate.body.body_id.clone();
            return self.open_body(&body_id, revision);
        }
        if let Some(plot) = self
            .plots
            .iter()
            .find(|plot| format!("plot/{}", plot.checked_plot_id.as_str()) == subject)
        {
            let checked_plot_id = plot.checked_plot_id.clone();
            return self.open_plot(&checked_plot_id, revision);
        }
        Err("OPEN requires a current body or Plot subject".into())
    }

    pub fn join_open_body(self, revision: u64) -> Result<LocalFrontDoor, String> {
        self.require_revision(revision)?;
        let OpenedFrontDoorSubject::Body {
            body_id,
            observed_at,
        } = self.opened.clone().ok_or("JOIN requires an opened Body")?
        else {
            return Err("JOIN requires an opened Body".into());
        };
        let candidate = self
            .body_candidates
            .into_iter()
            .find(|candidate| candidate.body.body_id == body_id)
            .ok_or("opened Body is no longer available")?;
        if candidate.freshness_sequence != observed_at {
            return Err("opened Body observation is stale".into());
        }
        LocalFrontDoor::join_existing(self.adapter, self.model, candidate, self.revision)
    }

    pub fn birth(self, revision: u64) -> Result<LocalFrontDoor, String> {
        self.require_revision(revision)?;
        let OpenedFrontDoorSubject::Plot {
            checked_plot_id,
            observed_at,
        } = self.opened.clone().ok_or("BIRTH requires an opened Plot")?
        else {
            return Err("BIRTH requires an opened Plot".into());
        };
        let plot = self
            .plots
            .into_iter()
            .find(|candidate| candidate.checked_plot_id == checked_plot_id)
            .ok_or("opened Plot is no longer available")?;
        if plot.freshness_sequence != observed_at {
            return Err("opened Plot observation is stale".into());
        }
        LocalFrontDoor::born_from_plot(self.adapter, self.model, plot, self.revision)
    }

    pub(super) fn require_revision(&self, revision: u64) -> Result<(), String> {
        if revision != self.revision {
            Err(format!(
                "stale front-door revision {revision}; current is {}",
                self.revision
            ))
        } else {
            Ok(())
        }
    }

    pub(super) fn advance(&mut self) -> Result<(), String> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or("front-door presentation revision exhausted")?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "zero_body_front_door_tests.rs"]
mod tests;
