//! The continuing owner's bounded document projection. Edits execute the checked
//! Source transition; a Mask never runs the reducer or supplies the current state.
use super::{debug, state, BoundedOutput, DeadlineTimer, Owner};
use conduit_body::{BodyPlotPlan, ResidentPlot};
use conduit_core::{
    port_id, BaseImplementationId, ConnectionTrack, PortDirection, TerminalDisposition,
};
use conduit_semantic_catalog::{
    TextDocument, TextEdit, MAXIMUM_TEXT_DOCUMENT_BYTES, MAXIMUM_TEXT_EDIT_BYTES,
};
use conduit_std_host::body_execution::{BodyForeOutputAdapter, BodyRunRequest};
use conduit_std_host::{ExternalForeDelivery, ExternalForeInput, RunControl};
use std::{
    collections::BTreeMap,
    path::Path,
    time::{Duration, Instant},
};

#[path = "notebook_checkpoint.rs"]
mod checkpoint;
#[path = "notebook_face.rs"]
mod face;
pub(crate) use face::NotebookAction;
#[path = "notebook_navigation.rs"]
mod navigation;
#[path = "notebook_waiting.rs"]
mod waiting;
pub(crate) use waiting::NotebookWaitingWorker;
#[cfg(target_os = "linux")]
#[path = "notebook_file.rs"]
mod file;

pub(super) const SOURCE: &str = include_str!("../../../../plots/notebook/main.conduit");

pub(super) struct NotebookDocument {
    name: String,
    /// Exact canonical output of the last successfully completed Source step.
    current: Vec<u8>,
    committed: Option<Vec<u8>>,
    save_failed: bool,
    navigation: navigation::NotebookNavigation,
    #[cfg(target_os = "linux")]
    storage: Option<std::fs::File>,
    #[cfg(target_os = "linux")]
    storage_path: Option<std::path::PathBuf>,
}

struct CurrentOutput(Vec<u8>);
impl BodyForeOutputAdapter for CurrentOutput {
    fn deliver(&mut self, value: &ExternalForeDelivery) -> Result<(), String> {
        if !self.0.is_empty()
            || value.front_port_id != port_id("current")
            || value.track != ConnectionTrack::Payload
            || value.bytes.len() > MAXIMUM_TEXT_DOCUMENT_BYTES as usize
        {
            return Err("notebook step emitted an unexpected document".into());
        }
        TextDocument::decode(&value.bytes).map_err(debug)?;
        self.0.extend_from_slice(&value.bytes);
        Ok(())
    }
}

impl Owner {
    /// Birth the empty document only for a new, exact notebook resident. Recovery
    /// must load a selected committed generation instead of calling this again.
    pub(crate) fn initialize_notebook(&mut self, name: &str) -> Result<(), String> {
        let source = crate::plot_source::parse(SOURCE)?;
        let plot = source.expand_entry_for_authoring()?;
        let resident = ResidentPlot::new(
            plot.expanded.source_document_id,
            plot.expanded.checked_plot_id,
        );
        if self.resident.as_ref() != Some(&resident)
            || self.notebook.is_some()
            || self.last_execution.is_some()
            || self.session.realization().is_some()
            || name.is_empty()
            || name.len() > 128
            || name.chars().any(char::is_control)
        {
            return Err("new notebook requires its exact idle resident and a bounded name".into());
        }
        let mut current = vec![0; MAXIMUM_TEXT_DOCUMENT_BYTES as usize];
        let length = TextDocument {
            revision: 0,
            text: b"",
        }
        .encode_into(&mut current)
        .map_err(debug)?;
        current.truncate(length);
        self.notebook = Some(NotebookDocument {
            name: name.into(),
            current,
            committed: None,
            save_failed: false,
            navigation: navigation::NotebookNavigation::default(),
            #[cfg(target_os = "linux")]
            storage: None,
            #[cfg(target_os = "linux")]
            storage_path: None,
        });
        Ok(())
    }

    pub(crate) fn notebook_body_id(&self) -> &conduit_body::BodyId {
        &self.session.evidence().body_id
    }

    pub(crate) fn notebook_document(&self) -> Option<(&str, TextDocument<'_>)> {
        let notebook = self.notebook.as_ref()?;
        Some((
            &notebook.name,
            TextDocument::decode(&notebook.current).ok()?,
        ))
    }

