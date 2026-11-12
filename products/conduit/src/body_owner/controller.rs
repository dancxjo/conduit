//! Body lifecycle truth; all execution identities come from the installed kernel.
use super::state;
#[path = "participants/invited.rs"]
mod invited;
#[path = "participants/mod.rs"]
mod participants;
use conduit_body::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyLifecycleSession,
    BodyMembership, BodyWorkset, MembershipProofId, PartId, ResidentPlot,
};
use conduit_core::bind_sign;
use conduit_core::HostAdvertisement;
use conduit_face::{Face, OwnerFaceSnapshotRequest};
use conduit_std_host::{RunControl, RunControlRequestId, StdHost, ThreadTimer, TimerAdapter};
#[cfg(unix)]
pub(crate) use participants::run_service_window;
pub(crate) use participants::serve_member_presence;
#[cfg(test)]
pub(crate) use participants::serve_with_carrier;
#[cfg(test)]
pub(crate) use participants::serve_with_dispatch;
pub(crate) use participants::{
    BrowserAdmittedSnapshot, BrowserCarrierLineEvidence, BrowserWindowAuthorization,
};
use std::{
    io::Write,
    ops::{Deref, DerefMut},
    path::Path,
    time::{Duration, Instant},
};

#[cfg(test)]
#[path = "reference_execution_fixture.rs"]
mod reference_execution_fixture;

