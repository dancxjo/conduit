//! Body lifecycle truth; all execution identities come from the installed kernel.
use super::state;
#[path = "participants/invited.rs"]
mod invited;
#[path = "participants/mod.rs"]
mod participants;
use conduit_body::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyLifecycleSession,
    BodyMembership, BodyPlotPlan, BodyWorkset, MembershipProofId, PartId, ResidentPlot,
};
use conduit_core::HostAdvertisement;
use conduit_core::{bind_sign, port_id, BaseImplementationId, ConnectionTrack, PortDirection};
use conduit_presentation::{OwnerFaceSnapshotRequest, Presentation};
use conduit_std_host::body_execution::BodyRunRequest;
use conduit_std_host::{RunControl, RunControlRequestId, StdHost, ThreadTimer, TimerAdapter};
#[cfg(unix)]
pub(crate) use participants::run_service_window;
pub(crate) use participants::{
    BrowserAdmittedSnapshot, BrowserCarrierLineEvidence, BrowserWindowAuthorization,
};
use std::{
    collections::BTreeMap,
    io::Write,
    ops::{Deref, DerefMut},
    path::Path,
    time::{Duration, Instant},
};

// The installed request entrance remains gated on admitted restore and Face
// action routing; this exact first-action path is exercised by focused proof.
#[allow(dead_code)]
#[path = "checkpoint_once.rs"]
mod checkpoint_once;
#[cfg(test)]
pub(crate) use checkpoint_once::tests::published_fixture as published_todo_test_fixture;
#[path = "continuing.rs"]
mod continuing;
pub(crate) use continuing::RunWorker;
mod birth;
#[path = "clock_interval.rs"]
mod clock_interval;
#[path = "direct_spoken_route.rs"]
mod direct_spoken_route;
pub(crate) use direct_spoken_route::DirectSpokenStart;
#[path = "llm_spoken_route.rs"]
mod llm_spoken_route;
pub(crate) use llm_spoken_route::LlmSpokenStart;
#[path = "native_mask_route.rs"]
mod native_mask_route;
#[path = "presentation_wardrobe.rs"]
mod presentation_wardrobe;
#[path = "presentation_wardrobe_report.rs"]
mod presentation_wardrobe_report;
#[path = "presentation_wardrobe_runtime.rs"]
mod presentation_wardrobe_runtime;
#[path = "presentation_wardrobe_witness.rs"]
mod presentation_wardrobe_witness;
#[path = "wardrobe_face.rs"]
mod wardrobe_face;
pub(crate) use clock_interval::{is_clock_control_intent, ClockAction, CLOCK_RUN_MAXIMUM_MILLIS};
#[cfg(unix)]
#[path = "terminal_route.rs"]
mod terminal_route;
#[path = "todo_face.rs"]
mod todo_face;
#[path = "todo_next.rs"]
mod todo_next;
#[path = "todo_read.rs"]
mod todo_read;
#[path = "todo_read_failure.rs"]
mod todo_read_failure;
#[path = "todo_reencounter.rs"]
mod todo_reencounter;
#[path = "todo_waiting.rs"]
#[allow(dead_code)] // Waiting Play enters the installed service after Host selection lands.
mod todo_waiting;
#[allow(unused_imports)] // The installed service return wires this worker next.
pub(crate) use todo_waiting::TodoWaitingWorker;
pub(crate) fn clock_interval_action() -> &'static str {
    clock_interval::CLOCK_INTERVAL_ACTION
}

/// The owner keeps its Boot advertisement while its one Host executes the
/// admitted Body Play on a worker. No second Host is constructed.
pub(crate) struct OwnerHost {
    current: Option<StdHost>,
    advertised: HostAdvertisement,
}

impl OwnerHost {
    fn new(host: StdHost) -> Self {
        Self {
            advertised: host.advertisement().clone(),
            current: Some(host),
        }
    }

