//! Checked Source-to-resident-editor handoff; never replaces resident Source.
use super::*;
use conduit_body::ResidentPlot;
use conduit_core::{ActivePlayId, PlanId};
use patchbay_application::PatchbayApplicationRequest;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceEditRefusal {
    NotLive,
    WrongOwner,
    WrongTarget,
    SourceUnavailable,
    SourceSpan,
    EditorUnavailable,
    ExistingDraft,
    SourceCapacity,
    InputPressure,
}
impl SourceEditRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotLive => "native-source-edit-not-live",
            Self::WrongOwner => "native-source-edit-wrong-owner",
            Self::WrongTarget => "native-source-edit-wrong-target",
            Self::SourceUnavailable => "native-source-edit-source-unavailable",
            Self::SourceSpan => "native-source-edit-span-refused",
            Self::EditorUnavailable => "native-source-editor-not-admitted",
            Self::ExistingDraft => "native-editor-existing-draft",
            Self::SourceCapacity => "native-editor-source-capacity",
            Self::InputPressure => "native-source-editor-input-pressure",
        }
    }
}
#[derive(Debug)]
pub enum SourceEditError {
    Refused(SourceEditRefusal),
    Fatal(JourneyError),
}
impl From<JourneyError> for SourceEditError {
    fn from(error: JourneyError) -> Self {
        Self::Fatal(error)
    }
}
#[derive(Debug, serde::Serialize)]
pub struct SourceEditReceipt {
    pub source: ResidentPlot,
    pub expanded_plot_id: ExpandedPlotId,
    pub subject: String,
    pub body_plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    /// Checked origin coordinates. This is provenance, not an enacted selection.
    pub source_byte_span: Option<(usize, usize)>,
    pub source_scalar_span: Option<(usize, usize)>,
    pub selection_enacted: bool,
}
fn refuse<T>(error: SourceEditRefusal) -> Result<T, SourceEditError> {
    Err(SourceEditError::Refused(error))
}
impl ProductJourney {
    pub fn open_patchbay_source(
        &mut self,
        request: PatchbayApplicationRequest,
        identities: &BootIdentities,
    ) -> Result<SourceEditReceipt, SourceEditError> {
        if self.status != JourneyStatus::QuiescentAwaitingInput
            || self.kernel.is_none()
            || self.current_play().is_none()
            || self.current_plan().is_none()
        {
            return refuse(SourceEditRefusal::NotLive);
        }
        if self.host_id.as_str() != crate::identity::hex(&identities.host)
            || self.boot_id.as_str() != crate::identity::hex(&identities.boot)
            || self.plots[self.foreground_index()] != Some(NativePlot::Patchbay)
        {
            return refuse(SourceEditRefusal::WrongOwner);
        }
        let PatchbayApplicationRequest::EditCurrent {
            expanded_plot_id,
            subject_identity,
        } = request
        else {
            return refuse(SourceEditRefusal::WrongTarget);
        };
        if subject_identity.len() > 256 {
            return refuse(SourceEditRefusal::WrongTarget);
        }
        let graph = self
            .foreground_patchbay_graph()
            .ok_or(SourceEditError::Refused(SourceEditRefusal::WrongOwner))?;
        if graph.expanded_plot_id != expanded_plot_id
            || graph
                .subject_identities()
                .filter(|id| *id == subject_identity.as_str())
                .count()
                != 1
        {
            return refuse(SourceEditRefusal::WrongTarget);
        }
        let source = ResidentPlot::new(
            graph.source_document_id.clone(),
            graph.checked_plot_id.clone(),
        );
        let plot = native_workset::resolve(&source)
            .map_err(|_| SourceEditError::Refused(SourceEditRefusal::SourceUnavailable))?;
        let expanded = native_workset::checked(plot)
            .map_err(|_| SourceEditError::Refused(SourceEditRefusal::SourceUnavailable))?;
        if expanded.source_document_id != source.source_document_id
            || expanded.checked_plot_id != source.checked_plot_id
            || expanded.expanded_plot_id != expanded_plot_id
        {
            return refuse(SourceEditRefusal::SourceUnavailable);
        }
        let text = plot.source();
        let span = exact_span(graph, &expanded, &subject_identity)?;
        let scalar_span = if let Some((start, end)) = span {
            if start >= end
                || end > text.len()
                || !text.is_char_boundary(start)
                || !text.is_char_boundary(end)
            {
                return refuse(SourceEditRefusal::SourceSpan);
            }
            Some((text[..start].chars().count(), text[..end].chars().count()))
        } else {
            None
        };
        let body = self
            .body()
            .ok_or(SourceEditError::Refused(SourceEditRefusal::NotLive))?;
        if body
            .workset
            .plots()
            .iter()
            .filter(|resident| **resident == source)
            .count()
            != 1
        {
            return refuse(SourceEditRefusal::SourceUnavailable);
        }
        let editor = native_workset::resident(NativePlot::MemoryLantern)
            .map_err(|_| SourceEditError::Refused(SourceEditRefusal::EditorUnavailable))?;
        let editor_index = body
            .workset
            .plots()
            .iter()
            .position(|resident| *resident == editor)
            .ok_or(SourceEditError::Refused(
                SourceEditRefusal::EditorUnavailable,
            ))?;
        let plan = self.current_plan().unwrap();
        let active = self.current_play().unwrap();
        if active.plan_id != plan.plan_id
            || active.body_id != body.body_id
            || plan
                .plots
                .iter()
                .zip(body.workset.plots())
                .any(|(part, resident)| part.plot != *resident)
            || plan.plots.len() != body.workset.len()
            || plan.plots.iter().any(|part| {
                part.plan.fragments.iter().any(|fragment| {
                    fragment.host_id != self.host_id
                        || fragment.boot_id != self.boot_id
                        || fragment.offer_generation != self.offer_generation
                })
            })
        {
            return refuse(SourceEditRefusal::WrongOwner);
        }
        let receipt = SourceEditReceipt {
            source,
            expanded_plot_id,
            subject: subject_identity,
            body_plan_id: plan.plan_id.clone(),
            active_play_id: self.current_play().unwrap().active_play_id.clone(),
            source_byte_span: span,
            source_scalar_span: scalar_span,
            selection_enacted: false,
        };
        let next = self
            .input_count
            .checked_add(1)
            .ok_or(JourneyError::InputSequenceExhausted)?;
        self.revision
            .checked_add(2)
            .ok_or(JourneyError::RevisionExhausted)?;
        let result = self
            .kernel
            .as_mut()
            .unwrap()
            .open_source_editor(editor_index, text);
        if let Err(error) = result {
            use native_workset::PlayRefusal;
            let refusal = match error {
                PlayRefusal::EditorDraftExists => Some(SourceEditRefusal::ExistingDraft),
                PlayRefusal::EditorSourceBound => Some(SourceEditRefusal::SourceCapacity),
                PlayRefusal::InputPressure => Some(SourceEditRefusal::InputPressure),
                PlayRefusal::InputOwnership | PlayRefusal::Preparation => {
                    Some(SourceEditRefusal::EditorUnavailable)
                }
                _ => None,
            };
            if let Some(refusal) = refusal {
                return refuse(refusal);
            }
            self.retire_realization()?;
            self.status = JourneyStatus::Stopped;
            self.advance()?;
            return Err(SourceEditError::Fatal(JourneyError::Play(error)));
        }
        let value = match self.kernel.as_mut().unwrap().take_face(editor_index) {
            Some(value) => value,
            None => {
                self.retire_realization()?;
                self.status = JourneyStatus::Stopped;
                self.advance()?;
                return Err(SourceEditError::Fatal(JourneyError::Kernel));
            }
        };
        self.results[editor_index].record(
            NativePlot::MemoryLantern,
            value,
            SignId::from(format!(
                "conduitos/product/result/{}/{}/{}",
                receipt.active_play_id.as_str(),
                editor_index,
                self.input_count
            )),
        )?;
        self.results[editor_index].input_sequence = Some(next);
        self.input_count = next;
        self.advance()?;
        self.select_plot(&editor, self.revision)?;
        Ok(receipt)
    }
}

