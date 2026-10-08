//! Supplemental private-runtime fixture; completions are synthetic, without effects.
use super::refuse;
use crate::{
    arch,
    offer::HostOffer,
    protected_region::{DomainBackend, DomainReturn},
};

pub(super) fn run(offer: &HostOffer<'_>) {
    let ids = crate::identity::BootIdentities {
        host: offer.host_id,
        boot: offer.boot_id,
    };
    let prepared =
        crate::tour_timer_plan::prepare(&ids, offer, crate::make::EMBEDDED_MAKE.build_id)
            .unwrap_or_else(|_| refuse("timer-runtime-plan"));
    let fragment = &prepared.plan.fragments[0];
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment)
        .unwrap_or_else(|_| refuse("timer-runtime-lowering"));
    let graph = crate::tour_timer_kernel::TourTimerKernel::prepare_graph(fragment, &lowered)
        .unwrap_or_else(|_| refuse("timer-runtime-graph"));
    let mut domain =
        arch::TextDomain::install().unwrap_or_else(|_| refuse("timer-runtime-install"));
    // Fixture labels are not issued capabilities and never authorize a Base.
    domain
        .initialize_timer(graph, 101, 202)
        .unwrap_or_else(|_| refuse("timer-runtime-initialize"));
    if domain.enter(1) != Ok(DomainReturn::Yielded) {
        refuse("timer-runtime-initialize-return");
    }
    domain
        .timer_initialized()
        .unwrap_or_else(|_| refuse("timer-runtime-initialized"));
    let mut pending_timer = None;
    let mut timer_requests = 0;
    let mut counts = 0;
    for _ in 0..32 {
        domain
            .timer_command(11)
            .unwrap_or_else(|_| refuse("timer-runtime-command"));
        let returned = domain.enter(1);
        let observation = domain
            .timer_observation()
            .unwrap_or_else(|_| refuse("timer-runtime-observation"));
        if counts == 2 && pending_timer.is_some() && observation.pending == 1 {
            if returned != Ok(DomainReturn::Yielded)
                || observation.status != 1
                || timer_requests != 2
                || observation.decisions == 0
                || observation.signs == 0
            {
                refuse("timer-runtime-causal-retirement");
            }
            domain
                .timer_command(13)
                .unwrap_or_else(|_| refuse("timer-runtime-stop"));
            if domain.enter(1) != Ok(DomainReturn::Yielded)
                || domain
                    .timer_observation()
                    .unwrap_or_else(|_| refuse("timer-runtime-stop-observation"))
                    .status
                    != 2
            {
                refuse("timer-runtime-cancelled");
            }
            domain.quarantine();
            if domain.cost().teardown_zeroed_bytes != domain.cost().reserved_bytes {
                refuse("timer-runtime-zeroization");
            }
            arch::early_write(b"CONDUIT_DOMAIN_TIMER_RUNTIME private-production-kernel causal-counts-0-1 synthetic-completions no-base-effects cancelled zeroed\n");
            return;
        }
        match returned {
            Ok(DomainReturn::Gate) => {
                let request = domain
                    .timer_request()
                    .unwrap_or_else(|_| refuse("timer-runtime-request"));
                if request.work_units != 1 {
                    refuse("timer-runtime-work");
                }
                match request.kind {
                    1 => {
                        if request.handle != 101
                            || request.operation != 10
                            || request.request.node != graph.timer
                            || request.length != 8
                            || request.payload[..8] != graph.period.to_le_bytes()
                            || pending_timer.replace(request.request).is_some()
                        {
                            refuse("timer-runtime-wait");
                        }
                        timer_requests += 1;
                    }
                    2 => {
                        let expected = match counts {
                            0 => b"0",
                            1 => b"1",
                            _ => refuse("timer-runtime-extra-count"),
                        };
                        if request.handle != 202
                            || request.operation != 11
                            || request.request.node != graph.presentation
                            || &request.payload[..request.length] != expected
                        {
                            refuse("timer-runtime-count");
                        }
                        counts += 1;
                        complete(&mut domain, 2, request.request);
                    }
                    _ => refuse("timer-runtime-kind"),
                }
            }
            Ok(DomainReturn::Yielded) if observation.status == 1 && counts == 1 => {
                let request = pending_timer
                    .take()
                    .unwrap_or_else(|| refuse("timer-runtime-missing-wait"));
                complete(&mut domain, 1, request);
            }
            _ => refuse("timer-runtime-unexpected-return"),
        }
    }
    refuse("timer-runtime-step-bound");
}

fn complete(
    domain: &mut arch::TextDomain,
    kind: u32,
    request: conduit_kernel::scheduler::HostCallRequest,
) {
    domain
        .timer_completion(kind, request.node, request.request)
        .unwrap_or_else(|_| refuse("timer-runtime-completion"));
    if domain.enter(1) != Ok(DomainReturn::Yielded)
        || domain
            .timer_observation()
            .unwrap_or_else(|_| refuse("timer-runtime-completion-observation"))
            .status
            != 0
    {
        refuse("timer-runtime-completion-return");
    }
}
