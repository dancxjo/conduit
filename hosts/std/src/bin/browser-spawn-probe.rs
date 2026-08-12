//! Body-directed browser spawn and single-use replay conformance server.

use conduit_body::{
    AdmissionManager, AdmissionRefusal, AdmissionSigns, Body, BodyMembership, SpawnAdmissionProof,
    SpawnInvitationSecret,
};
use conduit_core::{CheckedFormId, SignId, SourceDocumentId};
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress, BrowserAdmissionIngress, BrowserAdmissionListener,
    BROWSER_ADMISSION_PROTOCOL,
};
use serde_json::json;

fn main() -> Result<(), String> {
    let body = Body::born(
        SourceDocumentId::from("source/browser-spawn-probe"),
        CheckedFormId::from("checked/browser-spawn-probe"),
        1,
        SignId::from("sign/browser-spawn-probe/body-born"),
    )
    .map_err(debug("Body birth"))?;
    let mut membership = BodyMembership::new(body.body_id.clone()).map_err(debug("membership"))?;
    let mut manager = AdmissionManager::new(body.body_id.clone()).map_err(debug("manager"))?;
    let secret_bytes = [13; 32];
    let invitation = manager
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes(secret_bytes).map_err(debug("secret"))?,
            [17; 32],
            1_000,
            2_000,
        )
        .map_err(debug("invitation"))?;
    let listener = BrowserAdmissionListener::bind_loopback().map_err(debug("bind"))?;
    let body_url = listener.url().map_err(debug("URL"))?;
    let envelope = serde_json::to_vec(&json!({
        "claim": invitation.claim(),
        "secret": secret_bytes,
    }))
    .map_err(|error| format!("encode invitation: {error}"))?;
    let envelope_hex = envelope
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    println!("body_url={body_url}");
    println!("spawn_hex={envelope_hex}");

    for attempt in 0..2 {
        let mut socket = listener.accept().map_err(debug("accept"))?;
        let advertisement = match socket.receive().map_err(debug("advertisement"))? {
            BrowserAdmissionIngress::Advertise { advertisement, .. } => advertisement,
            _ => return Err("spawn did not begin with an advertisement".into()),
        };
        let proof = match socket.receive().map_err(debug("spawn proof"))? {
            BrowserAdmissionIngress::SpawnProof {
                invitation_id,
                body_id,
                host_id,
                boot_id,
                nonce,
                signature,
                ..
            } => SpawnAdmissionProof {
                invitation_id,
                body_id,
                host_id,
                boot_id,
                nonce: nonce.try_into().map_err(|_| "invalid spawn nonce")?,
                signature: signature
                    .try_into()
                    .map_err(|_| "invalid spawn signature")?,
            },
            _ => return Err("spawn did not provide a spawn proof".into()),
        };
        let result = manager.complete_spawn(
            &mut membership,
            &advertisement,
            &proof,
            1_100,
            AdmissionSigns {
                part_admitted: SignId::from(format!("sign/browser-spawn/{attempt}/part")),
                host_attached: SignId::from(format!("sign/browser-spawn/{attempt}/host")),
                candidate_admitted: SignId::from(format!("sign/browser-spawn/{attempt}/candidate")),
            },
        );
        match (attempt, result) {
            (0, Ok(credential)) => socket
                .send(&BrowserAdmissionEgress::Admitted {
                    protocol: BROWSER_ADMISSION_PROTOCOL,
                    credential,
                })
                .map_err(debug("send admitted"))?,
            (1, Err(AdmissionRefusal::Replay)) => socket
                .send(&BrowserAdmissionEgress::Refused {
                    protocol: BROWSER_ADMISSION_PROTOCOL,
                    code: "replay".into(),
                })
                .map_err(debug("send replay"))?,
            (_, Ok(_)) => {
                return Err("replayed invitation unexpectedly admitted another Part".into())
            }
            (_, Err(error)) => return Err(format!("unexpected spawn refusal: {error:?}")),
        }
    }
    println!(
        "spawn_admitted=1 replay_refused=true members={}",
        membership.parts.len()
    );
    Ok(())
}

fn debug<T: core::fmt::Debug>(context: &'static str) -> impl FnOnce(T) -> String {
    move |error| format!("{context}: {error:?}")
}
