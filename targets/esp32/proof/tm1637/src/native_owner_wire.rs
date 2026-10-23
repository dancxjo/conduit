//! Bounded authenticated owner control; preparation stays dormant until start.
use crate::{native_owner_control as owner, native_root::NativeRoot};
#[path = "native_owner_reply.rs"]
mod reply;
use embassy_time::{with_timeout, Duration};
use esp_hal::{uart::Uart, Async};
const READY: &[u8] =
    b"\n{\"event\":\"native-owner-control-ready\",\"maximum_request_bytes\":2048,\"protocol\":2,\"ingress_credit_bytes\":64}\n";
async fn read_exact(uart: &mut Uart<'_, Async>, mut bytes: &mut [u8]) -> Result<(), ()> {
    while !bytes.is_empty() {
        let count = uart.read_async(bytes).await.map_err(|_| ())?;
        if count == 0 {
            return Err(());
        }
        bytes = &mut bytes[count..];
    }
    Ok(())
}
/// One credit covers at most 64 bytes, below the UART FIFO extent. The owner
/// cannot overrun ingress while another cooperative task performs finite work.
/// Credits confer no authority; the complete command is authenticated afterward.
async fn read_credited(uart: &mut Uart<'_, Async>, mut bytes: &mut [u8]) -> Result<(), ()> {
    const CREDIT: &[u8] = b"\n{\"event\":\"native-owner-ingress-credit\",\"maximum_bytes\":64}\n";
    while !bytes.is_empty() {
        crate::native_control::write_all(uart, CREDIT).await?;
        let length = bytes.len().min(64);
        let (chunk, remaining) = bytes.split_at_mut(length);
        read_exact(uart, chunk).await?;
        bytes = remaining;
    }
    Ok(())
}

pub async fn receive(
    root: &mut NativeRoot,
    uart: &mut Uart<'_, Async>,
    buffer: &mut [u8],
) -> Result<(), ()> {
    with_timeout(
        Duration::from_secs(2),
        crate::native_control::write_all(uart, READY),
    )
    .await
    .map_err(|_| ())??;
    let received = with_timeout(Duration::from_secs(5), async {
        let mut length = [0; 2];
        read_exact(uart, &mut length).await?;
        let length = usize::from(u16::from_le_bytes(length));
        if length == 0 || length > owner::MAXIMUM_REQUEST_BYTES {
            return Err(());
        }
        read_credited(uart, buffer.get_mut(..length).ok_or(())?).await?;
        let mut signature = [0; 64];
        read_credited(uart, &mut signature).await?;
        Ok((length, signature))
    })
    .await;
    let result = match received {
        Ok(Ok((length, signature))) => authenticate(root, &buffer[..length], &signature),
        _ => Err(owner::Refusal::Malformed),
    };
    let authenticated = result.is_ok();
    let mut inspect_command = None;
    let mut acknowledgement = None;
    let mut operation = None;
    let mut applied = false;
    let refusal = match result {
        Ok(token) => {
            let selected = token.operation();
            operation = Some(selected);
            match selected {
                owner::Operation::Inspect => {
                    inspect_command = token.into_request().transcript().ok();
                    None
                }
                owner::Operation::Reserve => match root.reserve_owner(token) {
                    Ok(()) => {
                        applied = true;
                        None
                    }
                    Err(reason) => Some(reason),
                },
                owner::Operation::Prepare => match root.authorize_native_preparation(token) {
                    Ok(permit) => match receive_assignment(root, permit, uart, buffer).await {
                        Ok(()) => {
                            applied = true;
                            None
                        }
                        Err(reason) => Some(reason),
                    },
                    Err(reason) => Some(reason),
                },
                owner::Operation::Start | owner::Operation::Release | owner::Operation::Cancel => {
                    match root.apply_owner_lifecycle(token).await {
                        Ok(receipt) => {
                            acknowledgement = Some(receipt);
                            applied = true;
                            None
                        }
                        Err(reason) => Some(reason),
                    }
                }
            }
        }
        Err(owner::Refusal::Disabled) => Some("owner-control-disabled"),
        Err(owner::Refusal::InvalidKey) => Some("invalid-owner-key"),
        Err(owner::Refusal::Malformed) => Some("malformed"),
        Err(owner::Refusal::WrongHost) => Some("wrong-host"),
        Err(owner::Refusal::StaleBoot) => Some("stale-boot"),
        Err(owner::Refusal::WrongGeneration) => Some("wrong-generation"),
        Err(owner::Refusal::Replay) => Some("replay"),
        Err(owner::Refusal::Signature) => Some("signature"),
    };
    let encoded = reply::encode(
        root,
        buffer,
        reply::Outcome {
            authenticated,
            operation,
            applied,
            refusal,
            inspect_command,
            acknowledgement,
        },
    )?;
    let (publication, output) = buffer.split_at(4096);
    with_timeout(Duration::from_secs(2), async {
        crate::native_control::write_all(uart, b"\n").await?;
        crate::native_control::write_all(uart, &output[..encoded.metadata_bytes]).await?;
        crate::native_control::write_all(uart, b"\n").await?;
        crate::native_control::write_all(uart, &publication[..encoded.publication_bytes]).await
    })
    .await
    .map_err(|_| ())?
}

#[inline(never)]
fn authenticate<'a>(
    root: &mut NativeRoot,
    input: &'a [u8],
    signature: &[u8; 64],
) -> Result<owner::AuthenticatedRequest<'a>, owner::Refusal> {
    match serde_json_core::from_slice::<owner::Request<'_>>(input) {
        Ok((request, consumed)) if input[consumed..].iter().all(u8::is_ascii_whitespace) => {
            root.authenticate_owner(request, signature)
        }
        _ => Err(owner::Refusal::Malformed),
    }
}

async fn receive_assignment(
    root: &mut NativeRoot,
    permit: crate::native_owner_reservation::PreparationPermit,
    uart: &mut Uart<'_, Async>,
    buffer: &mut [u8],
) -> Result<(), &'static str> {
    use conduit_environment_runtime::appliance::assignment_transfer::MAXIMUM_WIRE_BYTES;
    with_timeout(
        Duration::from_secs(2),
        crate::native_control::write_all(
            uart,
            b"\n{\"event\":\"native-owner-assignment-ready\",\"maximum_bytes\":8192}\n",
        ),
    )
    .await
    .map_err(|_| "transfer-ready-timeout")?
    .map_err(|_| "transfer-ready")?;
    let length = with_timeout(Duration::from_secs(5), async {
        let mut bytes = [0; 2];
        read_exact(uart, &mut bytes).await?;
        let length = usize::from(u16::from_le_bytes(bytes));
        if length == 0 || length > MAXIMUM_WIRE_BYTES {
            return Err(());
        }
        read_credited(uart, buffer.get_mut(..length).ok_or(())?).await?;
        Ok(length)
    })
    .await
    .map_err(|_| "transfer-timeout")?
    .map_err(|_| "transfer-frame")?;
    root.prepare_native_assignment(permit, &buffer[..length])
        .await
}
