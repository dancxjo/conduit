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
    let prepared = crate::tour_timer_plan::prepare_description(
        &ids,
        offer,
        crate::make::EMBEDDED_MAKE.build_id,
    )
    .unwrap_or_else(|_| refuse("timer-runtime-plan"));
    let graph = prepared.graph;
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

/// The ordinary product entrance uses actual issued capabilities and providers.
/// Assertions observe its result; they never substitute semantic completions.
pub(super) fn run_product(offer: &HostOffer<'_>) {
    use crate::machine::{IdleBase, SerialBase, TimerBase};
    let ids = crate::identity::BootIdentities {
        host: offer.host_id,
        boot: offer.boot_id,
    };
    let mut prepared =
        crate::tour_play::prepare_timer_stage(&ids, offer, crate::make::EMBEDDED_MAKE.build_id)
            .unwrap_or_else(|_| refuse("timer-product-plan"));
    let mut clock = arch::Clock::new();
    let mut timer = arch::Timer::new();
    let mut serial = arch::Serial::new();
    let mut interrupts = arch::Interrupts::new();
    let mut idle = arch::Idle::new();
    let evidence = crate::tour_play::run_timer_stage(
        &mut prepared,
        &mut clock,
        &mut timer,
        &mut serial,
        &mut interrupts,
        &mut idle,
    )
    .unwrap_or_else(|_| refuse("timer-product-run"));
    if evidence.run.timer_irq_wakes != 1
        || timer.wake_count() != 1
        || serial.presentation_count() != 2
        || idle.idle_count() == 0
        || evidence.run.pending_host_calls != 1
        || !evidence.run.clock_monotonic
    {
        refuse("timer-product-lifecycle");
    }
    let cost = prepared.kernel.cost();
    if cost.entries == 0
        || cost.base_gate_transitions != 4
        || cost.teardown_zeroed_bytes != cost.reserved_bytes
    {
        refuse("timer-product-domain-cost");
    }
    drop(prepared);
    arch::early_write(b"CONDUIT_DOMAIN_TIMER_PRODUCT private-production-kernel physical-duration-120ms exact-capabilities counts-presented-2 timer-wakes-1 cancelled zeroed\n");
}
