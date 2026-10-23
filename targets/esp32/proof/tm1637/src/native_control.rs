//! UART management and autonomous Root setup polling; no Body lifecycle.
//! Root setup advances independently of UART input through its selected kernel.
use crate::{
    native_command::{Command, CommandBuffer},
    native_root::NativeRoot,
};
use embassy_time::{with_timeout, Duration};
use esp_hal::{uart::Uart, Async};
const PREFIX: &[u8] = b"\n{\"event\":\"native-root-offer\",\"body_installed\":false,\"native_plan_installed\":false,\"image_sha256\":null,\"network_configuration\":\"";
const REFUSED: &[u8] =
    b"{\"event\":\"native-root-request-refused\",\"native_plan_installed\":false}\n";
/// UART0 owns GPIO1 TX/GPIO3 RX. Command storage holds at most 64 bytes;
/// reception has a five-second deadline and each response has two seconds.
pub async fn serve(root: &mut NativeRoot, mut uart: Uart<'static, Async>) -> Result<(), ()> {
    let mut output = [0; 8192];
    offer(root, &mut uart, &mut output).await?;
    loop {
        let mut byte = [0];
        root.advance_setup().await;
        match with_timeout(Duration::from_millis(100), uart.read_async(&mut byte)).await {
            Err(_) => continue,
            Ok(Ok(0)) | Ok(Err(_)) => return Err(()),
            Ok(Ok(_)) => {}
        }
        let command = with_timeout(Duration::from_secs(5), async {
            let mut command = CommandBuffer::new();
            loop {
                if let Some(command) = command.push(byte[0]) {
                    return Ok(command);
                }
                if uart.read_async(&mut byte).await.map_err(|_| ())? == 0 {
                    return Err(());
                }
            }
        })
        .await;
        match command {
            Ok(Ok(Command::Admission)) => {
                // Malformed proof requests retire only this exchange, not Root.
                if crate::native_admission_wire::serve(root, &mut uart, &mut output)
                    .await
                    .is_err()
                {
                    with_timeout(Duration::from_secs(2), write_all(&mut uart, REFUSED))
                        .await
                        .map_err(|_| ())??;
                }
            }
            Ok(Ok(Command::AssignmentTransfer)) => {
                crate::native_assignment::receive(root, &mut uart, &mut output).await?
            }
            Ok(Ok(Command::Offer)) => offer(root, &mut uart, &mut output).await?,
            Ok(Ok(Command::SoftwareOffer)) => {
                crate::native_software_offer::write(root, &mut uart).await?
            }
            Ok(Ok(Command::AppliancePolicy)) => {
                let encoded = crate::native_appliance_policy::encode(root, &mut output);
                with_timeout(Duration::from_secs(2), async {
                    write_all(&mut uart, b"\n").await?;
                    match encoded {
                        Ok(len) => {
                            write_all(&mut uart, &output[..len]).await?;
                            write_all(&mut uart, b"\n").await
                        }
                        Err(()) => write_all(&mut uart, REFUSED).await,
                    }
                })
                .await
                .map_err(|_| ())??;
            }
            Ok(Err(())) => return Err(()),
            _ => with_timeout(Duration::from_secs(2), write_all(&mut uart, REFUSED))
                .await
                .map_err(|_| ())??,
        }
    }
}
async fn offer(
    root: &mut NativeRoot,
    uart: &mut Uart<'_, Async>,
    output: &mut [u8],
) -> Result<(), ()> {
    let configuration = root.control_status().as_bytes();
    let (failure_phase, failure_reason) = root.control_failure();
    let (steps, dispatches, receipt) = root.control_progress();
    let mut binding = [0; 512];
    let len = serde_json_core::to_slice(&root.control_binding(), &mut binding).map_err(|_| ())?;
    let binding = core::str::from_utf8(&binding[..len]).map_err(|_| ())?;
    let memory = crate::sdk_memory::stats();
    let root_memory = esp_alloc::HEAP.stats();
    let mut memory_json = heapless::String::<1280>::new();
    use core::fmt::Write;
    write!(
        memory_json,
        "\",\"native_sdk_heap_bytes\":{},\"native_sdk_heap_used_bytes\":{},\"native_sdk_heap_peak_bytes\":{},\"native_root_heap_bytes\":{},\"native_root_heap_used_bytes\":{},\"native_root_heap_peak_bytes\":{},\"prechecked_network_source\":\"{}\",\"network_plan_preparation\":\"{}\",\"native_network_ready\":{},\"network_control_installed\":{},\"network_control_failure_phase\":\"{}\",\"network_control_failure_reason\":\"{}\",\"network_control_steps\":{},\"network_control_dispatches\":[{},{},{}],\"network_control_receipt\":{},\"network_control_kernel_bytes\":{},\"network_control_binding\":{},\"assignment_staged\":{},\"advertisement\":",
        memory.size, memory.current_usage, memory.max_usage, root_memory.size, root_memory.current_usage, root_memory.max_usage, root.network_source().source_document_id.as_str(), root.network_plan_status(), root.network_ready(), root.control_installed(), failure_phase, failure_reason, steps, dispatches[0], dispatches[1], dispatches[2], receipt, conduit_network_join_call::CONTROL_KERNEL_BYTES, binding, root.assignment.is_some()
    )
    .map_err(|_| ())?;
    // Start a fresh line after any pre-ownership Boot ROM/SDK log fragment.
    let length = root.write_offer(output)?;
    with_timeout(Duration::from_secs(2), async {
        write_all(uart, PREFIX).await?;
        write_all(uart, configuration).await?;
        write_all(uart, memory_json.as_bytes()).await?;
        write_all(uart, &output[..length]).await?;
        write_all(uart, b"}\n").await
    })
    .await
    .map_err(|_| ())?
}
pub(super) async fn write_all(uart: &mut Uart<'_, Async>, mut bytes: &[u8]) -> Result<(), ()> {
    while !bytes.is_empty() {
        let count = uart.write_async(bytes).await.map_err(|_| ())?;
        if count == 0 {
            return Err(());
        }
        bytes = &bytes[count..];
    }
    Ok(())
}
