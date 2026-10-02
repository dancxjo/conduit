//! Body lifecycle truth; all execution identities come from the installed kernel.
use super::state;
#[path = "participants/mod.rs"]
mod participants;
use conduit_body::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyLifecycleSession,
    BodyMembership, BodyPlotPlan, BodyWorkset, MembershipProofId, PartId, ResidentPlot,
};
use conduit_core::{bind_sign, BaseImplementationId};
use conduit_std_host::body_execution::BodyRunRequest;
use conduit_std_host::{RunControl, RunControlRequestId, StdHost, TimerAdapter};
use std::{
    collections::BTreeMap,
    io::Write,
    path::Path,
    time::{Duration, Instant},
};

pub(super) struct Owner {
    host: StdHost,
    session: BodyLifecycleSession,
    resident: ResidentPlot,
    last_execution: Option<serde_json::Value>,
    admissions: Option<conduit_body::AdmissionManager>,
}
impl Owner {
    pub(super) fn open(
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
            host,
            session,
            resident,
            last_execution: None,
            admissions: None,
        })
    }
    pub(super) fn persist(&mut self, root: &Path) -> Result<(), String> {
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
    pub(super) fn restore_execution(&mut self, root: &Path) -> Result<(), String> {
        self.last_execution = state::execution(root)?;
        self.admissions = state::admissions(root, &self.session.evidence().body_id)?;
        Ok(())
    }
    pub(super) fn truth(&self) -> serde_json::Value {
        serde_json::json!({"schema":"conduit.body/owner-truth@1", "host":self.host.advertisement(), "biography":self.session.evidence(), "realization":self.session.realization(), "last_execution":self.last_execution})
    }
    pub(super) fn plan(
        &mut self,
        plot: &conduit_plot::ExpandedAuthoringPlot,
    ) -> Result<(), String> {
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
        self.session
            .propose(
                vec![BodyPlotPlan {
                    plot: self.resident.clone(),
                    plan,
                }],
                &hosts[0].host_id,
                &hosts[0].boot_id,
            )
            .map_err(debug)?;
        Ok(())
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
    pub(super) fn lull(&mut self) -> Result<(), String> {
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
        while Instant::now() < end && Instant::now() < self.until {
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