    pub(crate) fn advertisement(&self) -> &HostAdvertisement {
        self.current
            .as_ref()
            .map(StdHost::advertisement)
            .unwrap_or(&self.advertised)
    }

    pub(crate) fn take_for_play(&mut self) -> Result<StdHost, String> {
        let host = self
            .current
            .take()
            .ok_or("Body Play already owns the Host")?;
        self.advertised = host.advertisement().clone();
        Ok(host)
    }

    pub(crate) fn restore_after_play(&mut self, host: StdHost) -> Result<(), String> {
        if self.current.is_some() || host.advertisement() != &self.advertised {
            return Err(
                "Body Play returned a different Host advertisement or duplicate Host".into(),
            );
        }
        self.advertised = host.advertisement().clone();
        self.current = Some(host);
        Ok(())
    }

    pub(crate) fn transition_todo_checkpoint_offer(
        &mut self,
        root: &Path,
        content: conduit_core::ResourceContentRequirement,
    ) -> Result<(), String> {
        let host = self
            .current
            .as_mut()
            .ok_or("Todo checkpoint transition requires an idle Host")?;
        host.transition_todo_checkpoint_offer(root, content)?;
        self.advertised = host.advertisement().clone();
        Ok(())
    }

    pub(crate) fn current(&self) -> &StdHost {
        self.current
            .as_ref()
            .expect("Host effects require an idle owner")
    }

    pub(crate) fn current_mut(&mut self) -> &mut StdHost {
        self.current
            .as_mut()
            .expect("Host effects require an idle owner")
    }

    pub(crate) fn is_playing(&self) -> bool {
        self.current.is_none()
    }
}

impl Deref for OwnerHost {
    type Target = StdHost;

    fn deref(&self) -> &Self::Target {
        self.current()
    }
}

impl DerefMut for OwnerHost {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.current_mut()
    }
}

pub(crate) struct Owner {
    pub(crate) host: OwnerHost,
    session: BodyLifecycleSession,
    resident: Option<ResidentPlot>,
    resident_name: Option<String>,
    clock_interval_ms: Option<u64>,
    last_execution: Option<serde_json::Value>,
    admissions: Option<conduit_body::AdmissionManager>,
    pending_browser: Option<participants::BrowserWindow>,
    pending_native_mask: Option<native_mask_route::NativeMaskRoute>,
    attached_terminal_route: Option<conduit_presentation::LocalOwnerMaskRouteSeal>,
    direct_spoken_route: Option<conduit_presentation::LocalOwnerMaskRouteSeal>,
    llm_spoken_route: Option<conduit_presentation::LocalOwnerMaskRouteSeal>,
    presentation_wardrobe: Option<presentation_wardrobe::OwnerPresentationWardrobe>,
    /// Projection cache for the exact currently Playing Todo encounter. The
    /// next Play must restore through its admitted read Host Call.
    todo_live: Option<(conduit_core::ActivePlayId, conduit_todo_plot::TodoState)>,
    /// Bounded display cache from an exact selected read Host Call and both
    /// terminal Signs. It is never a reducer or a source of authority.
    todo_verified: Option<(
        conduit_presentation::CommittedStateContributionBasis,
        conduit_todo_plot::TodoState,
    )>,
}
impl Owner {
    pub(crate) fn selected_speech_host_is_idle(&self) -> bool {
        !self.host.is_playing() && self.session.realization().is_none()
    }

