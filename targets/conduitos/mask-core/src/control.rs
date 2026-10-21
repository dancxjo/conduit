//! General entrance reading policy. Decisions are presentation only, never effects.
use crate::data::Refusal;
use conduit_human::{KeyEvent, KeyTransition};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub session: u64,
    pub host: conduit_core::HostId,
    pub boot: conduit_core::BootId,
}
impl Scope {
    fn validate(&self) -> Result<(), Refusal> {
        if self.session == 0
            || [self.host.as_str(), self.boot.as_str()]
                .iter()
                .any(|s| s.is_empty() || s.len() > 2048 || s.chars().any(char::is_control))
        {
            return Err(Refusal::Bounds);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct View {
    pub selected_plot: bool,
    pub plot_open: bool,
    pub exact_details_open: bool,
    pub details_page: u8,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub scope: Scope,
    pub revision: u64,
    pub view: View,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum Operation {
    Read,
    Key([u8; 3]),
    Inspect,
    ObservePlot { opened: bool },
    BodyAdmitted,
}

impl Operation {
    /// Root uses this pure preflight solely to reserve its outer revision;
    /// the service remains the only native owner of the resulting state.
    pub fn changes(self) -> Result<bool, Refusal> {
        Ok(match self {
            Self::Read => false,
            Self::Key(bytes) => {
                let key = KeyEvent::decode(&bytes).map_err(|_| Refusal::Malformed)?;
                key.transition() == KeyTransition::Pressed
                    && matches!(key.usage(), 43 | 79 | 80 | 81 | 82 | 59 | 40 | 41)
            }
            _ => true,
        })
    }
}

/// The one canonical reducer used by the protected service and host reference.
/// Root supplies lifecycle observations; none of these operations opens a Body,
/// selects a Plan, submits a semantic input, or acknowledges a Show.
pub fn apply(view: &mut View, operation: Operation) -> Result<bool, Refusal> {
    match operation {
        Operation::Read => Ok(false),
        Operation::Inspect => {
            view.exact_details_open = true;
            Ok(true)
        }
        Operation::ObservePlot { opened } => {
            view.plot_open = opened;
            Ok(true)
        }
        Operation::BodyAdmitted => {
            view.plot_open = false;
            Ok(true)
        }
        Operation::Key(bytes) => {
            let event = KeyEvent::decode(&bytes).map_err(|_| Refusal::Malformed)?;
            if event.transition() != KeyTransition::Pressed {
                return Ok(false);
            }
            match event.usage() {
                43 | 79 | 80 | 81 | 82 => {
                    view.selected_plot = !view.selected_plot;
                    view.exact_details_open = false;
                }
                59 => {
                    if view.exact_details_open {
                        view.details_page = (view.details_page + 1) % 16;
                    }
                    view.exact_details_open = true;
                }
                40 => {
                    view.plot_open = view.selected_plot;
                    view.exact_details_open = !view.plot_open;
                }
                41 => {
                    view.exact_details_open = false;
                }
                _ => return Ok(false),
            }
            Ok(true)
        }
    }
}
#[derive(Default)]
pub struct Controller {
    current: Option<Snapshot>,
}
impl Controller {
    pub fn at(&self, scope: &Scope, revision: u64) -> Result<View, Refusal> {
        self.current
            .as_ref()
            .filter(|current| &current.scope == scope && current.revision == revision)
            .map(|current| current.view)
            .ok_or(Refusal::Order)
    }

    pub fn view(&self, scope: &Scope) -> Result<View, Refusal> {
        self.current
            .as_ref()
            .filter(|current| &current.scope == scope)
            .map(|current| current.view)
            .ok_or(Refusal::Order)
    }

    pub fn execute(
        &mut self,
        scope: Scope,
        expected: u64,
        operation: Operation,
    ) -> Result<(bool, Snapshot), Refusal> {
        scope.validate()?;
        let old = match &self.current {
            Some(current) if current.scope == scope && current.revision == expected => {
                current.clone()
            }
            Some(_) => return Err(Refusal::Order),
            None if expected == 0 => Snapshot {
                scope,
                revision: 0,
                view: View::default(),
            },
            None => return Err(Refusal::Order),
        };
        let mut next = old;
        let revision = if operation.changes()? {
            next.revision.checked_add(1).ok_or(Refusal::Capacity)?
        } else {
            next.revision
        };
        let changed = apply(&mut next.view, operation)?;
        if changed {
            next.revision = revision;
        }
        self.current = Some(next.clone());
        Ok((changed, next))
    }
}
