//! Host-side custody of distinct Body and appliance fragment Play identities.
//! This grants no device/storage resources and is not a start acknowledgement.
use super::{PreparedNativeAppliance, PreparedRecordingRecovery};
use conduit_body::{BodyLifecycleSession, BodyPlayIdentity, MembershipState, WakeLifecycle};
use conduit_core::{bind_active_play, ActivePlayIdentity, FragmentId, Plan};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeBodyBindingRefusal {
    StaleProposal,
    StalePlay,
    WrongFragment,
    StaleHost,
    NotStarted,
    AlreadyConfirmed,
    Revoked,
}

/// Root retains this alongside actual native admission. It does not replace the
/// capability table, trusted resident image, finite storage admission or kernel.
/// There is no Clone/Deserialize shortcut. This is lifecycle correlation, not
/// an exclusive execution claim; actual provider ownership remains independent.
pub struct NativeBodyBinding {
    body_play: BodyPlayIdentity,
    fragment_play: ActivePlayIdentity,
    fragment: FragmentId,
    confirmed: bool,
    revoked: bool,
}
impl NativeBodyBinding {
    /// Root supplies both prospective identities from its own issuers. In
    /// particular it allocates the fragment sequence in the current Host/Boot,
    /// across all Body starts; a Body-local sequence is not a substitute.
    pub fn prepare(
        native: &PreparedNativeAppliance,
        owner: &BodyLifecycleSession,
        prospective_body_play: &BodyPlayIdentity,
        prospective_fragment_play: &ActivePlayIdentity,
    ) -> Result<Self, NativeBodyBindingRefusal> {
        Self::prepare_plan(
            native.plan(),
            owner,
            prospective_body_play,
            prospective_fragment_play,
        )
    }
    /// The recorder uses the same Body/fragment custody gate. Root still owns
    /// storage admission and actual execution; a selected Plan is not a start.
    pub fn prepare_recording(
        recording: &PreparedRecordingRecovery,
        owner: &BodyLifecycleSession,
        prospective_body_play: &BodyPlayIdentity,
        prospective_fragment_play: &ActivePlayIdentity,
    ) -> Result<Self, NativeBodyBindingRefusal> {
        Self::prepare_plan(
            recording.plan(),
            owner,
            prospective_body_play,
            prospective_fragment_play,
        )
    }
    /// The output construction diagnostic uses the same exact Body custody gate.
    /// Neither this binding nor dormant preparation starts its Body.
    pub fn prepare_display(
        display: &crate::PreparedDisplayOutput,
        owner: &BodyLifecycleSession,
        prospective_body_play: &BodyPlayIdentity,
        prospective_fragment_play: &ActivePlayIdentity,
    ) -> Result<Self, NativeBodyBindingRefusal> {
        Self::prepare_plan(
            display.plan(),
            owner,
            prospective_body_play,
            prospective_fragment_play,
        )
    }
    fn prepare_plan(
        plan: &Plan,
        owner: &BodyLifecycleSession,
        prospective_body_play: &BodyPlayIdentity,
        prospective_fragment_play: &ActivePlayIdentity,
    ) -> Result<Self, NativeBodyBindingRefusal> {
        use NativeBodyBindingRefusal as E;
        let [fragment] = plan.fragments.as_slice() else {
            return Err(E::WrongFragment);
        };
        if !conduit_core::verify_plan(plan) {
            return Err(E::WrongFragment);
        }
        let current = owner.realization().ok_or(E::StaleProposal)?;
        if current.play.is_some()
            || current.wake.lifecycle != WakeLifecycle::AwaitingPlan
            || !prospective_body_play.validate_for(&current.plan)
        {
            return Err(E::StaleProposal);
        }
        if prospective_fragment_play.plan_id != plan.plan_id
            || prospective_fragment_play.host_id != fragment.host_id
            || prospective_fragment_play.boot_id != fragment.boot_id
            || *prospective_fragment_play
                != bind_active_play(
                    &plan.plan_id,
                    &fragment.host_id,
                    &fragment.boot_id,
                    prospective_fragment_play.play_sequence,
                )
            || !current.plan.plots.iter().any(|partition| {
                partition.plan == *plan
                    && partition.plot.source_document_id == plan.source_document_id
                    && partition.plot.checked_plot_id == plan.checked_plot_id
            })
        {
            return Err(E::WrongFragment);
        }
        let binding = Self {
            body_play: prospective_body_play.clone(),
            fragment_play: prospective_fragment_play.clone(),
            fragment: fragment.fragment_id.clone(),
            confirmed: false,
            revoked: false,
        };
        binding.current(owner, false)?;
        Ok(binding)
    }
    /// Call only after actual admission/start has been attributed and retained
    /// through the ordinary Body owner. A proposal alone cannot confirm custody.
    pub fn confirm_started(
        &mut self,
        owner: &BodyLifecycleSession,
    ) -> Result<(), NativeBodyBindingRefusal> {
        if self.revoked {
            return Err(NativeBodyBindingRefusal::Revoked);
        }
        if self.confirmed {
            return Err(NativeBodyBindingRefusal::AlreadyConfirmed);
        }
        self.current(owner, true)?;
        self.confirmed = true;
        Ok(())
    }
    /// Both identities stay distinct. The fragment identity is the one used by
    /// the existing native binder and its separately issued local capability.
    /// The caller supplies its retained current owner, never a historical copy.
    /// Current-owner checks allocate nothing; Lull, replan or membership loss
    /// cannot obtain a usable identity through this old custody object.
    pub fn fragment_play<'a>(
        &'a self,
        owner: &'a BodyLifecycleSession,
    ) -> Result<&'a ActivePlayIdentity, NativeBodyBindingRefusal> {
        if self.revoked {
            return Err(NativeBodyBindingRefusal::Revoked);
        }
        if !self.confirmed {
            return Err(NativeBodyBindingRefusal::NotStarted);
        }
        self.current(owner, true)?;
        Ok(&self.fragment_play)
    }
    pub fn body_play<'a>(
        &'a self,
        owner: &'a BodyLifecycleSession,
    ) -> Result<&'a BodyPlayIdentity, NativeBodyBindingRefusal> {
        self.fragment_play(owner)?;
        Ok(&self.body_play)
    }
    /// Root must not step the native kernel before confirmation.
    /// Root must also revoke the real native capabilities and retire execution.
    /// Closing this metadata gate does not claim either effect happened.
    pub fn revoke(&mut self) {
        self.revoked = true;
    }

    fn current(
        &self,
        owner: &BodyLifecycleSession,
        started: bool,
    ) -> Result<(), NativeBodyBindingRefusal> {
        use NativeBodyBindingRefusal as E;
        let current = owner.realization().ok_or(E::StaleProposal)?;
        if current.plan.body_id != self.body_play.body_id
            || current.plan.wake_id != self.body_play.wake_id
            || current.plan.plan_id != self.body_play.plan_id
            || !current.plan.plots.iter().any(|partition| {
                partition.plan.plan_id == self.fragment_play.plan_id
                    && partition.plan.fragments.iter().any(|fragment| {
                        fragment.fragment_id == self.fragment
                            && fragment.host_id == self.fragment_play.host_id
                            && fragment.boot_id == self.fragment_play.boot_id
                    })
            })
        {
            return Err(E::StaleProposal);
        }
        if !owner.evidence().membership.parts.iter().any(|part| {
            part.state == MembershipState::Admitted
                && part.current.as_ref().is_some_and(|host| {
                    host.host_id == self.fragment_play.host_id
                        && host.boot_id == self.fragment_play.boot_id
                })
        }) {
            return Err(E::StaleHost);
        }
        if started {
            if current.wake.lifecycle != WakeLifecycle::Playing || current.play.is_none() {
                return Err(E::NotStarted);
            }
            if current.play.as_ref() != Some(&self.body_play) {
                return Err(E::StalePlay);
            }
        }
        Ok(())
    }
}
