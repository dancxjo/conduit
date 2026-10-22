//! One real pulse transaction through the production kernel. This bounded bench
//! profile is not the autonomous logger, a Body membership claim, or AP service.
extern crate alloc;
use crate::{
    kernel_back::{Back, Observed},
    probe_heap,
    usb_link::UsbLinkSession,
};
use alloc::{vec, vec::Vec};
use conduit_core as c;
use conduit_environment_runtime::PreparedCaptureSelector;
use conduit_kernel::{
    self as k,
    scheduler::{FixedScheduler, SchedulerStatus},
};

use conduit_pulse_capture::{installation::*, owner::*, *};
use conduit_rp2040_pulse_capture::PicoPulseCapture;
use core::fmt::Write;
use embassy_rp::{clocks::RoscRng, peripherals::PIO1};
use embassy_time::{Duration, Instant, Timer};
#[path = "kernel_execution.rs"]
mod execution;
#[path = "kernel_preparation.rs"]
mod preparation;
const N: usize = 15;
const C: usize = 14;
const P: usize = 1;
const R: usize = N * P;
#[derive(Clone, Copy)]
enum Role {
    #[allow(
        dead_code,
        reason = "shared Make supports raw and cadence bench profiles"
    )]
    Begin,
    Observe(usize),
    Pulse,
    Observation,
    Tick(u64, u64),
    Trigger,
    Expression(&'static [u8]),
    Selector(&'static str),
}
include!(concat!(env!("OUT_DIR"), "/kernel-roles.rs"));
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    let location = info.location();
    crate::kernel_fault::record(
        3,
        0,
        0,
        0,
        0,
        location.map_or(0, |l| l.line()),
        location.map_or(0, |l| l.column()),
    );
    embassy_rp::rom_data::reset_to_usb_boot(0, 0);
    loop {
        cortex_m::asm::wfi();
    }
}

