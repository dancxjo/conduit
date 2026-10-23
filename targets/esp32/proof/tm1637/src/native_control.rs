//! Read-only UART management beneath Root; no Body lifecycle or authority issuer.
use crate::{
    native_command::{Command, CommandBuffer},
    native_root::NativeRoot,
};
use embassy_time::{with_timeout, Duration};
use esp_hal::{uart::Uart, Async};
const PREFIX: &[u8] = b"\n{\"event\":\"native-root-offer\",\"body_installed\":false,\"native_plan_installed\":false,\"image_sha256\":null,\"advertisement\":";
const REFUSED: &[u8] =
    b"{\"event\":\"native-root-request-refused\",\"native_plan_installed\":false}\n";
/// UART0 owns GPIO1 TX/GPIO3 RX. Command storage holds at most 64 bytes;
/// reception has a five-second deadline and each response has two seconds.
pub async fn serve(root: &NativeRoot, mut uart: Uart<'static, Async>) -> Result<(), ()> {
    let mut output = [0; 8192];
    offer(root, &mut uart, &mut output).await?;
    loop {
        let mut byte = [0];
        if uart.read_async(&mut byte).await.map_err(|_| ())? == 0 {
            return Err(());
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
            Ok(Ok(Command::Offer)) => offer(root, &mut uart, &mut output).await?,
            Ok(Err(())) => return Err(()),
            _ => with_timeout(Duration::from_secs(2), write_all(&mut uart, REFUSED))
                .await
                .map_err(|_| ())??,
        }
    }
}
async fn offer(root: &NativeRoot, uart: &mut Uart<'_, Async>, output: &mut [u8]) -> Result<(), ()> {
    // Start a fresh line after any pre-ownership Boot ROM/SDK log fragment.
    let length = root.write_offer(output)?;
    with_timeout(Duration::from_secs(2), async {
        write_all(uart, PREFIX).await?;
        write_all(uart, &output[..length]).await?;
        write_all(uart, b"}\n").await
    })
    .await
    .map_err(|_| ())?
}
async fn write_all(uart: &mut Uart<'_, Async>, mut bytes: &[u8]) -> Result<(), ()> {
    while !bytes.is_empty() {
        let count = uart.write_async(bytes).await.map_err(|_| ())?;
        if count == 0 {
            return Err(());
        }
        bytes = &bytes[count..];
    }
    Ok(())
}
