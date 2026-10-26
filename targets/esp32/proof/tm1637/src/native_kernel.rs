//! Prepare the ordinary scheduler from retained software. Never step it here.
//! Native ownership, authenticated installation and Body start remain separate.
use crate::native_kernel_storage::assemble;
pub use crate::native_kernel_storage::{Kernel, PreparedKernel};
use alloc::boxed::Box;
use conduit_environment_runtime::{appliance::assigned::*, PreparedCaptureSelector};
use conduit_kernel as k;
/// Conservative additional live preparation bound. The already retained drivers
/// and value backing are counted by Root's current heap usage, not allocated again.
/// Native capability/control admission still needs its own actual proof.
pub fn requirements() -> Result<(usize, u16), &'static str> {
    requirements_for(
        crate::appliance_layout::SIGN_ITEMS,
        crate::appliance_layout::SIGN_BYTES,
        core::mem::size_of::<PreparedAssignedAppliance<k::HostedValueStore>>(),
    )
}
fn requirements_for(
    sign_items: u16,
    sign_bytes: u32,
    transfer: usize,
) -> Result<(usize, u16), &'static str> {
    let bytes = usize::try_from(sign_bytes).map_err(|_| "sign-budget")?;
    let slots = usize::from(sign_items).max(bytes.div_ceil(core::mem::size_of::<k::KernelEvent>()));
    // The transfer destination and scheduler destination are separate stages.
    // assemble() explicitly retires the transfer box before allocating selectors
    // or the scheduler; sign backing coexists with both stages. Existing source
    // boxes remain conservatively included in Root's current usage throughout.
    let scheduler = core::mem::size_of::<Kernel>()
        + core::mem::size_of::<[Option<PreparedCaptureSelector>; NODES]>();
    let needed =
        transfer.max(scheduler) + slots * core::mem::size_of::<Option<k::KernelEvent>>() + 8192; // Explicit Root preparation/control headroom; not effect authority.
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
/// Preparation-only local path: no native provider transfer, Step or Body start.
#[cfg(feature = "target-memory-check")]
#[inline(never)]
pub fn prepare_local<'a>(
    software: &mut Option<Box<crate::native_software::Software>>,
    verified: &'a conduit_plan_lowering::local_tables::VerifiedLocalLowering<'a>,
    image: &conduit_plot::flat_image::indexed::IndexedSourceImage<'_>,
    scope: (
        &conduit_core::HostId,
        &conduit_core::BootId,
        conduit_core::OfferGeneration,
    ),
) -> Result<PreparedKernel, &'static str> {
    let lowered = verified.lowered();
    let (needed, slots) = requirements_for(
        lowered.sign_items,
        lowered.sign_bytes,
        core::mem::size_of::<PreparedLocalAppliance<'_, k::HostedValueStore>>(),
    )?;
    let memory = esp_alloc::HEAP.stats();
    if memory.size.saturating_sub(memory.current_usage) < needed {
        return Err("local-kernel-capacity");
    }
    // Exact Source/offer admission happens before transferring pure owners and
    // before allocating Sign backing, keeping decoding out of that live peak.
    let prepared = crate::native_software::bind_local(software, verified, image, scope)?;
    let signs =
        k::HostedSignLog::new(slots, lowered.sign_bytes).map_err(|_| "local-sign-budget")?;
    crate::native_kernel_storage::assemble_local(prepared, signs)
}
pub async fn report(
    root: &mut crate::native_root::NativeRoot,
    uart: &mut crate::native_carrier::Carrier<'_>,
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
        body_installed: root.body_installed(),
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
