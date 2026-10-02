#[cfg(feature = "authenticated-admission")]
use crate::AdmissionRefusal;
use crate::{
    BodyBiographyArchiveSegment, BodyBiographyError, BodyBiographyEvidence, BodyFulfillment,
    BodyLifecycleError, BodyLifecycleEvent, BodyPlan, BodyPlanError, BodyPlayIdentity,
    BodyPlotPlan, BodyState, FulfillmentObligation, MembershipRefusal, MembershipState,
    ResidentPlot, Wake,
};
use alloc::{vec, vec::Vec};
use conduit_core::{bind_sign, AuthorityGrantId, BootId, HostId, SignId};
use serde::{Deserialize, Serialize};

#[cfg(feature = "authenticated-admission")]
mod browser_admission;
mod continuity;
mod flow;
mod membership;
mod workload;

/// An exact current proposal and its optional admitted play, never a scheduler.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyLifecycleRealization {
    pub wake: Wake,
    pub plan: BodyPlan,
    pub play: Option<BodyPlayIdentity>,
}

#[derive(Clone, Debug)]
pub struct BodyLifecycleSession {
    evidence: BodyBiographyEvidence,
    realization: Option<BodyLifecycleRealization>,
    foreground: Option<ResidentPlot>,
    pub(crate) pending_archives: Vec<BodyBiographyArchiveSegment>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyLifecycleSessionError {
    Biography(BodyBiographyError),
    #[cfg(feature = "authenticated-admission")]
    Admission(AdmissionRefusal),
    Lifecycle(BodyLifecycleError),
    Plan(BodyPlanError),
    Membership(MembershipRefusal),
    NotLulled,
    NoProposal,
    AlreadyPlaying,
    StaleHost,
    StalePlay,
    StaleWorkload,
    UninstalledPlot,
    SequenceExhausted,
    UnreconciledWake,
    ArchivePersistenceRequired,
}

impl BodyLifecycleSession {
    /// A Crèche handoff or retained body may be opened for inspection. Only a
    /// Lulled Body can subsequently mutate; Fulfilled remains terminal.
    /// An Awake snapshot alone never proves its previous Play has ended.
    pub fn open(evidence: BodyBiographyEvidence) -> Result<Self, BodyLifecycleSessionError> {
        evidence
            .validate()
            .map_err(BodyLifecycleSessionError::Biography)?;
        if !matches!(
            evidence.body.state,
            BodyState::Lulled | BodyState::Fulfilled { .. }
        ) {
            return Err(BodyLifecycleSessionError::UnreconciledWake);
        }
        let foreground = evidence.body.workset.plots().first().cloned();
        Ok(Self {
            foreground,
            evidence,
            realization: None,
            pending_archives: Vec::new(),
        })
    }

    /// Open a freshly admitted body snapshot for the exact receiving Host.
    /// Unlike reload continuity, the current Boot must already be present.
    pub fn open_admitted(
        evidence: BodyBiographyEvidence,
        host: &HostId,
        boot: &BootId,
    ) -> Result<Self, BodyLifecycleSessionError> {
        let body = Self::open(evidence)?;
        body.require_host(host, boot)?;
        Ok(body)
    }

    pub fn evidence(&self) -> &BodyBiographyEvidence {
        &self.evidence
    }
    pub fn realization(&self) -> Option<&BodyLifecycleRealization> {
        self.realization.as_ref()
    }

    pub fn foreground(&self) -> Option<&ResidentPlot> {
        self.foreground.as_ref()
    }

    pub fn pending_archives(&self) -> &[BodyBiographyArchiveSegment] {
        &self.pending_archives
    }

    /// A host calls this only after the archive segments and the newer active
    /// evidence committed in one durable transaction.
    pub fn acknowledge_archives(
        &mut self,
        head_digest: [u8; 32],
    ) -> Result<(), BodyLifecycleSessionError> {
        if self.pending_archives.last().map(|segment| segment.digest) != Some(head_digest) {
            return Err(BodyLifecycleSessionError::ArchivePersistenceRequired);
        }
        self.pending_archives.clear();
        Ok(())
    }

