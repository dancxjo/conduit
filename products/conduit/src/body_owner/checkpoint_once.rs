//! One selected checkpoint publication under the Owner's retained Body.
//! Current state and authority are explicit caller-provided inputs; this is
//! not a Mask action entrance or a next-Play restore path.

use super::{debug, state, BoundedOutput, DeadlineTimer, Owner};
use conduit_body::{BodyPlotPlan, ResidentPlot};
use conduit_core::{
    port_id, AuthorityGrant, BaseImplementationId, ConnectionTrack, PortDirection,
    TerminalDisposition,
};
use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions};
use conduit_std_host::body_execution::{
    BodyForeExchange, BodyForeOutputAdapter, BodyRunRequest, TodoCheckpointSelection,
};
use conduit_std_host::todo_durable_resource::CheckpointIdentity;
use conduit_std_host::{ExternalForeDelivery, ExternalForeInput, RunControl, RunControlRequestId};
use conduit_todo_plot::{TodoCommand, TodoState, COMMAND_MAX_BYTES, STATE_MAX_BYTES};
use std::{
    collections::BTreeMap,
    path::Path,
    time::{Duration, Instant},
};

struct OneCommittedFore(u8);

impl BodyForeOutputAdapter for OneCommittedFore {
    fn deliver(&mut self, output: &ExternalForeDelivery) -> Result<(), String> {
        if self.0 != 0
            || output.front_port_id != port_id("committed")
            || output.track != ConnectionTrack::Payload
            || output.bytes.len() > STATE_MAX_BYTES
        {
            return Err("Todo checkpoint emitted a different or excess Fore output".into());
        }
        self.0 = 1;
        Ok(())
    }
}

impl Owner {
    /// Select the exact checkpoint Back, resource version, and caller-owned
    /// authority grant before publishing a Body proposal.
    pub(super) fn plan_checkpoint_once(
        &mut self,
        source: &crate::plot_source::CanonicalSource,
        plot: &conduit_plot::ExpandedAuthoringPlot,
        grant: &AuthorityGrant,
    ) -> Result<(), String> {
        if plot.expanded.name != "todo/checkpoint-once"
            || !plot.expanded.activations.is_empty()
            || self.session.realization().is_some()
        {
            return Err("checkpoint-once requires its exact idle authored Plot".into());
        }
        let resident = ResidentPlot::new(
            plot.expanded.source_document_id.clone(),
            plot.expanded.checked_plot_id.clone(),
        );
        if self.resident.as_ref() != Some(&resident) {
            return Err("checkpoint-once source differs from the Body resident Plot".into());
        }
        let advertised = self.host.advertisement().clone();
        if grant.host_id != advertised.host_id || grant.boot_id != advertised.boot_id {
            return Err("checkpoint-once grant belongs to another Host Boot".into());
        }
        let hosts = [advertised.clone()];
        let placements =
            conduit_planner::default_expanded_placements(&plot.expanded, &hosts).map_err(debug)?;
        let boundary = |direction, name, byte_capacity| {
            (
                ForeBoundaryKey {
                    direction,
                    front_port_id: port_id(name),
                    track: ConnectionTrack::Payload,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity,
                },
            )
        };
        let boundaries = BTreeMap::from([
            boundary(PortDirection::Input, "current", STATE_MAX_BYTES as u32),
            boundary(PortDirection::Input, "command", COMMAND_MAX_BYTES as u32),
            boundary(PortDirection::Output, "committed", STATE_MAX_BYTES as u32),
        ]);
        let plan = conduit_planner::plan_expanded_authoring_with_activations(
            &source.check()?,
            plot,
            source.authoring_catalog(),
            &conduit_plot::CanonicalBackCatalog::new(),
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: STATE_MAX_BYTES as u32,
                authority_grants: std::slice::from_ref(grant),
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
                &advertised.host_id,
                &advertised.boot_id,
            )
            .map_err(debug)?;
        Ok(())
    }

