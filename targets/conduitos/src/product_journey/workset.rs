//! Authoritative foreground selection and bounded Presentation projections.
use super::*;
use conduit_body::ResidentForm;
use native_workset::NativePresentation;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceForm {
    pub form: ResidentForm,
    pub title: &'static str,
    pub foreground: bool,
    pub input: Option<native_workset::AdmittedFormInput>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceProjection {
    pub body_id: conduit_body::BodyId,
    pub revision: u64,
    pub forms: Vec<WorkspaceForm>,
}

pub(super) struct FormResult {
    pub(super) input_sequence: Option<u32>,
    recent: ResultWindow,
    current: Option<NativePresentation>,
    pub(super) sign: Option<SignId>,
}
impl FormResult {
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
        form: NativeForm,
        value: NativePresentation,
        sign: SignId,
    ) -> Result<(), JourneyError> {
        match form {
            NativeForm::KeyboardCanvas => self
                .recent
                .append(value.text())
                .map_err(|_| JourneyError::Kernel)?,
            NativeForm::MemoryLantern => self.current = Some(value),
            NativeForm::Tour => self.current = Some(value),
            NativeForm::Patchbay => self.current = Some(value),
        }
        self.sign = Some(sign);
        Ok(())
    }
    fn text(&self, form: Option<NativeForm>) -> Option<&str> {
        match form? {
            NativeForm::KeyboardCanvas => self.sign.as_ref().map(|_| self.recent.as_str()),
            NativeForm::MemoryLantern => self.current.as_ref().map(NativePresentation::text),
            NativeForm::Tour => self.current.as_ref().map(NativePresentation::text),
            NativeForm::Patchbay => self.current.as_ref().map(NativePresentation::text),
        }
    }
    pub(super) fn omitted_bytes(&self, form: Option<NativeForm>) -> u64 {
        if form == Some(NativeForm::KeyboardCanvas) {
            self.recent.omitted_bytes()
        } else {
            0
        }
    }
}

impl ProductJourney {
    pub(super) fn admit_next_form(&mut self) -> Result<(), JourneyError> {
        if self.status != JourneyStatus::QuiescentAwaitingInput {
            return Err(JourneyError::InvalidTransition);
        }
        let body = self.body.as_ref().ok_or(JourneyError::BodyAbsent)?;
        let form = native_workset::profile()
            .installed()
            .iter()
            .copied()
            .find(|candidate| {
                native_workset::resident(*candidate)
                    .is_ok_and(|resident| !body.workset.contains(&resident))
            })
            .ok_or(JourneyError::InvalidTransition)?;
        let resident = native_workset::resident(form).map_err(JourneyError::Workset)?;
        let next_body = body
            .admit_form(
                resident,
                SignId::from(format!("conduitos/product/form-admitted/{}", self.revision)),
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
        if let Some(kernel) = self.kernel.as_mut() {
            kernel.cancel().map_err(JourneyError::Play)?;
            self.retained_kernel_sign_gap = kernel.sign_retention_gap();
        }
        let slot = body.workset.len();
        self.forms[slot] = Some(form);
        self.body = Some(next_body);
        self.wake = Some(next_wake);
        self.plan = None;
        self.planned_play = None;
        self.play = None;
        self.kernel = None;
        self.input_owners = core::array::from_fn(|_| None);
        self.application_request = None;
        self.presenter_control = None;
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
            forms: body
                .workset
                .forms()
                .iter()
                .enumerate()
                .map(|(index, form)| WorkspaceForm {
                    form: form.clone(),
                    title: self.forms[index].expect("admitted workset").title(),
                    foreground: index == self.foreground,
                    input: self.input_owners[index].clone(),
                })
                .collect(),
        })
    }

    pub fn select_form(&mut self, form: &ResidentForm, revision: u64) -> Result<(), JourneyError> {
        if revision != self.revision {
            return Err(JourneyError::StalePresentation);
        }
        let body = self.body.as_ref().ok_or(JourneyError::BodyAbsent)?;
        let index = body
            .workset
            .forms()
            .iter()
            .position(|candidate| candidate == form)
            .ok_or(JourneyError::WrongTarget)?;
        if index == self.foreground {
            return Ok(());
        }
        let previous = self.foreground;
        if self.forms[index] == Some(NativeForm::Patchbay) {
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
            let form = &plan.forms[index].plan;
            KeyboardTextFormIdentity {
                source_document_id: form.source_document_id.clone(),
                checked_form_id: form.checked_form_id.clone(),
                expanded_form_id: form.expanded_form_id.clone(),
            }
        } else {
            // A born or awaiting-plan Body still has exact resident meaning.
            // Reviewing it here grants no execution; an active play always
            // uses the already prepared identities above.
            let form = native_workset::checked(self.forms[index].ok_or(JourneyError::WrongTarget)?)
                .map_err(JourneyError::Workset)?;
            KeyboardTextFormIdentity {
                source_document_id: form.source_document_id,
                checked_form_id: form.checked_form_id,
                expanded_form_id: form.expanded_form_id,
            }
        };
        self.form = Some(identity);
        self.foreground = index;
        self.advance()
    }

    pub fn select_next_form(&mut self, revision: u64) -> Result<(), JourneyError> {
        let body = self.body.as_ref().ok_or(JourneyError::BodyAbsent)?;
        let form = body.workset.forms()[(self.foreground + 1) % body.workset.len()].clone();
        self.select_form(&form, revision)
    }

    pub(super) fn foreground_result(&self) -> Option<&str> {
        self.results[self.foreground].text(self.forms[self.foreground])
    }
}