    /// Foreground is presentation focus within the current workset. Selecting a
    /// surface changes neither its body lifecycle nor the exact admitted play.
    pub fn select_plot(&mut self, plot: &ResidentPlot) -> Result<(), BodyLifecycleSessionError> {
        if !self.evidence.body.workset.plots().contains(plot) {
            return Err(BodyLifecycleSessionError::UninstalledPlot);
        }
        self.foreground = Some(plot.clone());
        Ok(())
    }

    /// Seal the complete workset before publishing a wake. Planning refusal
    /// therefore preserves the prior Body exactly. The host still acquires and
    /// admits resources before starting the returned proposal.
    pub fn propose(
        &mut self,
        plots: Vec<BodyPlotPlan>,
        host: &HostId,
        boot: &BootId,
    ) -> Result<&BodyLifecycleRealization, BodyLifecycleSessionError> {
        self.require_mutable()?;
        if self.evidence.body.state != BodyState::Lulled || self.realization.is_some() {
            return Err(BodyLifecycleSessionError::NotLulled);
        }
        self.require_host(host, boot)?;
        for partition in &plots {
            for fragment in &partition.plan.fragments {
                self.require_host(&fragment.host_id, &fragment.boot_id)?;
            }
        }
        // Reserve the whole Wake boundary up front: Woke now and the eventual
        // retained lull. A proposal must not publish a wake that cannot close.
        self.make_lifecycle_room(2, 1)?;
        let sequence = self.next_sequence()?;
        let (body, wake) = self
            .evidence
            .body
            .wake(sequence, sign(host, boot, sequence))
            .map_err(BodyLifecycleSessionError::Lifecycle)?;
        let plan = BodyPlan::seal(&wake, plots).map_err(BodyLifecycleSessionError::Plan)?;
        let mut evidence = self.evidence.clone();
        evidence
            .append_wake(body, wake.clone(), sequence)
            .map_err(BodyLifecycleSessionError::Biography)?;
        self.evidence = evidence;
        self.realization = Some(BodyLifecycleRealization {
            wake,
            plan,
            play: None,
        });
        Ok(self.realization.as_ref().expect("published realization"))
    }

    /// Accept only the exact lifecycle returned by an admitted host start.
    pub fn started(
        &mut self,
        host: &HostId,
        boot: &BootId,
        play: BodyPlayIdentity,
        wake_at_start: Wake,
    ) -> Result<(), BodyLifecycleSessionError> {
        self.require_host(host, boot)?;
        let current = self
            .realization
            .as_ref()
            .ok_or(BodyLifecycleSessionError::NoProposal)?;
        if current.play.is_some() {
            return Err(BodyLifecycleSessionError::AlreadyPlaying);
        }
        if !play.validate_for(&current.plan) {
            return Err(BodyLifecycleSessionError::StalePlay);
        }
        let evidence_sign =
            |sequence| bind_sign(host, boot, Some(&play.active_play_id), sequence).sign_id;
        let expected = current
            .wake
            .body_plan_ready(&current.plan, evidence_sign(0))
            .and_then(|wake| wake.body_play_started(&current.plan, &play, evidence_sign(1)))
            .map_err(BodyLifecycleSessionError::Lifecycle)?;
        if expected != wake_at_start {
            return Err(BodyLifecycleSessionError::StalePlay);
        }
        let mut evidence = self.evidence.clone();
        evidence
            .append_wake(
                evidence.body.clone(),
                expected.clone(),
                self.next_sequence()?,
            )
            .map_err(BodyLifecycleSessionError::Biography)?;
        self.evidence = evidence;
        let realization = self.realization.as_mut().expect("validated realization");
        realization.wake = expected;
        realization.play = Some(play);
        Ok(())
    }