// The installed request entrance remains gated on admitted restore and Face
// action routing; this exact first-action path is exercised by focused proof.
#[allow(dead_code)]
#[path = "checkpoint_once.rs"]
mod checkpoint_once;
#[cfg(test)]
pub(crate) use checkpoint_once::tests::published_fixture as published_todo_test_fixture;
#[path = "continuing.rs"]
mod continuing;
#[path = "execution_retention.rs"]
mod execution_retention;
pub(crate) use continuing::RunWorker;
#[path = "admission_retirement.rs"]
mod admission_retirement;
mod birth;
#[path = "clock_interval.rs"]
mod clock_interval;
#[path = "creche_export.rs"]
mod creche_export;
#[path = "creche_planning.rs"]
mod creche_planning;
#[path = "creche_route_custody.rs"]
mod creche_route_custody;
#[path = "creche_worker.rs"]
mod creche_worker;
#[path = "direct_spoken_route.rs"]
mod direct_spoken_route;
#[path = "fore_line_planning.rs"]
mod fore_line_planning;
#[path = "member_carrier.rs"]
mod member_carrier;
#[path = "member_leave.rs"]
mod member_leave;
#[path = "member_offers.rs"]
mod member_offers;
#[path = "member_return.rs"]
mod member_return;
#[path = "planning.rs"]
mod planning;
#[cfg(target_os = "linux")]
pub(crate) use fore_line_planning::OwnedForeLineSelection;
#[path = "remote_preparation.rs"]
mod remote_preparation;
#[path = "retained_planning.rs"]
mod retained_planning;
#[path = "workset_revision.rs"]
mod workset_revision;
pub(crate) use direct_spoken_route::DirectSpokenStart;
#[path = "llm_spoken_route.rs"]
mod llm_spoken_route;
pub(crate) use llm_spoken_route::LlmSpokenStart;
#[path = "face_wardrobe.rs"]
mod face_wardrobe;
#[path = "face_wardrobe_report.rs"]
mod face_wardrobe_report;
#[path = "face_wardrobe_runtime.rs"]
mod face_wardrobe_runtime;
#[path = "face_wardrobe_witness.rs"]
mod face_wardrobe_witness;
#[path = "native_mask_route.rs"]
mod native_mask_route;
#[path = "remote_spoken_route.rs"]
mod remote_spoken_route;
#[path = "remote_terminal_route.rs"]
mod remote_terminal_route;
#[path = "wardrobe_face.rs"]
mod wardrobe_face;
pub(crate) use clock_interval::{is_clock_control_intent, ClockAction, CLOCK_RUN_MAXIMUM_MILLIS};
#[cfg(unix)]
#[path = "terminal_route.rs"]
mod terminal_route;
#[path = "thermostat_face.rs"]
mod thermostat_face;
#[path = "thermostat_worker.rs"]
mod thermostat_worker;
pub(crate) use thermostat_worker::ThermostatWorker;
// Bounded source/owner assembly; excluded from initial discovery (#5610).
#[allow(dead_code)]
#[path = "commons_encounter.rs"]
mod commons_encounter;
#[path = "notebook.rs"]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
mod notebook;
pub(crate) use notebook::NotebookWaitingWorker;
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

    pub(crate) fn verify_selected_image_fore_input(
        &self,
        input: &conduit_std_host::body_execution::ImageForeInput,
    ) -> Result<(), String> {
        self.current
            .as_ref()
            .ok_or("selected IMAGE ingress requires an idle owner Host")?
            .verify_selected_image_fore_input(input)
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
    execution_persistence_pending: bool,
    workset_recovery_required: bool,
    local_fore_adapters: Option<crate::durable_host::local_fore_policy::SelectedLocalForeSet>,
    #[cfg(target_os = "linux")]
    selected_file_workset: Option<crate::durable_host::source_revision::RetainedFileWorkset>,
    admissions: Option<conduit_body::AdmissionManager>,
    prepared_admission_route: Option<crate::durable_host::PreparedAdmissionRoute>,
    pending_browser: Option<participants::BrowserWindow>,
    pending_native_mask: Option<native_mask_route::NativeMaskRoute>,
    pending_member_return: Option<member_return::PendingMemberReturn>,
    member_carrier: Option<Box<member_carrier::MemberCarrierCustody>>,
    current_host_offers: conduit_body::CurrentHostOffers,
    attached_terminal_route: Option<conduit_face::LocalOwnerMaskRouteSeal>,
    direct_spoken_route: Option<conduit_face::LocalOwnerMaskRouteSeal>,
    direct_spoken_template:
        Option<conduit_std_host::direct_spoken_template_mask::PreparedDirectSpokenTemplateMask>,
    llm_spoken_route: Option<conduit_face::LocalOwnerMaskRouteSeal>,
    face_wardrobe: Option<face_wardrobe::OwnerFaceWardrobe>,
    /// Bounded projection cache from the configured initial Form and the exact
    /// current Thermostat scan Fore. The kernel owns its retained accumulator.
    thermostat_live: Option<thermostat_face::ThermostatLive>,
    /// Projection cache for the exact currently Playing Todo encounter. The
    /// next Play must restore through its admitted read Host Call.
    todo_live: Option<(conduit_core::ActivePlayId, conduit_todo_plot::TodoState)>,
    /// Canonical document projection adopted from completed notebook Source steps.
    notebook: Option<notebook::NotebookDocument>,
    creche_factory: Option<conduit::creche_encounter::RetainedCrecheFactory>,
    creche_live: Option<creche_worker::CrecheLive>,
    creche_products: Option<conduit::creche_encounter::CrecheProducts>,
    /// Bounded display cache from an exact selected read Host Call and both
    /// terminal Signs. It is never a reducer or a source of authority.
    todo_verified: Option<(
        conduit_face::CommittedStateContributionBasis,
        conduit_todo_plot::TodoState,
    )>,
}
impl Owner {
    pub(crate) fn selected_speech_host_is_idle(&self) -> bool {
        !self.workset_recovery_required
            && !self.host.is_playing()
            && self.session.realization().is_none()
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
            execution_persistence_pending: false,
            workset_recovery_required: false,
            local_fore_adapters: None,
            #[cfg(target_os = "linux")]
            selected_file_workset: None,
            admissions: None,
            prepared_admission_route: None,
            pending_browser: None,
            pending_native_mask: None,
            pending_member_return: None,
            member_carrier: None,
            current_host_offers: conduit_body::CurrentHostOffers::new(),
            attached_terminal_route: None,
            direct_spoken_route: None,
            direct_spoken_template: None,
            llm_spoken_route: None,
            face_wardrobe: None,
            thermostat_live: None,
            todo_live: None,
            todo_verified: None,
            notebook: None,
            creche_factory: None,
            creche_live: None,
            creche_products: None,
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
            execution_persistence_pending: false,
            workset_recovery_required: false,
            local_fore_adapters: None,
            #[cfg(target_os = "linux")]
            selected_file_workset: None,
            admissions: None,
            prepared_admission_route: None,
            pending_browser: None,
            pending_native_mask: None,
            pending_member_return: None,
            member_carrier: None,
            current_host_offers: conduit_body::CurrentHostOffers::new(),
            attached_terminal_route: None,
            direct_spoken_route: None,
            direct_spoken_template: None,
            llm_spoken_route: None,
            face_wardrobe: None,
            thermostat_live: None,
            todo_live: None,
            todo_verified: None,
            notebook: None,
            creche_factory: None,
            creche_live: None,
            creche_products: None,
        })
    }
    pub(crate) fn persist(&mut self, root: &Path) -> Result<(), String> {
        self.require_workset_recovered()?;
        let result = state::retain_session(
            root,
            &mut self.session,
            self.last_execution.as_ref(),
            self.admissions.as_ref(),
            None,
            None,
        );
        self.execution_persistence_pending = result.is_err() && self.last_execution.is_some();
        result
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
        self.execution_persistence_pending = false;
        self.admissions = state::admissions(root, &self.session.evidence().body_id)?;
        Ok(())
    }
    pub(crate) fn truth(&self) -> serde_json::Value {
        serde_json::json!({"schema":"conduit.body/owner-truth@1", "host":self.host.advertisement(), "biography":self.session.evidence(), "realization":self.session.realization(), "last_execution":self.last_execution, "execution_persistence_pending":self.execution_persistence_pending, "current_member_offers":self.current_offer_summary()})
    }
    /// Project one canonical Face from the current retained owner session.
    /// Credential matching and current incarnation are checked here, even
    /// though the routed caller has already authenticated the same Line.
    pub(crate) fn face_snapshot(&self, request: &OwnerFaceSnapshotRequest) -> Result<Face, String> {
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
    pub(crate) fn local_face_snapshot(&self) -> Result<Face, String> {
        self.require_workset_recovered()?;
        if let Some(live) = &self.creche_live {
            if let Some(face) = live.controller.face()? {
                if face.basis.active_play_id.as_ref() == self.current_play_id()
                    && self.current_play_id().is_some()
                {
                    return Ok((*face).clone());
                }
            }
        }
        if self.has_open_notebook() {
            return if self.current_play_id().is_some() {
                self.project_notebook_face()
            } else {
                self.project_notebook_idle_face()
            };
        }
        if let Some(live) = &self.thermostat_live {
            return self.project_thermostat_face(live);
        }
        let face = match self.todo_live.as_ref() {
            Some((play, state)) if self.current_play_id() == Some(play) => {
                self.project_face(Some((state, true)))
            }
            Some(_) => Err("Todo Face projection cache differs from current Play".into()),
            None => match self.todo_verified.as_ref() {
                Some((basis, state)) => self.project_verified_todo_face(basis, state),
                None => self.project_face(None),
            },
        }?;
        self.project_creche_products(face)
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

    fn monotonic_ticks_on(&mut self, basis: &conduit_core::MonotonicClockIdentity) -> Option<u64> {
        ThreadTimer.monotonic_ticks_on(basis)
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

#[cfg(test)]
pub(crate) use tests::member_return_owner_fixture;

#[cfg(test)]
#[path = "service_workset_tests.rs"]
mod service_workset_tests;
