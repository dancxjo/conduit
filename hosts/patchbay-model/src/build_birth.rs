//! Canonical Patchbay Build/Birth control over Form and Body lifecycle truth.

use conduit_body::{Body, BodyLifecycleError, BodyState, Wake, WakeLifecycle};
use conduit_core::{ActivePlayIdentity, Plan, PlanId, SignId};

use crate::FormEditor;

pub const MAX_BUILD_DOCUMENT_LINES: usize = 1_024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildBirthError {
    UncheckedRevision,
    MissingOpenForm,
    AlreadyBorn,
    BodyNotBorn,
    BodyNotAwake,
    DocumentTooLarge,
    Form(String),
    Lifecycle(BodyLifecycleError),
}

impl core::fmt::Display for BuildBirthError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            formatter,
            "Patchbay Build/Birth transition failed: {self:?}"
        )
    }
}

impl std::error::Error for BuildBirthError {}

impl From<BodyLifecycleError> for BuildBirthError {
    fn from(value: BodyLifecycleError) -> Self {
        Self::Lifecycle(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchbayMode {
    Build,
    BornLulled,
    Awake(WakeLifecycle),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildRevisionStatus {
    pub current_revision: u64,
    pub saved_revision: u64,
    pub checked_revision: Option<u64>,
    pub born_revision: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildBirthDocument {
    pub mode: PatchbayMode,
    pub revisions: BuildRevisionStatus,
    pub body: Option<Body>,
    pub wake: Option<Wake>,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct BuildBirthController {
    body: Option<Body>,
    wake: Option<Wake>,
    born_revision: Option<u64>,
}

impl BuildBirthController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn body(&self) -> Option<&Body> {
        self.body.as_ref()
    }

    pub fn wake_value(&self) -> Option<&Wake> {
        self.wake.as_ref()
    }

    pub fn birth(
        &mut self,
        editor: &FormEditor,
        birth_sequence: u64,
        sign_id: SignId,
    ) -> Result<(), BuildBirthError> {
        if self.body.is_some() {
            return Err(BuildBirthError::AlreadyBorn);
        }
        let view = editor.view();
        let checked = current_checked(&view)?;
        let form = checked
            .forms
            .iter()
            .find(|form| form.name == view.open_form)
            .ok_or(BuildBirthError::MissingOpenForm)?;
        self.body = Some(Body::born(
            checked
                .source_document_id
                .clone()
                .ok_or(BuildBirthError::UncheckedRevision)?,
            form.checked_form_id.clone(),
            birth_sequence,
            sign_id,
        )?);
        self.born_revision = Some(view.revision);
        self.wake = None;
        Ok(())
    }

    pub fn wake(&mut self, wake_sequence: u64, sign_id: SignId) -> Result<(), BuildBirthError> {
        let body = self.body.as_ref().ok_or(BuildBirthError::BodyNotBorn)?;
        let (body, wake) = body.wake(wake_sequence, sign_id)?;
        self.body = Some(body);
        self.wake = Some(wake);
        Ok(())
    }

    pub fn plan_ready(&mut self, plan: &Plan, sign_id: SignId) -> Result<(), BuildBirthError> {
        let wake = self.wake.as_ref().ok_or(BuildBirthError::BodyNotAwake)?;
        self.wake = Some(wake.plan_ready(plan, sign_id)?);
        Ok(())
    }

    pub fn play_started(
        &mut self,
        play: &ActivePlayIdentity,
        sign_id: SignId,
    ) -> Result<(), BuildBirthError> {
        let wake = self.wake.as_ref().ok_or(BuildBirthError::BodyNotAwake)?;
        self.wake = Some(wake.play_started(play, sign_id)?);
        Ok(())
    }

    pub fn became_unsatisfied(
        &mut self,
        plan_id: &PlanId,
        sign_id: SignId,
    ) -> Result<(), BuildBirthError> {
        let wake = self.wake.as_ref().ok_or(BuildBirthError::BodyNotAwake)?;
        self.wake = Some(wake.became_unsatisfied(plan_id, sign_id)?);
        Ok(())
    }

    pub fn same_plan_observed(
        &mut self,
        plan_id: &PlanId,
        sign_id: SignId,
    ) -> Result<(), BuildBirthError> {
        let wake = self.wake.as_ref().ok_or(BuildBirthError::BodyNotAwake)?;
        self.wake = Some(wake.same_plan_observed(plan_id, sign_id)?);
        Ok(())
    }

    pub fn lull(&mut self, sign_id: SignId, retained_sign: SignId) -> Result<(), BuildBirthError> {
        let wake = self
            .wake
            .as_ref()
            .ok_or(BuildBirthError::BodyNotAwake)?
            .lull(sign_id)?;
        self.retain_terminal_wake(wake, retained_sign)
    }

    pub fn fail_wake(
        &mut self,
        sign_id: SignId,
        retained_sign: SignId,
    ) -> Result<(), BuildBirthError> {
        let wake = self
            .wake
            .as_ref()
            .ok_or(BuildBirthError::BodyNotAwake)?
            .fail(sign_id)?;
        self.retain_terminal_wake(wake, retained_sign)
    }

    fn retain_terminal_wake(
        &mut self,
        wake: Wake,
        retained_sign: SignId,
    ) -> Result<(), BuildBirthError> {
        let body = self.body.as_ref().ok_or(BuildBirthError::BodyNotBorn)?;
        self.body = Some(body.retain_after_lull(&wake, retained_sign)?);
        self.wake = Some(wake);
        Ok(())
    }

    pub fn document(&self, editor: &FormEditor) -> Result<BuildBirthDocument, BuildBirthError> {
        let view = editor.view();
        let checked_revision = current_checked(&view).ok().map(|checked| checked.revision);
        let revisions = BuildRevisionStatus {
            current_revision: view.revision,
            saved_revision: view.saved_revision,
            checked_revision,
            born_revision: self.born_revision,
        };
        let mode = match self.body.as_ref().map(|body| &body.state) {
            None => PatchbayMode::Build,
            Some(BodyState::Lulled) => PatchbayMode::BornLulled,
            Some(BodyState::Awake { .. }) => PatchbayMode::Awake(
                self.wake
                    .as_ref()
                    .ok_or(BuildBirthError::BodyNotAwake)?
                    .lifecycle,
            ),
        };
        let mut lines = vec![format!(
            "BUILD current={} saved={} checked={} last-born={}",
            revisions.current_revision,
            revisions.saved_revision,
            optional_revision(revisions.checked_revision),
            optional_revision(revisions.born_revision)
        )];
        append_form_lines(editor, &view, &mut lines)?;
        append_lifecycle_lines(self.body.as_ref(), self.wake.as_ref(), &mut lines);
        if lines.len() > MAX_BUILD_DOCUMENT_LINES {
            return Err(BuildBirthError::DocumentTooLarge);
        }
        Ok(BuildBirthDocument {
            mode,
            revisions,
            body: self.body.clone(),
            wake: self.wake.clone(),
            lines,
        })
    }
}

fn current_checked(
    view: &crate::FormDocumentView,
) -> Result<&crate::CheckedRevision, BuildBirthError> {
    if view.checked.revision != view.revision
        || view.checked.source_document_id.is_none()
        || !view.checked.diagnostics.is_empty()
    {
        return Err(BuildBirthError::UncheckedRevision);
    }
    Ok(&view.checked)
}

fn optional_revision(value: Option<u64>) -> String {
    value.map_or_else(|| "not-present".into(), |value| value.to_string())
}

fn append_form_lines(
    editor: &FormEditor,
    view: &crate::FormDocumentView,
    lines: &mut Vec<String>,
) -> Result<(), BuildBirthError> {
    let Ok(checked) = current_checked(view) else {
        lines.push("FORM unchecked — Birth unavailable".into());
        return Ok(());
    };
    for form in &checked.forms {
        lines.push(format!(
            "FORM {} checked={} FACE inputs={} outputs={}",
            form.name,
            form.checked_form_id.as_str(),
            form.face.inputs().len(),
            form.face.outputs().len()
        ));
        for port in form.face.inputs().iter().chain(form.face.outputs()) {
            lines.push(format!(
                "PORT {} direction={:?} info={} temporal={:?}",
                port.port_id.as_str(),
                port.direction,
                port.value_kind.as_str(),
                port.temporal
            ));
        }
    }
    let expanded = match editor.expand_form(&view.open_form) {
        Ok(expanded) => expanded,
        Err(error) => {
            lines.push(format!(
                "BACK {} checked; closed expansion unavailable: {error}",
                view.open_form
            ));
            if let Some(form) = checked
                .forms
                .iter()
                .find(|form| form.name == view.open_form)
            {
                lines.extend(
                    form.items
                        .iter()
                        .map(|item| format!("{:?} {} {}", item.kind, item.identity, item.label)),
                );
            }
            return Ok(());
        }
    };
    for gear in &expanded.gears {
        lines.push(format!(
            "GEAR {} kind={} inputs={} outputs={}",
            gear.gear_id.as_str(),
            gear.kind_id.as_str(),
            gear.inputs.len(),
            gear.outputs.len()
        ));
    }
    for cord in &expanded.connections {
        lines.push(format!(
            "CORD {}.{} -> {}.{} info={} temporal={:?}",
            cord.source_gear_id.as_str(),
            cord.source_port_id.as_str(),
            cord.sink_gear_id.as_str(),
            cord.sink_port_id.as_str(),
            cord.value_kind.as_str(),
            cord.temporal
        ));
    }
    Ok(())
}

fn append_lifecycle_lines(body: Option<&Body>, wake: Option<&Wake>, lines: &mut Vec<String>) {
    let Some(body) = body else {
        lines.push("BODY not born — action: Birth Body".into());
        return;
    };
    lines.push(format!(
        "BODY {} seed={} birth-sequence={} state={:?} checked={}",
        body.body_id.as_str(),
        body.seed_id.as_str(),
        body.birth_sequence,
        body.state,
        body.checked_form_id.as_str()
    ));
    for event in &body.events {
        lines.push(format!("BODY EVENT {event:?}"));
    }
    let Some(wake) = wake else {
        lines.push("BORN · LULLED — action: Wake Body".into());
        return;
    };
    lines.push(format!(
        "WAKE {} sequence={} lifecycle={:?}",
        wake.wake_id.as_str(),
        wake.wake_sequence,
        wake.lifecycle
    ));
    for plan in &wake.plans {
        lines.push(format!(
            "PLAN {} state={:?} PLAY {}",
            plan.plan_id.as_str(),
            plan.state,
            plan.active_play_id
                .as_ref()
                .map_or("not-present", |play| play.as_str())
        ));
    }
    for event in &wake.events {
        lines.push(format!("WAKE EVENT {event:?}"));
    }
}
