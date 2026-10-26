//! Public read-only replay. A signed projection is neither membership nor an
//! invitation; the consumer must validate its independently owned Body scope.
use core::fmt::Write;
use ed25519_dalek::{Signer, SigningKey};
use static_cell::StaticCell;
static SIGNER: StaticCell<SigningKey> = StaticCell::new();
/// Only initialized Root admission installs this Boot's actual signing key.
/// The private key never crosses a callback, response or flash boundary.
// StaticCell intentionally has no retrieval entrance: retain a separate
// immutable reference for signing without holding the flash critical section.
static KEY: embassy_sync::blocking_mutex::Mutex<
    embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
    core::cell::Cell<Option<&'static SigningKey>>,
> = embassy_sync::blocking_mutex::Mutex::new(core::cell::Cell::new(None));
pub fn install(key: SigningKey) -> Result<(), ()> {
    let key = SIGNER.try_init(key).ok_or(())?;
    KEY.lock(|slot| slot.set(Some(key)));
    Ok(())
}
fn hex(output: &mut heapless::String<2048>, bytes: &[u8]) -> Option<()> {
    for byte in bytes {
        write!(output, "{byte:02x}").ok()?;
    }
    Some(())
}
#[inline(never)]
pub fn response(after: u32) -> Option<heapless::String<2048>> {
    let batch = crate::native_observed_archive::copy(after).ok()?;
    let records = &batch.records[..batch.used];
    let digest = crate::native_observed_wire::transcript(
        &batch.scope,
        &batch.forms,
        after,
        batch.committed,
        records,
    );
    let key = KEY.lock(|slot| slot.get())?;
    let signature = key.sign(&digest).to_bytes();
    let mut output = heapless::String::new();
    // Close-delimited HTTP/1.0: one prebounded response, never a growing body.
    output.push_str("HTTP/1.0 200 OK\r\nContent-Type: application/json\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{\"profile\":\"observed-history@1\",\"scope\":\"").ok()?;
    hex(&mut output, &batch.scope)?;
    output.push_str("\",\"forms\":\"").ok()?;
    hex(&mut output, &batch.forms)?;
    write!(
        output,
        "\",\"after\":{after},\"committed\":{},\"records\":\"",
        batch.committed
    )
    .ok()?;
    hex(&mut output, records)?;
    output.push_str("\",\"signature\":\"").ok()?;
    hex(&mut output, &signature)?;
    output.push_str("\"}\n").ok()?;
    Some(output)
}