    /// Caller authorization is checked at the Mask route. Source checks the
    /// revision again. Refusal keeps the owner's current document byte-for-byte.
    pub(crate) fn edit_notebook(&mut self, root: &Path, edit: TextEdit<'_>) -> Result<(), String> {
        self.ensure_execution_retained(root)?;
        let current = self
            .notebook
            .as_ref()
            .ok_or("notebook is not open")?
            .current
            .clone();
        let mut proposal = vec![0; MAXIMUM_TEXT_EDIT_BYTES as usize];
        let length = edit.encode_into(&mut proposal).map_err(debug)?;
        proposal.truncate(length);
        self.plan_notebook_edit()?;
        self.persist(root)?;
        state::require_execution_capacity(root)?;
        let proposed = self
            .session
            .realization()
            .ok_or("notebook Plan required")?
            .clone();
        let authority = self.host.advertisement().clone();
        let inputs = [
            ExternalForeInput {
                front_port_id: port_id("document"),
                track: ConnectionTrack::Payload,
                bytes: current,
            },
            ExternalForeInput {
                front_port_id: port_id("edit"),
                track: ConnectionTrack::Payload,
                bytes: proposal,
            },
        ];
        let control = RunControl::default();
        let session = &mut self.session;
        let admissions = self.admissions.as_ref();
        let host = &mut self.host;
        let (report, candidate) = std::thread::scope(|scope| {
            std::thread::Builder::new()
                .name("conduit-notebook-edit".into())
                .stack_size(4 * 1024 * 1024)
                .spawn_scoped(scope, || {
                    let mut candidate =
                        CurrentOutput(Vec::with_capacity(MAXIMUM_TEXT_DOCUMENT_BYTES as usize));
                    let mut output = BoundedOutput(Vec::with_capacity(32 * 1024));
                    let mut timer = DeadlineTimer {
                        until: Instant::now() + Duration::from_secs(1),
                        control: control.clone(),
                    };
                    let report = host.run_body_plan_with_fore_to_with_start(
                        BodyRunRequest {
                            wake: &proposed.wake,
                            plan: &proposed.plan,
                            control: &control,
                            keyboard: None,
                        },
                        &inputs,
                        false,
                        &mut candidate,
                        &mut output,
                        &mut timer,
                        |play, wake| {
                            let mut next = session.clone();
                            next.started(
                                &authority.host_id,
                                &authority.boot_id,
                                play.clone(),
                                wake.clone(),
                            )
                            .map_err(debug)?;
                            state::retain_session(root, &mut next, None, admissions, None, None)?;
                            *session = next;
                            Ok(())
                        },
                    );
                    (report, candidate)
                })
                .map_err(|error| format!("start notebook worker: {error}"))?
                .join()
                .map_err(|_| "notebook worker panicked".to_string())
        })?;
        let report = report?;
        let accepted = report.terminal == TerminalDisposition::Completed
            && report.failure.is_none()
            && report.cleanup_failure.is_none()
            && report.fore_deliveries.len() == 1
            && !candidate.0.is_empty()
            && report.fore_deliveries[0].bytes.as_slice() == candidate.0.as_slice()
            && report.terminal_sign.active_play_id == Some(report.play.active_play_id.clone());
        let mut next = self.session.clone();
        if next
            .realization()
            .and_then(|realization| realization.play.as_ref())
            != Some(&report.play)
        {
            return Err("notebook report differs from retained started Play".into());
        }
        next.lull(&authority.host_id, &authority.boot_id, Some(&report.play))
            .map_err(debug)?;
        let receipt = serde_json::json!({
            "schema":"conduit.notebook/edit-receipt@1", "body_id":proposed.wake.body_id,
            "host_id":authority.host_id, "boot_id":authority.boot_id,
            "plan_id":proposed.plan.plan_id, "wake_id":proposed.wake.wake_id,
            "play":report.play, "terminal":report.terminal, "failure":report.failure,
            "cleanup_failure":report.cleanup_failure, "terminal_sign":report.terminal_sign,
            "document_adopted":accepted,
        });
        // Keep the completed Source output even if biography persistence fails:
        // a failed metadata write never licenses replay of an acknowledged edit.
        if accepted {
            self.notebook
                .as_mut()
                .ok_or("notebook disappeared")?
                .current = candidate.0;
        }
        self.retain_execution(root, next, receipt)?;
        if accepted {
            Ok(())
        } else {
            Err("notebook edit was refused; current document preserved".into())
        }
    }

    fn plan_notebook_edit(&mut self) -> Result<(), String> {
        if self.host.is_playing() || self.session.realization().is_some() {
            return Err("notebook edit requires an idle owner".into());
        }
        let source = crate::plot_source::parse(SOURCE)?;
        let plot = source.expand_entry_for_authoring()?;
        let resident = ResidentPlot::new(
            plot.expanded.source_document_id.clone(),
            plot.expanded.checked_plot_id.clone(),
        );
        if self.resident.as_ref() != Some(&resident) {
            return Err("notebook Source differs from resident".into());
        }
        let hosts = [self.host.advertisement().clone()];
        let placements =
            conduit_planner::default_expanded_placements(&plot.expanded, &hosts).map_err(debug)?;
        let boundaries = [
            (PortDirection::Input, "document", 264),
            (PortDirection::Input, "edit", 272),
            (PortDirection::Output, "current", 264),
        ]
        .into_iter()
        .map(|(direction, name, byte_capacity)| {
            (
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: port_id(name),
                    track: ConnectionTrack::Payload,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity,
                },
            )
        })
        .collect();
        let plan = conduit_planner::plan_expanded_authoring_with_activations(
            &source.check()?,
            &plot,
            source.authoring_catalog(),
            &conduit_plot::CanonicalBackCatalog::new(),
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 272,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &boundaries,
        )
        .map_err(debug)?;
        self.session
            .propose(
                vec![BodyPlotPlan {
                    plot: resident,
                    plan,
                }],
                &hosts[0].host_id,
                &hosts[0].boot_id,
            )
            .map_err(debug)?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "notebook_tests.rs"]
mod tests;