    /// The caller first obtains the exact terminal receipt from its host.
    /// Refusal before start supplies no Play. No cancellation or retry is invented.
    pub fn lull(
        &mut self,
        host: &HostId,
        boot: &BootId,
        terminated_play: Option<&BodyPlayIdentity>,
    ) -> Result<(), BodyLifecycleSessionError> {
        self.require_host(host, boot)?;
        let current = self
            .realization
            .as_ref()
            .ok_or(BodyLifecycleSessionError::NoProposal)?;
        if current.play.as_ref() != terminated_play {
            return Err(BodyLifecycleSessionError::StalePlay);
        }
        let sequence = self.next_sequence()?;
        let retain_sequence = sequence
            .checked_add(1)
            .ok_or(BodyLifecycleSessionError::SequenceExhausted)?;
        let wake = current
            .wake
            .lull(sign(host, boot, sequence))
            .map_err(BodyLifecycleSessionError::Lifecycle)?;
        let body = self
            .evidence
            .body
            .retain_after_lull(&wake, sign(host, boot, retain_sequence))
            .map_err(BodyLifecycleSessionError::Lifecycle)?;
        let mut evidence = self.evidence.clone();
        evidence
            .append_wake(body, wake, sequence)
            .map_err(BodyLifecycleSessionError::Biography)?;
        self.evidence = evidence;
        self.realization = None;
        Ok(())
    }

    /// Retain a pre-play refusal as part of the exact wake biography. The
    /// caller supplies typed facts from the layer that made the decision.
    pub fn fail(
        &mut self,
        host: &HostId,
        boot: &BootId,
        rejections: Vec<crate::WakeRejectionEvidence>,
    ) -> Result<(), BodyLifecycleSessionError> {
        self.require_host(host, boot)?;
        let current = self
            .realization
            .as_ref()
            .ok_or(BodyLifecycleSessionError::NoProposal)?;
        if current.play.is_some() {
            return Err(BodyLifecycleSessionError::StalePlay);
        }
        if rejections.is_empty()
            || rejections.iter().any(|rejection| {
                rejection.host_id != *host
                    || rejection.boot_id != *boot
                    || rejection.plan_id.as_ref() != Some(&current.plan.plan_id)
                    || rejection.checked_plot_ids.is_empty()
                    || rejection.checked_plot_ids.iter().any(|checked| {
                        !current
                            .plan
                            .plots
                            .iter()
                            .any(|plot| &plot.plot.checked_plot_id == checked)
                    })
            })
        {
            return Err(BodyLifecycleSessionError::StalePlay);
        }
        let plan_sequence = self.next_sequence()?;
        let failure_sequence = plan_sequence
            .checked_add(1)
            .ok_or(BodyLifecycleSessionError::SequenceExhausted)?;
        let retain_sequence = failure_sequence
            .checked_add(1)
            .ok_or(BodyLifecycleSessionError::SequenceExhausted)?;
        let wake = current
            .wake
            .body_plan_ready(&current.plan, sign(host, boot, plan_sequence))
            .and_then(|wake| {
                wake.fail_with_rejections(sign(host, boot, failure_sequence), rejections)
            })
            .map_err(BodyLifecycleSessionError::Lifecycle)?;
        let body = self
            .evidence
            .body
            .retain_after_lull(&wake, sign(host, boot, retain_sequence))
            .map_err(BodyLifecycleSessionError::Lifecycle)?;
        let mut evidence = self.evidence.clone();
        evidence
            .append_wake(body, wake, plan_sequence)
            .map_err(BodyLifecycleSessionError::Biography)?;
        self.evidence = evidence;
        self.realization = None;
        Ok(())
    }

    /// Record the explicit operator conclusion only after the browser runtime
    /// has proved that no Play or implementation remains active.
    pub fn fulfill(
        &mut self,
        host: &HostId,
        boot: &BootId,
        authority_grant_id: AuthorityGrantId,
        attribution: alloc::string::String,
    ) -> Result<(), BodyLifecycleSessionError> {
        self.require_mutable()?;
        self.require_host(host, boot)?;
        if self.evidence.body.state != BodyState::Lulled || self.realization.is_some() {
            return Err(BodyLifecycleSessionError::NotLulled);
        }
        self.make_lifecycle_room(1, 0)?;
        let sequence = self.next_sequence()?;
        let sign_id = sign(host, boot, sequence);
        let final_wake_id = self
            .evidence
            .body
            .events
            .iter()
            .rev()
            .find_map(|event| match event {
                BodyLifecycleEvent::LullRetained { wake_id, .. } => Some(wake_id.clone()),
                _ => None,
            });
        let fulfillment = BodyFulfillment {
            final_wake_id,
            authority_grant_id,
            attribution,
            settled_obligations: vec![FulfillmentObligation {
                obligation_id: "obligation/workspace-runtime-empty".into(),
                settlement_sign_id: sign_id.clone(),
            }],
        };
        let body = self
            .evidence
            .body
            .fulfill(fulfillment, sign_id.clone())
            .map_err(BodyLifecycleSessionError::Lifecycle)?;
        let mut evidence = self.evidence.clone();
        evidence
            .append_body_lifecycle_events(body, &[(sign_id, sequence)])
            .map_err(BodyLifecycleSessionError::Biography)?;
        self.evidence = evidence;
        Ok(())
    }

