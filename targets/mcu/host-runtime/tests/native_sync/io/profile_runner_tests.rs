//! The real runner accepts a bounded storage profile without changing owners.
use super::*;
use conduit_mcu_host_runtime::native_runner::{Exit, Observer, Refusal as RunRefusal, Runner};

// No active Cords; one queue slot remains required by kernel admission. The
// ordinary fixture reserves two slots. Both original native requests still run.
type Profile = PairKernel<1, 0, 0, 0>;
#[derive(Default)]
struct ProfileObserver {
    requests: [Option<HostCallRequest>; 2],
}
impl Observer<Profile> for ProfileObserver {
    type Error = ();
    fn before_step(&mut self, _: &mut Profile) -> Result<(), ()> {
        Ok(())
    }
    fn completed(&mut self, kernel: &mut Profile, dispatch: Dispatch) -> Result<(), ()> {
        let Dispatch::Completed { request, outcome } = dispatch else {
            return Err(());
        };
        assert_eq!(request.call, HostCallId(0));
        assert_eq!(request.request, RequestId(0));
        let slot = &mut self.requests[usize::from(request.node.0)];
        assert!(
            slot.replace(request).is_none(),
            "a request was dispatched twice"
        );
        assert_eq!(outcome.disposition, HostCallDisposition::Completed);
        assert_eq!(
            c::validate_canonical_structured_value(
                kernel.host_value(outcome.output.unwrap().value).unwrap()
            )
            .unwrap()
            .variant_tag()
            .unwrap(),
            "completed"
        );
        Ok(())
    }
}
#[test]
fn specialized_profile_runs_original_i2c_clock_and_retires_without_allocating() {
    for mode in 0..3 {
        let effects = Rc::new(Cell::new(0));
        let polls = Rc::new(Cell::new(0));
        let bus_revoked = Rc::new(Cell::new(false));
        let clock_revoked = Rc::new(Cell::new(false));
        let (mut bus, mut clock) = paired_owners(
            Bus {
                effects: effects.clone(),
                revoked: bus_revoked.clone(),
            },
            Clock {
                polls: polls.clone(),
                next: Rc::new(Cell::new((mode == 0).then_some(23))),
                revoked: clock_revoked.clone(),
            },
        );
        let mut kernel = pair_kernel_profile::<1, 0, 0, 0>();
        let mut output = [0; I2C_RESULT_VALUE_BYTES];
        let mut bus_service = Service::new(&mut bus, &mut output).unwrap();
        let mut clock_service = deadline::Service::new(&mut clock);
        let mut observer = ProfileObserver::default();
        let activity = NativeActivity::new();
        let cancelled = || mode == 1 && polls.get() > 0;
        let pause = Yield;
        let runner = Runner::new(&mut kernel, &mut observer, &pause, &cancelled);
        let runner = if mode == 2 {
            runner
        } else {
            runner.with_activity(&activity)
        };
        let result = allocations::without_allocations(|| {
            finish(runner.run_with_i2c_clock(&mut bus_service, &mut clock_service))
        });
        match mode {
            0 => {
                assert!(matches!(result, Ok(Exit::Drained)));
                assert!(observer.requests.iter().all(Option::is_some));
                assert_eq!((effects.get(), polls.get()), (1, 1));
            }
            1 => {
                assert!(matches!(
                    result,
                    Err(RunRefusal::Kernel(SchedulerError::Cancelled))
                ));
                assert_eq!(polls.get(), 1);
                assert!(observer.requests[1].is_none());
            }
            _ => {
                assert!(matches!(
                    result,
                    Err(RunRefusal::Synchronous(Refusal::MissingActivity))
                ));
                assert_eq!((effects.get(), polls.get()), (0, 0));
                assert!(observer.requests.iter().all(Option::is_none));
            }
        }
        assert_eq!(bus_revoked.get(), mode != 0);
        assert!(clock_revoked.get());
        for inspection in [
            bus_service.bound_owner().inspection(),
            clock_service.bound_owner().inspection(),
        ] {
            assert_eq!(inspection.lifecycle, c::CapabilityLifecycle::Revoked);
            assert_eq!(inspection.in_flight_operations, 0);
        }
    }
}
