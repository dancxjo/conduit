//! Native keyboard controller; canonical BirthDraft remains the only reducer.
use alloc::{format, string::String, vec::Vec};
use conduit_birth_plot::{
    BirthActionOutcome, BirthActions, BirthDraft, BirthDraftRefusal, BirthSelection,
    BirthWidgetFace,
};
use conduit_face::ApplicationEventKind;
use conduit_human::{KeyEvent, KeyModifiers, KeyTransition, PreparedKeymap};
#[derive(serde::Serialize, serde::Deserialize)]
pub enum NativeOutcome {
    Unchanged,
    Changed,
    Birth(BirthSelection),
}
pub(crate) struct Controller {
    pub(crate) draft: BirthDraft,
    pub(crate) focus: usize,
    replace_name: bool,
    keymap: PreparedKeymap,
    pub(crate) refusal: Option<String>,
}
impl Controller {
    pub(crate) fn allow_idle(mut self) -> Self {
        self.draft = self.draft.allow_idle_body();
        self
    }
    pub(crate) fn new(draft: BirthDraft) -> Self {
        Self {
            draft,
            focus: 0,
            replace_name: true,
            keymap: PreparedKeymap::default_layout(),
            refusal: None,
        }
    }
}
impl core::ops::Deref for Controller {
    type Target = BirthDraft;
    fn deref(&self) -> &BirthDraft {
        &self.draft
    }
}
impl core::ops::DerefMut for Controller {
    fn deref_mut(&mut self) -> &mut BirthDraft {
        &mut self.draft
    }
}
impl Controller {
    pub(crate) fn accept(&mut self, event: KeyEvent) -> NativeOutcome {
        if event.transition() != KeyTransition::Pressed {
            // Preserve portable transition handling while a dead key is pending.
            let _ = self.keymap.apply(event);
            return NativeOutcome::Unchanged;
        }
        let controls = self.controls();
        let Some(action) = controls.get(self.focus) else {
            return NativeOutcome::Unchanged;
        };
        let composing = self.keymap.is_composing();
        let result = match event.usage() {
            43 | 81 | 82 => {
                let backward = event.usage() == 82
                    || (event.usage() == 43
                        && event.modifiers_after().bits()
                            & (KeyModifiers::LEFT_SHIFT.bits() | KeyModifiers::RIGHT_SHIFT.bits())
                            != 0);
                let count = controls.len();
                self.focus = (self.focus + if backward { count - 1 } else { 1 }) % count;
                self.replace_name = self.focus == 0;
                self.keymap.reset();
                Ok(())
            }
            59 => self.suggest(), // F2: the same suggestion action as the Crèche button.
            79 | 80 if self.focus == 1 => self.cycle_tradition(event.usage() == 80),
            60 => return self.submit(),
            40 if action == "creche.name" => match self.keymap.apply(event) {
                Ok(Some(fragment)) if !composing && fragment.as_slice() == b"\n" => {
                    return self.submit();
                }
                Ok(Some(fragment)) => self.append_name(fragment.as_slice()),
                Err(_) => {
                    self.refusal = Some("The keyboard could not complete that character.".into());
                    return NativeOutcome::Changed;
                }
                _ => return NativeOutcome::Unchanged,
            },
            40 if action == "creche.birth" => {
                return self.submit();
            }
            40 | 44 if self.focus == 2 => self.suggest(),
            40 | 44 if self.focus == 1 => self.cycle_tradition(false),
            40 | 44 if action.starts_with("creche.plot.") => {
                let Some(index) = action
                    .strip_prefix("creche.plot.")
                    .and_then(|value| value.parse::<usize>().ok())
                else {
                    return NativeOutcome::Unchanged;
                };
                let value = if self.draft.choices()[index].selected {
                    "false"
                } else {
                    "true"
                };
                self.change(
                    &format!("creche.plot.{index}"),
                    ApplicationEventKind::Change,
                    value,
                )
            }
            42 if action == "creche.search" => {
                let mut search = String::from(self.draft.search());
                search.pop();
                self.change("creche.search", ApplicationEventKind::Input, &search)
            }
            _ if action == "creche.search" => match self.keymap.apply(event) {
                Ok(Some(fragment)) => {
                    let mut search = String::from(self.draft.search());
                    search.push_str(core::str::from_utf8(fragment.as_slice()).unwrap());
                    self.change("creche.search", ApplicationEventKind::Input, &search)
                }
                _ => return NativeOutcome::Unchanged,
            },
            42 if self.focus == 0 => {
                let mut name = self.draft.friendly_name().into();
                if self.replace_name {
                    name = String::new();
                } else {
                    String::pop(&mut name);
                }
                self.replace_name = false;
                self.change("creche.name", ApplicationEventKind::Input, &name)
            }
            _ if self.focus == 0 => match self.keymap.apply(event) {
                Ok(Some(fragment)) => self.append_name(fragment.as_slice()),
                _ => return NativeOutcome::Unchanged,
            },
            _ => return NativeOutcome::Unchanged,
        };
        self.refusal = result.err().map(|error| refusal_text(error).into());
        NativeOutcome::Changed
    }

