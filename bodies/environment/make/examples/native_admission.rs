//! Host half of the attended physical proof; used only through xtask.
use conduit_body::*;
use conduit_core as c;
use std::{
    error::Error,
    io::{self, BufRead, Read, Write},
};
fn line(input: &mut impl BufRead) -> Result<String, Box<dyn Error>> {
    let mut line = String::new();
    input.take(8193).read_line(&mut line)?;
    if line.len() > 8192 || !line.ends_with('\n') {
        return Err("bounded complete line required".into());
    }
    Ok(line)
}
fn main() -> Result<(), Box<dyn Error>> {
    let mut input = io::stdin().lock();
    let offered: PicoAdmissionAdvertisement = serde_json::from_str(&line(&mut input)?)?;
    if offered.protocol != PICO_ADMISSION_PROTOCOL
        || offered.verifying_key.len() != 32
        || offered.advertisement.host_id.as_str() != "esp32/b0a732286178"
    {
        return Err("not the attended ESP32/protocol/key".into());
    }
    let mut nonce = [0; 32];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut nonce)?;
    if nonce == [0; 32] {
        return Err("entropy unavailable".into());
    }
    let host: c::HostId = "host/environment-preparation".into();
    let boot: c::BootId = format!(
        "boot/preparation/{:016x}",
        u64::from_le_bytes(nonce[..8].try_into()?)
    )
    .into();
    let sign = |seq| c::bind_sign(&host, &boot, None, seq).sign_id;
    let source = conduit_environment_make::compile_appliance_source()?;
    let birth_sequence = (u64::from_le_bytes(nonce[8..16].try_into()?) & i64::MAX as u64).max(1);
    let body = Body::born(
        source.expanded.source_document_id.clone(),
        source.expanded.checked_plot_id.clone(),
        birth_sequence,
        sign(birth_sequence),
    )
    .map_err(|e| format!("birth: {e:?}"))?;
    let mut membership =
        BodyMembership::new(body.body_id.clone()).map_err(|e| format!("membership: {e:?}"))?;
    let mut candidates =
        CandidateInventory::new(body.body_id.clone()).map_err(|e| format!("candidates: {e:?}"))?;
    let candidate = candidates
        .observe(CandidateObservation {
            advertisement: offered.advertisement.clone(),
            friendly_label: offered.friendly_label.clone(),
            observed_binding_id: "observed/attended-uart0".into(),
            observation_sign_id: sign(birth_sequence + 1),
            proof_id: DiscoveryProofId::bind("attended/ttyUSB0")
                .map_err(|e| format!("discovery: {e:?}"))?,
            freshness_sequence: offered.freshness_sequence,
            encoded_bytes: serde_json::to_vec(&offered)?.len() as u32,
        })
        .map_err(|e| format!("candidate: {e:?}"))?;
    let mut manager =
        AdmissionManager::new(body.body_id.clone()).map_err(|e| format!("manager: {e:?}"))?;
    let started = std::time::Instant::now();
    let challenge = manager
        .begin_ambient(
            &mut candidates,
            &candidate,
            offered.verifying_key.as_slice().try_into()?,
            nonce,
            1,
            30001,
            sign(birth_sequence + 2),
        )
        .map_err(|e| format!("challenge: {e:?}"))?;
    println!(
        "{}",
        serde_json::to_string(&PicoAdmissionChallenge {
            protocol: PICO_ADMISSION_PROTOCOL,
            admission_id: challenge.admission_id.as_str(),
            body_id: challenge.body_id.as_str(),
            host_id: challenge.host_id.as_str(),
            boot_id: challenge.boot_id.as_str(),
            offer_generation: challenge.offer_generation.0,
            nonce: challenge.nonce,
            issued_at_millis: challenge.issued_at_millis,
            expires_at_millis: challenge.expires_at_millis,
        })?
    );
    io::stdout().flush()?;
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Proof {
        protocol: u16,
        admission_id: AdmissionId,
        body_id: BodyId,
        host_id: c::HostId,
        boot_id: c::BootId,
        nonce: [u8; 32],
        signature: Vec<u8>,
    }
    let proof: Proof = serde_json::from_str(&line(&mut input)?)?;
    if proof.protocol != PICO_ADMISSION_PROTOCOL {
        return Err("wrong proof protocol".into());
    }
    let proof = AmbientAdmissionProof {
        admission_id: proof.admission_id,
        body_id: proof.body_id,
        host_id: proof.host_id,
        boot_id: proof.boot_id,
        nonce: proof.nonce,
        signature: proof.signature.as_slice().try_into()?,
    };
    let signs = || AdmissionSigns {
        part_admitted: sign(birth_sequence + 3),
        host_attached: sign(birth_sequence + 4),
        candidate_admitted: sign(birth_sequence + 5),
    };
    let now = 1 + u64::try_from(started.elapsed().as_millis())?;
    let mut tampered = proof.clone();
    tampered.signature[0] ^= 1;
    let original = membership.clone();
    if manager
        .complete_ambient(&mut candidates, &mut membership, &tampered, now, signs())
        .is_ok()
        || membership != original
    {
        return Err("tampered signature admitted".into());
    }
    let credential = manager
        .complete_ambient(&mut candidates, &mut membership, &proof, now, signs())
        .map_err(|e| format!("admission: {e:?}"))?;
    if manager.complete_ambient(&mut candidates, &mut membership, &proof, now, signs())
        != Err(AdmissionRefusal::Replay)
    {
        return Err("replay accepted".into());
    }
    let mut biography = BodyBiographyEvidence::born(body, original, "Mica Quail".into())
        .map_err(|e| format!("biography: {e:?}"))?;
    let events: Vec<_> = membership
        .events
        .iter()
        .zip([birth_sequence + 3, birth_sequence + 4])
        .map(|(event, seq)| (event.change_id.clone(), seq))
        .collect();
    biography
        .append_membership_events(membership, &events)
        .map_err(|e| format!("evidence: {e:?}"))?;
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "proof_class":"physical-ambient-admission", "credential":credential,
            "biography":biography, "receipts":manager.receipts,
            "verified_boot_key": {"host_id":offered.advertisement.host_id,
                "boot_id":offered.advertisement.boot_id, "verifying_key":offered.verifying_key},
            "key_provenance":"attended UART of expected ESP32", "image_attestation_claimed":false,
            "tampering_refused":true, "replay_refused":true,
            "body_installed_on_mcu":false, "play_started":false,
        }))?
    );
    Ok(())
}
