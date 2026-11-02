//! Real ordinary Body/scheduler execution, CPU-only, with selected IMAGE custody.
use super::*;
use conduit_body::{Body, BodyPlan, BodyPlotPlan, ResidentPlot};
use conduit_core::{OfferGeneration, SignId, TerminalDisposition};
use conduit_host_make::*;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use std::sync::atomic::Ordering;
const SOURCE: &str = include_str!("../../../profiles/raspberry-pi-3-tick-workset.host.conduit");
fn fixture(heap: u64) -> (StdHost, Wake, BodyPlan) {
    let packages = conduit_workspace_make::package_set();
    let catalog = conduit_workspace_make::catalog();
    let mut configuration = parse_host_configuration_conduit(SOURCE).unwrap();
    configuration.limits.heap_arena_bytes = heap;
    let checked = check_host_configuration(configuration, &catalog, &packages).unwrap();
    let image = build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/owned-tick".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0;
    let host = StdHost::from_selected_timer_image(
        crate::StdHostConfig {
            host_id: "fixture/tick".into(),
            boot_id: "fixture/boot".into(),
            offer_generation: OfferGeneration(1),
        },
        &image,
        &catalog,
    )
    .unwrap();
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_time::install_tick_catalog(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_tick_face_catalog(&mut startup, &mut profile).unwrap();
    let source = "plot ticks {\nclock: time/tick(count = 2, period-ms = 0)\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_plot(&checked, "ticks", &profile).unwrap();
    let plan = host.plan_expanded_local(&expanded).unwrap();
    let body = Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        SignId::from("fixture/birth"),
    )
    .unwrap();
    let wake = body.wake(1, SignId::from("fixture/wake")).unwrap().1;
    let body_plan = BodyPlan::seal(
        &wake,
        vec![BodyPlotPlan {
            plot: ResidentPlot::new(
                plan.source_document_id.clone(),
                plan.checked_plot_id.clone(),
            ),
            plan,
        }],
    )
    .unwrap();
    (host, wake, body_plan)
}
fn receipt(host: &StdHost, wake: &Wake, plan: &BodyPlan) -> usize {
    let play = BodyPlayIdentity::bind(plan, 0);
    let control = RunControl::default();
    let request = BodyRunRequest {
        wake,
        plan,
        control: &control,
        keyboard: None,
    };
    let (prepared, sign) = prepare_lifecycle(&request, &play, host.advertisement()).unwrap();
    let mut kernel = BodyKernel::prepare_on_host(
        &plan.plots,
        false,
        &play.active_play_id,
        &[],
        false,
        false,
        host.advertisement(),
        #[cfg(target_os = "linux")]
        None,
    )
    .unwrap();
    let bytes = crate::installed_std::body_kernel::memory::lifecycle_bytes(&play, &prepared, &sign)
        .unwrap();
    kernel
        .prepare_memory(
            &mut crate::ThreadTimer,
            &host.advertisement().host_id,
            &host.advertisement().boot_id,
            bytes,
        )
        .unwrap()
        .total_bytes()
}
#[test]
fn selected_tick_completes_without_poststart_allocation_and_result_holds_only_bytes() {
    let (mut host, wake, plan) = fixture(64 * 1024 * 1024);
    let control = RunControl::default();
    let observed = host.kernel_resources.image_memory_observer();
    let mut probe = None;
    let result = host
        .run_selected_image_body_plan_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &mut Vec::new(),
            &mut crate::ThreadTimer,
            |_, _| {
                assert!(observed.load(Ordering::Acquire) > 0);
                probe = Some(crate::allocation_probe::begin());
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(probe.take().unwrap().finish(), 0);
    assert_eq!(result.report().terminal, TerminalDisposition::Completed);
    assert!(!result.report().clock_observations.is_empty());
    assert!(
        host.kernel_resources.is_idle(),
        "physical timer slots retire independently"
    );
    assert!(
        observed.load(Ordering::Acquire) > 0,
        "report backing remains charged"
    );
    drop(result);
    assert_eq!(observed.load(Ordering::Acquire), 0);
}
#[test]
fn exact_managed_budget_admits_and_one_byte_short_refuses_before_start() {
    let (host, wake, plan) = fixture(64 * 1024 * 1024);
    let exact = receipt(&host, &wake, &plan);
    let (mut host, wake, plan) = fixture(exact as u64);
    let control = RunControl::default();
    let result = host
        .run_selected_image_body_plan_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &mut Vec::new(),
            &mut crate::ThreadTimer,
            |_, _| Ok(()),
        )
        .unwrap();
    assert_eq!(result.report().terminal, TerminalDisposition::Completed);
    drop(result);
    let (mut host, wake, plan) = fixture(exact as u64 - 1);
    let mut started = false;
    assert!(host
        .run_selected_image_body_plan_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None
            },
            &mut Vec::new(),
            &mut crate::ThreadTimer,
            |_, _| {
                started = true;
                Ok(())
            }
        )
        .is_err());
    assert!(!started);
    assert_eq!(
        host.kernel_resources
            .image_memory_observer()
            .load(Ordering::Acquire),
        0
    );
    assert!(host.kernel_resources.is_idle());
}
#[test]
fn cancellation_preserves_result_charge_and_naked_selected_path_stays_refused() {
    let (mut host, wake, plan) = fixture(64 * 1024 * 1024);
    let control = RunControl::default();
    let request_id = crate::RunControlRequestId::new("fixture/cancel").unwrap();
    let observed = host.kernel_resources.image_memory_observer();
    let result = host
        .run_selected_image_body_plan_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &mut Vec::new(),
            &mut crate::ThreadTimer,
            |_, _| {
                assert!(observed.load(Ordering::Acquire) > 0);
                control.request_stop(request_id.clone()).unwrap();
                Ok(())
            },
        )
        .unwrap();
    assert!(matches!(
        result.report().terminal,
        TerminalDisposition::Cancelled { .. }
    ));
    assert!(host.kernel_resources.is_idle());
    assert!(observed.load(Ordering::Acquire) > 0);
    drop(result);
    assert_eq!(observed.load(Ordering::Acquire), 0);
    let control = RunControl::default();
    assert!(host
        .run_body_plan_to(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None
            },
            &mut Vec::new(),
            &mut crate::ThreadTimer
        )
        .is_err());
}

