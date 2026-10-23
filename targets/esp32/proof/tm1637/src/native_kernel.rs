//! Prepare the ordinary scheduler from retained software. Never step it here.
//! Native ownership, authenticated installation and Body start remain separate.
use alloc::boxed::Box;
use conduit_environment_runtime::{
    appliance::{assigned::*, ApplianceBack},
    PreparedCaptureSelector,
};
use conduit_kernel::{self as k, scheduler::FixedScheduler};
pub type Kernel = FixedScheduler<
    ApplianceBack,
    k::HostedValueStore,
    k::HostedSignLog,
    NODES,
    CORDS,
    PORTS,
    CORDS,
    { NODES * PORTS },
    CORDS,
    NODES,
    NODES,
>;
pub struct PreparedKernel {
    _kernel: Box<Kernel>,
    _selectors: Box<[Option<PreparedCaptureSelector>; NODES]>,
}
/// Conservative additional live preparation bound. The already retained drivers
/// and value backing are counted by Root's current heap usage, not allocated again.
/// Native capability/control admission still needs its own actual proof.
pub fn requirements() -> Result<(usize, u16), &'static str> {
    let bytes = usize::try_from(crate::appliance_layout::SIGN_BYTES).map_err(|_| "sign-budget")?;
    let slots = usize::from(crate::appliance_layout::SIGN_ITEMS)
        .max(bytes.div_ceil(core::mem::size_of::<k::KernelEvent>()));
    let needed = core::mem::size_of::<Kernel>()
        + core::mem::size_of::<[Option<PreparedCaptureSelector>; NODES]>()
        + core::mem::size_of::<PreparedAssignedAppliance<k::HostedValueStore>>()
        + slots * core::mem::size_of::<Option<k::KernelEvent>>()
        + 8192; // Explicit Root preparation/control headroom; not effect authority.
    Ok((needed, slots.try_into().map_err(|_| "sign-budget")?))
}
#[inline(never)]
pub fn prepare(
    software: &mut Option<Box<crate::native_software::Software>>,
    assignment: &mut Option<Box<ValidatedAssignedAppliance>>,
) -> Result<PreparedKernel, &'static str> {
    if software.is_none() || assignment.is_none() {
        return Err("unavailable");
    }
    let (needed, sign_slots) = requirements()?;
    let memory = esp_alloc::HEAP.stats();
    if memory.size.saturating_sub(memory.current_usage) < needed {
        return Err("kernel-capacity");
    }
    let signs = k::HostedSignLog::new(sign_slots, crate::appliance_layout::SIGN_BYTES)
        .map_err(|_| "sign-budget")?;
    let prepared = PreparedResidentSoftware::bind_boxed_retained(software, assignment)
        .map_err(|_| "software-binding")?;
    assemble(prepared, signs)
}
#[inline(never)]
fn assemble(
    prepared: Box<PreparedAssignedAppliance<k::HostedValueStore>>,
    signs: k::HostedSignLog,
) -> Result<PreparedKernel, &'static str> {
    let prepared = *prepared;
    let selectors = Box::new(prepared.selectors);
    let kernel = Kernel::new_boxed_with_active_counts_and_host_calls(
        NODES,
        CORDS,
        prepared.tables.nodes,
        prepared.tables.cords,
        prepared.tables.routes,
        prepared.tables.calls,
        prepared.drivers,
        prepared.values,
        signs,
    )
    .map_err(|_| "kernel-tables")?;
    Ok(PreparedKernel {
        _kernel: kernel,
        _selectors: selectors,
    })
}
pub async fn report(
    root: &mut crate::native_root::NativeRoot,
    uart: &mut esp_hal::uart::Uart<'_, esp_hal::Async>,
    output: &mut [u8],
) -> Result<(), ()> {
    let result = root.prepare_appliance_kernel();
    let memory = esp_alloc::HEAP.stats();
    #[derive(serde::Serialize)]
    struct Reply<'a> {
        event: &'static str,
        prepared: bool,
        refusal: Option<&'a str>,
        kernel_bytes: usize,
        additional_preparation_bound: usize,
        root_heap_used_bytes: usize,
        root_heap_peak_bytes: usize,
        body_installed: bool,
        native_plan_installed: bool,
        capability_possession_issued: bool,
    }
    let reply = Reply {
        event: "native-kernel-preparation",
        prepared: result.is_ok(),
        refusal: result.err(),
        kernel_bytes: core::mem::size_of::<Kernel>(),
        additional_preparation_bound: requirements().map_or(0, |v| v.0),
        root_heap_used_bytes: memory.current_usage,
        root_heap_peak_bytes: memory.max_usage,
        body_installed: false,
        native_plan_installed: false,
        capability_possession_issued: false,
    };
    let size = serde_json_core::to_slice(&reply, output).map_err(|_| ())?;
    embassy_time::with_timeout(embassy_time::Duration::from_secs(2), async {
        crate::native_control::write_all(uart, b"\n").await?;
        crate::native_control::write_all(uart, &output[..size]).await?;
        crate::native_control::write_all(uart, b"\n").await
    })
    .await
    .map_err(|_| ())?
}
