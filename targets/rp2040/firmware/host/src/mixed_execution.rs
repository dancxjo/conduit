//! One synchronous Root Start decision, then bounded production-kernel steps.
use super::*;
use crate::native_preparation::NativeDispatch;
use conduit_kernel::scheduler::{SchedulerError, SchedulerStatus};

pub struct StartedMixedWorkset<'w, 'o, P> {
    committed: CommittedMixedWorkset<'w, 'o, P>,
}
pub struct MixedStep {
    pub status: SchedulerStatus,
    pub dispatch: NativeDispatch,
}
pub struct RetiredMixedWorkset<'w, P> {
    pub approved: b::DelegatedBodyFragment<'w>,
    pub terminal: crate::mixed_preparation::RetiredMixedKernel<P>,
    pub stamp: b::RetainedHostCommandStamp,
}
impl<'w, 'o, P: I2cProvider> CommittedMixedWorkset<'w, 'o, P> {
    /// Refusal retains the exact committed kernel, providers and reservations.
    /// Success is one local transition: no await, IO, input grant or step occurs.
    /// Attribution expiry after this transition is not a lease on an active Play.
    #[allow(
        clippy::result_large_err,
        reason = "refusal retains original admitted custody"
    )]
    pub fn start<const N: usize>(
        self,
        workspace: &b::HostCommandInput<N>,
        policy: Policy<'_>,
        context: b::NativePreparationContext<'_>,
        resources: &c::ResourceAdmissionOwner,
    ) -> Result<StartedMixedWorkset<'w, 'o, P>, (Self, Refusal)> {
        if let Err(reason) = self.check_start(workspace, policy, context, resources) {
            return Err((self, reason));
        }
        Ok(StartedMixedWorkset { committed: self })
    }
    fn check_start<const N: usize>(
        &self,
        workspace: &b::HostCommandInput<N>,
        policy: Policy<'_>,
        context: b::NativePreparationContext<'_>,
        resources: &c::ResourceAdmissionOwner,
    ) -> Result<(), Refusal> {
        let command = workspace
            .reborrow(
                context.member,
                context.actual,
                &context.bootstrap.owner,
                b::HOST_PREPARED_WORKSET_CONTRACT,
                context.now_millis,
            )
            .map_err(Refusal::Retention)?;
        self.stamp
            .check_command(&command)
            .map_err(Refusal::Retention)?;
        let workset = b::receive_delegated_body_workset::<1>(policy.preparation, context, command)
            .map_err(Refusal::Workset)?;
        workset
            .check_execution_delegation(
                policy.execution,
                policy.preparation,
                context,
                policy.active_worksets,
            )
            .map_err(Refusal::Execution)?;
        let m = self.approved.metadata();
        if resources.host() != context.actual
            || resources.admissions().len() != 2
            || m.body != context.member.body_id.as_str()
            || m.host != context.actual.host_id.as_str()
            || m.boot != context.actual.boot_id.as_str()
            || m.offer_generation != context.actual.offer_generation.0
            || self.approved.receiver_profile()
                != c::AssignedIdentity::from_text(context.actual.profile.as_str())
        {
            return Err(Refusal::Resources(b::DelegatedResourceRefusal::Scope));
        }
        // This bounded profile has one distinct resource-bearing placement per
        // node. Match exact ledger items, not just the existence of a Plan ID.
        let mut count = 0;
        for resource in b::PlainFragmentResources::new(m.resource_bindings) {
            let resource = resource
                .map_err(|_| Refusal::Resources(b::DelegatedResourceRefusal::Projection))?;
            let matches = resources
                .admissions()
                .iter()
                .filter(|a| {
                    a.plan_id.as_str() == m.body_plan
                        && a.placement_id.as_str() == resource.placement
                        && a.host_id.as_str() == m.host
                        && a.boot_id.as_str() == m.boot
                        && a.offer_generation.0 == m.offer_generation
                        && a.items.len() == 1
                        && a.items.iter().all(|item| {
                            item.binding.pool_id.as_str() == resource.pool
                                && item.binding.class_id.as_str() == resource.class
                                && item.binding.units == resource.units
                                && item.requirement.class_id.as_str() == resource.class
                                && item.requirement.units == resource.units
                        })
                })
                .count();
            if matches != 1 {
                return Err(Refusal::Resources(b::DelegatedResourceRefusal::Scope));
            }
            count += 1;
        }
        if count != 2 {
            return Err(Refusal::Resources(b::DelegatedResourceRefusal::Scope));
        }
        Ok(())
    }
}
impl<'w, 'o, P: I2cProvider> StartedMixedWorkset<'w, 'o, P> {
    /// One existing scheduler decision and at most one already pending native
    /// dispatch. No loop, carrier dependency, implicit retry or new authority.
    pub fn step(&mut self) -> Result<MixedStep, SchedulerError> {
        let status = self.committed.bound.kernel.step()?;
        let dispatch = self.committed.bound.dispatch_one()?;
        Ok(MixedStep { status, dispatch })
    }
    /// Root's admitted ingress/egress controller uses the original kernel. This
    /// access creates no input or Line permission; those remain caller duties.
    pub(crate) fn bound_mut(&mut self) -> &mut BoundMixedKernel<'o, P> {
        &mut self.committed.bound
    }

    #[allow(
        clippy::result_large_err,
        reason = "failure retains exact running custody"
    )]
    pub fn cancel_and_retire(self) -> Result<RetiredMixedWorkset<'w, P>, (Self, SchedulerError)> {
        let CommittedMixedWorkset {
            approved,
            bound,
            stamp,
        } = self.committed;
        match bound.cancel_and_retire() {
            Ok(terminal) => Ok(RetiredMixedWorkset {
                approved,
                terminal,
                stamp,
            }),
            Err((bound, reason)) => Err((
                Self {
                    committed: CommittedMixedWorkset {
                        approved,
                        bound,
                        stamp,
                    },
                },
                reason,
            )),
        }
    }
}
