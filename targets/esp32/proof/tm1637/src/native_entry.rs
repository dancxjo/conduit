//! Initialize actual Root resources and install an optional linked Body.
//! Plot planning, effect authority, and execution remain separate admission.
use embassy_executor::Spawner;
use esp_hal::{
    clock::CpuClock, interrupt::software::SoftwareInterruptControl, timer::timg::TimerGroup,
};
// An async function's inline attribute governs its constructor, not the poll
// implementation that LLVM can merge into main's permanent ancestor frame.
// Poll the same pinned future through an explicit boundary, without allocation,
// another task, or any change to its waker and readiness ordering.
#[cfg(feature = "local-appliance")]
#[inline(never)]
fn poll_outlined<F: core::future::Future>(
    future: core::pin::Pin<&mut F>,
    context: &mut core::task::Context<'_>,
) -> core::task::Poll<F::Output> {
    future.poll(context)
}
#[cfg(feature = "local-appliance")]
async fn await_outlined<F: core::future::Future>(future: F) -> F::Output {
    let mut future = core::pin::pin!(future);
    core::future::poll_fn(|context| poll_outlined(future.as_mut(), context)).await
}

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    let p = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));
    esp_alloc::heap_allocator!(size: crate::sdk_memory::ROOT_INTERNAL_BYTES);
    let sdk_reservation = crate::sdk_memory::initialize();
    let timer = TimerGroup::new(p.TIMG0);
    let interrupts = SoftwareInterruptControl::new(p.SW_INTERRUPT);
    esp_rtos::start(timer.timer0, interrupts.software_interrupt0);
    #[cfg(any(
        feature = "local-evidence-store",
        feature = "network-credential-store",
        feature = "body-persistence",
        feature = "observation-store"
    ))]
    {
        let regions =
            crate::native_evidence_region::prepare(p.FLASH).expect("dedicated evidence backing");
        // Authenticate the retained Body and its layout receipts before any
        // observation epoch can be considered reusable after a power cut.
        #[cfg(feature = "body-persistence")]
        crate::native_body_image::initialize_restart(regions.body_state)
            .expect("Body restart backing");
        #[cfg(feature = "local-evidence-store")]
        crate::native_evidence_archive::initialize(regions.evidence)
            .expect("display evidence backing");
        #[cfg(any(feature = "local-evidence-store", feature = "observation-store"))]
        crate::native_observed_archive::initialize_epochs(
            regions.observations,
            regions.observation_epochs,
        )
        .expect("observed publication backing");
        #[cfg(feature = "network-credential-store")]
        crate::native_credentials::initialize_journal(regions.credentials)
            .expect("credential backing");
        #[cfg(feature = "network-credential-store")]
        crate::native_play::journal::initialize(regions.network_recovery)
            .expect("network recovery backing");
    }
    #[cfg(feature = "local-appliance")]
    let (reclaim_submitter, maintenance_lane, pulse_gate) =
        crate::native_flash_domain::prepare().expect("finite cooperative flash domain");
    let sensor = conduit_esp32_pulse_capture::Esp32PulseCapture::new(p.GPIO27);
    #[cfg(feature = "local-appliance")]
    let sensor = crate::native_pulse_fence::GatedPulse::new(sensor, pulse_gate);
    let seed = u64::from(esp_hal::rng::Rng::new().random());
    let radio = crate::network::start(spawner, p.WIFI, seed);
    #[cfg(all(feature = "owner-wifi", not(feature = "biography-maintenance")))]
    let owner_stacks = radio.stacks;
    // Boot observation is construction scratch. Root and each initialized
    // provider retain their owned exact IDs before the first await; do not
    // retain a second copy in the permanent main task frame.
    let mut root = {
        let boot =
            crate::boot::ObservedBoot::observe().expect("initialized radio entropy required");
        // SAFETY: the actual TimerGroup was consumed by esp_rtos::start above.
        // Root retains one pending software-timer slot; Play admission is separate.
        #[cfg(not(feature = "local-appliance"))]
        let wait = unsafe { crate::native_wait::prepare(&boot) }
            .expect("initialized monotonic timer slot");
        #[cfg(feature = "local-appliance")]
        let wait = unsafe { crate::native_wait::prepare_with_maintenance(&boot, maintenance_lane) }
            .expect("initialized monotonic timer slot and maintenance lane");
        let display = crate::native_display::NativeDisplay::prepare(p.GPIO18, p.GPIO19, &boot)
            .expect("initialized Root display provider");
        let source = crate::native_source::identity();
        crate::native_root::NativeRoot::prepare(
            sensor,
            radio,
            &boot,
            source,
            sdk_reservation,
            wait,
            display,
        )
    };
    #[cfg(feature = "local-appliance")]
    root.retain_reclaim_submitter(reclaim_submitter)
        .expect("unique Root maintenance endpoint");
    #[cfg(all(feature = "local-appliance", feature = "body-persistence"))]
    let mut invitation_uart = esp_hal::uart::Uart::new(p.UART0, esp_hal::uart::Config::default())
        .expect("private invitation UART0")
        .with_rx(p.GPIO3)
        .with_tx(p.GPIO1)
        .into_async();
    #[cfg(feature = "biography-service")]
    await_outlined(crate::native_biography_uart::serve_cold(
        &mut root,
        &mut invitation_uart,
    ))
    .await
    .expect("cold biography custody service");
    // Maintenance preserves the restored owner and archive exactly. No linked
    // Body installation, new Boot, or autonomous mutation follows this service.
    #[cfg(feature = "biography-maintenance")]
    loop {
        core::hint::black_box(&root);
        embassy_time::Timer::after_secs(60).await;
    }
    #[cfg(not(feature = "biography-maintenance"))]
    {
        #[cfg(feature = "body-image-root")]
        root.install_linked_body().expect("linked body biography");
        root.start_preparation_service(spawner)
            .await
            .expect("Root preparation service must start");
        // The flat qualification constructs its checked plan from the actual
        // ready owners when joining. Do not also construct the legacy owned
        // network preparation, which that profile never consumes.
        #[cfg(not(feature = "flat-network-appliance"))]
        root.prepare_network_plan()
            .expect("current initialized Root advertisement");
        #[cfg(any(feature = "target-memory-check", feature = "target-mask-memory-check"))]
        {
            #[cfg(feature = "target-memory-check")]
            root.measure_local_sampler()
                .expect("target sampler measurement");
            #[cfg(feature = "local-appliance")]
            await_outlined(root.start_local_appliance())
                .await
                .expect("autonomous local plots");
            #[cfg(all(feature = "target-mask-memory-check", not(feature = "local-appliance")))]
            root.measure_local_mask()
                .expect("target ordinary Mask measurement");
            // This preparation diagnostic accepts no management assignments. Keep
            // its retained Root alive for memory inspection without linking the
            // unrelated remote installation/control interpreter into the image.
            #[cfg(feature = "local-appliance")]
            let mut next_report = embassy_time::Instant::now();
            #[cfg(all(feature = "local-appliance", feature = "body-persistence"))]
            let mut invitation_bootstrap = crate::native_body_join_uart::Bootstrap::configured()
                .expect("provisioned owner invitation key");
            loop {
                #[cfg(feature = "local-appliance")]
                {
                    // Defer new mutations without consuming their requests while
                    // Root owns an unpublished replacement proposal. Network tasks
                    // and history readers run independently; emergency cancellation
                    // remains a separate scoped entrance.
                    if !root.local_renewal_pending() {
                        let _ = await_outlined(root.advance_setup()).await;
                        #[cfg(feature = "body-persistence")]
                        {
                            root.advance_join(embassy_time::Instant::now().as_millis());
                            let _ = await_outlined(
                                invitation_bootstrap.poll(&mut root, &mut invitation_uart),
                            )
                            .await;
                        }
                    }
                    let _ = await_outlined(root.advance_local_renewal()).await;
                    if embassy_time::Instant::now() >= next_report {
                        root.report_local_progress();
                        next_report =
                            embassy_time::Instant::now() + embassy_time::Duration::from_secs(15);
                    }
                    embassy_time::Timer::after_millis(50).await;
                }
                #[cfg(not(feature = "local-appliance"))]
                {
                    core::hint::black_box(&root);
                    embassy_time::Timer::after_secs(60).await;
                }
            }
        }
        #[cfg(all(
            not(feature = "owner-wifi"),
            not(any(feature = "target-memory-check", feature = "target-mask-memory-check"))
        ))]
        {
            let mut uart = esp_hal::uart::Uart::new(p.UART0, esp_hal::uart::Config::default())
                .expect("Root UART0 must initialize")
                .with_rx(p.GPIO3)
                .with_tx(p.GPIO1)
                .into_async();
            crate::native_control::serve(&mut root, &mut uart)
                .await
                .expect("Root UART management failed");
        }
        #[cfg(all(
            feature = "owner-wifi",
            not(any(feature = "target-memory-check", feature = "target-mask-memory-check"))
        ))]
        {
            let mut listener = crate::native_carrier::Listener::new(owner_stacks);
            loop {
                let carrier = listener.accept(&mut root).await;
                let _ = crate::native_control::serve(&mut root, carrier).await;
                // Lost transport never resets Root, revokes providers, or invents
                // lifecycle completion. Existing workers retain their exact scope.
                carrier.abort();
            }
        }
    }
}
