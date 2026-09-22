use alloc::{vec, vec::Vec};
use conduit_body::{
    AdmissionManager, AdmissionRefusal, AdmissionSigns, BodyBiographyArchiveSegment,
    BodyBiographyError, BodyBiographyEvidence, BodyFormPlan, BodyFulfillment, BodyLifecycleError,
    BodyLifecycleEvent, BodyPlan, BodyPlanError, BodyPlayIdentity, BodyState,
    FulfillmentObligation, MembershipCredential, MembershipRefusal, MembershipState, ResidentForm,
    SpawnAdmissionProof, Wake,
};
use conduit_core::{AuthorityGrantId, BootId, HostAdvertisement, HostId, SignId, bind_sign};
use serde::{Deserialize, Serialize};

/// An exact current proposal and its optional admitted play, never a scheduler.
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
    pub(crate) pending_archives: Vec<BodyBiographyArchiveSegment>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceBodyError {
    Biography(BodyBiographyError),
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
    UninstalledForm,
    SequenceExhausted,
    UnreconciledWake,
    ArchivePersistenceRequired,
}

impl WorkspaceBody {
    /// A Crèche handoff or retained body may be opened for inspection. Only a
    /// Lulled Body can subsequently mutate; Fulfilled remains terminal.
    /// An Awake snapshot alone never proves its previous Play has ended.
    pub fn open(evidence: BodyBiographyEvidence) -> Result<Self, WorkspaceBodyError> {
        evidence.validate().map_err(WorkspaceBodyError::Biography)?;
        if !matches!(
            evidence.body.state,
            BodyState::Lulled | BodyState::Fulfilled { .. }
        ) {
            return Err(WorkspaceBodyError::UnreconciledWake);
        }
        let foreground = evidence.body.workset.forms().first().cloned();
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
    ) -> Result<Self, WorkspaceBodyError> {
        let body = Self::open(evidence)?;
        body.require_host(host, boot)?;
        Ok(body)
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

    pub fn pending_archives(&self) -> &[BodyBiographyArchiveSegment] {
        &self.pending_archives
    }

    /// A host calls this only after the archive segments and the newer active
    /// evidence committed in one durable transaction.
    pub fn acknowledge_archives(
        &mut self,
        head_digest: [u8; 32],
    ) -> Result<(), WorkspaceBodyError> {
        if self.pending_archives.last().map(|segment| segment.digest) != Some(head_digest) {
            return Err(WorkspaceBodyError::ArchivePersistenceRequired);
        }
        self.pending_archives.clear();
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn issue_invitation(
        &self,
        admissions: &mut AdmissionManager,
        secret: conduit_body::SpawnInvitationSecret,
        nonce: [u8; 32],
        now_millis: u64,
        expires_at_millis: u64,
        authority_host: &HostId,
        authority_boot: &BootId,
    ) -> Result<conduit_body::SpawnInvitationClaim, WorkspaceBodyError> {
        self.require_host(authority_host, authority_boot)?;
        admissions
            .issue_spawn_invitation(secret, nonce, now_millis, expires_at_millis)
            .map(|invitation| invitation.claim())
            .map_err(WorkspaceBodyError::Admission)
    }

    /// Complete one canonical single-use invitation at the body authority.
    /// Admission and authenticated presence are separate membership events;
    /// neither grants Form or effect authority.
    pub fn admit_invited_host(
        &mut self,
        admissions: &mut AdmissionManager,
        advertisement: &HostAdvertisement,
        proof: &SpawnAdmissionProof,
        now_millis: u64,
        authority_host: &HostId,
        authority_boot: &BootId,
    ) -> Result<MembershipCredential, WorkspaceBodyError> {
        self.require_host(authority_host, authority_boot)?;
        self.make_membership_room(2)?;
        let first_sequence = self.next_sequence()?;
        let second_sequence = first_sequence
            .checked_add(1)
            .ok_or(WorkspaceBodyError::SequenceExhausted)?;
        let signs = AdmissionSigns {
            part_admitted: sign(authority_host, authority_boot, first_sequence),
            host_attached: sign(authority_host, authority_boot, second_sequence),
            candidate_admitted: sign(authority_host, authority_boot, second_sequence),
        };
        let mut membership = self.evidence.membership.clone();
        let mut next_admissions = admissions.clone();
        let prior_events = membership.events.len();
        let credential = next_admissions
            .complete_spawn(&mut membership, advertisement, proof, now_millis, signs)
            .map_err(WorkspaceBodyError::Admission)?;
        let events = membership.events[prior_events..]
            .iter()
            .zip([first_sequence, second_sequence])
            .map(|(event, sequence)| (event.change_id.clone(), sequence))
            .collect::<Vec<_>>();
        if events.len() != 2 {
            return Err(WorkspaceBodyError::Biography(
                BodyBiographyError::InvalidEvidence,
            ));
        }
        let mut evidence = self.evidence.clone();
        evidence
            .append_membership_events(membership, &events)
            .map_err(WorkspaceBodyError::Biography)?;
        self.evidence = evidence;
        *admissions = next_admissions;
        Ok(credential)
    }

    /// Record an observed carrier loss without revoking the admitted Part.
    /// The exact host/Boot ceases to be current, so its offers can no longer
    /// participate in planning; a future authenticated observation may attach
    /// the Part again under fresh truth.
    pub fn observe_host_lost(
        &mut self,
        lost_host: &HostId,
        lost_boot: &BootId,
        authority_host: &HostId,
        authority_boot: &BootId,
    ) -> Result<(), WorkspaceBodyError> {
        self.require_host(authority_host, authority_boot)?;
        let part_id = self
            .evidence
            .membership
            .parts
            .iter()
            .find(|part| {
                part.state == MembershipState::Admitted
                    && part.current.as_ref().is_some_and(|current| {
                        &current.host_id == lost_host && &current.boot_id == lost_boot
                    })
            })
            .map(|part| part.part_id.clone())
            .ok_or(WorkspaceBodyError::StaleHost)?;
        self.make_membership_room(1)?;
        let sequence = self.next_sequence()?;
        let mut membership = self.evidence.membership.clone();
        let prior_events = membership.events.len();
        let change = membership
            .observe_offline(
                &self.evidence.body_id,
                membership.revision,
                &part_id,
                lost_boot,
                sign(authority_host, authority_boot, sequence),
            )
            .map_err(WorkspaceBodyError::Membership)?;
        if membership.events.len() != prior_events + 1 {
            return Err(WorkspaceBodyError::Biography(
                BodyBiographyError::InvalidEvidence,
            ));
        }
        let mut evidence = self.evidence.clone();
        evidence
            .append_membership_events(membership, &[(change, sequence)])
            .map_err(WorkspaceBodyError::Biography)?;
        self.evidence = evidence;
        Ok(())
    }

    /// Foreground is presentation focus within the current workset. Selecting a
    /// surface changes neither its body lifecycle nor the exact admitted play.
    pub fn select_form(&mut self, form: &ResidentForm) -> Result<(), WorkspaceBodyError> {
        if !self.evidence.body.workset.forms().contains(form) {
            return Err(WorkspaceBodyError::UninstalledForm);
        }
        self.foreground = Some(form.clone());
        Ok(())
    }

    /// Seal the complete workset before publishing a wake. Planning refusal
    /// therefore preserves the prior Body exactly. The host still acquires and
    /// admits resources before starting the returned proposal.
    pub fn propose(
        &mut self,
        forms: Vec<BodyFormPlan>,
        host: &HostId,
        boot: &BootId,
    ) -> Result<&WorkspaceRealization, WorkspaceBodyError> {
        self.require_mutable()?;
        if self.evidence.body.state != BodyState::Lulled || self.realization.is_some() {
            return Err(WorkspaceBodyError::NotLulled);
        }
        self.require_host(host, boot)?;
        for partition in &forms {
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

    /// Accept only the exact lifecycle returned by an admitted host start.
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

    /// The caller first obtains the exact terminal receipt from its host.
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

    /// Retain a pre-play refusal as part of the exact wake biography. The
    /// caller supplies typed facts from the layer that made the decision.
    pub fn fail(
        &mut self,
        host: &HostId,
        boot: &BootId,
        rejections: Vec<conduit_body::WakeRejectionEvidence>,
    ) -> Result<(), WorkspaceBodyError> {
        self.require_host(host, boot)?;
        let current = self
            .realization
            .as_ref()
            .ok_or(WorkspaceBodyError::NoProposal)?;
        if current.play.is_some() {
            return Err(WorkspaceBodyError::StalePlay);
        }
        if rejections.is_empty()
            || rejections.iter().any(|rejection| {
                rejection.host_id != *host
                    || rejection.boot_id != *boot
                    || rejection.plan_id.as_ref() != Some(&current.plan.plan_id)
                    || rejection.checked_form_ids.is_empty()
                    || rejection.checked_form_ids.iter().any(|checked| {
                        !current
                            .plan
                            .forms
                            .iter()
                            .any(|form| &form.form.checked_form_id == checked)
                    })
            })
        {
            return Err(WorkspaceBodyError::StalePlay);
        }
        let plan_sequence = self.next_sequence()?;
        let failure_sequence = plan_sequence
            .checked_add(1)
            .ok_or(WorkspaceBodyError::SequenceExhausted)?;
        let retain_sequence = failure_sequence
            .checked_add(1)
            .ok_or(WorkspaceBodyError::SequenceExhausted)?;
        let wake = current
            .wake
            .body_plan_ready(&current.plan, sign(host, boot, plan_sequence))
            .and_then(|wake| {
                wake.fail_with_rejections(sign(host, boot, failure_sequence), rejections)
            })
            .map_err(WorkspaceBodyError::Lifecycle)?;
        let body = self
            .evidence
            .body
            .retain_after_lull(&wake, sign(host, boot, retain_sequence))
            .map_err(WorkspaceBodyError::Lifecycle)?;
        let mut evidence = self.evidence.clone();
        evidence
            .append_wake(body, wake, plan_sequence)
            .map_err(WorkspaceBodyError::Biography)?;
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
    ) -> Result<(), WorkspaceBodyError> {
        self.require_mutable()?;
        self.require_host(host, boot)?;
        if self.evidence.body.state != BodyState::Lulled || self.realization.is_some() {
            return Err(WorkspaceBodyError::NotLulled);
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
            .map_err(WorkspaceBodyError::Lifecycle)?;
        let mut evidence = self.evidence.clone();
        evidence
            .append_body_lifecycle_events(body, &[(sign_id, sequence)])
            .map_err(WorkspaceBodyError::Biography)?;
        self.evidence = evidence;
        Ok(())
    }

    /// Add checked meaning while Lulled. Current play replacement is a separate
    /// Host-orchestrated lifecycle; this cannot mutate an admitted plan.
    pub fn admit_form(
        &mut self,
        expected_revision: u64,
        form: ResidentForm,
        host: &HostId,
        boot: &BootId,
    ) -> Result<(), WorkspaceBodyError> {
        self.require_mutable()?;
        self.require_host(host, boot)?;
        if self.evidence.body.state != BodyState::Lulled {
            return Err(WorkspaceBodyError::NotLulled);
        }
        if self.evidence.body.workload_revision != expected_revision {
            return Err(WorkspaceBodyError::StaleWorkload);
        }
        self.make_lifecycle_room(1, 0)?;
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

    /// Remove checked meaning only after the host has retired its actual Play.
    /// The body and membership survive even when the last Form is removed.
    pub fn remove_form(
        &mut self,
        expected_revision: u64,
        form: &ResidentForm,
        host: &HostId,
        boot: &BootId,
    ) -> Result<(), WorkspaceBodyError> {
        self.require_mutable()?;
        self.require_host(host, boot)?;
        if self.evidence.body.state != BodyState::Lulled || self.realization.is_some() {
            return Err(WorkspaceBodyError::NotLulled);
        }
        if self.evidence.body.workload_revision != expected_revision {
            return Err(WorkspaceBodyError::StaleWorkload);
        }
        self.make_lifecycle_room(1, 0)?;
        let sequence = self.next_sequence()?;
        let sign_id = sign(host, boot, sequence);
        let body = self
            .evidence
            .body
            .remove_form(form, sign_id.clone())
            .map_err(WorkspaceBodyError::Lifecycle)?;
        let mut evidence = self.evidence.clone();
        evidence
            .append_body_workload_events(body, &[(sign_id, sequence)])
            .map_err(WorkspaceBodyError::Biography)?;
        if self.foreground.as_ref() == Some(form) {
            self.foreground = evidence.body.workset.forms().first().cloned();
        }
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

    fn require_mutable(&self) -> Result<(), WorkspaceBodyError> {
        self.evidence
            .body
            .ensure_mutable()
            .map_err(WorkspaceBodyError::Lifecycle)
    }

    fn next_sequence(&self) -> Result<u64, WorkspaceBodyError> {
        self.evidence
            .last_sequence()
            .checked_add(1)
            .ok_or(WorkspaceBodyError::SequenceExhausted)
    }

    fn make_lifecycle_room(
        &mut self,
        body_signs: usize,
        wakes: usize,
    ) -> Result<(), WorkspaceBodyError> {
        while self.evidence.body.sign_ids.len().saturating_add(body_signs)
            > conduit_body::MAX_BODY_SIGNS
            || self.evidence.wakes.len().saturating_add(wakes)
                > conduit_body::MAX_BODY_BIOGRAPHY_WAKES
            || self.evidence.records.len().saturating_add(5)
                > conduit_body::MAX_BODY_BIOGRAPHY_RECORDS
        {
            if self.pending_archives.len() >= conduit_body::MAX_BODY_BIOGRAPHY_WAKES {
                return Err(WorkspaceBodyError::ArchivePersistenceRequired);
            }
            let segment = self
                .evidence
                .seal_oldest_terminal_wake()
                .map_err(WorkspaceBodyError::Biography)?;
            let segment = match segment {
                Some(segment) => Some(segment),
                None => self
                    .evidence
                    .seal_body_workload_history()
                    .map_err(WorkspaceBodyError::Biography)?,
            };
            let segment = match segment {
                Some(segment) => Some(segment),
                None => self
                    .evidence
                    .seal_membership_history()
                    .map_err(WorkspaceBodyError::Biography)?,
            };
            let Some(segment) = segment else {
                return Err(WorkspaceBodyError::Biography(
                    BodyBiographyError::CapacityExhausted,
                ));
            };
            self.pending_archives.push(segment);
        }
        Ok(())
    }

    fn make_membership_room(&mut self, events: usize) -> Result<(), WorkspaceBodyError> {
        if self.evidence.membership.events.len().saturating_add(events)
            <= conduit_body::MAX_MEMBERSHIP_EVENTS
            && self.evidence.records.len().saturating_add(events)
                <= conduit_body::MAX_BODY_BIOGRAPHY_RECORDS
        {
            return Ok(());
        }
        if self.pending_archives.len() >= conduit_body::MAX_BODY_BIOGRAPHY_WAKES {
            return Err(WorkspaceBodyError::ArchivePersistenceRequired);
        }
        let segment = self
            .evidence
            .seal_membership_history()
            .map_err(WorkspaceBodyError::Biography)?
            .ok_or(WorkspaceBodyError::Biography(
                BodyBiographyError::CapacityExhausted,
            ))?;
        self.pending_archives.push(segment);
        Ok(())
    }
}

fn sign(host: &HostId, boot: &BootId, sequence: u64) -> SignId {
    bind_sign(host, boot, None, sequence).sign_id
}
