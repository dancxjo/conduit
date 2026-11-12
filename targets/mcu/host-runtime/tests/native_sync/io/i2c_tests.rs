//! Synchronous I2C effects, pressure and rejected completion.
use super::*;
#[test]
fn i2c_preflight_preserves_original_request_then_completes_once_without_allocations() {
    for address in [0x76, 0x50] {
        let effects = Rc::new(Cell::new(0));
        let revoked = Rc::new(Cell::new(false));
        let mut owner = bus(effects.clone(), revoked.clone());
        let input = bus_request(address);
        let fail = Rc::new(Cell::new(false));
        let mut kernel = request_kernel(
            &input,
            I2C_MAXIMUM_BYTES,
            I2C_RESULT_VALUE_BYTES as u32,
            8192,
            if address == 0x76 {
                "completed"
            } else {
                "refused"
            },
            fail.clone(),
        );
        kernel.step().unwrap();
        let mut output = [0; I2C_RESULT_VALUE_BYTES];
        let activity = NativeActivity::new();
        let mut service = Service::new(&mut owner, &mut output).unwrap();
        fail.set(true);
        allocations::without_allocations(|| {
            for _ in 0..8 {
                assert_eq!(
                    service.dispatch(&mut kernel, &activity),
                    Ok(Dispatch::Pressure)
                );
            }
        });
        assert_eq!(effects.get(), 0);
        fail.set(false);
        let timing = activity.try_timing().unwrap();
        assert_eq!(service.dispatch(&mut kernel, &activity), Ok(Dispatch::Idle));
        drop(timing);
        allocations::without_allocations(|| {
            assert!(matches!(
                service.dispatch(&mut kernel, &activity),
                Ok(Dispatch::Completed { .. })
            ));
            assert_eq!(service.dispatch(&mut kernel, &activity), Ok(Dispatch::Idle));
            kernel.step().unwrap();
        });
        assert_eq!(effects.get(), usize::from(address == 0x76));
        assert_eq!(service.bound_owner().inspection().in_flight_operations, 0);
        service.revoke().unwrap();
        assert!(revoked.get());
    }
}
#[test]
fn i2c_value_pressure_and_short_buffer_refuse_before_physical_effects() {
    let effects = Rc::new(Cell::new(0));
    let mut owner = bus(effects.clone(), Rc::new(Cell::new(false)));
    let input = bus_request(0x76);
    let mut kernel = request_kernel(
        &input,
        I2C_MAXIMUM_BYTES,
        I2C_RESULT_VALUE_BYTES as u32,
        input.len() as u32,
        "completed",
        Rc::new(Cell::new(false)),
    );
    kernel.step().unwrap();
    let mut short = [0; I2C_RESULT_VALUE_BYTES - 1];
    assert!(matches!(
        Service::new(&mut owner, &mut short),
        Err(Refusal::InvalidBounds)
    ));
    let mut output = [0; I2C_RESULT_VALUE_BYTES];
    let mut service = Service::new(&mut owner, &mut output).unwrap();
    assert_eq!(
        service.dispatch(&mut kernel, &NativeActivity::new()),
        Ok(Dispatch::Pressure)
    );
    assert_eq!(effects.get(), 0);
    service.revoke().unwrap();
}
#[test]
fn rejected_i2c_completion_never_repeats_the_physical_transaction() {
    let effects = Rc::new(Cell::new(0));
    let revoked = Rc::new(Cell::new(false));
    let fail = Rc::new(Cell::new(false));
    let mut owner = Interrupt {
        inner: bus(effects.clone(), revoked.clone()),
        fail: fail.clone(),
    };
    let mut kernel = request_kernel(
        &bus_request(0x76),
        I2C_MAXIMUM_BYTES,
        I2C_RESULT_VALUE_BYTES as u32,
        8192,
        "completed",
        fail.clone(),
    );
    kernel.step().unwrap();
    let mut output = [0; I2C_RESULT_VALUE_BYTES];
    let mut service = Service::new(&mut owner, &mut output).unwrap();
    let activity = NativeActivity::new();
    assert!(matches!(
        service.dispatch(&mut kernel, &activity),
        Err(Refusal::Kernel(SchedulerError::Sign(_)))
    ));
    assert_eq!(effects.get(), 1);
    fail.set(false);
    assert_eq!(service.dispatch(&mut kernel, &activity), Ok(Dispatch::Idle));
    assert_eq!(effects.get(), 1);
    service.revoke().unwrap();
    kernel.cancel().unwrap();
    assert!(revoked.get());
}
