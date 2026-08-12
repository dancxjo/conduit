//! One bounded live browser admission proof server for conformance tests.

use conduit_body::{
    AdmissionManager, AdmissionSigns, AmbientAdmissionProof, Body, BodyMembership,
    CandidateInventory, CandidateObservation, DiscoveryProofId,
};
use conduit_core::{CheckedFormId, LinkBindingId, SignId, SourceDocumentId};
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress, BrowserAdmissionIngress, BrowserAdmissionListener,
    BROWSER_ADMISSION_PROTOCOL,
};

fn main() -> Result<(), String> {
    let body = Body::born(
        SourceDocumentId::from("source/browser-admission-probe"),
        CheckedFormId::from("checked/browser-admission-probe"),
        1,
        SignId::from("sign/browser-admission-probe/body-born"),
    )
    .map_err(|error| format!("Body birth: {error:?}"))?;
    let mut membership = BodyMembership::new(body.body_id.clone())
        .map_err(|error| format!("membership: {error:?}"))?;
    let mut candidates = CandidateInventory::new(body.body_id.clone())
        .map_err(|error| format!("candidate inventory: {error:?}"))?;
    let mut admission = AdmissionManager::new(body.body_id.clone())
        .map_err(|error| format!("admission manager: {error:?}"))?;
    let listener = BrowserAdmissionListener::bind_loopback()
        .map_err(|error| format!("bind admission Line: {error:?}"))?;
    println!(
        "{}",
        listener.url().map_err(|error| format!("URL: {error:?}"))?
    );
    let mut socket = listener
        .accept()
        .map_err(|error| format!("accept browser: {error:?}"))?;
    let (frame, encoded_bytes) = socket
        .receive_with_size()
        .map_err(|error| format!("receive advertisement: {error:?}"))?;
    let BrowserAdmissionIngress::Advertise {
        advertisement,
        friendly_label,
        verifying_key,
        freshness_sequence,
        ..
    } = frame
    else {
        return Err("first browser admission frame was not an advertisement".into());
    };
    let verifying_key: [u8; 32] = verifying_key
        .try_into()
        .map_err(|_| "browser verifying key was not 32 bytes")?;
    let candidate = candidates
        .observe(CandidateObservation {
            advertisement,
            friendly_label,
            observed_binding_id: LinkBindingId::from("line/browser-admission-probe"),
            observation_sign_id: SignId::from("sign/browser-admission-probe/observed"),
            proof_id: DiscoveryProofId::bind("proof/browser-admission-probe/loopback")
                .map_err(|error| format!("discovery proof: {error:?}"))?,
            freshness_sequence,
            encoded_bytes,
        })
        .map_err(|error| format!("observe candidate: {error:?}"))?;
    // Running this conformance probe is the explicit admission decision. Mere
    // reachability above created only an inert candidate.
    let challenge = admission
        .begin_ambient(
            &mut candidates,
            &candidate,
            verifying_key,
            [9; 32],
            1_000,
            2_000,
            SignId::from("sign/browser-admission-probe/requested"),
        )
        .map_err(|error| format!("begin admission: {error:?}"))?;
    socket
        .send(&BrowserAdmissionEgress::Challenge {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            challenge,
        })
        .map_err(|error| format!("send challenge: {error:?}"))?;
    let proof = match socket
        .receive()
        .map_err(|error| format!("receive proof: {error:?}"))?
    {
        BrowserAdmissionIngress::AmbientProof {
            admission_id,
            body_id,
            host_id,
            boot_id,
            nonce,
            signature,
            ..
        } => AmbientAdmissionProof {
            admission_id,
            body_id,
            host_id,
            boot_id,
            nonce: nonce.try_into().map_err(|_| "invalid proof nonce")?,
            signature: signature
                .try_into()
                .map_err(|_| "invalid proof signature")?,
        },
        _ => return Err("second browser admission frame was not an ambient proof".into()),
    };
    let credential = admission
        .complete_ambient(
            &mut candidates,
            &mut membership,
            &proof,
            1_100,
            AdmissionSigns {
                part_admitted: SignId::from("sign/browser-admission-probe/part-admitted"),
                host_attached: SignId::from("sign/browser-admission-probe/host-attached"),
                candidate_admitted: SignId::from("sign/browser-admission-probe/candidate-admitted"),
            },
        )
        .map_err(|error| format!("complete admission: {error:?}"))?;
    socket
        .send(&BrowserAdmissionEgress::Admitted {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            credential: credential.clone(),
        })
        .map_err(|error| format!("send credential: {error:?}"))?;
    println!(
        "admitted body={} part={} host={} boot={} candidates={} members={}",
        credential.body_id.as_str(),
        credential.part_id.as_str(),
        credential.host_id.as_str(),
        credential.boot_id.as_str(),
        candidates.candidates.len(),
        membership.parts.len()
    );
    Ok(())
}
