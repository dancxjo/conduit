use alloc::vec::Vec;
use conduit_body::{
    BodyBiographyError, BodyBiographyEvidence, BodyFormPlan, BodyLifecycleError, BodyPlan,
    BodyPlanError, BodyPlayIdentity, BodyState, MembershipState, ResidentForm, Wake,
};
use conduit_core::{BootId, HostId, SignId, bind_sign};
use serde::{Deserialize, Serialize};

/// An exact current proposal and its optional admitted Play, never a scheduler.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceRealization {
    pub wake: Wake,
    pub plan: BodyPlan,
    pub play: Option<BodyPlayIdentity>,
}

#[derive(Clone, Debug)]
pub struct WorkspaceBody {
    evidence: BodyBiographyEvidence,
    realization: Option<WorkspaceRealization>,
    foreground: Option<ResidentForm>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceBodyError {
    Biography(BodyBiographyError),
    Lifecycle(BodyLifecycleError),
    Plan(BodyPlanError),
    NotLulled,
    NoProposal,
    AlreadyPlaying,
    StaleHost,
    StalePlay,
    StaleWorkload,
    UninstalledForm,
    SequenceExhausted,
    UnreconciledWake,
}

impl WorkspaceBody {
    /// A Crèche handoff or a retained Lulled Body is ready for fresh admission.
    /// An Awake snapshot alone never proves its previous Play has ended.
    pub fn open(evidence: BodyBiographyEvidence) -> Result<Self, WorkspaceBodyError> {
        evidence.validate().map_err(WorkspaceBodyError::Biography)?;
        if evidence.body.state != BodyState::Lulled {
            return Err(WorkspaceBodyError::UnreconciledWake);
        }
        let foreground = evidence.body.workset.forms().first().cloned();
        Ok(Self {
            foreground,
            evidence,
            realization: None,
        })
    }

    pub fn evidence(&self) -> &BodyBiographyEvidence {
        &self.evidence
    }
    pub fn realization(&self) -> Option<&WorkspaceRealization> {
        self.realization.as_ref()
    }

    pub fn foreground(&self) -> Option<&ResidentForm> {
        self.foreground.as_ref()
    }

    /// Foreground is presentation focus within the current workset. Selecting a
    /// surface changes neither its Body lifecycle nor the exact admitted Play.
    pub fn select_form(&mut self, form: &ResidentForm) -> Result<(), WorkspaceBodyError> {
        if !self.evidence.body.workset.forms().contains(form) {
            return Err(WorkspaceBodyError::UninstalledForm);
        }
        self.foreground = Some(form.clone());
        Ok(())
    }

    /// Seal the complete workset before publishing a Wake. Planning refusal
    /// therefore preserves the prior Body exactly. The Host still acquires and
    /// admits resources before starting the returned proposal.
    pub fn propose(
        &mut self,
        forms: Vec<BodyFormPlan>,
        host: &HostId,
        boot: &BootId,
    ) -> Result<&WorkspaceRealization, WorkspaceBodyError> {
        if self.evidence.body.state != BodyState::Lulled || self.realization.is_some() {
            return Err(WorkspaceBodyError::NotLulled);
        }
        self.require_host(host, boot)?;
        for partition in &forms {
            for fragment in &partition.plan.fragments {
                self.require_host(&fragment.host_id, &fragment.boot_id)?;
            }
        }
        let sequence = self.next_sequence()?;
        let (body, wake) = self
            .evidence
            .body
            .wake(sequence, sign(host, boot, sequence))
            .map_err(WorkspaceBodyError::Lifecycle)?;
        let plan = BodyPlan::seal(&wake, forms).map_err(WorkspaceBodyError::Plan)?;
        let mut evidence = self.evidence.clone();
        evidence
            .append_wake(body, wake.clone(), sequence)
            .map_err(WorkspaceBodyError::Biography)?;
        self.evidence = evidence;
        self.realization = Some(WorkspaceRealization {
            wake,
            plan,
            play: None,
        });
        Ok(self.realization.as_ref().expect("published realization"))
    }

