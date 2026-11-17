//! Borrowed closed cooperative Tick/startup footprint before materialization.
mod startup;
use super::*;
use crate::{
    body_execution::BodyRunRequest,
    kernel_preparation::{ImageMemoryLease, KernelResourceLedger},
};
use conduit_core::{HostAdvertisement, PortDirection};
use conduit_kernel::{HostedSignLog, ValueRef};
use conduit_plan_lowering::fragment_set::{
    lower_local_fragment_set_layout_from_plans, FragmentSetLayout,
};
use core::mem::size_of;

pub(crate) struct TickPreparationLayout {
    pub(crate) peak_bytes: usize,
    pub(crate) lowering_bytes: usize,
    pub(crate) value_bytes: usize,
    pub(crate) clock_identity_bytes: usize,
}
fn add(total: &mut usize, bytes: usize) -> Result<(), String> {
    *total = total
        .checked_add(bytes)
        .ok_or("selected Tick preparation backing overflow")?;
    Ok(())
}
fn array<T>(count: usize) -> Result<usize, String> {
    count
        .checked_mul(size_of::<T>())
        .ok_or_else(|| "selected Tick vector backing overflow".into())
}
pub(crate) fn bounds() -> FragmentSetBounds {
    FragmentSetBounds {
        fragments: conduit_body::MAX_BODY_PLOTS as u16,
        nodes: MAX_NODES as u16,
        cords: MAX_CORDS as u16,
        queue_slots: MAX_QUEUE_SLOTS as u16,
        value_bytes: 16 * 1024 * 1024,
        sign_items: 4096,
        sign_bytes: 4 * 1024 * 1024,
    }
}
impl TickPreparationLayout {
    pub(crate) fn prepare_delegated<T: TimerAdapter>(
        request: &BodyRunRequest<'_>,
        global_play: &conduit_body::BodyPlayIdentity,
        host: &HostAdvertisement,
        ledger: &KernelResourceLedger,
        timer: &T,
        selected_playback: Option<&crate::hosted_audio::HostedPlaybackSelection>,
    ) -> Result<Self, String> {
        let mut layout =
            Self::prepare_with_playback(request, host, ledger, timer, false, selected_playback)?;
        let extra = crate::body_execution::delegated_preparation_backing_bytes(
            request.plan,
            global_play,
            host,
        )?;
        add(&mut layout.peak_bytes, extra)?;
        Ok(layout)
    }
    #[cfg(test)]
    pub(crate) fn prepare<T: TimerAdapter>(
        request: &BodyRunRequest<'_>,
        host: &HostAdvertisement,
        ledger: &KernelResourceLedger,
        timer: &T,
        has_output: bool,
    ) -> Result<Self, String> {
        Self::prepare_with_playback(request, host, ledger, timer, has_output, None)
    }
    /// Only dimensions and borrowed identities are read here. No Plan validation
    /// clone, lowering, timer observation, Fore payload or value slot is built.
    pub(crate) fn prepare_with_playback<T: TimerAdapter>(
        request: &BodyRunRequest<'_>,
        host: &HostAdvertisement,
        ledger: &KernelResourceLedger,
        timer: &T,
        has_output: bool,
        selected_playback: Option<&crate::hosted_audio::HostedPlaybackSelection>,
    ) -> Result<Self, String> {
        if request.keyboard.is_some()
            || request.plan.body_time_requirement.is_some()
            || !request.plan.mask_topologies.is_empty()
            || request.plan.plots.is_empty()
            || request.plan.plots.len() > conduit_body::MAX_BODY_PLOTS
        {
            return Err("selected IMAGE funded profile supports finite Tick/startup only".into());
        }
        ledger.preflight_image_shape(
            request
                .plan
                .plots
                .iter()
                .flat_map(|part| &part.plan.fragments),
        )?;
        let first = &request.plan.plots[0].plan;
        let mut plans = [first; conduit_body::MAX_BODY_PLOTS];
        let mut reservations = [(first, false); conduit_body::MAX_BODY_PLOTS];
        let mut items = 0u16;
        let mut value_quota = 0u32;
        let mut maximum = 1u32;
        let mut signs = 32u16;
        let mut requests = 0usize;
        let mut tick_vectors = 0usize;
        let mut maximum_deliveries = 0u16;
        let mut fore_count = 0usize;
        for (index, part) in request.plan.plots.iter().enumerate() {
            plans[index] = &part.plan;
            reservations[index] = (&part.plan, false);
            if part.plan.fragments.len() != 1 {
                return Err("funded Tick requires one local fragment per Plot".into());
            }
            for gear in &part.plan.fragments[0].placements {
                if !matches!(
                    gear.implementation_id.as_str(),
                    conduit_std_offers::TICK_IMPLEMENTATION
                        | conduit_std_offers::BODY_WAKE_IMPLEMENTATION
                        | conduit_std_offers::STARTUP_CHIME_IMPLEMENTATION
                ) {
                    return Err(
                        "selected IMAGE receipt has no owned accounting for this Back".into(),
                    );
                }
                let budget = super::super::preparation::back_budget(gear)?;
                items = items
                    .checked_add(budget.value_items)
                    .ok_or("Tick value item budget overflow")?;
                value_quota = value_quota
                    .checked_add(budget.value_bytes)
                    .filter(|n| *n <= 16 * 1024 * 1024)
                    .ok_or("Tick value byte budget overflow")?;
                maximum = maximum.max(budget.maximum_value_bytes);
                signs = signs
                    .checked_add(budget.sign_items)
                    .ok_or("Tick Sign budget overflow")?;
                requests = requests
                    .checked_add(budget.host_requests)
                    .ok_or("Tick request budget overflow")?;
                add(
                    &mut tick_vectors,
                    startup::backing_bytes(gear, &budget, selected_playback)?,
                )?;
            }
            for port in &part.plan.fragments[0].fore_ports {
                if !has_output
                    || port.direction != PortDirection::Output
                    || port.selected_line.is_some()
                {
                    return Err("funded Tick accepts only acknowledged local output Fores".into());
                }
                maximum_deliveries = maximum_deliveries.max(port.item_capacity);
                fore_count = fore_count
                    .checked_add(1)
                    .ok_or("Tick Fore count overflow")?;
            }
        }
        let n = request.plan.plots.len();
        let plans = &plans[..n];
        let lowered = lower_local_fragment_set_layout_from_plans(
            plans,
            FIXED_KERNEL_STORAGE_PROFILE,
            bounds(),
        )
        .map_err(|_| "selected Tick lowering footprint is unsupported")?;
        if lowered.nodes == 0 {
            return Err("funded IMAGE requires actual installed Back".into());
        }
        for _ in 0..fore_count {
            signs = signs
                .checked_add(
                    maximum_deliveries
                        .checked_mul(3)
                        .and_then(|n| n.checked_add(1))
                        .ok_or("Tick Fore Sign overflow")?,
                )
                .ok_or("Tick Fore Sign overflow")?;
        }
        let value_bytes =
            HostedValueStore::canonical_storage_bytes((0..items.max(1)).map(|_| maximum))
                .map_err(|_| "Tick value storage footprint is invalid")?;
        let clock_identity_bytes = timer
            .monotonic_identity_backing_bound(&host.host_id, &host.boot_id)
            .ok_or("selected timer has no finite clock identity preparation bound")?;
        let mut peak = ImageMemoryLease::owned_backing_bytes();
        // Initial lowering and the later exact resource reservation lowering
        // are separately funded while retained kernel identities coexist.
        add(&mut peak, lowered.peak_owned_bytes)?;
        add(&mut peak, lowered.peak_owned_bytes)?;
        add(&mut peak, lowered.retained_owned_bytes)?;
        add(&mut peak, size_of::<InstalledScheduler>())?;
        add(&mut peak, size_of::<KernelTables>())?;
        add(&mut peak, value_bytes)?;
        add(&mut peak, array::<u32>(usize::from(items.max(1)))?)?;
        add(&mut peak, tick_vectors)?;
        add(
            &mut peak,
            HostedSignLog::canonical_storage_bytes(signs, if fore_count == 0 { 0 } else { signs })
                .ok_or("Tick Sign storage footprint invalid")?,
        )?;
        add(&mut peak, array::<HostCallRequest>(requests)?)?;
        add(&mut peak, array::<KernelEvent>(usize::from(signs))?)?;
        add(
            &mut peak,
            array::<crate::body_execution::ObservedKernelEvent>(usize::from(signs))?,
        )?;
        // The reference observation vector coexists until prepared clock ownership replaces it.
        add(
            &mut peak,
            array::<crate::body_execution::ObservedKernelEvent>(usize::from(signs))?,
        )?;
        add(
            &mut peak,
            array::<conduit_core::MonotonicClockIdentity>(usize::from(signs))?,
        )?;
        add(&mut peak, array::<(u32, u64)>(usize::from(signs))?)?;
        add(
            &mut peak,
            clock_identity_bytes
                .checked_mul(usize::from(signs) + 2)
                .ok_or("Tick clock backing overflow")?,
        )?;
        add(&mut peak, diagnostic::CAPACITY)?;
        add(&mut peak, diagnostic::CAPACITY)?;
        metadata(&mut peak, &lowered, n)?;
        fore_backing(&mut peak, request)?;
        lifecycle_backing(&mut peak, request, host)?;
        add(
            &mut peak,
            ledger.tick_reservation_backing_bytes(&reservations[..n])?,
        )?;
        Ok(Self {
            peak_bytes: peak,
            lowering_bytes: lowered.peak_owned_bytes,
            value_bytes,
            clock_identity_bytes,
        })
    }
}
fn metadata(bytes: &mut usize, layout: &FragmentSetLayout, parts: usize) -> Result<(), String> {
    let nodes = usize::from(layout.nodes);
    add(bytes, array::<Option<String>>(MAX_NODES)?)?;
    add(
        bytes,
        array::<Option<super::super::typed_record_back::TypedRecordHost>>(nodes)?,
    )?;
    add(
        bytes,
        array::<Option<super::super::image_text_back::ImageTextHost>>(nodes)?,
    )?;
    add(
        bytes,
        array::<Option<super::super::image_text_record_back::ImageTextRecordHost>>(nodes)?,
    )?;
    add(
        bytes,
        array::<Option<super::super::text_state_back::TextStateHost>>(nodes)?,
    )?;
    add(
        bytes,
        array::<super::super::data_text_host::DataTextGenerationHosts<'_>>(parts)?,
    )?;
    add(
        bytes,
        "value/text"
            .len()
            .checked_mul(parts)
            .ok_or("Tick data metadata overflow")?,
    )?;
    add(bytes, array::<LoweredFragmentActivations>(parts)?)?;
    // Fragment/Plan reference lists plus one table binding reference list.
    add(bytes, array::<&conduit_core::PlanFragment>(parts)?)?;
    add(bytes, array::<&conduit_core::Plan>(parts)?)?;
    add(
        bytes,
        array::<&conduit_plan_lowering::lowering::LoweredPlanFragment>(parts)?,
    )?;
    Ok(())
}
fn fore_backing(bytes: &mut usize, request: &BodyRunRequest<'_>) -> Result<(), String> {
    use conduit_plan_lowering::lowering::LoweredForePort;
    let mut outputs = 0usize;
    for port in request
        .plan
        .plots
        .iter()
        .flat_map(|p| &p.plan.fragments[0].fore_ports)
    {
        let mut descriptor = port
            .front_port_id
            .0
            .len()
            .checked_add(port.value_kind.0.len())
            .ok_or("Tick Fore identity overflow")?;
        if let Some(abnormal) = &port.abnormal_kind {
            add(&mut descriptor, abnormal.0.len())?;
        }
        if let Some(contract) = &port.value_contract {
            add(&mut descriptor, contract.value_kind.0.len())?;
        }
        // ports + output slot each retain an independent descriptor clone.
        add(bytes, descriptor)?;
        add(bytes, descriptor)?;
        add(bytes, size_of::<LoweredForePort>())?;
        add(bytes, size_of::<fore_route::OutputSlots>())?;
        add(bytes, size_of::<(usize, &LoweredForePort)>())?;
        let count = usize::from(port.item_capacity);
        outputs = outputs
            .checked_add(count)
            .ok_or("Tick Fore slot overflow")?;
        add(bytes, array::<ExternalForeDelivery>(count)?)?;
        add(bytes, array::<ExternalForeDelivery>(count)?)?;
        let identity = size_of::<crate::fore_delivery::ExternalForeIdentity>()
            .checked_add(2 * size_of::<usize>())
            .and_then(|n| n.checked_add(port.front_port_id.0.len()))
            .and_then(|n| n.checked_add(port.value_kind.0.len()))
            .ok_or("Tick Fore identity overflow")?;
        // Shared identity may be counted conservatively per escaped slot.
        add(
            bytes,
            identity
                .checked_mul(count)
                .ok_or("Tick Fore identity backing overflow")?,
        )?;
        // Vec-to-Arc publication temporarily retains the source payload too.
        add(
            bytes,
            (port.byte_capacity as usize)
                .checked_mul(count)
                .ok_or("Tick Fore payload staging overflow")?,
        )?;
        add(
            bytes,
            crate::fore_delivery::payload_backing_bytes(port.byte_capacity as usize)
                .and_then(|size| size.checked_mul(count))
                .ok_or("Tick Fore payload backing overflow")?,
        )?;
    }
    let _ = outputs;
    add(
        bytes,
        array::<fore_route::AcknowledgedInput>(
            super::super::keyboard_input_completion::MAX_COMPLETED_INPUT_EVENTS.next_power_of_two(),
        )?,
    )?;
    Ok(())
}
fn lifecycle_backing(
    bytes: &mut usize,
    request: &BodyRunRequest<'_>,
    host: &HostAdvertisement,
) -> Result<(), String> {
    // PlanReady and Started each own a Wake clone; the intermediate and final
    // copies can coexist during the second transition. Immutable worksets share.
    let prior = memory::wake_bytes(request.wake)?;
    add(bytes, prior)?;
    add(bytes, prior)?;
    // Existing finite lifecycle ceilings bound growth of the two cloned vectors.
    let signs = conduit_body::MAX_WAKE_SIGNS;
    let plans = conduit_body::MAX_WAKE_PLANS;
    for _ in 0..2 {
        add(bytes, array::<conduit_core::SignId>(signs)?)?;
        add(bytes, array::<conduit_body::WakeLifecycleEvent>(signs)?)?;
        add(bytes, array::<conduit_body::WakePlan>(plans)?)?;
    }
    let plan_id = request.plan.plan_id.0.len();
    let active_id = "body-play/sha256:".len() + 64;
    // Each transition creates an event/sign and PlanReady adds a plan row.
    for _ in 0..2 {
        add(bytes, plan_id)?;
        add(bytes, 64)?;
        add(bytes, 64)?;
    }
    add(bytes, plan_id)?;
    add(bytes, active_id)?;
    add(bytes, active_id)?;
    // The validation of Started compares a separately derived expected Play.
    add(bytes, request.plan.body_id.as_str().len())?;
    add(bytes, request.plan.wake_id.as_str().len())?;
    add(bytes, plan_id)?;
    add(bytes, active_id)?;
    // A borrowed BodyPlan seal comparison temporarily owns its canonical ID.
    add(bytes, "body-plan/sha256:".len() + 64)?;
    // Bound actual prospective Play and three scoped SignIdentity values.
    add(bytes, request.plan.body_id.as_str().len())?;
    add(bytes, request.plan.wake_id.as_str().len())?;
    add(bytes, plan_id)?;
    add(bytes, active_id)?;
    for _ in 0..3 {
        add(bytes, host.host_id.0.len())?;
        add(bytes, host.boot_id.0.len())?;
        add(bytes, 64)?;
        add(bytes, active_id)?;
    }
    // Wake validation and lifecycle derivation produce one hexadecimal identity
    // at a time. Keep all four validation call results conservatively funded.
    add(bytes, 4 * 64)?;
    Ok(())
}

impl<'a> BodyKernel<'a> {
    pub(crate) fn prepare_funded(
        partitions: &[BodyPlotPlan],
        parent_play: &conduit_core::ActivePlayId,
        has_output: bool,
        funding: &TickPreparationLayout,
        advertisement: &HostAdvertisement,
        selected_playback: Option<&crate::hosted_audio::HostedPlaybackSelection>,
    ) -> Result<Self, String> {
        if partitions
            .iter()
            .flat_map(|part| &part.plan.fragments)
            .any(|fragment| {
                fragment.host_id != advertisement.host_id
                    || fragment.boot_id != advertisement.boot_id
                    || fragment.offer_generation != advertisement.offer_generation
            })
        {
            return Err("funded kernel differs from current Host/Boot/offer".into());
        }
        let mut kernel = Self::prepare_with_live(
            partitions,
            false,
            parent_play,
            &[],
            false,
            has_output,
            None,
            None,
            #[cfg(target_os = "linux")]
            None,
            Some(funding),
        )?;
        kernel.prepare_startup_outputs(partitions, selected_playback)?;
        Ok(kernel)
    }
}
