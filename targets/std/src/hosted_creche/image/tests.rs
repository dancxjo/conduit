use super::*;
use crate::hosted_creche::test_support::{planned_on_host, Adapter};
use std::sync::atomic::Ordering;

fn fixture() -> (StdHost, Adapter, BodyPlan, BodyPlayIdentity) {
    let adapter = Adapter::new();
    let mut host = StdHost::new_with_creche_encounter(
        crate::StdHostConfig {
            host_id: "host/image-creche-context".into(),
            boot_id: "boot/image-creche-context".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        crate::StdHostComposition::minimal(),
        adapter.resource_offer(),
    )
    .unwrap();
    // Isolated memory-lifetime fixture, not selected IMAGE construction or
    // qualification of Crèche execution. The normal runner still refuses it.
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = conduit_host_make::check_host_configuration(
        conduit_host_make::parse_host_configuration_conduit(include_str!(
            "../../../profiles/raspberry-pi-3-tick-workset.host.conduit"
        ))
        .unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let image = conduit_host_make::build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &conduit_host_make::BuildInputs {
            source_identity: "fixture/image-creche-context".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0;
    host.kernel_resources
        .bind_image_bounds(&image.manifest.bounds)
        .unwrap();
    let (_, plan, play, _) = planned_on_host(host.advertisement().clone());
    (host, adapter, plan, play)
}

#[test]
fn image_creche_context_retains_original_charge_and_does_not_enter_adapter() {
    let (mut host, adapter, plan, play) = fixture();
    let used = host.kernel_resources.image_memory_observer();
    let before = used.load(Ordering::Acquire);
    // SAFETY: fixed software fixture context fits this conservative envelope;
    // no external operation or adapter method is entered by preparation.
    let prepared = unsafe {
        host.prepare_selected_image_creche_context(
            &plan,
            &play,
            adapter.resource_offer(),
            adapter.limits(),
            4 * 1024 * 1024,
            2 * 1024 * 1024,
        )
    }
    .unwrap();
    host.verify_selected_image_prepared_value(&prepared)
        .unwrap();
    let (other, _, _, _) = fixture();
    assert!(other
        .verify_selected_image_prepared_value(&prepared)
        .is_err());
    assert!(used.load(Ordering::Acquire) >= before + 2 * 1024 * 1024);
    assert_eq!(prepared.value().host(), host.advertisement());
    assert_eq!(prepared.value().plan(), &plan);
    drop(host);
    assert_eq!(prepared.value().play(), &play);
    assert_eq!((adapter.begins, adapter.polls, adapter.drains), (0, 0, 0));
    drop(prepared);
    assert_eq!(used.load(Ordering::Acquire), before);
}

#[test]
fn image_creche_context_is_borrowed_by_the_real_owner_without_a_second_copy() {
    let (mut host, mut adapter, plan, play) = fixture();
    // SAFETY: fixed context only; the fixture adapter retains its separate
    // bounded storage and has no asynchronous worker or external effects.
    let prepared = unsafe {
        host.prepare_selected_image_creche_context(
            &plan,
            &play,
            adapter.resource_offer(),
            adapter.limits(),
            4 * 1024 * 1024,
            2 * 1024 * 1024,
        )
    }
    .unwrap();
    let used = host.kernel_resources.image_memory_observer();
    let charged = used.load(Ordering::Acquire);
    let (other, _, _, _) = fixture();
    assert!(CrecheHost::prepare_from_image_context(&other, &mut adapter, &prepared).is_err());
    // A counter mismatch must not enter or cancel an untouched adapter.
    assert_eq!((adapter.begins, adapter.polls, adapter.drains), (0, 0, 0));
    // SAFETY: oversized reservation refuses before entering the adapter.
    assert!(unsafe {
        host.prepare_selected_image_creche_owner(
            &mut adapter,
            &prepared,
            65 * 1024 * 1024,
            256 * 1024,
        )
    }
    .is_err());
    assert_eq!(used.load(Ordering::Acquire), charged);
    assert_eq!((adapter.begins, adapter.polls, adapter.drains), (0, 0, 0));
    // SAFETY: fixed fixture lifecycle and request serialization fit this
    // independent envelope; the adapter has no worker or external effects.
    let prepared_owner = unsafe {
        host.prepare_selected_image_creche_owner(&mut adapter, &prepared, 1024 * 1024, 256 * 1024)
    }
    .unwrap();
    let owner_charge = used.load(Ordering::Acquire);
    assert!(owner_charge >= charged + 256 * 1024);
    let mut owner = prepared_owner.value().borrow_mut();
    assert!(owner.verify_image_memory(&other).is_err());
    owner.verify_image_memory(&host).unwrap();
    assert!(core::ptr::eq(owner.context(), prepared.value()));
    let request = owner.request();
    assert!(owner.begin(request).is_err());
    owner.confirm_started(host.advertisement(), &play).unwrap();
    assert_eq!(owner.begin(request), Ok(CrechePoll::Pending));
    assert_eq!(used.load(Ordering::Acquire), owner_charge);
    owner.retire().unwrap();
    drop(owner);
    drop(prepared_owner);
    assert_eq!((adapter.begins, adapter.drains), (1, 1));
    assert_eq!(used.load(Ordering::Acquire), charged);
    drop(prepared);
    assert_eq!(used.load(Ordering::Acquire), 0);
}

#[test]
fn image_creche_context_refuses_pressure_and_stale_plan_without_retention() {
    let (mut host, adapter, plan, play) = fixture();
    let used = host.kernel_resources.image_memory_observer();
    let before = used.load(Ordering::Acquire);
    // SAFETY: pressure refuses before construction. The invalid prospective
    // identity has only bounded fixture validation and retains no context.
    unsafe {
        assert!(host
            .prepare_selected_image_creche_context(
                &plan,
                &play,
                adapter.resource_offer(),
                adapter.limits(),
                65 * 1024 * 1024,
                2 * 1024 * 1024,
            )
            .err()
            .unwrap()
            .contains("backing capacity"));
        assert!(host
            .prepare_selected_image_creche_context(
                &plan,
                &play,
                adapter.resource_offer(),
                adapter.limits(),
                usize::MAX,
                2 * 1024 * 1024,
            )
            .is_err());
        assert!(host
            .prepare_selected_image_creche_context(
                &plan,
                &BodyPlayIdentity::bind(&plan, 1),
                adapter.resource_offer(),
                CrecheLimits {
                    maximum_polls: 0,
                    ..adapter.limits()
                },
                4 * 1024 * 1024,
                2 * 1024 * 1024,
            )
            .is_err());
    }
    let mut stale = play.clone();
    stale.plan_id = "plan/stale-creche".into();
    // SAFETY: fixed invalid fixture context is checked within these bounds.
    assert!(unsafe {
        host.prepare_selected_image_creche_context(
            &plan,
            &stale,
            adapter.resource_offer(),
            adapter.limits(),
            4 * 1024 * 1024,
            2 * 1024 * 1024,
        )
    }
    .is_err());
    assert_eq!(used.load(Ordering::Acquire), before);
    assert_eq!((adapter.begins, adapter.polls, adapter.drains), (0, 0, 0));
}
#[test]
fn funded_creche_executes_through_selected_kernel_and_retains_result_charge() {
    use crate::body_execution::{BodyForeOutputAdapter, BodyRunRequest};
    struct Accept;
    impl BodyForeOutputAdapter for Accept {
        fn deliver(&mut self, _: &crate::ExternalForeDelivery) -> Result<(), String> {
            Ok(())
        }
    }
    let (mut host, mut adapter, _, _) = fixture();
    let (_, plan, play, wake) = planned_on_host(host.advertisement().clone());
    // SAFETY: bounded software-only adapter and all outputs fit the retained
    // envelope; no worker, device or deployment effect is present. Fixture
    // construction is not evidence of selected production Host initialization.
    let external =
        unsafe { host.prepare_selected_image_value(1024 * 1024, 1024 * 1024, || Ok(())) }.unwrap();
    let context = unsafe {
        host.prepare_selected_image_creche_context(
            &plan,
            &play,
            adapter.resource_offer(),
            adapter.limits(),
            4 * 1024 * 1024,
            2 * 1024 * 1024,
        )
    }
    .unwrap();
    let owner = unsafe {
        host.prepare_selected_image_creche_owner(&mut adapter, &context, 1024 * 1024, 256 * 1024)
    }
    .unwrap();
    let request = owner.value().borrow().request();
    let input = unsafe {
        host.prepare_selected_image_fore_input(4096, || {
            Ok(crate::ExternalForeInput {
                front_port_id: "request".into(),
                track: conduit_core::ConnectionTrack::Payload,
                bytes: request.0.to_vec(),
            })
        })
    }
    .unwrap();
    let used = host.kernel_resources.image_memory_observer();
    let before = used.load(Ordering::Acquire);
    let control = crate::RunControl::default();
    let report = unsafe {
        host.run_selected_image_body_plan_with_creche_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &input,
            &owner,
            &mut Vec::new(),
            &mut crate::ThreadTimer,
            &mut Accept,
            |issued, _| {
                assert_eq!(issued, &play);
                Ok(())
            },
        )
    }
    .unwrap();
    assert_eq!(
        report.report().terminal,
        conduit_core::TerminalDisposition::Completed,
        "failure={:?}, cleanup={:?}",
        report.report().failure,
        report.report().cleanup_failure
    );
    assert_eq!(report.report().fore_deliveries.len(), 1);
    assert!(used.load(Ordering::Acquire) > before);
    let outcome = conduit_body_make::deployment_runtime::CrecheOutcome::decode(
        &report.report().fore_deliveries[0].bytes,
    )
    .unwrap();
    assert_eq!(outcome.request, request);
    drop(report);
    assert_eq!(used.load(Ordering::Acquire), before);
    drop(owner);
    assert_eq!((adapter.begins, adapter.polls, adapter.drains), (1, 1, 1));
    drop(input);
    drop(context);
    drop(external);
    assert_eq!(used.load(Ordering::Acquire), 0);
}
#[test]
fn selected_creche_preparation_refusal_drains_and_quarantines_uncertainty() {
    use crate::body_execution::{BodyForeOutputAdapter, BodyRunRequest};
    struct NoOutput;
    impl BodyForeOutputAdapter for NoOutput {
        fn deliver(&mut self, _: &crate::ExternalForeDelivery) -> Result<(), String> {
            panic!("preparation refusal cannot deliver")
        }
    }
    for uncertain in [false, true] {
        let (mut host, mut adapter, _, _) = fixture();
        adapter.fail_drain = uncertain;
        let (_, plan, play, wake) = planned_on_host(host.advertisement().clone());
        // SAFETY: bounded software fixture, no worker/device effects; the
        // deliberate failed drain must leave this Host unavailable.
        let external =
            unsafe { host.prepare_selected_image_value(1024 * 1024, 1024 * 1024, || Ok(())) }
                .unwrap();
        let context = unsafe {
            host.prepare_selected_image_creche_context(
                &plan,
                &play,
                adapter.resource_offer(),
                adapter.limits(),
                4 * 1024 * 1024,
                2 * 1024 * 1024,
            )
        }
        .unwrap();
        let owner = unsafe {
            host.prepare_selected_image_creche_owner(
                &mut adapter,
                &context,
                1024 * 1024,
                256 * 1024,
            )
        }
        .unwrap();
        let input = unsafe {
            host.prepare_selected_image_fore_input(4096, || {
                Ok(crate::ExternalForeInput {
                    front_port_id: "not-the-selected-request".into(),
                    track: conduit_core::ConnectionTrack::Payload,
                    bytes: owner.value().borrow().request().0.to_vec(),
                })
            })
        }
        .unwrap();
        let control = crate::RunControl::default();
        let error = unsafe {
            host.run_selected_image_body_plan_with_creche_to_with_start(
                BodyRunRequest {
                    wake: &wake,
                    plan: &plan,
                    control: &control,
                    keyboard: None,
                },
                &input,
                &owner,
                &mut Vec::new(),
                &mut crate::ThreadTimer,
                &mut NoOutput,
                |_, _| panic!("refusal must precede Play"),
            )
        }
        .err()
        .expect("wrong Fore must refuse");
        assert_eq!(
            host.kernel_resources.require_available().is_err(),
            uncertain
        );
        assert_eq!(error.contains("pre-start retirement"), uncertain);
        drop(owner);
        assert_eq!((adapter.begins, adapter.polls), (0, 0));
        assert!(adapter.drains >= 1);
        drop(external);
    }
}

#[test]
fn owner_preparation_failed_drain_quarantines_before_any_execution_reservation() {
    for uncertain in [false, true] {
        let (mut host, mut adapter, plan, play) = fixture();
        // SAFETY: fixed immutable CPU fixture only; no effect or worker starts.
        let context = unsafe {
            host.prepare_selected_image_creche_context(
                &plan,
                &play,
                adapter.resource_offer(),
                adapter.limits(),
                4 * 1024 * 1024,
                2 * 1024 * 1024,
            )
        }
        .unwrap();
        let used = host.kernel_resources.image_memory_observer();
        let before = used.load(Ordering::Acquire);
        adapter.limits.maximum_polls += 1; // differs from the exact prepared context
        adapter.fail_drain = uncertain;
        // SAFETY: bounded refusal/drain fixture; uncertainty must remain terminal.
        let error = unsafe {
            host.prepare_selected_image_creche_owner(
                &mut adapter,
                &context,
                1024 * 1024,
                256 * 1024,
            )
        }
        .err()
        .expect("changed adapter must refuse");
        assert!(error.contains("differs from its current adapter"));
        assert_eq!(
            host.kernel_resources.require_available().is_err(),
            uncertain
        );
        assert_eq!((adapter.begins, adapter.polls, adapter.drains), (0, 0, 1));
        assert_eq!(used.load(Ordering::Acquire), before);
    }
}
