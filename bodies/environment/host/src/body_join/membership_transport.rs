//! Existing attended admission transport for an already prepared membership
//! request. No Host identity, invitation, credential or authority is minted here.
//! The installed product must independently verify the pinned Root receipt.
use super::*;
use conduit_body::{
    PortableMembershipAdmissionReceipt as Receipt, PortableMembershipAdmissionRequest as Request,
};

const QUEUED: &str = "conduit.environment/body-join-queued@1";

fn queued(bytes: &[u8], expected: Option<u32>) -> Result<Ticket> {
    valid_ticket(serde_json::from_slice(bytes)?, expected)
}
fn valid_ticket(ticket: Ticket, expected: Option<u32>) -> Result<Ticket> {
    if ticket.schema != QUEUED
        || ticket.ticket == 0
        || expected.is_some_and(|expected| expected != ticket.ticket)
    {
        return Err("membership transport ticket changed".into());
    }
    Ok(ticket)
}
fn request(path: &Path) -> Result<Request> {
    let value: Request = load(path)?;
    // Use the existing wire bound, even when a private input was pretty-printed.
    checked(Request::decode(&serde_json::to_vec(&value)?))
}
fn candidate(path: &Path, request: &Request) -> Result<()> {
    let value: Receipt = load(path)?;
    let value = checked(Receipt::decode(&serde_json::to_vec(&value)?))?;
    checked(value.validate_against(request))
}
pub(crate) fn run(input: &Path, state: &Path, peer: &str) -> Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    let request = request(input)?;
    if !state.exists() {
        std::fs::DirBuilder::new().mode(0o700).create(state)?;
        std::fs::File::open(
            state
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
        )?
        .sync_all()?;
    }
    private(state, true)?;
    let submission = state.join("submission-private.json");
    if submission.exists() {
        if self::request(&submission)? != request {
            return Err(
                "retained membership transport request differs; original attempt preserved".into(),
            );
        }
    } else {
        // Retain the exact proof before a POST may have an irreversible effect.
        save(&submission, &request)?;
    }
    let output = state.join("membership-candidate-receipt.json");
    if output.exists() {
        return candidate(&output, &request);
    }
    let pending = state.join("pending-ticket.json");
    let ticket = if pending.exists() {
        valid_ticket(load(&pending)?, None)?
    } else {
        // A lost reply reuses exactly this original proof. The Root's existing
        // finite request-digest cache deduplicates it while retained. Refusal or
        // an evicted receipt never authorizes a fresh invitation/Boot/attempt.
        let (status, bytes) = http(peer, "POST", "/body/join", &serde_json::to_vec(&request)?)?;
        if status != 202 {
            return Err(
                format!("membership submission HTTP {status}; original proof retained").into(),
            );
        }
        let ticket = queued(&bytes, None)?;
        save(&pending, &ticket)?;
        ticket
    };
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let response = http(
            peer,
            "GET",
            &format!("/body/join/result?id={}", ticket.ticket),
            &[],
        );
        let (status, bytes) = match response {
            Ok(value) => value,
            Err(_) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_secs(1));
                continue;
            }
            Err(error) => return Err(error),
        };
        match status {
            200 => {
                let receipt = checked(Receipt::decode(&bytes))?;
                checked(receipt.validate_against(&request))?;
                save(&output, &receipt)?;
                // Correlation is not authentication. No recorder binding or
                // installed membership is written by this transport helper.
                return Ok(());
            }
            202 if Instant::now() < deadline => {
                queued(&bytes, Some(ticket.ticket))?;
                std::thread::sleep(Duration::from_millis(250));
            }
            _ => return Err(format!(
                "membership result HTTP {status}; exact pending ticket retained for reconciliation"
            )
            .into()),
        }
    }
}

#[cfg(test)]
#[path = "membership_transport_tests.rs"]
mod tests;
