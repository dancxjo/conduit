//! Stream exact descriptors while borrowing actual retained output software.
//! The read-only inspection grants no native possession or start permission.
use crate::{native_control::write_all, native_root::NativeRoot};
use alloc::vec::Vec;
use esp_hal::{uart::Uart, Async};
const GEAR_BYTES: usize = 32 * 1024;
const FRAME_BYTES: usize = 64 * 1024;
#[derive(serde::Serialize)]
struct Header<'a> {
    event: &'static str,
    host_id: &'a conduit_core::HostId,
    boot_id: &'a conduit_core::BootId,
    offer_generation: conduit_core::OfferGeneration,
    source_document_id: &'a conduit_core::SourceDocumentId,
    checked_plot_id: &'a conduit_core::CheckedPlotId,
    expanded_plot_id: &'a conduit_core::ExpandedPlotId,
    capabilities_bytes: usize,
    authority_grants: [conduit_core::AuthorityGrant; 1],
    capability_possession_issued: bool,
    body_installed: bool,
    native_plan_installed: bool,
}
pub async fn write(root: &mut NativeRoot, uart: &mut Uart<'_, Async>) -> Result<(), ()> {
    let memory = esp_alloc::HEAP.stats();
    if memory.size.saturating_sub(memory.current_usage) < GEAR_BYTES + 8192 {
        return Err(());
    }
    let grant = root.display_policy_grant()?;
    let offers = root.display_software_offers()?;
    let mut buffer = Vec::new();
    buffer.try_reserve_exact(GEAR_BYTES).map_err(|_| ())?;
    buffer.resize(GEAR_BYTES, 0);
    let mut array_bytes = 2_usize;
    for (i, offer) in offers.iter().enumerate() {
        let len = serde_json_core::to_slice(offer, &mut buffer).map_err(|_| ())?;
        array_bytes = array_bytes
            .checked_add(len + usize::from(i != 0))
            .ok_or(())?;
    }
    let header = Header {
        event: "native-display-software-offer",
        host_id: offers.host(),
        boot_id: offers.boot(),
        offer_generation: offers.generation(),
        source_document_id: offers.source(),
        checked_plot_id: offers.checked(),
        expanded_plot_id: offers.expanded(),
        capabilities_bytes: array_bytes,
        authority_grants: [grant],
        capability_possession_issued: false,
        body_installed: false,
        native_plan_installed: false,
    };
    let len = serde_json_core::to_slice(&header, &mut buffer).map_err(|_| ())?;
    drop(header);
    if buffer.get(len.wrapping_sub(1)) != Some(&b'}')
        || array_bytes.checked_add(len + 18).ok_or(())? > FRAME_BYTES
    {
        return Err(());
    }
    embassy_time::with_timeout(embassy_time::Duration::from_secs(12), async {
        write_all(uart, b"\n").await?;
        write_all(uart, &buffer[..len - 1]).await?;
        write_all(uart, b",\"capabilities\":[").await?;
        for (i, offer) in offers.iter().enumerate() {
            let len = serde_json_core::to_slice(offer, &mut buffer).map_err(|_| ())?;
            if i != 0 {
                write_all(uart, b",").await?;
            }
            write_all(uart, &buffer[..len]).await?;
        }
        write_all(uart, b"]}\n").await
    })
    .await
    .map_err(|_| ())?
}
