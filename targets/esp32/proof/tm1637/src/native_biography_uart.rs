//! Physical owner maintenance before current-Boot Body resume. Archive export
//! is provisional; only a scoped owner ACK can commit the format-only rewrite.
use crate::{
    native_body_join::owner::{BiographyChallenge, BiographyRequest, Gate},
    native_root::NativeRoot,
};
use alloc::vec::Vec;
use embassy_time::{with_timeout, Duration, Instant};
use esp_hal::uart::Uart;
const MAX_COMMAND: usize = 1100;

#[derive(serde::Serialize)]
struct Offered<'a> {
    event: &'static str,
    offer: &'a crate::native_restart::payload::wire::Offer,
    challenge: BiographyChallenge,
}
fn blocking_write(
    uart: &mut Uart<'_, esp_hal::Async>,
    mut bytes: &[u8],
) -> Result<(), &'static str> {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !bytes.is_empty() {
        if Instant::now() >= deadline {
            return Err("biography-uart-timeout");
        }
        if !uart.write_ready() {
            core::hint::spin_loop();
            continue;
        }
        let n = uart.write(bytes).map_err(|_| "biography-uart-write")?;
        if n == 0 {
            return Err("biography-uart-closed");
        }
        bytes = &bytes[n..];
    }
    Ok(())
}
fn json(
    uart: &mut Uart<'_, esp_hal::Async>,
    value: &impl serde::Serialize,
) -> Result<(), &'static str> {
    // Bounded scalar reply, never an entire archive or private checkpoint.
    let mut bytes = [0; 2048];
    let result = (|| {
        let n = serde_json_core::to_slice(value, &mut bytes).map_err(|_| "biography-uart-json")?;
        blocking_write(uart, b"\n")?;
        blocking_write(uart, &bytes[..n])?;
        blocking_write(uart, b"\n")
    })();
    bytes.fill(0);
    result
}
fn offer(
    root: &NativeRoot,
    uart: &mut Uart<'_, esp_hal::Async>,
    gate: &mut Gate,
    export: &crate::native_restart::biography::Export,
) -> Result<(), &'static str> {
    let mut nonce = [0; 32];
    esp_hal::rng::Trng::try_new()
        .map_err(|_| "biography-entropy")?
        .read(&mut nonce);
    gate.challenge(nonce, Instant::now().as_millis())?;
    let (host, boot, _) = root.offer_scope();
    let advertised = export.offer();
    let key = crate::native_restart::biography::root_key()?;
    json(
        uart,
        &Offered {
            event: "biography-offer",
            offer: advertised,
            challenge: BiographyChallenge {
                host: host.as_str().into(),
                boot: boot.as_str().into(),
                body: advertised.body.as_str().into(),
                root_key: key,
                nonce,
                sequence: gate.next_sequence().ok_or("biography-owner-exhausted")?,
                offer: advertised.transcript(&key),
            },
        },
    )
}
/// No control request adds only a two-second boot window. Once a session is
/// entered, normal Body resume waits for completion or an explicit cancellation.
/// No new membership/Boot checkpoint exists during this service.
#[inline(never)]
pub(crate) async fn serve_cold(
    root: &mut NativeRoot,
    uart: &mut Uart<'_, esp_hal::Async>,
) -> Result<(), &'static str> {
    mod owner_key {
        include!(concat!(env!("OUT_DIR"), "/owner-control-key.rs"));
    }
    let mut gate = Gate::new(owner_key::PUBLIC_KEY)?;
    let mut export = None;
    let mut command = Vec::new();
    let mut overflowed = false;
    command
        .try_reserve_exact(MAX_COMMAND)
        .map_err(|_| "biography-command-memory")?;
    let mut deadline = Instant::now() + Duration::from_secs(2);
    #[cfg(feature = "biography-maintenance")]
    crate::native_observed_archive::cold::resume_pending()?;
    blocking_write(uart, b"\n{\"event\":\"biography-window\"}\n")?;
    while Instant::now() < deadline {
        let mut input = [0; 32];
        let count = match with_timeout(Duration::from_millis(10), uart.read_async(&mut input)).await
        {
            Err(_) => continue,
            Ok(Err(_)) => return Err("biography-uart-read"),
            Ok(Ok(n)) => n,
        };
        for byte in &input[..count] {
            if overflowed {
                if *byte == b'\n' {
                    overflowed = false;
                }
                continue;
            }
            if *byte == b'\r' {
                continue;
            }
            if command.len() == MAX_COMMAND {
                command.fill(0);
                command.clear();
                overflowed = *byte != b'\n';
                continue;
            }
            if *byte != b'\n' {
                if !command.is_empty() || *byte == b'B' || *byte == b'O' {
                    command.push(*byte);
                }
                continue;
            }
            if command.is_empty() {
                continue;
            }
            let result = (|| -> Result<bool, &'static str> {
                #[cfg(feature = "biography-maintenance")]
                {
                    use crate::native_observed_archive::{cold, custody_acceptance::wire};
                    if command == b"OBS TIME?" {
                        json(
                            uart,
                            &crate::native_evidence_region::measurement::snapshot(),
                        )?;
                        deadline = Instant::now() + Duration::from_secs(60);
                        return Ok(false);
                    }
                    if let Some(slot) = command.strip_prefix(b"OBS? ") {
                        if slot.len() != 1 || !(b'0'..=b'5').contains(&slot[0]) {
                            return Err("observed-cold-slot");
                        }
                        let slot = slot[0] - b'0';
                        #[derive(serde::Serialize)]
                        struct Offered {
                            event: &'static str,
                            slot: u8,
                            offer: wire::Offer<'static>,
                        }
                        json(
                            uart,
                            &Offered {
                                event: "observed-custody-offer",
                                slot,
                                offer: cold::offer(slot)?,
                            },
                        )?;
                        deadline = Instant::now() + Duration::from_secs(60);
                        return Ok(false);
                    }
                    if let Some(bytes) = command.strip_prefix(b"OBS ACK ") {
                        let request: wire::Request<'_> =
                            serde_json::from_slice(bytes).map_err(|_| "observed-cold-request")?;
                        let slot = cold::acknowledge(&request)?;
                        #[derive(serde::Serialize)]
                        struct Reclaimed {
                            event: &'static str,
                            slot: u8,
                            offer: wire::Hex<32>,
                            reclaimed: bool,
                        }
                        json(
                            uart,
                            &Reclaimed {
                                event: "observed-custody-reclaimed",
                                slot,
                                offer: request.offer,
                                reclaimed: true,
                            },
                        )?;
                        deadline = Instant::now() + Duration::from_secs(60);
                        return Ok(false);
                    }
                }
                if command == b"BIO CANCEL" {
                    return Ok(true);
                }
                if command == b"BIO?" {
                    export = Some(crate::native_restart::biography::prepare()?);
                    deadline = Instant::now() + Duration::from_secs(60);
                    return offer(
                        root,
                        uart,
                        &mut gate,
                        export.as_ref().ok_or("biography-no-offer")?,
                    )
                    .map(|_| false);
                }
                let offered = export.as_ref().ok_or("biography-no-offer")?;
                if command == b"BIO CHALLENGE" {
                    offer(root, uart, &mut gate, offered)?;
                } else if command == b"BIO DATA" {
                    blocking_write(uart, b"\n{\"event\":\"biography-data\"}\n")?;
                    crate::native_restart::biography::export(offered, &mut |bytes| {
                        // Encoder output is bounded to128bytes at this boundary.
                        if bytes.len() > 128 {
                            return Err(crate::native_body_checkpoint::Refusal::Corrupt);
                        }
                        blocking_write(uart, &(bytes.len() as u32).to_le_bytes())
                            .and_then(|_| blocking_write(uart, bytes))
                            .map_err(|_| crate::native_body_checkpoint::Refusal::Storage)
                    })?;
                    blocking_write(uart, &[0; 4])?;
                    blocking_write(uart, b"\n")?;
                } else if let Some(bytes) = command.strip_prefix(b"BIO ACK ") {
                    let request: BiographyRequest =
                        serde_json::from_slice(bytes).map_err(|_| "biography-owner-request")?;
                    let (host, boot, _) = root.offer_scope();
                    let key = crate::native_restart::biography::root_key()?;
                    let authorized = gate.authorize_biography(
                        &request,
                        host.as_str(),
                        boot.as_str(),
                        offered.offer().body.as_str(),
                        &key,
                        &offered.offer().transcript(&key),
                        Instant::now().as_millis(),
                    )?;
                    let receipt = root.endorse_owner_biography(offered, authorized)?;
                    crate::native_restart::biography::retain_receipt(receipt)?;
                    #[derive(serde::Serialize)]
                    struct Retained {
                        event: &'static str,
                        generation: u64,
                    }
                    json(
                        uart,
                        &Retained {
                            event: "biography-retained",
                            generation: crate::native_restart::generation()
                                .ok_or("biography-generation")?,
                        },
                    )?;
                    return Ok(true);
                } else {
                    return Err("biography-command");
                }
                deadline = Instant::now() + Duration::from_secs(60);
                Ok(false)
            })();
            let observed_command = command.starts_with(b"OBS");
            command.fill(0);
            command.clear();
            match result {
                Ok(true) => return Ok(()),
                Ok(false) => {}
                Err(reason) if observed_command => {
                    #[derive(serde::Serialize)]
                    struct Refused {
                        event: &'static str,
                        reason: &'static str,
                    }
                    json(
                        uart,
                        &Refused {
                            event: "native-root-request-refused",
                            reason,
                        },
                    )?;
                }
                Err(_) => blocking_write(uart, b"\n{\"event\":\"biography-refused\"}\n")?,
            }
        }
    }
    if export.is_some() {
        return Err("biography-session-expired");
    }
    Ok(())
}