async fn progress(link: &mut UsbLinkSession, phase: &str) {
    let (peak, retained, _) = probe_heap::ALLOCATOR.observation();
    let mut line = heapless::String::<256>::new();
    writeln!(line,"{{\"schema\":\"conduit.kernel-pulse-hil/v1\",\"phase\":\"{phase}\",\"heap_peak\":{peak},\"heap_retained\":{retained}}}").unwrap();
    let _ = link.write_text(&line).await;
}
pub async fn run(
    electrical: PicoPulseCapture<'static, PIO1, 0>,
    link: &mut UsbLinkSession,
    #[cfg(feature = "http-ap")] network: crate::http_radio::Resources,
) {
    Timer::after(Duration::from_secs(2)).await;
    if let Some(fault) = crate::kernel_fault::take() {
        link.wait_connection().await;
        while !link.class.dtr() {
            Timer::after(Duration::from_millis(25)).await;
        }
        let mut line = heapless::String::<384>::new();
        writeln!(line,"{{\"schema\":\"conduit.kernel-pulse-hil/v1\",\"phase\":\"recovered-fault\",\"kind\":{},\"heap_peak\":{},\"heap_retained\":{},\"requested\":{},\"alignment\":{},\"line\":{},\"column\":{}}}",fault[1],fault[2],fault[3],fault[4],fault[5],fault[6],fault[7]).unwrap();
        let _ = link.write_text(&line).await;
        loop {
            let _ = crate::bootsel::wait_for_request(link).await;
        }
    }
    progress(link, "deserialize-inert-template").await;
    let mut model: c::Plan = match serde_json::from_slice(include_bytes!(concat!(
        env!("OUT_DIR"),
        "/kernel-template.json"
    ))) {
        Ok(plan) => plan,
        Err(error) => {
            let mut line = heapless::String::<256>::new();
            writeln!(
                line,
                "{{\"phase\":\"template-refused\",\"line\":{},\"column\":{}}}",
                error.line(),
                error.column()
            )
            .unwrap();
            let _ = link.write_text(&line).await;
            loop {
                let _ = crate::bootsel::wait_for_request(link).await;
            }
        }
    };
    let mut rng = RoscRng;
    let mut boot = heapless::String::<96>::new();
    write!(
        boot,
        "pico-kernel-hil/{:016x}{:016x}",
        rng.next_u64(),
        rng.next_u64()
    )
    .unwrap();
    let mut fragment = model.fragments.remove(0);
    assert!(model.fragments.is_empty() && model.activations.is_empty());
    assert!(
        fragment.host_id.as_str() == "fixture/environment"
            && fragment.shared_pools.is_empty()
            && fragment.fore_ports.is_empty()
    );
    assert!(fragment
        .connections
        .iter()
        .all(|c| c.selected_line.is_none() && c.admitted_lines.is_empty() && c.resource.is_none()));
    fragment.boot_id = boot.as_str().into();
    for gear in &mut fragment.placements {
        gear.boot_id = fragment.boot_id.clone();
        for grant in &mut gear.authority {
            grant.boot_id = fragment.boot_id.clone();
        }
    }
    progress(link, "seal-fresh-local-plan").await;
    let plan = c::seal_plan_with_realization_backs_and_completion(
        c::PlotIdentity {
            source_document_id: model.source_document_id,
            checked_plot_id: model.checked_plot_id,
            expanded_plot_id: model.expanded_plot_id,
        },
        model.completion_policy,
        model.realization_backs,
        vec![fragment],
    );
    let fragment = &plan.fragments[0];
    let active = c::bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment).unwrap();
    let identity = preparation::identity(boot.as_str());
    // SAFETY: this bench Root owns exactly PIO1/SM0/DMA2/GP2. Its independent
    // fixed policy authorizes four bounded pulse reads for this fresh local Plan.
    let ready = unsafe {
        ReadyPulseBase::new(
            identity.clone(),
            PulseAttachment {
                generation: 1,
                resource_bytes: 64,
                limits: conduit_rp2040_pulse_capture::LIMITS,
            },
            electrical,
        )
    }
    .unwrap();
    let pulse_gear = fragment
        .placements
        .iter()
        .find(|g| g.kind_id.as_str() == PULSE_READ_KIND)
        .unwrap();
    let pulse_node = lowered
        .identity
        .node_for_placement(&pulse_gear.placement_id)
        .unwrap();
    progress(link, "bind-real-native-owner").await;
    let (table, handle, claim) =
        crate::kernel_authority::possession(&identity, &plan.plan_id, &active);
    let mut owner = ready
        .bind_selected(
            table,
            handle,
            claim,
            &PulseReadContract::prepare().unwrap(),
            conduit_plan_lowering::selected_operation::SelectedOperationPlan {
                fragment,
                lowered: &lowered,
                active: &active,
                placement_id: &pulse_gear.placement_id,
            },
        )
        .unwrap();
    let mut routes = k::FixedRoutes::<R, C>::new(P as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let mut calls = k::FixedHostCallBindings::<N>::new(1);
    for call in &lowered.host_calls {
        calls.install(call.node, call.binding).unwrap();
    }
    calls.seal().unwrap();
    assert!(fragment
        .placements
        .iter()
        .all(|gear| gear.inputs.len() <= P && gear.outputs.len() <= P));
    let nodes = core::array::from_fn(|i| {
        let spec = lowered.node_specs[i];
        assert!(spec.input_cords[P..].iter().all(Option::is_none));
        k::scheduler::NodeSpec::<P> {
            input_cords: [spec.input_cords[0]],
            maximum_step_fuel: spec.maximum_step_fuel,
        }
    });
    let startup: [usize; N] = core::array::from_fn(|i| {
        usize::from(
            lowered
                .identity
                .node_for_placement(&fragment.startup_order[i])
                .unwrap()
                .0,
        )
    });
    let cords = core::array::from_fn(|i| lowered.cords[i].spec);
    let sign_slots = lowered.sign_items.max(
        u16::try_from(
            (lowered.sign_bytes as usize).div_ceil(core::mem::size_of::<k::KernelEvent>()),
        )
        .unwrap(),
    );
    let signs = k::HostedSignLog::new(sign_slots, lowered.sign_bytes).unwrap();
    let mut identity_line = heapless::String::<768>::new();
    writeln!(identity_line,"{{\"schema\":\"conduit.kernel-pulse-hil/v1\",\"phase\":\"prepared\",\"plan\":\"{}\",\"fragment\":\"{}\",\"host\":\"{}\",\"boot\":\"{}\",\"active_play\":\"{}\"}}",plan.plan_id.as_str(),fragment.fragment_id.as_str(),fragment.host_id.as_str(),boot,active.active_play_id.as_str()).unwrap();
    let _ = link.write_text(&identity_line).await;
    let face_basis = conduit_face::FaceBasis {
        body_id: None,
        wake_id: None,
        source_document_id: Some(plan.source_document_id.clone()),
        checked_plot_id: Some(plan.checked_plot_id.clone()),
        expanded_plot_id: Some(plan.expanded_plot_id.clone()),
        plan_id: Some(plan.plan_id.clone()),
        active_play_id: Some(active.active_play_id.clone()),
        sign_ids: vec![],
    };
    drop(lowered);
    drop(plan);
    let (values, wait) = preparation::prepare_values();
    let mut observed = preparation::observation_witnesses();
    let mut selectors: Vec<Option<PreparedCaptureSelector>> = (0..N).map(|_| None).collect();
    progress(link, "prepare-backs").await;
    let mut prepared = preparation::vacant_backs();
    for i in startup {
        let mut phase = heapless::String::<48>::new();
        write!(phase, "prepare-back-{i}").unwrap();
        progress(link, &phase).await;
        prepared[i] = Some(preparation::prepare_back(
            ROLES[i],
            &observed,
            &mut selectors[i],
            wait,
        ));
    }
    progress(link, "backs-ready").await;
    let mut kernel =
        preparation::construct_kernel(nodes, cords, routes, calls, prepared, signs, values);
    #[cfg(feature = "http-ap")]
    crate::http_radio::launch(network);
    probe_heap::ALLOCATOR.seal();
    progress(link, "play-allocation-sealed").await;
    let mut pulse_calls = 0;
    let mut drained = false;
    let mut pending_timer: Option<(k::NodeId, k::RequestId, Instant)> = None;
    let mut sample_times = [0_u64; 4];
    let sample_limit = if cfg!(feature = "http-ap") { 4096 } else { 4 };
    #[cfg(feature = "http-ap")]
    let mut published = [0; 4];
    for _ in 0..(sample_limit * 250) {
        if let Some((node, request, deadline)) = pending_timer {
            if Instant::now() >= deadline {
                execution::complete(
                    &mut kernel,
                    node,
                    request,
                    k::HostCallOutcome {
                        disposition: k::HostCallDisposition::Completed,
                        output: None,
                        failure: None,
                    },
                );
                pending_timer = None;
            }
        }
        let mut serviced = false;
        if let Some(request) = kernel.next_host_request() {
            serviced = true;
            let input = kernel.host_value(request.input.value).unwrap();
            let bytes = if request.node == pulse_node {
                sample_times[pulse_calls % 4] = Instant::now().as_millis();
                pulse_calls += 1;
                Some(
                    owner
                        .invoke(request.node, request.call, request.request, input)
                        .await
                        .unwrap(),
                )
            } else if ROLES[usize::from(request.node.0)].is_tick() {
                let duration = u64::from_le_bytes(input.try_into().unwrap());
                assert!(pending_timer.is_none());
                pending_timer = Some((
                    request.node,
                    request.request,
                    Instant::now() + Duration::from_millis(duration),
                ));
                continue;
            } else {
                selectors[usize::from(request.node.0)]
                    .as_mut()
                    .unwrap()
                    .select(input)
                    .unwrap()
            };
            let output = bytes.map(|b| {
                k::BoundedValueRef::new(kernel.store_host_value(b).unwrap(), 1024).unwrap()
            });
            execution::complete(
                &mut kernel,
                request.node,
                request.request,
                k::HostCallOutcome {
                    disposition: k::HostCallDisposition::Completed,
                    output,
                    failure: None,
                },
            );
        }
        let status = execution::advance(&mut kernel);
        #[cfg(feature = "http-ap")]
        let mut changed = false;
        #[cfg(feature = "http-ap")]
        for (index, witness) in observed.iter().enumerate() {
            if witness.count.get() != published[index] {
                published[index] = witness.count.get();
                changed = true;
                let bytes = witness.bytes.borrow();
                crate::http_view::publish(index, &bytes.0[..bytes.1], Instant::now().as_millis());
            }
        }
        #[cfg(feature = "http-ap")]
        if changed {
            let mut line = heapless::String::<256>::new();
            writeln!(line, "{{\"event\":\"http-observation\",\"samples\":{pulse_calls},\"outcome_counts\":[{},{},{},{}],\"ap_ready\":{}}}", published[0],published[1],published[2],published[3],crate::http_view::ap_ready()).unwrap();
            let _ = link.write_text(&line).await;
        }
        match status {
            SchedulerStatus::Drained => {
                drained = true;
                break;
            }
            SchedulerStatus::Idle if !serviced => {
                if let Some((_, _, deadline)) = pending_timer {
                    #[cfg(not(feature = "http-ap"))]
                    Timer::at(deadline).await;
                    #[cfg(feature = "http-ap")]
                    {
                        let _ = embassy_futures::select::select(
                            Timer::at(deadline),
                            crate::bootsel::wait_for_request(link),
                        )
                        .await;
                    }
                }
            }
            _ => {}
        }
    }
    assert!(
        drained
            && pulse_calls == sample_limit
            && owner.inspection().completed_operations == sample_limit as u32
    );
    drop(kernel);
    let reader_present_at_drain = link.class.dtr();
    // Sampling has completed independently. An optional diagnostic reader may
    // attach later to inspect retained results; it does not start or pace Play.
    link.wait_connection().await;
    while !link.class.dtr() {
        Timer::after(Duration::from_millis(25)).await;
    }
    let reader_attached_tick = Instant::now().as_millis();
    progress(link, "play-drained").await;
    let mut counts = heapless::String::<256>::new();
    writeln!(counts, "{{\"schema\":\"conduit.kernel-pulse-hil/v1\",\"phase\":\"sampling-complete\",\"pulse_calls\":{pulse_calls},\"observations\":{},\"timeouts\":{},\"provider_failures\":{},\"geometry_refusals\":{}}}", observed[0].count.get(), observed[1].count.get(), observed[2].count.get(), observed[3].count.get()).unwrap();
    let _ = link.write_text(&counts).await;
    counts.clear();
    writeln!(counts, "{{\"schema\":\"conduit.kernel-pulse-hil/v1\",\"phase\":\"sampling-times\",\"clock\":\"boot-monotonic-milliseconds\",\"start_ticks\":[{},{},{},{}]}}", sample_times[0],sample_times[1],sample_times[2],sample_times[3]).unwrap();
    let _ = link.write_text(&counts).await;
    counts.clear();
    writeln!(counts, "{{\"schema\":\"conduit.kernel-pulse-hil/v1\",\"phase\":\"reader-attached\",\"tick\":{reader_attached_tick},\"present_at_drain\":{reader_present_at_drain}}}").unwrap();
    let _ = link.write_text(&counts).await;
    let period = ROLES
        .iter()
        .find_map(|r| {
            if let Role::Tick(_, p) = r {
                Some(*p)
            } else {
                None
            }
        })
        .unwrap();
    let fresh = reader_attached_tick.saturating_sub(sample_times[3]) <= period.saturating_mul(2);
    // SAFETY: the sole kernel has drained and been dropped. Ordinary Face
    // construction is a separate preparation phase in the same finite arena.
    unsafe { probe_heap::ALLOCATOR.prepare_after_drained_play() };
    progress(link, "prepare-face-and-mask").await;
    for (index, witness) in observed.iter_mut().enumerate() {
        if witness.count.get() > 0 {
            let bytes = witness.bytes.get_mut();
            crate::kernel_report::prepare(link, index, &bytes.0[..bytes.1], &face_basis, fresh)
                .await;
        }
    }
    drop(observed);
    probe_heap::ALLOCATOR.seal();
    progress(link, "kernel-drained").await;
    loop {
        let _ = crate::bootsel::wait_for_request(link).await;
    }
}

// Outlined preparation keeps large construction temporaries off the async poll
// frame. The returned kernel is the existing production scheduler.

type BenchKernel<'a> =
    FixedScheduler<Back<'a>, k::HostedValueStore, k::HostedSignLog, N, C, P, C, R, C, N, N>;

#[cortex_m_rt::exception]
unsafe fn HardFault(frame: &cortex_m_rt::ExceptionFrame) -> ! {
    crate::kernel_fault::record(4, 0, 0, 0, 0, frame.pc(), frame.lr());
    embassy_rp::rom_data::reset_to_usb_boot(0, 0);
    loop {
        cortex_m::asm::wfi();
    }
}

impl Role {
    fn is_tick(self) -> bool {
        matches!(self, Self::Tick(..))
    }
}