    pub(crate) fn open(
        host: StdHost,
        resident: ResidentPlot,
        retained: Option<BodyBiographyEvidence>,
        name: &str,
    ) -> Result<Self, String> {
        let advertised = host.advertisement();
        let session = if let Some(evidence) = retained {
            if !evidence.body.workset.plots().contains(&resident) {
                return Err("checked source is not in the retained Body workset".into());
            }
            BodyLifecycleSession::resume_here(evidence, &advertised.host_id, &advertised.boot_id)
                .map_err(debug)?
        } else {
            let mut entropy = [0; 8];
            getrandom::fill(&mut entropy).map_err(|e| e.to_string())?;
            let sequence = u64::from_le_bytes(entropy) >> 1;
            let sign = |offset| {
                bind_sign(
                    &advertised.host_id,
                    &advertised.boot_id,
                    None,
                    sequence + offset,
                )
                .sign_id
            };
            let body = Body::born_with_plots(
                BodyWorkset::one(resident.clone()).map_err(debug)?,
                sequence,
                sign(0),
            )
            .map_err(debug)?;
            let part =
                PartId::bind(&body.body_id, advertised.host_id.as_str(), 0).map_err(debug)?;
            let proof =
                MembershipProofId::bind("conduit/installed-host/local-birth").map_err(debug)?;
            let mut membership = BodyMembership::new(body.body_id.clone()).map_err(debug)?;
            let mut evidence =
                BodyBiographyEvidence::born(body.clone(), membership.clone(), name.into())
                    .map_err(debug)?;
            let admitted = membership
                .admit(
                    &body.body_id,
                    membership.revision,
                    part.clone(),
                    proof.clone(),
                    sign(1),
                )
                .map_err(debug)?;
            let present = membership
                .observe_present(
                    &body.body_id,
                    membership.revision,
                    &part,
                    AuthenticatedHostObservation {
                        host_id: advertised.host_id.clone(),
                        boot_id: advertised.boot_id.clone(),
                        offer_generation: advertised.offer_generation,
                        proof_id: proof,
                        sequence: 0,
                    },
                    sign(2),
                )
                .map_err(debug)?;
            evidence
                .append_membership_events(
                    membership,
                    &[(admitted, sequence + 1), (present, sequence + 2)],
                )
                .map_err(debug)?;
            BodyLifecycleSession::open(evidence).map_err(debug)?
        };
        Ok(Self {
            host: OwnerHost::new(host),
            session,
            resident: Some(resident),
            resident_name: None,
            clock_interval_ms: None,
            last_execution: None,
            admissions: None,
            pending_browser: None,
            pending_native_mask: None,
            attached_terminal_route: None,
            direct_spoken_route: None,
            llm_spoken_route: None,
            presentation_wardrobe: None,
            todo_live: None,
            todo_verified: None,
        })
    }
    /// Reattach a retained owner to the one fresh installed Host Boot.
    pub(super) fn resume(host: StdHost, retained: BodyBiographyEvidence) -> Result<Self, String> {
        let advertised = host.advertisement();
        let session =
            BodyLifecycleSession::resume_here(retained, &advertised.host_id, &advertised.boot_id)
                .map_err(debug)?;
        let resident = session.evidence().body.workset.plots().first().cloned();
        Ok(Self {
            host: OwnerHost::new(host),
            session,
            resident,
            resident_name: None,
            clock_interval_ms: None,
            last_execution: None,
            admissions: None,
            pending_browser: None,
            pending_native_mask: None,
            attached_terminal_route: None,
            direct_spoken_route: None,
            llm_spoken_route: None,
            presentation_wardrobe: None,
            todo_live: None,
            todo_verified: None,
        })
    }
    pub(crate) fn persist(&mut self, root: &Path) -> Result<(), String> {
        state::retain_session(
            root,
            &mut self.session,
            self.last_execution.as_ref(),
            self.admissions.as_ref(),
            None,
            None,
        )
    }
    /// A readable name may enter only with the checked source for the exact
    /// resident identity. It is rederived after Boot, never treated as a
    /// second retained authority.
    pub(crate) fn set_resident_plot_name(
        &mut self,
        checked: &conduit_plot::ExpandedAuthoringPlot,
    ) -> Result<(), String> {
        let resident = ResidentPlot::new(
            checked.expanded.source_document_id.clone(),
            checked.expanded.checked_plot_id.clone(),
        );
        if self.resident.as_ref() != Some(&resident) {
            return Err("checked source differs from the Body's resident Plot".into());
        }
        self.resident_name = Some(checked.expanded.name.clone());
        self.clock_interval_ms = clock_interval::recognized_interval(&resident);
        Ok(())
    }
    pub(super) fn restore_execution(&mut self, root: &Path) -> Result<(), String> {
        self.last_execution = state::execution(root)?;
        self.admissions = state::admissions(root, &self.session.evidence().body_id)?;
        Ok(())
    }
    pub(crate) fn truth(&self) -> serde_json::Value {
        serde_json::json!({"schema":"conduit.body/owner-truth@1", "host":self.host.advertisement(), "biography":self.session.evidence(), "realization":self.session.realization(), "last_execution":self.last_execution})
    }
    /// Project one canonical Face from the current retained owner session.
    /// Credential matching and current incarnation are checked here, even
    /// though the routed caller has already authenticated the same Line.
    pub(crate) fn face_snapshot(
        &self,
        request: &OwnerFaceSnapshotRequest,
    ) -> Result<Presentation, String> {
        let admitted = self.admissions.as_ref().is_some_and(|manager| {
            manager.receipts.iter().any(|receipt| {
                request.has_exact_basis()
                    && request.credential_id == receipt.credential.credential_id.as_str()
                    && request.body_id == receipt.credential.body_id
                    && request.part_id == receipt.credential.part_id
                    && request.host_id == receipt.credential.host_id
                    && request.boot_id == receipt.credential.boot_id
            })
        });
        if !admitted {
            return Err("owner-face-credential-not-admitted".into());
        }
        let present = self.session.evidence().membership.parts.iter().any(|part| {
            part.part_id == request.part_id
                && part.current.as_ref().is_some_and(|current| {
                    current.host_id == request.host_id && current.boot_id == request.boot_id
                })
        });
        if !present || self.session.evidence().body_id != request.body_id {
            return Err("owner-face-current-part-unavailable".into());
        }
        self.local_face_snapshot()
    }

