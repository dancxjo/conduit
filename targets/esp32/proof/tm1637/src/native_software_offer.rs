//! Bounded streaming over actual retained readiness; no copied offer catalogue.
use crate::{native_control::write_all, native_root::NativeRoot};
use esp_hal::{uart::Uart, Async};
use serde::Serialize;
#[derive(Serialize)]
struct Header<'a> {
    event: &'static str,
    host_id: &'a conduit_core::HostId,
    boot_id: &'a conduit_core::BootId,
    offer_generation: conduit_core::OfferGeneration,
    source_identity: [u8; 32],
    count: u64,
    period_ms: u64,
    capabilities_bytes: usize,
    body_installed: bool,
    native_plan_installed: bool,
}
/// One <=64 KiB reply, bounded by twelve seconds at the existing UART rate.
/// Root remains the owner throughout the reply; descriptors never start work.
pub async fn write(root: &NativeRoot, uart: &mut Uart<'_, Async>) -> Result<(), ()> {
    let owner = root.software().ok_or(())?;
    let offers = crate::native_software::offers(owner)?;
    let (host_id, boot_id, offer_generation) = root.offer_scope();
    let (source_identity, cadence) = crate::native_software::source_facts();
    let header = Header {
        event: "native-software-offer",
        host_id,
        boot_id,
        offer_generation,
        source_identity,
        count: cadence.count,
        period_ms: cadence.period_ms,
        capabilities_bytes: offers.array_bytes(),
        body_installed: root.body_installed(),
        native_plan_installed: false,
    };
    let mut buffer = [0; 1024];
    let len = serde_json_core::to_slice(&header, &mut buffer).map_err(|_| ())?;
    if buffer.get(len.wrapping_sub(1)) != Some(&b'}')
        || offers.array_bytes().checked_add(len + 18).ok_or(())? > 64 * 1024
    {
        return Err(());
    }
    embassy_time::with_timeout(embassy_time::Duration::from_secs(12), async {
        write_all(uart, b"\n").await?;
        write_all(uart, &buffer[..len - 1]).await?;
        write_all(uart, b",\"capabilities\":[").await?;
        for (index, json) in offers.iter().enumerate() {
            if index != 0 {
                write_all(uart, b",").await?;
            }
            write_all(uart, json).await?;
        }
        write_all(uart, b"]}\n").await
    })
    .await
    .map_err(|_| ())?
}
