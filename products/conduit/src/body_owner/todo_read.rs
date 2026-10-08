//! Re-encounter one published Todo generation through the retained Body.
//! The read Plot replaces the retired write Plot, and the selected Host Call
//! must return the exact committed state before any lulled Face may use it.

use super::{debug, state, BoundedOutput, DeadlineTimer, Owner};
use conduit_body::{BodyPlotPlan, BodyState, ResidentPlot};
use conduit_core::{
    port_id, ActivePlayId, AuthorityGrant, BaseImplementationId, ConnectionTrack, PlanId,
    PortDirection, ResourceAccessMode, ResourceContentRequirement, SignId, TerminalDisposition,
};
use conduit_kernel::KernelEventKind;
use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions};
use conduit_presentation::{
    CommittedStateContributionBasis, CommittedStateOperation, CommittedStateSelection,
};
use conduit_std_host::body_execution::{
    BodyForeOutputAdapter, BodyRunRequest, TodoCheckpointSelection,
};
use conduit_std_host::todo_durable_resource::{CheckpointIdentity, MissingV2Disposition};
use conduit_std_host::{ExternalForeDelivery, RunControl, RunControlRequestId};
use conduit_todo_plot::{TodoState, STATE_MAX_BYTES};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::mpsc,
    time::{Duration, Instant},
};

const READ_SOURCE: &str = include_str!("../../../../plots/todo/checkpoint-restore.conduit");
const WRITE_SOURCE: &str = include_str!("../../../../plots/todo/checkpoint-once.conduit");
const BODY_PLAY_STACK_BYTES: usize = 4 * 1024 * 1024;

struct OneRestoredFore(u8);

impl BodyForeOutputAdapter for OneRestoredFore {
    fn deliver(&mut self, output: &ExternalForeDelivery) -> Result<(), String> {
        if self.0 != 0
            || output.front_port_id != port_id("restored")
            || output.track != ConnectionTrack::Payload
            || output.bytes.len() > STATE_MAX_BYTES
        {
            return Err("Todo read Play emitted an unexpected Fore".into());
        }
        self.0 = 1;
        Ok(())
    }
}

impl Owner {
    pub(crate) fn todo_verified_read_receipt(&self) -> Option<&serde_json::Value> {
        self.last_execution.as_ref().filter(|receipt| {
            receipt["schema"] == "conduit.todo/verified-read-receipt@1"
                && receipt["verified"] == true
        })
    }

    /// Verify a just-published selected generation with a second Plan, Play,
    /// Host Call, Fore, and terminal Sign on this same Body and Host Boot.
    pub(crate) fn read_committed_todo(
        &mut self,
        state_root: &Path,
        checkpoint_root: &Path,
        selected_write: &ResourceContentRequirement,
        committed: &TodoState,
        maximum_millis: u64,
    ) -> Result<TodoState, String> {
        let write_receipt = self
            .todo_commit_receipt()
            .ok_or("Todo read has no retained write receipt")?
            .clone();
        self.read_selected_todo(
            state_root,
            checkpoint_root,
            selected_write,
            Some(committed),
            write_receipt,
            maximum_millis,
        )
    }

