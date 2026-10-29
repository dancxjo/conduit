//! Finite immutable challenge rounds for one retained observer Boot.
//! A signed Root replacement, never a local clock or HTTP error, advances a round.
use super::*;
const MAX_ROUNDS: usize = 8;

fn round_path(state: &Path, round: usize, proof: bool) -> std::path::PathBuf {
    let stem = if proof {
        "return-submission-private"
    } else {
        "return-challenge"
    };
    state.join(if round == 0 {
        format!("{stem}.json")
    } else {
        format!("{stem}-{round}.json")
    })
}
fn latest(
    state: &Path,
    bootstrap: &Bootstrap,
    old: &MembershipCredential,
    ad: &HostAdvertisement,
) -> Result<Option<(usize, Challenge)>> {
    let mut last = None;
    let mut gap = false;
    for round in 0..MAX_ROUNDS {
        let path = round_path(state, round, false);
        if !path.exists() {
            gap = true;
            continue;
        }
        if gap {
            return Err("observer return challenge round gap".into());
        }
        let next: Challenge = load(&path)?;
        next.verify(bootstrap, old, ad)?;
        if let Some((_, prior)) = &last {
            replacement(prior, &next)?;
        }
        last = Some((round, next));
    }
    Ok(last)
}
fn replacement(old: &Challenge, new: &Challenge) -> Result<()> {
    if old.challenge.admission_id == new.challenge.admission_id
        || old.challenge.nonce == new.challenge.nonce
        || (old.producer_boot == new.producer_boot
            && new.challenge.issued_at_millis <= old.challenge.expires_at_millis)
    {
        return Err("observer return replacement lacks signed expiry or authority restart".into());
    }
    Ok(())
}
fn begin(
    bootstrap: &Bootstrap,
    old: &MembershipCredential,
    ad: &HostAdvertisement,
    exchange: &mut impl FnMut(&serde_json::Value) -> Result<Vec<u8>>,
) -> Result<Challenge> {
    let request = serde_json::json!({"schema":"conduit.environment/observer-return-challenge-request@1", "body_id":old.body_id, "part_id":old.part_id,"host_advertisement":ad});
    let challenge: Challenge = serde_json::from_slice(&exchange(&request)?)?;
    challenge.verify(bootstrap, old, ad)?;
    Ok(challenge)
}
pub(super) fn run(
    bootstrap: &Bootstrap,
    old: &MembershipCredential,
    ad: &HostAdvertisement,
    state: &Path,
    peer: &str,
) -> Result<()> {
    execute(bootstrap, old, ad, state, |request| {
        super::exchange(peer, request)
    })
}
fn execute(
    bootstrap: &Bootstrap,
    old: &MembershipCredential,
    ad: &HostAdvertisement,
    state: &Path,
    mut exchange: impl FnMut(&serde_json::Value) -> Result<Vec<u8>>,
) -> Result<()> {
    let retained = latest(state, bootstrap, old, ad)?;
    let output = state.join("recorder-binding.json");
    if output.exists() {
        if retained.is_none() {
            return Err("returned binding lacks retained challenge".into());
        }
        let binding = crate::local_recorder::validated_binding(&output)?;
        if binding.root() != &bootstrap.root_key
            || binding.invitation() != &bootstrap.invitation.claim
            || binding.credential().host_id != ad.host_id
            || binding.credential().boot_id != ad.boot_id
            || binding.credential().part_id != old.part_id
        {
            return Err("retained returned binding differs".into());
        }
        return Ok(());
    }
    let (round, challenge) = if let Some((round, prior)) = retained {
        // A lost finish reply may already represent committed membership. Reconcile
        // it before asking Root to change an unfinished challenge's lifetime.
        let recovery_query = query(bootstrap, ad, prior.challenge.nonce);
        if let Ok(bytes) = exchange(&serde_json::to_value(&recovery_query)?) {
            let receipt = serde_json::from_slice(&bytes)?;
            verify_receipt(&receipt, bootstrap, old, &prior, &recovery_query)?;
            return retain(state, bootstrap, ad, &recovery_query, &receipt);
        }
        if round + 1 == MAX_ROUNDS {
            return Err("observer return challenge round capacity".into());
        }
        let next = begin(bootstrap, old, ad, &mut exchange)?;
        if next.transcript() == prior.transcript() {
            (round, prior)
        } else {
            replacement(&prior, &next)?;
            save(&round_path(state, round + 1, false), &next)?;
            (round + 1, next)
        }
    } else {
        let first = begin(bootstrap, old, ad, &mut exchange)?;
        save(&round_path(state, 0, false), &first)?;
        (0, first)
    };
    let recovery_query = query(bootstrap, ad, challenge.challenge.nonce);
    let finish = serde_json::json!({"schema":"conduit.environment/observer-return-request@1", "body_id":old.body_id,"part_id":old.part_id,"admission_id":challenge.challenge.admission_id,"host_advertisement":ad,"nonce":hex(&challenge.challenge.nonce),"signature":hex(&ed25519_dalek::SigningKey::from_bytes(&bootstrap.invitation.secret).sign(&challenge.challenge.signing_transcript()).to_bytes())});
    let path = round_path(state, round, true);
    if path.exists() {
        if load::<serde_json::Value>(&path)? != finish {
            return Err("retained return proof differs".into());
        }
    } else {
        save(&path, &finish)?;
    }
    let receipt = match exchange(&finish) {
        Ok(bytes) => serde_json::from_slice(&bytes)?,
        Err(finish_error) => match exchange(&serde_json::to_value(&recovery_query)?) {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(_) => return Err(finish_error),
        },
    };
    verify_receipt(&receipt, bootstrap, old, &challenge, &recovery_query)?;
    retain(state, bootstrap, ad, &recovery_query, &receipt)
}
fn retain(
    state: &Path,
    bootstrap: &Bootstrap,
    ad: &HostAdvertisement,
    query: &recovery::Request,
    receipt: &recovery::Receipt,
) -> Result<()> {
    let continuity = state.join("continuity-private.json");
    if continuity.exists() {
        if serde_json::to_vec(&load::<Bootstrap>(&continuity)?)? != serde_json::to_vec(bootstrap)? {
            return Err("retained return continuity differs".into());
        }
    } else {
        save(&continuity, bootstrap)?;
    }
    save(
        &state.join("recorder-binding.json"),
        &serde_json::json!({"schema":"conduit.environment/joined-recorder@2", "invitation":bootstrap.invitation.claim,"root_key":bootstrap.root_key,"local_host":ad.host_id,"local_boot":ad.boot_id,"recovery_request":query,"receipt":receipt}),
    )?;
    crate::local_recorder::validated_binding(&state.join("recorder-binding.json"))?;
    Ok(())
}
#[cfg(test)]
#[path = "return_attempt_tests.rs"]
mod tests;
