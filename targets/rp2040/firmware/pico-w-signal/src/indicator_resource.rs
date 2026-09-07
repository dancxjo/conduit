//! Acquired local LED peripheral. No Form evaluation or independent runtime.
mod protocol;

use cyw43::Control;
use embassy_rp::clocks::RoscRng;
use embassy_time::{with_timeout, Duration};
use protocol::{Command, Session, BYTES};

pub async fn run(line: crate::usb::PicoUsbCdcLine, control: &mut Control<'_>) -> ! {
    let (mut sender, mut receiver) = line.class.split();
    let mut rng = RoscRng;
    let mut boot = [0; 16];
    boot[..8].copy_from_slice(&rng.next_u64().to_le_bytes());
    boot[8..].copy_from_slice(&rng.next_u64().to_le_bytes());
    let build = conduit_core::semantic_digest(
        "conduit.device/pico-indicator-build@1",
        env!("CONDUIT_PICO_APPLIANCE_BUILD_ID").as_bytes(),
    );
    // Establish the peripheral's initial state before accepting any acquisition.
    // Failure here cannot yield READY or any successful state acknowledgment.
    if with_timeout(Duration::from_secs(2), control.gpio_set(0, false))
        .await
        .is_err()
    {
        core::future::pending::<()>().await;
    }
    loop {
        receiver.wait_connection().await;
        let mut session = Session::new(boot, build);
        let mut frame = [0; BYTES];
        let mut used = 0;
        let mut packet = [0; 64];
        // A timeout or refusal ends the acquisition; never acknowledge a
        // partially observed or timed-out effect as completed.
        loop {
            let Ok(Ok(length)) =
                with_timeout(Duration::from_secs(30), receiver.read_packet(&mut packet)).await
            else {
                break;
            };
            let mut failed = false;
            for byte in &packet[..length] {
                frame[used] = *byte;
                used += 1;
                if used != BYTES {
                    continue;
                }
                used = 0;
                let response = match session.accept(frame) {
                    Some(Command::Ready(response)) => response,
                    Some(Command::Set {
                        state,
                        acknowledgment,
                    }) => {
                        if with_timeout(Duration::from_secs(2), control.gpio_set(0, state))
                            .await
                            .is_err()
                        {
                            failed = true;
                            break;
                        }
                        acknowledgment
                    }
                    None => {
                        failed = true;
                        break;
                    }
                };
                // Short packets avoid a host waiting for a transfer terminator.
                for chunk in response.chunks(63) {
                    if !matches!(
                        with_timeout(Duration::from_secs(2), sender.write_packet(chunk)).await,
                        Ok(Ok(()))
                    ) {
                        failed = true;
                        break;
                    }
                }
                if failed {
                    break;
                }
            }
            if failed {
                break;
            }
        }
        // Fault cleanup is not a successful requested state or semantic release.
        let _ = with_timeout(Duration::from_secs(2), control.gpio_set(0, false)).await;
        // No in-place reacquisition: require this CDC connection to disappear.
        while sender.dtr() {
            embassy_time::Timer::after_millis(10).await;
        }
    }
}