    /// The installed owner's current Face. Only the authenticated local
    /// control service calls this; remote callers still need an exact admitted
    /// credential and current Part above.
    pub(crate) fn local_face_snapshot(&self) -> Result<Presentation, String> {
        match self.todo_live.as_ref() {
            Some((play, state)) if self.current_play_id() == Some(play) => {
                self.project_face(Some((state, true)))
            }
            Some(_) => Err("Todo Face projection cache differs from current Play".into()),
            None => match self.todo_verified.as_ref() {
                Some((basis, state)) => self.project_verified_todo_face(basis, state),
                None => self.project_face(None),
            },
        }
    }
    pub(super) fn plan(
        &mut self,
        plot: &conduit_plot::ExpandedAuthoringPlot,
    ) -> Result<(), String> {
        let resident = self.resident.as_ref().ok_or("Body has no resident Plot")?;
        let partition = self.plan_partition(plot, resident)?;
        let hosts = [self.host.advertisement().clone()];
        self.session
            .propose(vec![partition], &hosts[0].host_id, &hosts[0].boot_id)
            .map_err(debug)?;
        Ok(())
    }

    pub(super) fn plan_with_source(
        &mut self,
        source: &crate::plot_source::CanonicalSource,
        plot: &conduit_plot::ExpandedAuthoringPlot,
    ) -> Result<(), String> {
        let resident = self.resident.as_ref().ok_or("Body has no resident Plot")?;
        let partition =
            self.plan_partition_with_source(source, plot, resident, self.host.advertisement())?;
        let advertised = self.host.advertisement();
        self.session
            .propose(vec![partition], &advertised.host_id, &advertised.boot_id)
            .map_err(debug)?;
        Ok(())
    }