    /// Accept only the exact lifecycle returned by an admitted Host start.
    pub fn started(
        &mut self,
        host: &HostId,
        boot: &BootId,
        play: BodyPlayIdentity,
        wake_at_start: Wake,
    ) -> Result<(), WorkspaceBodyError> {
        self.require_host(host, boot)?;
        let current = self
            .realization
            .as_ref()
            .ok_or(WorkspaceBodyError::NoProposal)?;
        if current.play.is_some() {
            return Err(WorkspaceBodyError::AlreadyPlaying);
        }
        if !play.validate_for(&current.plan) {
            return Err(WorkspaceBodyError::StalePlay);
        }
        let evidence_sign =
            |sequence| bind_sign(host, boot, Some(&play.active_play_id), sequence).sign_id;
        let expected = current
            .wake
            .body_plan_ready(&current.plan, evidence_sign(0))
            .and_then(|wake| wake.body_play_started(&current.plan, &play, evidence_sign(1)))
            .map_err(WorkspaceBodyError::Lifecycle)?;
        if expected != wake_at_start {
            return Err(WorkspaceBodyError::StalePlay);
        }
        let mut evidence = self.evidence.clone();
        evidence
            .append_wake(
                evidence.body.clone(),
                expected.clone(),
                self.next_sequence()?,
            )
            .map_err(WorkspaceBodyError::Biography)?;
        self.evidence = evidence;
        let realization = self.realization.as_mut().expect("validated realization");
        realization.wake = expected;
        realization.play = Some(play);
        Ok(())
    }

    /// The caller first obtains the exact terminal receipt from its Host.
    /// Refusal before start supplies no Play. No cancellation or retry is invented.
    pub fn lull(
        &mut self,
        host: &HostId,
        boot: &BootId,
        terminated_play: Option<&BodyPlayIdentity>,
    ) -> Result<(), WorkspaceBodyError> {
        self.require_host(host, boot)?;
        let current = self
            .realization
            .as_ref()
            .ok_or(WorkspaceBodyError::NoProposal)?;
        if current.play.as_ref() != terminated_play {
            return Err(WorkspaceBodyError::StalePlay);
        }
        let sequence = self.next_sequence()?;
        let retain_sequence = sequence
            .checked_add(1)
            .ok_or(WorkspaceBodyError::SequenceExhausted)?;
        let wake = current
            .wake
            .lull(sign(host, boot, sequence))
            .map_err(WorkspaceBodyError::Lifecycle)?;
        let body = self
            .evidence
            .body
            .retain_after_lull(&wake, sign(host, boot, retain_sequence))
            .map_err(WorkspaceBodyError::Lifecycle)?;
        let mut evidence = self.evidence.clone();
        evidence
            .append_wake(body, wake, sequence)
            .map_err(WorkspaceBodyError::Biography)?;
        self.evidence = evidence;
        self.realization = None;
        Ok(())
    }

    /// Add checked meaning while Lulled. Current Play replacement is a separate
    /// Host-orchestrated lifecycle; this cannot mutate an admitted Plan.
    pub fn admit_form(
        &mut self,
        expected_revision: u64,
        form: ResidentForm,
        host: &HostId,
        boot: &BootId,
    ) -> Result<(), WorkspaceBodyError> {
        self.require_host(host, boot)?;
        if self.evidence.body.state != BodyState::Lulled {
            return Err(WorkspaceBodyError::NotLulled);
        }
        if self.evidence.body.workload_revision != expected_revision {
            return Err(WorkspaceBodyError::StaleWorkload);
        }
        let sequence = self.next_sequence()?;
        let sign_id = sign(host, boot, sequence);
        let body = self
            .evidence
            .body
            .admit_form(form, sign_id.clone())
            .map_err(WorkspaceBodyError::Lifecycle)?;
        let mut evidence = self.evidence.clone();
        evidence
            .append_body_workload_events(body, &[(sign_id, sequence)])
            .map_err(WorkspaceBodyError::Biography)?;
        self.evidence = evidence;
        Ok(())
    }

    fn require_host(&self, host: &HostId, boot: &BootId) -> Result<(), WorkspaceBodyError> {
        if self.evidence.membership.parts.iter().any(|part| {
            part.state == MembershipState::Admitted
                && part
                    .current
                    .as_ref()
                    .is_some_and(|current| &current.host_id == host && &current.boot_id == boot)
        }) {
            Ok(())
        } else {
            Err(WorkspaceBodyError::StaleHost)
        }
    }

    fn next_sequence(&self) -> Result<u64, WorkspaceBodyError> {
        self.evidence
            .records
            .last()
            .map_or(0, |record| record.sequence)
            .checked_add(1)
            .ok_or(WorkspaceBodyError::SequenceExhausted)
    }
}

fn sign(host: &HostId, boot: &BootId, sequence: u64) -> SignId {
    bind_sign(host, boot, None, sequence).sign_id
}