    /// Execute only the first explicitly supplied revision-zero command.
    /// Checkpoint publication precedes its output Fore; Owner retains the
    /// kernel's started Play before that Host Call may run.
    pub(super) fn execute_checkpoint_once(
        &mut self,
        state_root: &Path,
        checkpoint_root: &Path,
        identity: CheckpointIdentity,
        current: &TodoState,
        command: &TodoCommand,
        maximum_millis: u64,
    ) -> Result<TodoState, String> {
        if !(1..=60_000).contains(&maximum_millis) {
            return Err("checkpoint run duration must be 1..60000 milliseconds".into());
        }
        if current.revision != 0 || !current.items.is_empty() || self.last_execution.is_some() {
            return Err("checkpoint-once Owner admits only its first caller-provided state".into());
        }
        if identity.body != self.session.evidence().body_id.as_str()
            || self
                .resident
                .as_ref()
                .is_none_or(|resident| identity.plot != resident.checked_plot_id.as_str())
        {
            return Err("checkpoint namespace differs from the retained Body Plot".into());
        }
        let proposed = self
            .session
            .realization()
            .ok_or("checkpoint Plan required")?
            .clone();
        if proposed.plan.plots.len() != 1
            || proposed.plan.plots[0].plot
                != *self.resident.as_ref().ok_or("resident Plot required")?
            || proposed.plan.plots[0].plan.fragments.len() != 1
            || proposed.plan.plots[0].plan.fragments[0].placements.len() != 2
        {
            return Err("checkpoint-once requires one exact selected local Plan".into());
        }
        let current_bytes = current.encode_info().map_err(debug)?;
        let command_bytes = command.encode_info().map_err(debug)?;
        let inputs = [
            ExternalForeInput {
                front_port_id: port_id("current"),
                track: ConnectionTrack::Payload,
                bytes: current_bytes,
            },
            ExternalForeInput {
                front_port_id: port_id("command"),
                track: ConnectionTrack::Payload,
                bytes: command_bytes,
            },
        ];
        self.persist(state_root)?;
        let authority = self.host.advertisement().clone();
        let control = RunControl::default();
        let session = &mut self.session;
        let admissions = self.admissions.as_ref();
        let host = &mut self.host;
        let start_authority = authority.clone();
        let worker_proposed = proposed.clone();
        let result = std::thread::scope(|scope| {
            let (finished, receiver) = std::sync::mpsc::sync_channel::<()>(1);
            let stop = control.clone();
            scope.spawn(move || {
                if receiver
                    .recv_timeout(Duration::from_millis(maximum_millis))
                    .is_err()
                {
                    let _ = stop.request_stop(
                        RunControlRequestId::new("owner/checkpoint-deadline")
                            .expect("bounded request"),
                    );
                }
            });
            let worker = std::thread::Builder::new()
                .name("conduit-todo-checkpoint-play".into())
                .stack_size(4 * 1024 * 1024)
                .spawn_scoped(scope, move || {
                    let mut fore = OneCommittedFore(0);
                    let mut output = BoundedOutput(Vec::with_capacity(32 * 1024));
                    let mut timer = DeadlineTimer {
                        until: Instant::now() + Duration::from_millis(maximum_millis),
                        control: control.clone(),
                    };
                    let result = host.run_body_plan_with_todo_checkpoint_to_with_start(
                        BodyRunRequest {
                            wake: &worker_proposed.wake,
                            plan: &worker_proposed.plan,
                            control: &control,
                            keyboard: None,
                        },
                        BodyForeExchange {
                            inputs: &inputs,
                            output: &mut fore,
                        },
                        TodoCheckpointSelection {
                            root: checkpoint_root,
                            identity,
                        },
                        &mut output,
                        &mut timer,
                        |play, wake| {
                            let mut next = session.clone();
                            next.started(
                                &start_authority.host_id,
                                &start_authority.boot_id,
                                play.clone(),
                                wake.clone(),
                            )
                            .map_err(debug)?;
                            state::retain_session(
                                state_root, &mut next, None, admissions, None, None,
                            )?;
                            *session = next;
                            Ok(())
                        },
                    );
                    (result, fore)
                })
                .map_err(|error| format!("start checkpoint Body worker: {error}"))?;
            let result = worker
                .join()
                .map_err(|_| "checkpoint Body worker panicked".to_string())?;
            let _ = finished.send(());
            Ok::<_, String>(result)
        })?;
        let (result, fore) = result;
        let report = result?;
        let delivered = if report.terminal == TerminalDisposition::Completed
            && report.failure.is_none()
            && report.cleanup_failure.is_none()
            && fore.0 == 1
            && report.fore_deliveries.len() == 1
            && report.terminal_sign.active_play_id == Some(report.play.active_play_id.clone())
        {
            TodoState::decode_info(&report.fore_deliveries[0].bytes).map_err(debug)
        } else {
            Err("checkpoint Play did not produce one committed state".into())
        };
        let mut next = self.session.clone();
        if next
            .realization()
            .and_then(|realization| realization.play.as_ref())
            != Some(&report.play)
        {
            return Err("checkpoint report differs from retained started Play".into());
        }
        next.lull(&authority.host_id, &authority.boot_id, Some(&report.play))
            .map_err(debug)?;
        let receipt = serde_json::json!({
            "host_id":authority.host_id, "boot_id":authority.boot_id,
            "body_id":proposed.wake.body_id, "plan_id":proposed.plan.plan_id,
            "wake_id":proposed.wake.wake_id, "play":report.play,
            "terminal":report.terminal, "failure":report.failure,
            "cleanup_failure":report.cleanup_failure, "terminal_sign":report.terminal_sign,
            "committed_fore_count":report.fore_deliveries.len(),
            "committed_fore_sha256":report.fore_deliveries.first().map(|fore| super::super::super::digest(&fore.bytes)),
        });
        state::retain_session(
            state_root,
            &mut next,
            Some(&receipt),
            self.admissions.as_ref(),
            None,
            None,
        )?;
        self.session = next;
        self.last_execution = Some(receipt);
        delivered
    }
}

#[cfg(test)]
#[path = "checkpoint_once_tests.rs"]
mod tests;