    fn plan_partition(
        &self,
        plot: &conduit_plot::ExpandedAuthoringPlot,
        resident: &ResidentPlot,
    ) -> Result<BodyPlotPlan, String> {
        let hosts = [self.host.advertisement().clone()];
        // Ordinary planning does not attach child Plans for scan/fold/make.
        // Refuse before sealing a deceptively runnable parent-only Body Plan.
        if !plot.expanded.activations.is_empty() {
            return Err("installed Body execution has no activation-aware Plan or Play".into());
        }
        let placements =
            conduit_planner::default_expanded_placements(&plot.expanded, &hosts).map_err(debug)?;
        let plan = conduit_planner::plan_expanded_authoring_with_options(
            plot,
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: conduit_core::DEFAULT_CONNECTION_ITEM_CAPACITY,
                connection_byte_capacity: conduit_core::DEFAULT_CONNECTION_BYTE_CAPACITY,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &BTreeMap::new(),
        )
        .map_err(debug)?;
        Ok(BodyPlotPlan {
            plot: resident.clone(),
            plan,
        })
    }

    /// Plan the exact retained source when its Host actually advertises an
    /// activation coordinator. This seam is deliberately separate from Play:
    /// the installed owner cannot yet route commands into an active scan.
    fn plan_partition_with_source(
        &self,
        source: &crate::plot_source::CanonicalSource,
        plot: &conduit_plot::ExpandedAuthoringPlot,
        resident: &ResidentPlot,
        advertisement: &HostAdvertisement,
    ) -> Result<BodyPlotPlan, String> {
        if plot.expanded.activations.is_empty() {
            return self.plan_partition(plot, resident);
        }
        if plot.expanded.name != "todo/main" || plot.expanded.activations.len() != 1 {
            return Err("installed Body supports no other activation source".into());
        }
        let expected = ResidentPlot::new(
            plot.expanded.source_document_id.clone(),
            plot.expanded.checked_plot_id.clone(),
        );
        if &expected != resident || self.resident.as_ref() != Some(resident) {
            return Err("checked source differs from the Body's resident Plot".into());
        }
        let document = source.check()?;
        let hosts = [advertisement.clone()];
        let placements =
            conduit_planner::default_expanded_placements(&plot.expanded, &hosts).map_err(debug)?;
        let queue_bytes = (2 * conduit_todo_plot::STATE_MAX_BYTES
            + 2 * conduit_todo_plot::COMMAND_MAX_BYTES) as u32;
        let boundaries = BTreeMap::from([
            (
                conduit_planner::ForeBoundaryKey {
                    direction: PortDirection::Input,
                    front_port_id: port_id("commands"),
                    track: ConnectionTrack::Payload,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: conduit_todo_plot::COMMAND_MAX_BYTES as u32,
                },
            ),
            (
                conduit_planner::ForeBoundaryKey {
                    direction: PortDirection::Output,
                    front_port_id: port_id("states"),
                    track: ConnectionTrack::Payload,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: conduit_todo_plot::STATE_MAX_BYTES as u32,
                },
            ),
        ]);
        let plan = conduit_planner::plan_expanded_authoring_with_activations(
            &document,
            plot,
            source.authoring_catalog(),
            &conduit_plot::CanonicalBackCatalog::new(),
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: queue_bytes,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &boundaries,
        )
        .map_err(debug)?;
        Ok(BodyPlotPlan {
            plot: resident.clone(),
            plan,
        })
    }
    pub(super) fn execute(&mut self, maximum_millis: u64) -> Result<(), String> {
        if !(1..=60_000).contains(&maximum_millis) {
            return Err("run duration must be 1..60000 milliseconds".into());
        }
        let proposed = self
            .session
            .realization()
            .ok_or("plan required before run")?
            .clone();
        if proposed
            .plan
            .plots
            .iter()
            .any(|plot| !plot.plan.activations.is_empty())
        {
            return Err("installed Body Play has no activation ingress or egress".into());
        }
        let control = RunControl::default();
        let mut timer = DeadlineTimer {
            until: Instant::now() + Duration::from_millis(maximum_millis),
            control: control.clone(),
        };
        let mut output = BoundedOutput(Vec::with_capacity(32 * 1024));
        let result = std::thread::scope(|scope| {
            let (finished, receiver) = std::sync::mpsc::sync_channel::<()>(1);
            let stop = control.clone();
            scope.spawn(move || {
                if receiver
                    .recv_timeout(Duration::from_millis(maximum_millis))
                    .is_err()
                {
                    let _ = stop.request_stop(
                        RunControlRequestId::new("owner/run-deadline").expect("bounded request"),
                    );
                }
            });
            let result = self.host.run_body_plan_to(
                BodyRunRequest {
                    wake: &proposed.wake,
                    plan: &proposed.plan,
                    control: &control,
                    keyboard: None,
                },
                &mut output,
                &mut timer,
            );
            let _ = finished.send(());
            result
        });
        match result {
            Ok(report) => {
                let host = self.host.advertisement();
                self.session
                    .started(
                        &host.host_id,
                        &host.boot_id,
                        report.play.clone(),
                        report.wake_at_start.clone(),
                    )
                    .map_err(debug)?;
                self.last_execution = Some(
                    serde_json::json!({"host_id":host.host_id, "boot_id":host.boot_id, "body_id":proposed.wake.body_id, "plan_id":proposed.plan.plan_id, "wake_id":proposed.wake.wake_id, "play":report.play, "wake_at_start":report.wake_at_start, "terminal":report.terminal, "failure":report.failure, "cleanup_failure":report.cleanup_failure, "terminal_sign":report.terminal_sign, "output_utf8":String::from_utf8_lossy(&output.0)}),
                );
                self.session
                    .lull(&host.host_id, &host.boot_id, Some(&report.play))
                    .map_err(debug)?;
                Ok(())
            }
            Err(error) => {
                self.last_execution = Some(
                    serde_json::json!({"host_id":self.host.advertisement().host_id, "boot_id":self.host.advertisement().boot_id, "body_id":proposed.wake.body_id, "plan_id":proposed.plan.plan_id, "wake_id":proposed.wake.wake_id, "play":null, "pre_play_refusal":error, "refusal_scope":"untyped-host-admission"}),
                );
                // The Host API returns an untyped error. Keep the failed proposal
                // and exact durable refusal; do not invent resource rejection facts.
                Err(error)
            }
        }
    }
    pub(crate) fn lull(&mut self) -> Result<(), String> {
        if let Some(realization) = self.session.realization() {
            let play = realization.play.clone();
            let host = self.host.advertisement();
            self.session
                .lull(&host.host_id, &host.boot_id, play.as_ref())
                .map_err(debug)?;
        }
        Ok(())
    }
}
fn debug(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}
struct BoundedOutput(Vec<u8>);
impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.0.len() + bytes.len() > 32 * 1024 {
            return Err(std::io::Error::other("owner output bound exhausted"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
struct DeadlineTimer {
    until: Instant,
    control: RunControl,
}
impl TimerAdapter for DeadlineTimer {
    fn wait(&mut self, duration: Duration) {
        let end = Instant::now() + duration;
        while Instant::now() < end && Instant::now() < self.until && !self.control.stop_requested()
        {
            std::thread::sleep(
                Duration::from_millis(10).min(end.saturating_duration_since(Instant::now())),
            );
        }
        if Instant::now() >= self.until {
            let _ = self.control.request_stop(
                RunControlRequestId::new("owner/run-deadline").expect("bounded request"),
            );
        }
    }

    fn monotonic_observation(
        &mut self,
        host_id: &conduit_core::HostId,
        boot_id: &conduit_core::BootId,
    ) -> Option<conduit_core::MonotonicInstant> {
        ThreadTimer.monotonic_observation(host_id, boot_id)
    }

    fn monotonic_now_ms(&mut self) -> Option<u64> {
        ThreadTimer.monotonic_now_ms()
    }

    fn monotonic_now_micros(&mut self) -> Option<u64> {
        ThreadTimer.monotonic_now_micros()
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
