//! Authoritative foreground selection and bounded Presentation projections.
use super::*;
use conduit_body::ResidentPlot;
use native_workset::NativePresentation;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspacePlot {
    pub plot: ResidentPlot,
    pub title: &'static str,
    pub foreground: bool,
    pub input: Option<native_workset::AdmittedPlotInput>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceProjection {
    pub body_id: conduit_body::BodyId,
    pub revision: u64,
    pub plots: Vec<WorkspacePlot>,
}

pub(super) struct PlotResult {
    pub(super) input_sequence: Option<u32>,
    recent: ResultWindow,
    current: Option<NativePresentation>,
    pub(super) sign: Option<SignId>,
}
impl PlotResult {
    pub(super) fn new() -> Self {
        Self {
            input_sequence: None,
            recent: ResultWindow::new(),
            current: None,
            sign: None,
        }
    }
    pub(super) fn record(
        &mut self,
        plot: NativePlot,
        value: NativePresentation,
        sign: SignId,
    ) -> Result<(), JourneyError> {
        match plot {
            NativePlot::KeyboardCanvas => self
                .recent
                .append(value.text())
                .map_err(|_| JourneyError::Kernel)?,
            NativePlot::MemoryLantern => self.current = Some(value),
            NativePlot::Tour => self.current = Some(value),
            NativePlot::Patchbay => self.current = Some(value),
        }
        self.sign = Some(sign);
        Ok(())
    }
    fn text(&self, plot: Option<NativePlot>) -> Option<&str> {
        match plot? {
            NativePlot::KeyboardCanvas => self.sign.as_ref().map(|_| self.recent.as_str()),
            NativePlot::MemoryLantern => self.current.as_ref().map(NativePresentation::text),
            NativePlot::Tour => self.current.as_ref().map(NativePresentation::text),
            NativePlot::Patchbay => self.current.as_ref().map(NativePresentation::text),
        }
    }
    pub(super) fn omitted_bytes(&self, plot: Option<NativePlot>) -> u64 {
        if plot == Some(NativePlot::KeyboardCanvas) {
            self.recent.omitted_bytes()
        } else {
            0
        }
    }
}

impl ProductJourney {
    pub(super) fn admit_next_plot(&mut self) -> Result<(), JourneyError> {
        if self.status != JourneyStatus::QuiescentAwaitingInput {
            return Err(JourneyError::InvalidTransition);
        }
        let body = self.body.as_ref().ok_or(JourneyError::BodyAbsent)?;
        let plot = native_workset::profile()
            .installed()
            .iter()
            .copied()
            .find(|candidate| {
                native_workset::resident(*candidate)
                    .is_ok_and(|resident| !body.workset.contains(&resident))
            })
            .ok_or(JourneyError::InvalidTransition)?;
        let resident = native_workset::resident(plot).map_err(JourneyError::Workset)?;
        let next_body = body
            .admit_plot(
                resident.clone(),
                SignId::from(format!("conduitos/product/plot-admitted/{}", self.revision)),
            )
            .map_err(|_| JourneyError::InvalidTransition)?;
        let next_wake = self
            .wake
            .as_ref()
            .ok_or(JourneyError::BodyAbsent)?
            .workload_changed(
                &next_body,
                SignId::from(format!(
                    "conduitos/product/workload-changed/{}",
                    self.revision
                )),
            )
            .map_err(|_| JourneyError::InvalidTransition)?;
        let insertion = next_body
            .workset
            .plots()
            .iter()
            .position(|entry| entry == &resident)
            .ok_or(JourneyError::InvalidTransition)?;
        let previous_len = body.workset.len();
        if previous_len >= self.plots.len() || insertion > previous_len {
            return Err(JourneyError::InvalidTransition);
        }
        if let Some(kernel) = self.kernel.as_mut() {
            kernel.cancel().map_err(JourneyError::Play)?;
            self.retained_kernel_sign_gap = kernel.sign_retention_gap();
        }
        // BodyWorkset is canonically sorted, so admission can insert anywhere.
        // Keep semantic identities, retained results and foreground selection aligned.
        self.plots[insertion..=previous_len].rotate_right(1);
        self.plots[insertion] = Some(plot);
        self.results[insertion..=previous_len].rotate_right(1);
        self.results[insertion] = PlotResult::new();
        if self.foreground >= insertion {
            self.foreground += 1;
        }
        self.body = Some(next_body);
        self.wake = Some(next_wake);
        self.plan = None;
        self.planned_play = None;
        self.play = None;
        self.kernel = None;
        self.input_owners = core::array::from_fn(|_| None);
        self.application_request = None;
        self.mask_control = None;
        self.status = JourneyStatus::Awake;
        Ok(())
    }

    /// The accepted input sequence that last produced foreground Presentation.
    /// Releases consumed without output leave this value unchanged.
    pub fn foreground_presentation_sequence(&self) -> Option<u32> {
        self.results[self.foreground].input_sequence
    }

    pub fn workspace_projection(&self) -> Option<WorkspaceProjection> {
        let body = self.body.as_ref()?;
        Some(WorkspaceProjection {
            body_id: body.body_id.clone(),
            revision: self.revision,
            plots: body
                .workset
                .plots()
                .iter()
                .enumerate()
                .map(|(index, plot)| WorkspacePlot {
                    plot: plot.clone(),
                    title: self.plots[index].expect("admitted workset").title(),
                    foreground: index == self.foreground,
                    input: self.input_owners[index].clone(),
                })
                .collect(),
        })
    }

    pub fn select_plot(&mut self, plot: &ResidentPlot, revision: u64) -> Result<(), JourneyError> {
        if revision != self.revision {
            return Err(JourneyError::StalePresentation);
        }
        let body = self.body.as_ref().ok_or(JourneyError::BodyAbsent)?;
        let index = body
            .workset
            .plots()
            .iter()
            .position(|candidate| candidate == plot)
            .ok_or(JourneyError::WrongTarget)?;
        if index == self.foreground {
            return Ok(());
        }
        let previous = self.foreground;
        if self.plots[index] == Some(NativePlot::Patchbay) {
            self.kernel
                .as_mut()
                .ok_or(JourneyError::Kernel)?
                .select_patchbay_target(index, previous)
                .map_err(JourneyError::Play)?;
        }
        self.revision
            .checked_add(1)
            .ok_or(JourneyError::RevisionExhausted)?;
        let identity = if let Some(plan) = &self.plan {
            let resident = &body.workset.plots()[index];
            let plot = &plan
                .plots
                .iter()
                .find(|planned| &planned.plot == resident)
                .ok_or(JourneyError::WrongTarget)?
                .plan;
            KeyboardTextPlotIdentity {
                source_document_id: plot.source_document_id.clone(),
                checked_plot_id: plot.checked_plot_id.clone(),
                expanded_plot_id: plot.expanded_plot_id.clone(),
            }
        } else {
            // A born or awaiting-plan Body still has exact resident meaning.
            // Reviewing it here grants no execution; an active play always
            // uses the already prepared identities above.
            let plot = native_workset::checked(self.plots[index].ok_or(JourneyError::WrongTarget)?)
                .map_err(JourneyError::Workset)?;
            KeyboardTextPlotIdentity {
                source_document_id: plot.source_document_id,
                checked_plot_id: plot.checked_plot_id,
                expanded_plot_id: plot.expanded_plot_id,
            }
        };
        self.plot = Some(identity);
        self.foreground = index;
        self.advance()
    }

    pub fn select_next_plot(&mut self, revision: u64) -> Result<(), JourneyError> {
        let body = self.body.as_ref().ok_or(JourneyError::BodyAbsent)?;
        let plot = body.workset.plots()[(self.foreground + 1) % body.workset.len()].clone();
        self.select_plot(&plot, revision)
    }

    pub(super) fn foreground_result(&self) -> Option<&str> {
        self.results[self.foreground].text(self.plots[self.foreground])
    }
}

#[cfg(test)]
#[path = "workset_admission_tests.rs"]
mod admission_tests;
