#![cfg(feature = "alloc")]
//! Composition of retained sessions with the existing ordinary native chain.
use conduit_kernel::scheduler::prepared_session::{PreparedKernelSession, ReusableStepBack};
use conduit_kernel::{scheduler::*, *};
#[path = "native_region/fixture.rs"]
#[allow(dead_code)]
mod fixture;
use fixture::*;

impl ReusableStepBack<1> for Driver {
    type Next = Option<ValueRef>;
    fn preflight_next(&self, next: &Self::Next, values: &dyn ValueStorage) -> bool {
        match self {
            Self::Source {
                index,
                count,
                staged,
                ..
            } => {
                index == count
                    && !staged
                    && next.is_some_and(|v| values.get(v).is_ok_and(|b| b.len() == 8))
            }
            Self::Add { fail, .. } => !fail && next.is_none(),
            Self::Sink { count, staged, .. } => *count == 1 && staged.is_none() && next.is_none(),
        }
    }
    fn begin_next(&mut self, next: Self::Next) {
        match self {
            Self::Source {
                values,
                index,
                count,
                staged,
            } => {
                *values = [next, None, None, None];
                *index = 0;
                *count = 1;
                *staged = false;
            }
            Self::Add { bytes, .. } => *bytes = [0; 8],
            Self::Sink {
                values,
                count,
                delay,
                staged,
            } => {
                *values = [0; 4];
                *count = 0;
                *delay = 0;
                *staged = None;
            }
        }
    }
}

#[test]
fn native_bindings_and_value_generations_survive_eight_accepted_invocations() {
    let mut reference = PreparedKernelSession::new(Box::new(prepare(false, 1, 0, None)))
        .unwrap_or_else(|_| panic!("fresh canonical session"));
    let mut native = PreparedKernelSession::new(Box::new(prepare(true, 1, 0, None)))
        .unwrap_or_else(|_| panic!("fresh native session"));
    let reference_address = reference.kernel() as *const _ as usize;
    let native_address = native.kernel() as *const _ as usize;
    for frame in 0..8 {
        let old_inputs: Vec<_> = [&reference, &native]
            .iter()
            .map(|session| match &session.kernel().drivers()[0] {
                Driver::Source { values, .. } => values[0].unwrap(),
                _ => panic!("source"),
            })
            .collect();
        for session in [&mut reference, &mut native] {
            let mut drained = false;
            for _ in 0..10000 {
                if session.step().unwrap() == SchedulerStatus::Drained {
                    drained = true;
                    break;
                }
            }
            assert!(drained);
            assert_eq!(session.kernel().values().used_items(), 0);
            assert_eq!(session.kernel().values().native_used_items(), 0);
            assert_eq!(session.kernel().drivers()[N - 1].result().0[0], frame + 32);
            assert_eq!(session.kernel().drivers()[N - 1].result().1, 1);
        }
        let reference_events = reference.kernel().signs().events().collect::<Vec<_>>();
        let native_events = native.kernel().signs().events().collect::<Vec<_>>();
        assert_eq!(reference_events, native_events);
        assert_eq!(
            reference.kernel().logical_invocations(),
            native.kernel().logical_invocations()
        );
        assert_eq!(
            reference.kernel().values().canonical().stores,
            (frame + 1) * 33
        );
        assert_eq!(native.kernel().values().canonical().stores, (frame + 1) * 2);
        assert_eq!(reference.kernel() as *const _ as usize, reference_address);
        assert_eq!(native.kernel() as *const _ as usize, native_address);
        for (index, session) in [&mut reference, &mut native].into_iter().enumerate() {
            assert!(session.kernel().values().get(old_inputs[index]).is_err());
            if frame == 7 {
                continue;
            }
            let fresh = session
                .store_host_value(&(frame + 1).to_le_bytes())
                .unwrap();
            assert_ne!(fresh, old_inputs[index]);
            assert!(session.kernel().values().get(old_inputs[index]).is_err());
            let mut next = [None; N];
            next[0] = Some(fresh);
            let old = session
                .begin_next(next, evidence())
                .unwrap_or_else(|_| panic!("accepted boundary"));
            assert_eq!(old.signs.events().collect::<Vec<_>>(), reference_events);
            assert!(session.kernel().signs().is_empty());
        }
    }
}