    fn append_name(&mut self, fragment: &[u8]) -> Result<(), BirthDraftRefusal> {
        let text = core::str::from_utf8(fragment).map_err(|_| BirthDraftRefusal::InvalidName)?;
        let mut name = if self.replace_name {
            String::new()
        } else {
            self.draft.friendly_name().into()
        };
        name.push_str(text);
        self.change("creche.name", ApplicationEventKind::Input, &name)?;
        self.replace_name = false;
        Ok(())
    }

    fn controls(&self) -> Vec<String> {
        use conduit_face::FaceMechanism;
        let Ok(view) = self.draft.widget_face() else {
            return Vec::new();
        };
        let mut controls = Vec::new();
        for node in &view.root.children {
            match &node.mechanism {
                FaceMechanism::FormField(field) => {
                    controls.push(field.input_action.identity.clone())
                }
                FaceMechanism::Action(action) => controls.push(action.identity.clone()),
                FaceMechanism::ChoiceGroup { options, .. } => controls.extend(
                    options
                        .iter()
                        .map(|choice| choice.change_action.identity.clone()),
                ),
                _ => {}
            }
        }
        controls
    }

    fn submit(&mut self) -> NativeOutcome {
        match self.draft.apply_event(
            self.draft.revision(),
            "creche.birth",
            ApplicationEventKind::Activate,
            "",
        ) {
            Ok(BirthActionOutcome::Birth(selection)) => {
                self.refusal = None;
                NativeOutcome::Birth(selection)
            }
            Ok(BirthActionOutcome::Changed) => NativeOutcome::Changed,
            Err(error) => {
                self.refusal = Some(refusal_text(error).into());
                NativeOutcome::Changed
            }
        }
    }
    fn suggest(&mut self) -> Result<(), BirthDraftRefusal> {
        self.change("creche.suggest", ApplicationEventKind::Activate, "")?;
        self.replace_name = true;
        Ok(())
    }
    fn cycle_tradition(&mut self, backward: bool) -> Result<(), BirthDraftRefusal> {
        let count = self.draft.naming_systems().count();
        let current = self
            .draft
            .naming_systems()
            .position(|(id, _)| id == self.draft.requested_system())
            .unwrap_or(0);
        let next = (current + if backward { count - 1 } else { 1 }) % count;
        let system: String = self
            .draft
            .naming_systems()
            .nth(next)
            .expect("bounded naming index")
            .0
            .into();
        self.change("creche.naming", ApplicationEventKind::Change, &system)?;
        self.replace_name = true;
        Ok(())
    }

    fn change(
        &mut self,
        action: &str,
        event: ApplicationEventKind,
        value: &str,
    ) -> Result<(), BirthDraftRefusal> {
        match self
            .draft
            .apply_event(self.draft.revision(), action, event, value)?
        {
            BirthActionOutcome::Changed => Ok(()),
            BirthActionOutcome::Birth(_) => Err(BirthDraftRefusal::UnknownAction),
        }
    }
}

fn refusal_text(error: BirthDraftRefusal) -> &'static str {
    match error {
        BirthDraftRefusal::InvalidName => "Give your body a name of at most 64 bytes.",
        BirthDraftRefusal::EmptySelection => "Include at least one plot to begin.",
        BirthDraftRefusal::UnavailablePlot => "This plot cannot run with the current capabilities.",
        BirthDraftRefusal::StaleFace => "This choice has changed. Please choose again.",
        _ => "The Crèche could not accept this change.",
    }
}

mod projection;
mod scene;
