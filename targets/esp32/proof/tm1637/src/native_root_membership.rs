//! Affine live Root handoff through the existing cooperative flash domain.
//! Terminal failures retain the actual owner and accepted transport custody.
use super::NativeRoot;
use crate::native_body_join::membership::job::{Job, Phase};
use alloc::boxed::Box;
pub(super) struct Membership {
    accepted: crate::native_body_join_http::Accepted,
    job: Option<Box<Job>>,
    queued: Option<u64>,
    failure: Option<&'static str>,
}
impl NativeRoot {
    pub(crate) fn membership_busy(&self) -> bool {
        self.membership.is_some()
    }
    pub(crate) fn membership_ready(&self) -> bool {
        !self.membership_busy()
            && self.body.is_some()
            && !self.local_renewal_pending()
            && !self.network_write_inflight()
            && self.pending_network_workspace.is_none()
            && self.observed_offer.is_none()
            && self.observed_reclaim.is_none()
            && self
                .reclaim_submitter
                .as_ref()
                .is_some_and(|s| s.membership_ready())
            && crate::native_restart::publication_ready()
    }
    pub(crate) fn start_membership(
        &mut self,
        accepted: crate::native_body_join_http::Accepted,
        request: conduit_body::PortableMembershipAdmissionRequest,
    ) -> Result<(), crate::native_body_join_http::Accepted> {
        if !self.membership_ready() {
            return Err(accepted);
        }
        // Fallible backing is reserved before removing the actual Body.
        let raw = unsafe { alloc::alloc::alloc(core::alloc::Layout::new::<Membership>()) }
            .cast::<core::mem::MaybeUninit<Membership>>();
        if raw.is_null() {
            return Err(accepted);
        }
        let backing = unsafe { Box::from_raw(raw) };
        let mut host = alloc::string::String::new();
        let mut boot = alloc::string::String::new();
        if host.try_reserve_exact(self.host.as_str().len()).is_err()
            || boot.try_reserve_exact(self.boot.as_str().len()).is_err()
        {
            return Err(accepted);
        }
        host.push_str(self.host.as_str());
        boot.push_str(self.boot.as_str());
        let owner = self.body.take().expect("checked original Body");
        let job = match Job::reserve(
            owner,
            self.admission.verifying_key(),
            host.into(),
            boot.into(),
            request,
        ) {
            Ok(job) => job,
            Err((owner, _, _, _, _)) => {
                self.body = Some(owner);
                return Err(accepted);
            }
        };
        self.membership = Some(Box::write(
            backing,
            Membership {
                accepted,
                job: Some(job),
                queued: None,
                failure: None,
            },
        ));
        self.advance_membership();
        Ok(())
    }
    pub(crate) fn advance_membership(&mut self) {
        let Some(mut state) = self.membership.take() else {
            return;
        };
        if state.failure.is_some() {
            self.membership = Some(state);
            return;
        }
        let Some(submitter) = self.reclaim_submitter.as_mut() else {
            self.membership = Some(state);
            return;
        };
        if let Some(id) = state.queued {
            let done = match submitter.take_membership(id) {
                Ok(done) => done,
                Err(_) => {
                    self.membership = Some(state);
                    return;
                }
            };
            state.queued = None;
            state.job = Some(done.owner);
            if let Err(reason) = done.result {
                let job = state.job.take().expect("completed affine owner");
                match job.release_refused() {
                    Ok((owner, refusal)) => {
                        self.body = Some(owner);
                        crate::native_body_join_http::complete_accepted(
                            state.accepted,
                            Err(refusal),
                        );
                        return;
                    }
                    Err(job) => {
                        state.job = Some(job);
                        state.failure = Some(reason);
                    }
                }
                crate::native_body_join_http::retained_failure(&state.accepted, reason);
                self.membership = Some(state);
                return;
            }
        }
        let job = state.job.take().expect("Root or queue owns membership");
        if job.phase()
            == Phase::Checkpoint(crate::native_restart::owned_checkpoint::Phase::Complete)
        {
            match job.finish() {
                Ok((owner, admitted)) => {
                    // Restore identical Body before signing, in this synchronous turn.
                    self.body = Some(owner);
                    let result = self
                        .admission
                        .sign_membership(&admitted)
                        .map(|signature| admitted.signed(signature));
                    crate::native_body_join_http::complete_membership(state.accepted, result);
                    return;
                }
                Err(job) => {
                    state.job = Some(job);
                    state.failure = Some("membership-adopt");
                }
            }
        } else {
            match submitter.submit_membership(job) {
                Ok(id) => state.queued = Some(id),
                Err((_, job)) => state.job = Some(job),
            }
        }
        if let Some(reason) = state.failure {
            crate::native_body_join_http::retained_failure(&state.accepted, reason);
        }
        self.membership = Some(state);
    }
    pub(crate) fn membership_status(&self) -> &'static str {
        if let Some(state) = &self.membership {
            return state.failure.unwrap_or("membership-checkpoint-pending");
        }
        if !self
            .reclaim_submitter
            .as_ref()
            .is_some_and(|s| s.membership_preparation_qualified())
        {
            "membership-preparation-unqualified"
        } else {
            "membership-ready"
        }
    }
}
