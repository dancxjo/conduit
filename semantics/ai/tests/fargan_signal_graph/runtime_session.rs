//! Finite fixture controller around the production prepared-kernel lifecycle.
//! Host acceptance and fresh active identities remain distinct from kernel drain.
use super::{backs::Next, execution_kernel::ExecutionKernel, *};
use conduit_kernel::scheduler::prepared_session::DrainedInvocation;
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Prepared,
    Produced,
    Accepted,
    Faulted,
}
pub struct PreparedTrajectory {
    prepared: prepared::PreparedExecution,
    active: ActivePlayIdentity,
    inputs: BTreeMap<String, PreparedStructuredValueValidator>,
    evidence: Vec<HostedSignLog>,
    phase: Phase,
}
impl PreparedTrajectory {
    pub fn new(
        plan: Plan,
        resources: &Resources,
        inputs: BTreeMap<String, Vec<u8>>,
        invocations: usize,
    ) -> Self {
        assert!(
            (1..=96).contains(&invocations),
            "finite trajectory reservation"
        );
        let validators = inputs
            .iter()
            .map(|(name, bytes)| {
                let value = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
                (
                    name.clone(),
                    PreparedStructuredValueValidator::new(value.value_type(), 16384).unwrap(),
                )
            })
            .collect();
        let f = &plan.fragments[0];
        let active = bind_active_play(&f.plan_id, &f.host_id, &f.boot_id, 0);
        let evidence = (1..invocations)
            .map(|_| {
                HostedSignLog::new(32768, 32768 * core::mem::size_of::<KernelEvent>() as u32)
                    .unwrap()
            })
            .collect();
        Self {
            prepared: prepared::prepare(plan, resources, inputs, ExecutionMode::Normal, true, 0),
            active,
            inputs: validators,
            evidence,
            phase: Phase::Prepared,
        }
    }
    pub fn active(&self) -> &ActivePlayIdentity {
        &self.active
    }
    pub fn kernel_address(&self) -> usize {
        (&*self.prepared.scheduler as *const prepared::Kernel) as usize
    }
    pub fn run(&mut self, mode: ExecutionMode) -> Option<EncodedResultAndTiming> {
        if self.phase != Phase::Prepared {
            return None;
        }
        let result = execute_prepared(&mut self.prepared, mode, || {}, || {});
        self.phase = if result.is_some() {
            Phase::Produced
        } else {
            Phase::Faulted
        };
        result
    }
    /// An exact complete output set is acknowledged separately from ownership.
    /// This fixture adapter does not imply ordinary Body integration.
    pub fn accept_outputs(&mut self, active: &ActivePlayIdentity, names: &[String]) -> bool {
        if self.phase != Phase::Produced
            || active != &self.active
            || names.len() != self.prepared.received.len()
        {
            return false;
        }
        let names: std::collections::BTreeSet<_> = names.iter().collect();
        if names.len() != self.prepared.received.len()
            || self.prepared.received.keys().any(|n| !names.contains(n))
        {
            return false;
        }
        self.phase = Phase::Accepted;
        true
    }
    pub fn begin_next(
        &mut self,
        active: ActivePlayIdentity,
        inputs: BTreeMap<String, Vec<u8>>,
    ) -> Result<DrainedInvocation<HostedSignLog>, &'static str> {
        let sequence = self
            .active
            .play_sequence
            .checked_add(1)
            .ok_or("sequence exhausted")?;
        let f = &self.prepared.plan.fragments[0];
        if self.phase != Phase::Accepted
            || active != bind_active_play(&f.plan_id, &f.host_id, &f.boot_id, sequence)
        {
            return Err("unaccepted output or stale context");
        }
        if self.evidence.is_empty() {
            return Err("finite evidence reservation exhausted");
        }
        if inputs.len() != self.inputs.len()
            || self
                .inputs
                .iter()
                .any(|(n, v)| inputs.get(n).is_none_or(|b| v.validate(b).is_err()))
        {
            return Err("input schema changed");
        }
        let ExecutionKernel::Reusable(kernel) = &mut self.prepared.scheduler else {
            unreachable!()
        };
        let mut next = core::array::from_fn(|i| {
            if i < self.prepared.nodes {
                Next::Operation
            } else {
                Next::Inactive
            }
        });
        let mut staged = Vec::with_capacity(self.prepared.sources.len());
        for (node, name, resident) in &self.prepared.sources {
            let result = if let Some(value) = resident {
                kernel.retain_between_invocations(*value).map(|()| *value)
            } else {
                kernel.store_host_value(&inputs[name])
            };
            let value = match result {
                Ok(value) => value,
                Err(_) => {
                    for v in staged {
                        kernel.release_between_invocations(v).unwrap();
                    }
                    return Err("input storage refused");
                }
            };
            staged.push(value);
            next[*node] = Next::Source(value);
        }
        for (i, gear) in f.placements.iter().enumerate() {
            if let Some(received) = gear
                .kind_id
                .as_str()
                .strip_prefix("subframe-fixture/")
                .and_then(|name| self.prepared.received.get(name))
            {
                next[i] = Next::Sink(
                    received
                        .borrow_mut()
                        .take()
                        .expect("complete accepted output"),
                );
            }
        }
        let signs = self.evidence.pop().unwrap();
        match kernel.begin_next(next, signs) {
            Ok(old) => {
                self.active = active;
                self.phase = Phase::Prepared;
                self.prepared.preparation = Duration::ZERO;
                self.prepared.timing.borrow_mut().scheduler_bytes =
                    core::mem::size_of::<prepared::Kernel>();
                Ok(old)
            }
            Err(refused) => {
                self.evidence.push(refused.signs);
                for (i, input) in refused.next.into_iter().enumerate() {
                    if let Next::Sink(bytes) = input {
                        let name = f.placements[i]
                            .kind_id
                            .as_str()
                            .strip_prefix("subframe-fixture/")
                            .unwrap();
                        *self.prepared.received[name].borrow_mut() = Some(bytes);
                    }
                }
                for value in staged {
                    kernel.release_between_invocations(value).unwrap();
                }
                Err("prepared Back refused rearm")
            }
        }
    }
}
