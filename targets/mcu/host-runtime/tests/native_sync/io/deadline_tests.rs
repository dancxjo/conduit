//! Pending deadline custody, pressure and cancellation.
use super::*;
#[test]
fn deadline_pending_pressure_completion_and_revocation_keep_exact_lease() {
    for cancel in [false, true] {
        let polls = Rc::new(Cell::new(0));
        let next = Rc::new(Cell::new(None));
        let revoked = Rc::new(Cell::new(false));
        let mut owner = clock(polls.clone(), next.clone(), revoked.clone());
        let fail = Rc::new(Cell::new(false));
        let mut kernel = request_kernel(
            &clock_request(),
            CLOCK_MAXIMUM_BYTES,
            CLOCK_MAXIMUM_BYTES,
            4096,
            "completed",
            fail.clone(),
        );
        kernel.step().unwrap();
        let mut service = deadline::Service::new(&mut owner);
        allocations::without_allocations(|| {
            assert_eq!(service.dispatch(&mut kernel), Ok(Dispatch::Idle))
        });
        assert_eq!(polls.get(), 1);
        assert_eq!(service.bound_owner().inspection().in_flight_operations, 1);
        fail.set(true);
        assert_eq!(service.dispatch(&mut kernel), Ok(Dispatch::Pressure));
        assert_eq!(polls.get(), 1);
        next.set(Some(23));
        fail.set(false);
        if cancel {
            service.revoke().unwrap();
            kernel.cancel().unwrap();
            assert_eq!(service.dispatch(&mut kernel), Ok(Dispatch::Idle));
            assert_eq!(polls.get(), 1);
        } else {
            allocations::without_allocations(|| {
                assert!(matches!(
                    service.dispatch(&mut kernel),
                    Ok(Dispatch::Completed { .. })
                ));
                assert_eq!(service.dispatch(&mut kernel), Ok(Dispatch::Idle));
                kernel.step().unwrap();
            });
            assert_eq!(polls.get(), 2);
            service.revoke().unwrap();
        }
        assert_eq!(service.bound_owner().inspection().in_flight_operations, 0);
        assert!(revoked.get());
    }
}

#[test]
fn deadline_storage_pressure_precedes_start_and_revoked_owner_returns_cancelled() {
    for pressure in [false, true] {
        let polls = Rc::new(Cell::new(0));
        let mut owner = clock(
            polls.clone(),
            Rc::new(Cell::new(Some(23))),
            Rc::new(Cell::new(false)),
        );
        let input = clock_request();
        let mut kernel = request_kernel(
            &input,
            CLOCK_MAXIMUM_BYTES,
            CLOCK_MAXIMUM_BYTES,
            if pressure { input.len() as u32 } else { 4096 },
            "completed",
            Rc::new(Cell::new(false)),
        );
        kernel.step().unwrap();
        let mut service = deadline::Service::new(&mut owner);
        if pressure {
            assert_eq!(service.dispatch(&mut kernel), Ok(Dispatch::Pressure));
            assert_eq!(service.bound_owner().inspection().in_flight_operations, 0);
        } else {
            service.revoke().unwrap();
            let Dispatch::Completed { outcome, .. } = service.dispatch(&mut kernel).unwrap() else {
                panic!("missing cancellation")
            };
            assert_eq!(outcome.disposition, HostCallDisposition::Cancelled);
            assert_eq!(service.dispatch(&mut kernel), Ok(Dispatch::Idle));
        }
        assert_eq!(polls.get(), 0);
        service.revoke().unwrap();
        kernel.cancel().unwrap();
    }
}