    fn require_host(&self, host: &HostId, boot: &BootId) -> Result<(), BodyLifecycleSessionError> {
        if self.evidence.membership.parts.iter().any(|part| {
            part.state == MembershipState::Admitted
                && part
                    .current
                    .as_ref()
                    .is_some_and(|current| &current.host_id == host && &current.boot_id == boot)
        }) {
            Ok(())
        } else {
            Err(BodyLifecycleSessionError::StaleHost)
        }
    }

    fn require_mutable(&self) -> Result<(), BodyLifecycleSessionError> {
        self.evidence
            .body
            .ensure_mutable()
            .map_err(BodyLifecycleSessionError::Lifecycle)
    }

    fn next_sequence(&self) -> Result<u64, BodyLifecycleSessionError> {
        self.evidence
            .last_sequence()
            .checked_add(1)
            .ok_or(BodyLifecycleSessionError::SequenceExhausted)
    }

    fn make_lifecycle_room(
        &mut self,
        body_signs: usize,
        wakes: usize,
    ) -> Result<(), BodyLifecycleSessionError> {
        while self.evidence.body.sign_ids.len().saturating_add(body_signs) > crate::MAX_BODY_SIGNS
            || self.evidence.wakes.len().saturating_add(wakes) > crate::MAX_BODY_BIOGRAPHY_WAKES
            || self.evidence.records.len().saturating_add(5) > crate::MAX_BODY_BIOGRAPHY_RECORDS
        {
            if self.pending_archives.len() >= crate::MAX_BODY_BIOGRAPHY_WAKES {
                return Err(BodyLifecycleSessionError::ArchivePersistenceRequired);
            }
            let segment = self
                .evidence
                .seal_oldest_terminal_wake()
                .map_err(BodyLifecycleSessionError::Biography)?;
            let segment = match segment {
                Some(segment) => Some(segment),
                None => self
                    .evidence
                    .seal_body_workload_history()
                    .map_err(BodyLifecycleSessionError::Biography)?,
            };
            let segment = match segment {
                Some(segment) => Some(segment),
                None => self
                    .evidence
                    .seal_membership_history()
                    .map_err(BodyLifecycleSessionError::Biography)?,
            };
            let Some(segment) = segment else {
                return Err(BodyLifecycleSessionError::Biography(
                    BodyBiographyError::CapacityExhausted,
                ));
            };
            self.pending_archives.push(segment);
        }
        Ok(())
    }

    fn make_membership_room(&mut self, events: usize) -> Result<(), BodyLifecycleSessionError> {
        if self.evidence.membership.events.len().saturating_add(events)
            <= crate::MAX_MEMBERSHIP_EVENTS
            && self.evidence.records.len().saturating_add(events)
                <= crate::MAX_BODY_BIOGRAPHY_RECORDS
        {
            return Ok(());
        }
        if self.pending_archives.len() >= crate::MAX_BODY_BIOGRAPHY_WAKES {
            return Err(BodyLifecycleSessionError::ArchivePersistenceRequired);
        }
        let segment = self
            .evidence
            .seal_membership_history()
            .map_err(BodyLifecycleSessionError::Biography)?
            .ok_or(BodyLifecycleSessionError::Biography(
                BodyBiographyError::CapacityExhausted,
            ))?;
        self.pending_archives.push(segment);
        Ok(())
    }
}

fn sign(host: &HostId, boot: &BootId, sequence: u64) -> SignId {
    bind_sign(host, boot, None, sequence).sign_id
}
