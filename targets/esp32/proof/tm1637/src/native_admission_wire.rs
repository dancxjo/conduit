//! Bounded UART carrier for the existing borrowed Body admission documents.
use crate::native_carrier::Carrier;
use crate::native_root::NativeRoot;
use conduit_body::{PicoAdmissionChallenge, PicoAdmissionProof, PICO_ADMISSION_PROTOCOL};
use embassy_time::{with_timeout, Duration};

pub async fn serve(
    root: &mut NativeRoot,
    uart: &mut Carrier<'_>,
    buffer: &mut [u8],
) -> Result<(), ()> {
    let length = root.write_admission_offer(buffer)?;
    with_timeout(Duration::from_secs(2), async {
        crate::native_control::write_all(uart, b"\n").await?;
        crate::native_control::write_all(uart, &buffer[..length]).await?;
        crate::native_control::write_all(uart, b"\n").await
    })
    .await
    .map_err(|_| ())??;
    let (input, output) = buffer.split_at_mut(1024);
    let used = with_timeout(Duration::from_secs(5), async {
        let mut used = 0;
        loop {
            let slot = input.get_mut(used..used + 1).ok_or(())?;
            if uart.read_async(slot).await.map_err(|_| ())? != 1 {
                return Err(());
            }
            if slot[0] == b'\n' {
                return Ok(used);
            }
            used += 1;
        }
    })
    .await
    .map_err(|_| ())??;
    let (challenge, consumed) =
        serde_json_core::from_slice::<PicoAdmissionChallenge<'_>>(&input[..used])
            .map_err(|_| ())?;
    if input[consumed..used]
        .iter()
        .any(|b| !b.is_ascii_whitespace())
    {
        return Err(());
    }
    let signature = root.sign_admission(&challenge)?;
    let proof = PicoAdmissionProof {
        protocol: PICO_ADMISSION_PROTOCOL,
        admission_id: challenge.admission_id,
        body_id: challenge.body_id,
        host_id: challenge.host_id,
        boot_id: challenge.boot_id,
        nonce: challenge.nonce,
        signature: &signature,
    };
    let length = serde_json_core::to_slice(&proof, output).map_err(|_| ())?;
    with_timeout(Duration::from_secs(2), async {
        crate::native_control::write_all(uart, &output[..length]).await?;
        crate::native_control::write_all(uart, b"\n").await
    })
    .await
    .map_err(|_| ())?
}
