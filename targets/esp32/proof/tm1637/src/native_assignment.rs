//! Bounded UART staging into independently provisioned ordinary runtime tables.
//! Native ownership, Body/Play and complete runtime admission remain separate.
use alloc::{boxed::Box, vec::Vec};
use conduit_core as c;
use conduit_environment_runtime::appliance::{
    assigned::ValidatedAssignedAppliance, assignment_transfer as t,
};
use embassy_time::{with_timeout, Duration};
use esp_hal::{uart::Uart, Async};

pub async fn receive(
    root: &mut crate::native_root::NativeRoot,
    uart: &mut Uart<'_, Async>,
    buffer: &mut [u8],
) -> Result<(), ()> {
    with_timeout(
        Duration::from_secs(2),
        crate::native_control::write_all(
            uart,
            b"\n{\"event\":\"native-assignment-transfer-ready\"}\n",
        ),
    )
    .await
    .map_err(|_| ())??;
    let input = with_timeout(Duration::from_secs(5), async {
        read_exact(uart, buffer.get_mut(..12).ok_or(())?).await?;
        let size = usize::from(u16::from_le_bytes([buffer[8], buffer[9]]));
        if !(169..=t::MAXIMUM_WIRE_BYTES).contains(&size) || size > buffer.len() {
            return Err(());
        }
        read_exact(uart, &mut buffer[12..size]).await?;
        Ok(size)
    })
    .await;
    let result = match input {
        Ok(Ok(size)) => stage(root, &buffer[..size]),
        _ => Err("ingress"),
    };
    let stats = esp_alloc::HEAP.stats();
    #[derive(serde::Serialize)]
    struct Reply<'a> {
        event: &'static str,
        staged: bool,
        refusal: Option<&'a str>,
        plan: Option<[u8; 16]>,
        fragment: Option<[u8; 16]>,
        body_installed: bool,
        native_plan_installed: bool,
        capability_possession_issued: bool,
        root_heap_used_bytes: usize,
        root_heap_peak_bytes: usize,
    }
    let reply = Reply {
        event: "native-assignment-stage",
        staged: result.is_ok(),
        refusal: result.as_ref().err().copied(),
        plan: result.as_ref().ok().map(|view| view.plan.0),
        fragment: result.as_ref().ok().map(|view| view.fragment.0),
        body_installed: false,
        native_plan_installed: false,
        capability_possession_issued: false,
        root_heap_used_bytes: stats.current_usage,
        root_heap_peak_bytes: stats.max_usage,
    };
    let size = serde_json_core::to_slice(&reply, buffer).map_err(|_| ())?;
    with_timeout(Duration::from_secs(2), async {
        crate::native_control::write_all(uart, b"\n").await?;
        crate::native_control::write_all(uart, &buffer[..size]).await?;
        crate::native_control::write_all(uart, b"\n").await
    })
    .await
    .map_err(|_| ())?
}

#[inline(never)]
fn stage(
    root: &mut crate::native_root::NativeRoot,
    wire: &[u8],
) -> Result<c::AssignedPlanView, &'static str> {
    if root.assignment.is_some() {
        return Err("already-staged");
    }
    if root.software().is_none() {
        return Err("software-unavailable");
    }
    let resident = crate::native_software::resident().map_err(|_| "provisioning")?;
    // This bounds staging storage only, not the future kernel/Body closure.
    let memory = esp_alloc::HEAP.stats();
    let needed = t::MAXIMUM_RECORDS * core::mem::size_of::<c::AssignedRecord<'_>>()
        + core::mem::size_of::<ValidatedAssignedAppliance>()
        + 512;
    if memory.size.saturating_sub(memory.current_usage) < needed {
        return Err("staging-capacity");
    }
    let mut slots = Vec::new();
    slots
        .try_reserve_exact(t::MAXIMUM_RECORDS)
        .map_err(|_| "staging-capacity")?;
    slots.resize(
        t::MAXIMUM_RECORDS,
        c::AssignedRecord {
            tag: 0,
            payload: &[],
        },
    );
    let decoded = t::decode(
        wire,
        resident.source,
        crate::native_software::computation_records(),
        &mut slots,
    )
    .map_err(|_| "transfer")?;
    let (host, boot, _) = root.offer_scope();
    let required = crate::appliance_layout::requirements(
        c::AssignedIdentity::from_text(host.as_str()),
        c::AssignedIdentity::from_text(boot.as_str()),
    );
    let admitted = admit(decoded, required, &resident)?;
    let identity = admitted.identity();
    root.assignment = Some(admitted);
    Ok(identity)
}
#[inline(never)]
fn admit(
    decoded: t::BorrowedAssignment<'_, '_>,
    required: c::AssignedPlanRequirements<'_>,
    resident: &conduit_environment_runtime::appliance::assigned::ResidentAppliance<'_>,
) -> Result<Box<ValidatedAssignedAppliance>, &'static str> {
    ValidatedAssignedAppliance::admit_records_boxed(
        decoded.header,
        decoded.records,
        c::AssignedPlanMaxima {
            encoded_bytes: 48 * 1024,
            runtime_state_bytes: 32 * 1024,
            counts: resident.layout.counts,
        },
        required,
        resident,
    )
    .map_err(|_| "assignment")
}

async fn read_exact(uart: &mut Uart<'_, Async>, mut output: &mut [u8]) -> Result<(), ()> {
    while !output.is_empty() {
        let count = uart.read_async(output).await.map_err(|_| ())?;
        if count == 0 {
            return Err(());
        }
        output = &mut output[count..];
    }
    Ok(())
}