fn exact_span(
    graph: &patchbay_graph::PatchbayGraph,
    expanded: &conduit_plot::ExpandedCanonicalPlot,
    subject: &str,
) -> Result<Option<(usize, usize)>, SourceEditError> {
    if let Some(cord) = graph.cords.iter().find(|cord| cord.identity == subject) {
        let Some(origin) = &cord.source_origin else {
            return Ok(None);
        };
        if origin.source_document_id != expanded.source_document_id
            || origin.checked_plot_id != expanded.checked_plot_id
            || origin.source_plot != expanded.name
        {
            return refuse(SourceEditRefusal::SourceUnavailable);
        }
        return Ok(Some((origin.source_span.start, origin.source_span.end)));
    }
    let mut gears = graph.gears.iter().filter(|gear| {
        gear.identity == subject
            || gear
                .inputs
                .iter()
                .chain(&gear.outputs)
                .any(|port| port.identity == subject)
    });
    let Some(gear) = gears.next() else {
        return Ok(None);
    };
    if gears.next().is_some() || gear.source_plot != expanded.name {
        return refuse(SourceEditRefusal::SourceUnavailable);
    }
    let mut origins = expanded.provenance.iter().filter(|origin| {
        origin.gear_id == gear.gear_id.as_str() && origin.source_plot == expanded.name
    });
    match (origins.next(), origins.next()) {
        (Some(origin), None) => Ok(Some((origin.source_span.start, origin.source_span.end))),
        _ => refuse(SourceEditRefusal::SourceSpan),
    }
}
