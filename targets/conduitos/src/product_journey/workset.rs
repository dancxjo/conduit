//! Authoritative foreground selection and bounded Presentation projections.
use super::*;
use conduit_body::ResidentForm;
use native_workset::NativePresentation;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceForm {
    pub form: ResidentForm,
    pub title: &'static str,
    pub foreground: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceProjection {
    pub body_id: conduit_body::BodyId,
    pub revision: u64,
    pub forms: Vec<WorkspaceForm>,
}

pub(super) struct FormResult {
    recent: ResultWindow,
    current: Option<NativePresentation>,
    pub(super) sign: Option<SignId>,
}
impl FormResult {
    pub(super) fn new() -> Self {
        Self {
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
        }
        self.sign = Some(sign);
        Ok(())
    }
    fn text(&self, form: Option<NativeForm>) -> Option<&str> {
        match form? {
            NativeForm::KeyboardCanvas => self.sign.as_ref().map(|_| self.recent.as_str()),
            NativeForm::MemoryLantern => self.current.as_ref().map(NativePresentation::text),
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
            // Reviewing it here grants no execution; an active Play always
            // uses the already prepared identities above.
            let form = native_workset::checked(self.forms[index].ok_or(JourneyError::WrongTarget)?)
                .map_err(JourneyError::Workset)?;
            KeyboardTextFormIdentity {
                source_document_id: form.source_document_id,
                checked_form_id: form.checked_form_id,
                expanded_form_id: form.expanded_form_id,
            }
        };
        self.form = identity;
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
