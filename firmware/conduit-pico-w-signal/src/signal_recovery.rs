//! Selection and bounded recovery for the R1 Signal Plans.

use conduit_core::BootId;
use embassy_net::Stack;

use crate::receipts::{RuntimeTranscriptIdentity, UsbCdc};
use crate::usb_link::UsbLinkSession;

pub async fn run(
    stack: Stack<'static>,
    link: &mut UsbLinkSession,
    sign: &mut UsbCdc,
    control: &mut cyw43::Control<'_>,
    runtime: &RuntimeTranscriptIdentity,
) -> ! {
    if !crate::plan_b_signal_image::validate_replacement() {
        remain_bootsel(link).await
    }
    crate::panic_recovery::set_phase(crate::panic_recovery::PanicPhase::RecoveryAdmission);
    let plan_a = crate::signal_execution_identity::SignalExecutionIdentity::plan_a();
    let plan_b = crate::plan_b_signal_image::execution_identity();
    let plan_c = crate::plan_c_signal_image::execution_identity();
    let plan_a_runtime = runtime.for_plan(plan_a.plan_id, plan_a.host_id);
    let plan_b_runtime = runtime.for_plan(plan_b.plan_id, plan_b.host_id);
    let plan_c_runtime = runtime.for_plan(plan_c.plan_id, plan_c.host_id);
    let route_basis =
        conduit_r1_network_conformance::r1_line_basis(BootId::from(runtime.boot_id()));
    let mut plan_a_state =
        match crate::continuable_signal::ContinuableSignalSink::new_plan_a(&plan_a_runtime) {
            Ok(state) => Some(state),
            Err(_) => remain_bootsel(link).await,
        };
    let mut plan_b_state =
        match crate::continuable_signal::ContinuableSignalSink::new_plan_b(&plan_b_runtime) {
            Ok(state) => Some(state),
            Err(_) => remain_bootsel(link).await,
        };
    let mut plan_c_state =
        match crate::continuable_signal::ContinuableSignalSink::new(&plan_c_runtime) {
            Ok(state) => Some(state),
            Err(_) => remain_bootsel(link).await,
        };
    crate::ALLOCATOR.seal();
    crate::panic_recovery::set_phase(crate::panic_recovery::PanicPhase::KernelCompletion);

    loop {
        let mut continuation = None;
        if crate::websocket_route::run(
            stack,
            link,
            sign,
            control,
            &plan_a_runtime,
            &plan_c_runtime,
            &route_basis,
            &mut plan_a_state,
            &mut plan_c_state,
            &mut continuation,
        )
        .await
        .is_ok()
        {
            remain_bootsel(link).await
        }

        if let Some(mut state) = continuation {
            if crate::remote_signal::resume_plan_c_signal_sink(
                link,
                sign,
                control,
                &plan_c_runtime,
                &mut state,
            )
            .await
            .is_err()
            {
                remain_bootsel(link).await
            }
            remain_bootsel(link).await
        }

        let Some(mut state) = plan_b_state.take() else {
            remain_bootsel(link).await
        };
        if crate::remote_signal::run_plan_b_signal_sink(
            link,
            sign,
            control,
            &plan_b_runtime,
            &mut state,
        )
        .await
        .is_err()
        {
            remain_bootsel(link).await
        }
        // Plan B reached reciprocal terminal agreement. The Body may Lull and
        // a later Wake may query Plan C without rebooting this Pico.
    }
}

async fn remain_bootsel(link: &mut UsbLinkSession) -> ! {
    loop {
        crate::bootsel::wait_for_request(link).await.ok();
    }
}
