//! Recovery of the attended management carrier inside its admitted job lifetime.
//! Never replays Prepare/Start/Release/Cancel. A new connection is usable only
//! after a fresh inspection verifies the already admitted key and exact scope.
use crate::{
    admission::Authority, native_owner_control::Operation, owner_exchange, planning::Prepared,
    transport::Line, Result,
};
use serde_json::Value;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
fn next_sequence(offer: &Value, local: u64, host: &str, boot: &str) -> Result<u64> {
    if offer["advertisement"]["host_id"] != host
        || offer["advertisement"]["boot_id"] != boot
        || offer["owner_control_configured"] != true
    {
        return Err("reconnected Root differs from retained Host/Boot scope".into());
    }
    // Every command consumes its local counter before transmission. A lost
    // reply cannot put Root ahead of that retained next counter. Metadata must
    // not trick this owner into signing an arbitrarily high counter.
    let reported = offer["owner_control_sequence"]
        .as_u64()
        .ok_or("Root replay counter")?;
    if local == 0 || local == u64::MAX || reported >= local {
        return Err("Root counter requires explicit ownership reconciliation".into());
    }
    Ok(local)
}
#[allow(clippy::too_many_arguments)]
pub fn recover(
    line: &mut Line,
    authority: &Authority,
    prepared: &Prepared,
    key: &[u8; 32],
    sequence: &mut u64,
    deadline: Instant,
    stop: &AtomicBool,
) -> Result<()> {
    if !line.can_reconnect() {
        return Err("selected UART has no reconnect policy".into());
    }
    let maximum_attempts = deadline.saturating_duration_since(Instant::now()).as_secs() / 2 + 2;
    let mut delay = Duration::from_secs(2);
    eprintln!(
        "{}",
        serde_json::json!({"event":"owner-link-unavailable","job_retained":true})
    );
    for attempt in 0..maximum_attempts {
        if Instant::now() >= deadline || stop.load(Ordering::Relaxed) {
            break;
        }
        let candidate = (|| -> Result<bool> {
            line.reconnect()?;
            let offer = line.command("offer", "native-root-offer")?;
            *sequence = next_sequence(
                &offer,
                *sequence,
                prepared.active.host_id.as_str(),
                prepared.active.boot_id.as_str(),
            )?;
            let current = *sequence;
            *sequence = sequence.checked_add(1).ok_or("owner sequence exhausted")?;
            let exchange = owner_exchange::command(
                line,
                authority,
                prepared,
                key,
                Operation::Inspect,
                current,
            )?;
            Ok(exchange
                .receipt
                .and_then(|receipt| receipt.snapshot)
                .is_some())
        })();
        match candidate {
            Ok(true) => {
                println!(
                    "{}",
                    serde_json::json!({"event":"owner-link-recovered","attempts":attempt+1,"host":prepared.active.host_id,"boot":prepared.active.boot_id,"next_sequence":sequence})
                );
                return Ok(());
            }
            Ok(false) => (), // No signed publication yet: do not claim recovery.
            Err(error) if crate::transport::is_recoverable(error.as_ref()) => (),
            Err(error) => return Err(error),
        }
        let until = Instant::now()
            .checked_add(delay)
            .unwrap_or(deadline)
            .min(deadline);
        while Instant::now() < until && !stop.load(Ordering::Relaxed) {
            std::thread::sleep(
                until
                    .saturating_duration_since(Instant::now())
                    .min(Duration::from_millis(100)),
            );
        }
        delay = (delay * 2).min(Duration::from_secs(30));
    }
    Err(
        "owner carrier recovery lifetime exhausted or stopped; remote custody remains unresolved"
            .into(),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reported_counter_cannot_replace_retained_sequence_or_scope() {
        let offer = serde_json::json!({"advertisement":{"host_id":"host/a","boot_id":"boot/a"},"owner_control_configured":true,"owner_control_sequence":42});
        assert!(next_sequence(&offer, 40, "host/a", "boot/a").is_err());
        assert_eq!(next_sequence(&offer, 46, "host/a", "boot/a").unwrap(), 46);
        assert!(next_sequence(&offer, 46, "host/b", "boot/a").is_err());
        assert!(next_sequence(&offer, 46, "host/a", "boot/b").is_err());
        let mut exhausted = offer;
        exhausted["owner_control_sequence"] = u64::MAX.into();
        assert!(next_sequence(&exhausted, 46, "host/a", "boot/a").is_err());
    }
}
