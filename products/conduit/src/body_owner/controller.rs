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
use conduit_core::{bind_sign, BaseImplementationId};
use conduit_presentation::{
    Face, FaceContext, FaceFocus, FaceNames, FaceResidentPlotName, OwnerFaceSnapshotRequest,
    Presentation,
};
use conduit_std_host::body_execution::BodyRunRequest;
use conduit_std_host::{RunControl, RunControlRequestId, StdHost, TimerAdapter};
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

#[path = "continuing.rs"]
mod continuing;
pub(crate) use continuing::RunWorker;
mod birth;
#[path = "clock_interval.rs"]
mod clock_interval;
#[path = "native_mask_route.rs"]
mod native_mask_route;
pub(crate) use clock_interval::{is_clock_control_intent, ClockAction, CLOCK_RUN_MAXIMUM_MILLIS};
#[cfg(unix)]
#[path = "terminal_route.rs"]
mod terminal_route;
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
        if self.current.is_some()
            || host.advertisement().host_id != self.advertised.host_id
            || host.advertisement().boot_id != self.advertised.boot_id
        {
            return Err("Body Play returned a different or duplicate Host Boot".into());
        }
        self.advertised = host.advertisement().clone();
        self.current = Some(host);
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
        })
    }
    pub(crate) fn persist(&mut self, root: &Path) -> Result<(), String> {
        if !self.session.pending_archives().is_empty() {
            return Err(
                "owner biography archive capacity requires an admitted archive store".into(),
            );
        }
        state::retain(
            root,
            self.session.evidence(),
            self.last_execution.as_ref(),
            self.admissions.as_ref(),
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
        let plot_name = self
            .resident
            .as_ref()
            .zip(self.resident_name.as_deref())
            .map(|(resident, name)| FaceResidentPlotName {
                source_document_id: &resident.source_document_id,
                checked_plot_id: &resident.checked_plot_id,
                name,
            });
        let plot_names: Vec<_> = plot_name.into_iter().collect();
        let face = Face::project_with_names(
            &self.session.evidence().body,
            self.session
                .realization()
                .map(|realization| &realization.wake),
            self.session.evidence().last_sequence(),
            FaceContext::Overview,
            FaceFocus::Body,
            vec![],
            FaceNames {
                body_name: Some(&self.session.evidence().friendly_name),
                resident_plots: &plot_names,
            },
        )
        .map_err(|error| format!("owner-face-projection-refused:{error:?}"))?;
        face.presentation
            .validate()
            .map_err(|error| format!("owner-face-invalid:{error:?}"))?;
        clock_interval::with_clock_action(self, face.presentation)
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

    fn plan_partition(
        &self,
        plot: &conduit_plot::ExpandedAuthoringPlot,
        resident: &ResidentPlot,
    ) -> Result<BodyPlotPlan, String> {
        let hosts = [self.host.advertisement().clone()];
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
    pub(super) fn execute(&mut self, maximum_millis: u64) -> Result<(), String> {
        if !(1..=60_000).contains(&maximum_millis) {
            return Err("run duration must be 1..60000 milliseconds".into());
        }
        let proposed = self
            .session
            .realization()
            .ok_or("plan required before run")?
            .clone();
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
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
