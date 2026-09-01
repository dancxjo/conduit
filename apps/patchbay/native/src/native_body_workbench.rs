//! Native composition around the accepted Body attachment. Program keeps the
//! existing native Form canvas; Body and History are read-only manifestations
//! of the same bounded evidence document.

use crate::arguments::NativeBodyEntrance;
use patchbay_model::{
    CurrentBodyFrame, PatchbayBodyApplicationEntrance, PatchbayBodyAttachment, PatchbayGraph,
    ReadableBodyHistory,
};
use winit::keyboard::{Key, NamedKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeWorkbenchDestination {
    Program,
    Body,
    History,
}

impl NativeWorkbenchDestination {
    pub fn semantic_cursor(
        self,
    ) -> (
        conduit_presentation::PresentationPlace,
        conduit_presentation::PresentationAspect,
    ) {
        use conduit_presentation::{PresentationAspect, PresentationPlace};
        match self {
            Self::Program => (PresentationPlace::Program, PresentationAspect::Structure),
            Self::Body => (PresentationPlace::Body, PresentationAspect::Structure),
            Self::History => (PresentationPlace::Body, PresentationAspect::Signs),
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Program => Self::Body,
            Self::Body => Self::History,
            Self::History => Self::Program,
        }
    }
}

#[derive(Debug)]
pub struct NativeBodyWorkbench {
    evidence_revision: u64,
    encoded_evidence: Vec<u8>,
    current: CurrentBodyFrame,
    history: ReadableBodyHistory,
    destination: NativeWorkbenchDestination,
    history_selection: usize,
}

impl NativeBodyWorkbench {
    pub fn open(
        evidence_revision: u64,
        encoded_evidence: Vec<u8>,
        entrance: NativeBodyEntrance,
        graph: &PatchbayGraph,
    ) -> Result<Self, NativeBodyWorkbenchError> {
        if evidence_revision == 0 {
            return Err(NativeBodyWorkbenchError::InvalidRevision);
        }
        let attachment =
            PatchbayBodyAttachment::open_serialized(&encoded_evidence, model_entrance(entrance))
                .map_err(NativeBodyWorkbenchError::Entrance)?;
        if attachment.evidence().body.source_document_id != graph.source_document_id
            || attachment.evidence().body.checked_form_id != graph.checked_form_id
        {
            return Err(NativeBodyWorkbenchError::ProgramIdentityMismatch);
        }
        let current = CurrentBodyFrame::from_attachment(evidence_revision, &attachment);
        let history = ReadableBodyHistory::from_attachment(evidence_revision, &attachment)
            .map_err(NativeBodyWorkbenchError::History)?;
        Ok(Self {
            evidence_revision,
            encoded_evidence,
            current,
            history,
            destination: NativeWorkbenchDestination::Body,
            history_selection: 0,
        })
    }

    pub fn destination(&self) -> NativeWorkbenchDestination {
        self.destination
    }

    pub fn select(&mut self, destination: NativeWorkbenchDestination) {
        self.destination = destination;
    }

    pub fn cycle_destination(&mut self) {
        self.destination = self.destination.next();
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn current(&self) -> &CurrentBodyFrame {
        &self.current
    }

    pub fn lifecycle_flow(&self) -> crate::lifecycle_flow::LifecycleFlow {
        use crate::lifecycle_flow::{LifecycleFlow, LifecycleFlowAction};
        use patchbay_model::PatchbayAction;
        let (action, label, accelerator) = match self.current.salient_action {
            patchbay_model::CurrentBodyLifecycleAction::Wake => {
                (PatchbayAction::Wake, "WAKE", "F5")
            }
            patchbay_model::CurrentBodyLifecycleAction::Lull => {
                (PatchbayAction::Lull, "LULL", "F9")
            }
        };
        LifecycleFlow {
            state_code: match self.current.lifecycle {
                patchbay_model::CurrentBodyLifecycle::Lulled => "ATTACHED_LULLED",
                patchbay_model::CurrentBodyLifecycle::Awake { .. } => "ATTACHED_AWAKE",
            },
            state_text: self.current.status_line.clone(),
            detail: self.current.placement_line.into(),
            exact_basis: format!(
                "body={} evidence-revision={} source={} checked={}",
                self.current.body_id.as_str(),
                self.evidence_revision,
                self.current.program.source_document_id.as_str(),
                self.current.program.checked_form_id.as_str()
            ),
            actions: vec![LifecycleFlowAction {
                action,
                label,
                accelerator,
            }],
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn history(&self) -> &ReadableBodyHistory {
        &self.history
    }

    pub fn history_selection(&self) -> usize {
        self.history_selection
    }

    pub fn select_history(&mut self, index: usize) -> Result<(), String> {
        if index >= self.history.entries.len() {
            return Err("Body History selection is outside the finite biography".into());
        }
        self.history_selection = index;
        self.destination = NativeWorkbenchDestination::History;
        Ok(())
    }

    fn move_history(&mut self, delta: isize) {
        let maximum = self.history.entries.len().saturating_sub(1);
        self.history_selection = self
            .history_selection
            .saturating_add_signed(delta)
            .min(maximum);
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn encoded_evidence(&self) -> &[u8] {
        &self.encoded_evidence
    }

    pub fn lines(&self, linear: bool, exact: bool) -> Vec<String> {
        let (place, aspect) = self.destination.semantic_cursor();
        let mut lines = vec![
            format!(
                "{} · {:?} / {:?}",
                self.current.friendly_name, place, aspect
            ),
            "PROGRAM | BODY | HISTORY   CTRL-TAB TO MOVE   F2 LINEAR".into(),
        ];
        match self.destination {
            NativeWorkbenchDestination::Program => {
                lines.push(format!("PROGRAM {}", self.current.program.label));
                lines.push(
                    "The existing native Gear / Port / Cord canvas remains authoritative.".into(),
                );
            }
            NativeWorkbenchDestination::Body => self.append_body_lines(&mut lines),
            NativeWorkbenchDestination::History => self.append_history_lines(&mut lines, linear),
        }
        if exact {
            lines.extend([
                format!("EXACT evidence-revision={}", self.evidence_revision),
                format!("EXACT body={}", self.current.body_id.as_str()),
                format!(
                    "EXACT source={} checked={}",
                    self.current.program.source_document_id.as_str(),
                    self.current.program.checked_form_id.as_str()
                ),
                format!("EXACT evidence-bytes={}", self.encoded_evidence.len()),
            ]);
            if self.destination == NativeWorkbenchDestination::History {
                let entry = &self.history.entries[self.history_selection];
                lines.extend([
                    format!("EXACT focused-sign={}", entry.exact.record.sign_id.as_str()),
                    format!("EXACT focus-subject={}", entry.inspect.subject_identity),
                    format!(
                        "EXACT record={}",
                        serde_json::to_string(&entry.exact.record)
                            .expect("validated Body biography record remains serializable")
                    ),
                ]);
            }
        }
        lines
    }

    fn append_body_lines(&self, lines: &mut Vec<String>) {
        lines.extend([
            format!(
                "BODY {} · {:?}",
                self.current.body_id.as_str(),
                self.current.lifecycle
            ),
            self.current.status_line.clone(),
            self.current.placement_line.into(),
            format!("SALIENT ACTION {:?}", self.current.salient_action),
            format!("PARTS admitted={}", self.current.admitted_parts),
            "LINES not evidenced by the retained biography".into(),
            "CAPABILITIES / AVAILABILITY not evidenced by the retained biography".into(),
        ]);
        for host in &self.current.current_hosts {
            lines.push(format!(
                "HOST part={} host={} boot={} offer-generation={} observation-sequence={}",
                host.part_id.as_str(),
                host.host_id.as_str(),
                host.boot_id.as_str(),
                host.offer_generation.0,
                host.observation_sequence
            ));
        }
    }

    fn append_history_lines(&self, lines: &mut Vec<String>, linear: bool) {
        lines.push(format!(
            "HISTORY is BODY / SIGNS · {} finite entries · no authoritative clock time",
            self.history.entries.len()
        ));
        for (index, entry) in self.history.entries.iter().enumerate() {
            if linear {
                lines.push(entry.linear.clone());
            } else {
                lines.push(format!(
                    "{} {:?} · {}",
                    if index == self.history_selection {
                        ">"
                    } else {
                        " "
                    },
                    entry.moment,
                    entry.title
                ));
                lines.push(format!("  {}", entry.narrative));
            }
        }
    }

    pub fn detach(self) -> Vec<u8> {
        self.encoded_evidence
    }
}

#[derive(Debug, Default)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct NativeBodyWorkbenchSlot {
    last_revision: Option<u64>,
    current: Option<NativeBodyWorkbench>,
}

impl NativeBodyWorkbenchSlot {
    pub fn is_attached(&self) -> bool {
        self.current.is_some()
    }

    pub fn current(&self) -> Option<&NativeBodyWorkbench> {
        self.current.as_ref()
    }

    pub fn current_mut(&mut self) -> Option<&mut NativeBodyWorkbench> {
        self.current.as_mut()
    }

    pub fn replace(
        &mut self,
        revision: u64,
        encoded: Vec<u8>,
        entrance: NativeBodyEntrance,
        graph: &PatchbayGraph,
    ) -> Result<&NativeBodyWorkbench, NativeBodyWorkbenchError> {
        self.current = None;
        if let Some(current) = self.last_revision {
            if revision <= current {
                return Err(NativeBodyWorkbenchError::StaleRevision {
                    current,
                    offered: revision,
                });
            }
        }
        self.last_revision = Some(revision);
        self.current = Some(NativeBodyWorkbench::open(
            revision, encoded, entrance, graph,
        )?);
        Ok(self.current.as_ref().expect("installed native workbench"))
    }

    pub fn detach(&mut self) -> Option<Vec<u8>> {
        self.current.take().map(NativeBodyWorkbench::detach)
    }
}

#[derive(Debug)]
pub enum NativeBodyWorkbenchError {
    InvalidRevision,
    StaleRevision { current: u64, offered: u64 },
    Entrance(patchbay_model::PatchbayBodyEntranceError),
    History(patchbay_model::ReadableBodyHistoryError),
    ProgramIdentityMismatch,
}

impl core::fmt::Display for NativeBodyWorkbenchError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidRevision => formatter.write_str("evidence revision is zero"),
            Self::StaleRevision { current, offered } => write!(
                formatter,
                "stale evidence revision {offered}; current revision is {current}"
            ),
            Self::Entrance(error) => write!(formatter, "attachment refused: {error:?}"),
            Self::History(error) => write!(formatter, "history refused: {error:?}"),
            Self::ProgramIdentityMismatch => {
                formatter.write_str("Body Program identity does not match the open Form")
            }
        }
    }
}

fn model_entrance(entrance: NativeBodyEntrance) -> PatchbayBodyApplicationEntrance {
    match entrance {
        NativeBodyEntrance::Hosted {
            plan_id,
            implementation_id,
        } => PatchbayBodyApplicationEntrance::Hosted {
            plan_id: conduit_core::PlanId::from(plan_id),
            implementation_id: conduit_core::ImplementationId::from(implementation_id),
        },
        NativeBodyEntrance::ExternalReader => PatchbayBodyApplicationEntrance::ExternalReader,
    }
}

impl crate::PatchbayApplication {
    pub(super) fn handle_body_workbench_key(&mut self, key: &Key) -> bool {
        let Some(workbench) = self.body_workbench.current_mut() else {
            return false;
        };
        match key {
            Key::Named(NamedKey::Tab) if self.modifiers.control_key() => {
                workbench.cycle_destination();
                self.parts_open = false;
                true
            }
            Key::Named(NamedKey::F2) if self.modifiers.shift_key() => {
                self.exact_identity_open = !self.exact_identity_open;
                true
            }
            Key::Named(NamedKey::ArrowUp)
                if workbench.destination() == NativeWorkbenchDestination::History =>
            {
                workbench.move_history(-1);
                true
            }
            Key::Named(NamedKey::ArrowDown)
                if workbench.destination() == NativeWorkbenchDestination::History =>
            {
                workbench.move_history(1);
                true
            }
            Key::Named(NamedKey::Enter)
                if workbench.destination() == NativeWorkbenchDestination::History =>
            {
                self.exact_identity_open = !self.exact_identity_open;
                true
            }
            Key::Named(
                NamedKey::F4
                | NamedKey::F5
                | NamedKey::F6
                | NamedKey::F7
                | NamedKey::F8
                | NamedKey::F9
                | NamedKey::F12,
            ) => {
                self.publish_refusal(
                    "Attached Body action is unavailable: this reader has no lifecycle coordinator",
                );
                true
            }
            _ => false,
        }
    }
}