#[test]
fn provider_loss_uses_admitted_diagnostic_and_result_backing() {
    use std::cell::Cell;
    struct LostClock<'a>(&'a Cell<bool>);
    impl TimerAdapter for LostClock<'_> {
        fn wait(&mut self, _: std::time::Duration) {}
        fn monotonic_observation(
            &mut self,
            host: &conduit_core::HostId,
            boot: &conduit_core::BootId,
        ) -> Option<conduit_core::MonotonicInstant> {
            crate::ThreadTimer.monotonic_observation(host, boot)
        }
        fn monotonic_ticks_on(
            &mut self,
            basis: &conduit_core::MonotonicClockIdentity,
        ) -> Option<u64> {
            crate::ThreadTimer.monotonic_ticks_on(basis)
        }
        fn monotonic_now_ms(&mut self) -> Option<u64> {
            if self.0.get() {
                None
            } else {
                crate::ThreadTimer.monotonic_now_ms()
            }
        }
    }
    let (mut host, wake, plan) = fixture(64 * 1024 * 1024);
    let control = RunControl::default();
    let lost = Cell::new(false);
    let mut probe = None;
    let result = host
        .run_selected_image_body_plan_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &mut Vec::new(),
            &mut LostClock(&lost),
            |_, _| {
                lost.set(true);
                probe = Some(crate::allocation_probe::begin());
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(probe.take().unwrap().finish(), 0);
    assert!(matches!(
        result.report().terminal,
        TerminalDisposition::Failed { .. }
    ));
    assert!(result
        .report()
        .failure
        .as_ref()
        .unwrap()
        .contains("unavailable"));
    assert!(host.kernel_resources.is_idle());
}