    pub(super) fn read_selected_todo(
        &mut self,
        state_root: &Path,
        checkpoint_root: &Path,
        selected_write: &ResourceContentRequirement,
        committed: Option<&TodoState>,
        write_receipt: serde_json::Value,
        maximum_millis: u64,
    ) -> Result<TodoState, String> {
        self.todo_verified = None;
        let fresh_boot = committed.is_none();
        if !(1..=60_000).contains(&maximum_millis)
            || self.host.is_playing()
            || self.session.realization().is_some()
            || self.session.evidence().body.state != BodyState::Lulled
            || self.resident_name.as_deref()
                != Some(if fresh_boot {
                    "todo/checkpoint-restore"
                } else {
                    "todo/checkpoint-once"
                })
            || selected_write.access != ResourceAccessMode::WriteCandidatePublish
        {
            return Err("Todo read requires one retired selected write Play".into());
        }
        let expected_digest = write_receipt["committed_fore_sha256"]
            .as_str()
            .ok_or("Todo write receipt has no committed Fore digest")?
            .to_owned();
        let write_plot_id = crate::plot_source::parse(WRITE_SOURCE)?
            .expand_entry_for_authoring()?
            .expanded
            .checked_plot_id;
        if let Some(committed) = committed {
            let expected_bytes = committed.encode_info().map_err(debug)?;
            if super::super::super::digest(&expected_bytes) != expected_digest {
                return Err("Todo write receipt differs from committed state".into());
            }
        }
        if write_receipt["terminal"] != serde_json::json!(TerminalDisposition::Completed)
            || write_receipt["failure"] != serde_json::Value::Null
            || write_receipt["cleanup_failure"] != serde_json::Value::Null
            || write_receipt["committed_fore_sha256"] != expected_digest
            || !write_receipt["terminal_sign"]["sign_id"].is_string()
            || write_receipt["terminal_sign"]["active_play_id"]
                != write_receipt["play"]["active_play_id"]
            || write_receipt["selected_content"] != serde_json::json!(selected_write)
            || write_receipt["checkpoint_namespace"]["body_id"]
                != self.session.evidence().body_id.as_str()
            || write_receipt["checkpoint_namespace"]["write_plot_id"] != write_plot_id.as_str()
            || (!fresh_boot
                && self.resident.as_ref().map(|plot| &plot.checked_plot_id) != Some(&write_plot_id))
        {
            return Err("Todo read differs from the exact committed write".into());
        }
        let list_key = write_receipt["checkpoint_namespace"]["list_key"]
            .as_str()
            .ok_or("Todo write receipt has no list key")?
            .to_owned();
        let source = crate::plot_source::parse(READ_SOURCE)?;
        let plot = source.expand_entry_for_authoring()?;
        if plot.expanded.name != "todo/checkpoint-restore" {
            return Err("Todo read source differs from checked restore Plot".into());
        }
        let old_resident = self
            .resident
            .as_ref()
            .ok_or("Todo has no resident Plot")?
            .clone();
        let read_resident = ResidentPlot::new(
            plot.expanded.source_document_id.clone(),
            plot.expanded.checked_plot_id.clone(),
        );
        let mut read_content = selected_write.clone();
        read_content.access = ResourceAccessMode::ReadPublished;
        read_content.publication_slots = 0;
        self.host
            .transition_todo_checkpoint_offer(checkpoint_root, read_content.clone())?;
        let prepared = (|| {
            let advertised = self.host.advertisement().clone();
            let offer = advertised
                .capabilities
                .iter()
                .find(|offer| {
                    offer.implementation.implementation_id.as_str()
                        == conduit_std_offers::TODO_CHECKPOINT_READ_IMPLEMENTATION
                })
                .ok_or("selected Host has no Todo read offer")?;
            if offer.authority_requirements.len() != 1 {
                return Err("Todo read offer has unexpected authority".into());
            }
            let requirement = &offer.authority_requirements[0];
            let grant = AuthorityGrant {
                grant_id: format!(
                    "grant/todo/{}/read",
                    self.session.evidence().body_id.as_str()
                )
                .into(),
                contract_id: requirement.contract_id.clone(),
                host_call_contract_id: requirement.host_call_contract_id.clone(),
                subject_kind: requirement.subject_kind.clone(),
                host_id: advertised.host_id.clone(),
                boot_id: advertised.boot_id.clone(),
                capability_id: offer.capability_id.clone(),
            };
            let hosts = [advertised.clone()];
            let placements = conduit_planner::default_expanded_placements(&plot.expanded, &hosts)
                .map_err(debug)?;
            let boundaries = BTreeMap::from([(
                ForeBoundaryKey {
                    direction: PortDirection::Output,
                    front_port_id: port_id("restored"),
                    track: ConnectionTrack::Payload,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: STATE_MAX_BYTES as u32,
                },
            )]);
            let plan = conduit_planner::plan_expanded_authoring_with_activations(
                &source.check()?,
                &plot,
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
                    authority_grants: std::slice::from_ref(&grant),
                    protected_resource_grants: &[],
                    line_offers: &[],
                },
                &boundaries,
            )
            .map_err(debug)?;
            let mut staged = self.session.clone();
            let revision = staged.evidence().body.workload_revision;
            if fresh_boot {
                if old_resident != read_resident {
                    return Err("Todo re-encounter resident Plot changed".into());
                }
            } else {
                staged
                    .remove_plot(
                        revision,
                        &old_resident,
                        &advertised.host_id,
                        &advertised.boot_id,
                    )
                    .map_err(debug)?;
                staged
                    .admit_plot(
                        revision
                            .checked_add(1)
                            .ok_or("Todo workload revision exhausted")?,
                        read_resident.clone(),
                        &advertised.host_id,
                        &advertised.boot_id,
                    )
                    .map_err(debug)?;
            }
            staged
                .propose(
                    vec![BodyPlotPlan {
                        plot: read_resident.clone(),
                        plan,
                    }],
                    &advertised.host_id,
                    &advertised.boot_id,
                )
                .map_err(debug)?;
            Ok::<_, String>(staged)
        })();
        let staged = match prepared {
            Ok(staged) => staged,
            Err(error) => {
                self.host.transition_todo_checkpoint_offer(checkpoint_root, selected_write.clone())
                    .map_err(|rollback| format!("Todo read preparation failed: {error}; Host rollback failed: {rollback}"))?;
                return Err(error);
            }
        };
        if let Err(error) = state::retain_with_source(
            state_root,
            staged.evidence(),
            Some(&write_receipt),
            self.admissions.as_ref(),
            if fresh_boot {
                None
            } else {
                Some(READ_SOURCE.as_bytes())
            },
        ) {
            self.host
                .transition_todo_checkpoint_offer(checkpoint_root, selected_write.clone())
                .map_err(|rollback| {
                    format!("Todo read retention failed: {error}; Host rollback failed: {rollback}")
                })?;
            return Err(error);
        }
        self.session = staged;
        self.resident = Some(read_resident.clone());
        self.resident_name = Some(plot.expanded.name);
        let proposed = self
            .session
            .realization()
            .ok_or("Todo read Plan vanished")?
            .clone();
        let authority = self.host.advertisement().clone();
        let identity = CheckpointIdentity {
            body: proposed.wake.body_id.as_str().to_owned(),
            plot: read_resident.checked_plot_id.as_str().to_owned(),
            workload: list_key,
            missing_v2: MissingV2Disposition::Refuse,
        };
        let control = RunControl::default();
        let session = &mut self.session;
        let admissions = self.admissions.as_ref();
        let host = self.host.current_mut();
        let read_authority = &authority;
        let read_proposed = &proposed;
        let retained_write = &write_receipt;
        let (report, fore) = std::thread::scope(|scope| {
            let (finished, receiver) = mpsc::sync_channel::<()>(1);
            let stop = control.clone();
            scope.spawn(move || {
                if receiver
                    .recv_timeout(Duration::from_millis(maximum_millis))
                    .is_err()
                {
                    let _ = stop.request_stop(
                        RunControlRequestId::new("owner/todo-read-deadline")
                            .expect("bounded request"),
                    );
                }
            });
            let worker = std::thread::Builder::new()
                .name("conduit-todo-read-play".into())
                .stack_size(BODY_PLAY_STACK_BYTES)
                .spawn_scoped(scope, move || {
                    let mut fore = OneRestoredFore(0);
                    let mut output = BoundedOutput(Vec::with_capacity(32 * 1024));
                    let mut timer = DeadlineTimer {
                        until: Instant::now() + Duration::from_millis(maximum_millis),
                        control: control.clone(),
                    };
                    let result = host.run_body_plan_with_todo_checkpoint_read_to_with_start(
                        BodyRunRequest {
                            wake: &read_proposed.wake,
                            plan: &read_proposed.plan,
                            control: &control,
                            keyboard: None,
                        },
                        &mut fore,
                        TodoCheckpointSelection {
                            root: checkpoint_root,
                            identity,
                        },
                        &mut output,
                        &mut timer,
                        |play, wake| {
                            let mut next = session.clone();
                            next.started(
                                &read_authority.host_id,
                                &read_authority.boot_id,
                                play.clone(),
                                wake.clone(),
                            )
                            .map_err(debug)?;
                            state::retain(
                                state_root,
                                next.evidence(),
                                Some(retained_write),
                                admissions,
                            )?;
                            *session = next;
                            Ok(())
                        },
                    );
                    (result, fore)
                })
                .map_err(|error| format!("start Todo read worker: {error}"))?;
            let result = worker
                .join()
                .map_err(|_| "Todo read worker panicked".to_string())?;
            let _ = finished.send(());
            Ok::<_, String>(result)
        })?;
        let report = report?;
        let verified = (|| {
            let call = report
                .requests
                .first()
                .ok_or("Todo read made no Host Call")?;
            if report.terminal != TerminalDisposition::Completed
                || report.failure.is_some()
                || report.cleanup_failure.is_some()
                || fore.0 != 1
                || report.fore_deliveries.len() != 1
                || report.requests.len() != 1
                || report.terminal_sign.active_play_id != Some(report.play.active_play_id.clone())
                || ![
                    KernelEventKind::HostCallRequested,
                    KernelEventKind::HostCallCompleted,
                ]
                .into_iter()
                .all(|kind| {
                    report
                        .kernel_events
                        .iter()
                        .filter(|event| {
                            event.kind == kind
                                && event.node == call.node
                                && event.request == Some(call.request)
                        })
                        .count()
                        == 1
                })
            {
                return Err("Todo read Play did not verify one selected generation".into());
            }
            let restored_bytes = &report.fore_deliveries[0].bytes;
            let restored = TodoState::decode_info(restored_bytes).map_err(debug)?;
            if super::super::super::digest(restored_bytes) != expected_digest
                || committed.is_some_and(|expected| restored != *expected)
            {
                return Err("Todo read differs from the committed generation".into());
            }
            let write_plan = write_receipt["plan_id"]
                .as_str()
                .ok_or("Todo write receipt has no Plan ID")?;
            let write_play = write_receipt["play"]["active_play_id"]
                .as_str()
                .ok_or("Todo write receipt has no Play ID")?;
            let write_sign = write_receipt["terminal_sign"]["sign_id"]
                .as_str()
                .ok_or("Todo write receipt has no terminal Sign ID")?;
            let mut state_digest = [0; 32];
            state_digest.copy_from_slice(&Sha256::digest(restored_bytes));
            let basis = CommittedStateContributionBasis {
                body_id: proposed.wake.body_id.clone(),
                checked_plot_id: read_resident.checked_plot_id.clone(),
                selection: CommittedStateSelection {
                    resource: read_content.identity,
                    selected_version: read_content.version,
                    published_version: read_content.version,
                },
                write: CommittedStateOperation {
                    plan_id: PlanId::from(write_plan),
                    play_id: ActivePlayId::from(write_play),
                    terminal_sign_id: SignId::from(write_sign),
                },
                read: CommittedStateOperation {
                    plan_id: proposed.plan.plan_id.clone(),
                    play_id: report.play.active_play_id.clone(),
                    terminal_sign_id: report.terminal_sign.sign_id.clone(),
                },
                state_digest,
            };
            Ok((restored, basis))
        })();
        let mut next = self.session.clone();
        next.lull(&authority.host_id, &authority.boot_id, Some(&report.play))
            .map_err(debug)?;
        let verified = verified.and_then(|(restored, basis)| {
            basis
                .validate_shape_against(&next.evidence().body, &basis.selection)
                .map_err(debug)?;
            Ok((restored, basis))
        });
        let receipt = serde_json::json!({
            "schema":"conduit.todo/verified-read-receipt@1",
            "verified":verified.is_ok(),
            "refusal":verified.as_ref().err(),
            "body_id":proposed.wake.body_id,
            "write":write_receipt,
            "read_plan_id":proposed.plan.plan_id,
            "read_play":report.play,
            "read_terminal":report.terminal,
            "read_failure":report.failure,
            "read_cleanup_failure":report.cleanup_failure,
            "read_terminal_sign":report.terminal_sign,
            "read_host_call":report.requests.first().map(|call| serde_json::json!({
                "node":call.node.0,"request":call.request.0,"call":call.call.0
            })),
            "selected_content":read_content,
            "restored_fore_sha256":report.fore_deliveries.first().map(|fore| super::super::super::digest(&fore.bytes)),
        });
        state::retain(
            state_root,
            next.evidence(),
            Some(&receipt),
            self.admissions.as_ref(),
        )?;
        self.session = next;
        self.last_execution = Some(receipt);
        verified.map(|(restored, basis)| {
            self.todo_verified = Some((basis, restored.clone()));
            restored
        })
    }
}
