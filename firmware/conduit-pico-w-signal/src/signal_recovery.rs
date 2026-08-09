//! Selection and bounded recovery for the R1 Signal Plans.

use embassy_net::Stack;

use crate::receipts::{RuntimeTranscriptIdentity, UsbCdc};
use crate::usb_link::UsbLinkSession;

pub async fn run(
    stack: Stack<'static>,
    link: &mut UsbLinkSession,
    clue: &mut UsbCdc,
    control: &mut cyw43::Control<'_>,
    runtime: &RuntimeTranscriptIdentity,
) -> ! {
    if !crate::plan_b_signal_image::validate_replacement() {
        remain_bootsel(link).await
    }
    let mut continuation = None;
    if crate::websocket_route::run(stack, link, clue, control, runtime, &mut continuation)
        .await
        .is_ok()
    {
        remain_bootsel(link).await
    }

    if let Some(mut state) = continuation {
        let plan_c_runtime = runtime.for_plan(state.identity.plan_id, state.identity.host_id);
        if crate::remote_signal::resume_plan_c_signal_sink(
            link,
            clue,
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

    let plan_b = crate::plan_b_signal_image::execution_identity();
    let plan_b_runtime = runtime.for_plan(plan_b.plan_id, plan_b.host_id);
    if crate::remote_signal::run_plan_b_signal_sink(
        link,
        clue,
        control,
        &plan_b_runtime,
    )
    .await
    .is_err()
    {
        remain_bootsel(link).await
    }
    remain_bootsel(link).await
}

async fn remain_bootsel(link: &mut UsbLinkSession) -> ! {
    loop {
        crate::bootsel::wait_for_request(link).await.ok();
    }
}
