//! Mixed profile ownership for the same persistent Root supervision loop.
use crate::{
    execution_supervisor::OwnedWorkset,
    mixed_workset::{
        self as m, CommittedMixedWorkset, Policy, RetiredMixedWorkset, StartedMixedWorkset,
    },
    native_preparation::{NativeDispatch, NativeDispatchRefusal},
    transport::LIMIT,
};
use conduit_body as b;
use conduit_core as c;
use conduit_i2c_transaction::{owner::I2cHostCall, I2cProvider};
use conduit_kernel::{
    scheduler::{SchedulerError, SchedulerStatus},
    FixedSignLog, KernelEvent,
};

pub struct MixedWorkset<'w, 'o, 'p, P> {
    state: Option<State<'w, 'o, P>>,
    policy: Policy<'p>,
}
enum State<'w, 'o, P> {
    Prepared(CommittedMixedWorkset<'w, 'o, P>),
    Started(StartedMixedWorkset<'w, 'o, P>),
    Terminal(Terminal<'w, P>),
}
struct Terminal<'w, P> {
    approved: b::DelegatedBodyFragment<'w>,
    signs: FixedSignLog<16>,
    native: Option<I2cHostCall<P>>,
    residents_returned: bool,
    _stamp: b::RetainedHostCommandStamp,
}
#[derive(Debug)]
pub enum StepRefusal {
    Kernel(SchedulerError),
    Native(NativeDispatchRefusal),
}
impl<'w, 'o, 'p, P> MixedWorkset<'w, 'o, 'p, P> {
    pub fn new(committed: CommittedMixedWorkset<'w, 'o, P>, policy: Policy<'p>) -> Self {
        Self {
            state: Some(State::Prepared(committed)),
            policy,
        }
    }
}
impl<'w, 'o, 'p, P> MixedWorkset<'w, 'o, 'p, P> {
    /// Handoff of an already authorized, started workset; no new Start occurs.
    pub fn from_started(started: StartedMixedWorkset<'w, 'o, P>, policy: Policy<'p>) -> Self {
        Self {
            state: Some(State::Started(started)),
            policy,
        }
    }
}
impl<P: I2cProvider> OwnedWorkset for MixedWorkset<'_, '_, '_, P> {
    type StartError = m::Refusal;
    type StepError = StepRefusal;
    fn is_started(&self) -> bool {
        matches!(self.state, Some(State::Started(_)))
    }
    fn start(
        &mut self,
        input: &b::HostCommandInput<LIMIT>,
        context: b::NativePreparationContext<'_>,
        resources: &c::ResourceAdmissionOwner,
    ) -> Result<(), Self::StartError> {
        let state = self.state.take().expect("retained workset state");
        let State::Prepared(committed) = state else {
            self.state = Some(state);
            return Err(m::Refusal::Bounds);
        };
        match committed.start(input, self.policy, context, resources) {
            Ok(started) => {
                self.state = Some(State::Started(started));
                Ok(())
            }
            Err((committed, reason)) => {
                self.state = Some(State::Prepared(committed));
                Err(reason)
            }
        }
    }
    fn step(&mut self) -> Result<bool, Self::StepError> {
        let Some(State::Started(started)) = self.state.as_mut() else {
            return Err(StepRefusal::Kernel(SchedulerError::InvalidPlan));
        };
        let step = started.step().map_err(StepRefusal::Kernel)?;
        if let NativeDispatch::Refused(reason) = step.dispatch {
            return Err(StepRefusal::Native(reason));
        }
        Ok(matches!(
            step.status,
            SchedulerStatus::Drained | SchedulerStatus::Cancelled
        ))
    }
    fn retire(
        &mut self,
        resources: &mut c::ResourceAdmissionOwner,
        cause: c::ResourceReleaseCause,
    ) -> bool {
        let state = self.state.take().expect("retained workset state");
        let retired = match state {
            State::Prepared(CommittedMixedWorkset {
                approved,
                bound,
                stamp,
            }) => match bound.cancel_and_retire() {
                Ok(terminal) => RetiredMixedWorkset {
                    approved,
                    terminal,
                    stamp,
                },
                Err((bound, _)) => {
                    self.state = Some(State::Prepared(CommittedMixedWorkset {
                        approved,
                        bound,
                        stamp,
                    }));
                    return false;
                }
            },
            State::Started(started) => match started.cancel_and_retire() {
                Ok(retired) => retired,
                Err((started, _)) => {
                    self.state = Some(State::Started(started));
                    return false;
                }
            },
            State::Terminal(terminal) => {
                self.state = Some(State::Terminal(terminal));
                return self.release_terminal(resources, cause);
            }
        };
        self.state = Some(State::Terminal(Terminal {
            approved: retired.approved,
            signs: retired.terminal.signs,
            native: Some(retired.terminal.call),
            residents_returned: retired.terminal.residents_returned,
            _stamp: retired.stamp,
        }));
        self.release_terminal(resources, cause)
    }
    fn terminal_count(&self) -> usize {
        usize::from(matches!(self.state, Some(State::Terminal(_))))
    }
    fn terminal_event(&self, fragment: usize, event: usize) -> Option<KernelEvent> {
        if fragment != 0 {
            return None;
        }
        let State::Terminal(t) = self.state.as_ref()? else {
            return None;
        };
        t.signs.events().nth(event)
    }
}
impl<'w, 'o, 'p, P: I2cProvider> MixedWorkset<'w, 'o, 'p, P> {
    pub(crate) fn bound_mut(
        &mut self,
    ) -> Option<&mut crate::mixed_preparation::BoundMixedKernel<'o, P>> {
        match self.state.as_mut()? {
            State::Prepared(c) => Some(&mut c.bound),
            State::Started(s) => Some(s.bound_mut()),
            State::Terminal(_) => None,
        }
    }
    fn release_terminal(
        &mut self,
        resources: &mut c::ResourceAdmissionOwner,
        cause: c::ResourceReleaseCause,
    ) -> bool {
        let Some(State::Terminal(t)) = self.state.as_mut() else {
            return false;
        };
        if !t.residents_returned {
            return false;
        }
        if t.native.is_some() {
            let Ok((provider, _)) = I2cHostCall::finish_retained(&mut t.native) else {
                return false;
            };
            drop(provider);
        }
        crate::retired_resources::release(&[Some(&t.approved)], resources, cause)
    }
}
