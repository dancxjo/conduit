use super::*;

fn input(capacity: usize) -> ExternalForeInput {
    let mut bytes = Vec::with_capacity(capacity);
    bytes.extend_from_slice(b"exact request");
    ExternalForeInput {
        front_port_id: "request".into(),
        track: conduit_core::ConnectionTrack::Payload,
        bytes,
    }
}

#[test]
fn finite_input_retains_capacity_charge_until_actual_drop_even_after_host() {
    let (mut host, _, _) = fixture(1_048_576);
    let observer = host.kernel_resources.image_memory_observer();
    let before = observer.load(Ordering::Acquire);
    let input = input(4096);
    let pointer = input.bytes.as_ptr();
    let expected = core::mem::size_of::<ExternalForeInput>()
        + input.front_port_id.0.capacity()
        + input.bytes.capacity()
        + ImageMemoryLease::owned_backing_bytes();
    let retained = host.retain_selected_image_fore_input(input).unwrap();
    assert_eq!(retained.input().bytes.as_ptr(), pointer);
    assert_eq!(retained.input().bytes, b"exact request");
    assert_eq!(observer.load(Ordering::Acquire), before + expected);
    drop(host);
    assert!(observer.load(Ordering::Acquire) >= expected);
    drop(retained);
    assert_eq!(observer.load(Ordering::Acquire), 0);
}

#[test]
fn finite_input_pressure_refuses_spare_capacity_without_leaking_charge() {
    let (mut host, _, _) = fixture(1_048_576);
    let observer = host.kernel_resources.image_memory_observer();
    let before = observer.load(Ordering::Acquire);
    // Visible bytes fit easily; allocated spare capacity does not.
    let error = host
        .retain_selected_image_fore_input(input(1_048_576))
        .err()
        .expect("allocated capacity must be charged");
    assert!(error.contains("backing capacity"), "{error}");
    assert_eq!(observer.load(Ordering::Acquire), before);
    let retained = host.retain_selected_image_fore_input(input(64)).unwrap();
    drop(retained);
    assert_eq!(observer.load(Ordering::Acquire), before);
    assert!(StdHost::new()
        .retain_selected_image_fore_input(input(64))
        .is_err());
}

#[test]
fn retained_input_competes_with_execution_and_release_restores_admission() {
    let (host, wake, plan) = fixture(64 * 1024 * 1024);
    let exact = receipt(&host, &wake, &plan);
    let (mut host, wake, plan) = fixture(exact as u64);
    let retained = host.retain_selected_image_fore_input(input(64)).unwrap();
    let control = RunControl::default();
    let mut started = false;
    let result = host.run_selected_image_body_plan_with_output_to_with_start(
        BodyRunRequest {
            wake: &wake,
            plan: &plan,
            control: &control,
            keyboard: None,
        },
        &mut Vec::new(),
        &mut crate::ThreadTimer,
        Some(&mut AcceptOutput),
        |_, _| {
            started = true;
            Ok(())
        },
    );
    assert!(result.is_err());
    assert!(
        !started,
        "input backing must consume the same execution heap"
    );
    assert!(host.kernel_resources.is_idle());
    drop(retained);
    let report = host
        .run_selected_image_body_plan_with_output_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &mut Vec::new(),
            &mut crate::ThreadTimer,
            Some(&mut AcceptOutput),
            |_, _| Ok(()),
        )
        .unwrap();
    assert_eq!(report.report().terminal, TerminalDisposition::Completed);
}

#[test]
fn input_preparation_reserves_before_factory_and_retains_only_result() {
    let (mut host, _, _) = fixture(1_048_576);
    let observer = host.kernel_resources.image_memory_observer();
    let before = observer.load(Ordering::Acquire);
    let peak = 8192;
    // SAFETY: this synchronous fixture allocates a single finite input, has no
    // external effects/workers, and its complete peak is below the envelope.
    let retained = unsafe {
        host.prepare_selected_image_fore_input(peak, || {
            assert_eq!(
                observer.load(Ordering::Acquire),
                before + peak + ImageMemoryLease::owned_backing_bytes()
            );
            Ok(input(4096))
        })
    }
    .unwrap();
    let actual = core::mem::size_of::<ExternalForeInput>()
        + retained.input().front_port_id.0.capacity()
        + retained.input().bytes.capacity()
        + ImageMemoryLease::owned_backing_bytes();
    assert_eq!(observer.load(Ordering::Acquire), before + actual);
    drop(host);
    assert_eq!(observer.load(Ordering::Acquire), actual);
    drop(retained);
    assert_eq!(observer.load(Ordering::Acquire), 0);
}

#[test]
fn input_preparation_refusal_and_failure_release_custody_without_publication() {
    let (mut host, _, _) = fixture(1_048_576);
    let observer = host.kernel_resources.image_memory_observer();
    let before = observer.load(Ordering::Acquire);
    for bytes in [0, usize::MAX, 1_048_576] {
        // SAFETY: the invalid envelope must refuse without running this factory.
        let result = unsafe {
            host.prepare_selected_image_fore_input(bytes, || panic!("unadmitted factory"))
        };
        assert!(result.is_err());
        assert_eq!(observer.load(Ordering::Acquire), before);
    }
    // SAFETY: no allocation, external effects or outstanding workers.
    let failed =
        unsafe { host.prepare_selected_image_fore_input(8192, || Err("capture refused".into())) };
    assert_eq!(failed.err().unwrap(), "capture refused");
    assert_eq!(observer.load(Ordering::Acquire), before);
    // Adversarial factory violates its result bound. No input may escape even
    // though Root's independent peak qualification was wrong.
    let overflow = unsafe { host.prepare_selected_image_fore_input(64, || Ok(input(4096))) };
    assert!(overflow.err().unwrap().contains("exceeds admitted"));
    assert_eq!(observer.load(Ordering::Acquire), before);
    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        host.prepare_selected_image_fore_input(8192, || panic!("quiescent fixture panic"))
    }));
    assert!(unwound.is_err());
    assert_eq!(observer.load(Ordering::Acquire), before);
    assert!(host.kernel_resources.is_idle());
}

#[test]
fn retained_input_rejects_another_ledger_even_with_identical_host_labels() {
    let (mut owner, wake, plan) = fixture(64 * 1024 * 1024);
    let (mut other, _, _) = fixture(64 * 1024 * 1024);
    assert_eq!(owner.advertisement().host_id, other.advertisement().host_id);
    assert_eq!(owner.advertisement().boot_id, other.advertisement().boot_id);
    let retained = other.retain_selected_image_fore_input(input(64)).unwrap();
    other.verify_selected_image_fore_input(&retained).unwrap();
    assert!(owner.verify_selected_image_fore_input(&retained).is_err());
    let control = RunControl::default();
    let mut started = false;
    let error = owner
        .run_selected_image_body_plan_with_retained_input_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &retained,
            &mut Vec::new(),
            &mut crate::ThreadTimer,
            &mut AcceptOutput,
            |_, _| {
                started = true;
                Ok(())
            },
        )
        .err()
        .expect("foreign backing cannot fund this IMAGE");
    assert!(error.contains("another memory owner"));
    assert!(!started);
    drop(owner);
    other.verify_selected_image_fore_input(&retained).unwrap();
}
