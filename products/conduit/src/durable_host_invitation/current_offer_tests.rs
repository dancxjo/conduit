//! Local-control fixtures prove the portable join uses the complete live offer.
use super::*;
use std::{io::Write, os::unix::net::UnixListener, thread::JoinHandle};

pub(crate) fn serve_current(
    state: &Path,
    advertisement: Option<conduit_core::HostAdvertisement>,
) -> (JoinHandle<()>, std::path::PathBuf) {
    let marker: RuntimeStatus =
        serde_json::from_slice(&fs::read(state.join("runtime.json")).unwrap()).unwrap();
    let current = advertisement.unwrap_or_else(|| {
        conduit_std_host::StdHost::new_with_config(conduit_std_host::StdHostConfig {
            host_id: marker.host_id.into(),
            boot_id: marker.boot_id.into(),
            offer_generation: conduit_core::OfferGeneration(marker.offer_generation),
        })
        .advertisement()
        .clone()
    });
    let token = [73u8; 32];
    fs::write(state.join("control.token"), token).unwrap();
    let identity = digest(state.to_string_lossy().as_bytes());
    let alias =
        std::env::temp_dir().join(format!("cdj-{}-{}", std::process::id(), &identity[7..19]));
    std::os::unix::fs::symlink(state, &alias).unwrap();
    let listener = UnixListener::bind(alias.join("control.sock")).unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut bytes = Vec::new();
        (&mut stream).take(4096).read_to_end(&mut bytes).unwrap();
        let request: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(request["kind"], "status");
        assert_eq!(request["token"], serde_json::json!(token.to_vec()));
        let response = serde_json::json!({"kind":"status","protocol":1,
            "target_id":"std/x86_64/computer","image_content_digest":"sha256:fixture",
            "advertisement":current});
        stream
            .write_all(&serde_json::to_vec(&response).unwrap())
            .unwrap();
    });
    (server, alias)
}

#[test]
fn portable_join_preserves_full_current_offer_and_refuses_other_boot() {
    for wrong_boot in [false, true] {
        let (manifest, state) = super::super::tests::fixture();
        super::super::install(&manifest, &state).unwrap();
        let (_, runtime) = super::super::runtime_preparation::prepare_runtime(&state).unwrap();
        let mut advertisement = runtime.truth().advertisement;
        // Stand in for an initialized provider absent from a fresh default Host.
        advertisement.profile = "profile/initialized-provider-fixture".into();
        advertisement.offer_generation.0 += 1;
        if wrong_boot {
            advertisement.boot_id = "boot/foreign".into();
        }
        let (server, alias) = serve_current(&state, Some(advertisement.clone()));
        let body = conduit_body::Body::born(
            "source/join".into(),
            "checked/join".into(),
            1,
            conduit_core::SignId::from("sign/join"),
        )
        .unwrap();
        let mut manager = AdmissionManager::new(body.body_id).unwrap();
        let now = current_time_millis().unwrap();
        let secret = [89u8; 32];
        let invitation = manager
            .issue_spawn_invitation(
                SpawnInvitationSecret::from_csprng_bytes(secret).unwrap(),
                [31u8; 32],
                now,
                now + 30_000,
            )
            .unwrap();
        let result = prepare_body_join(
            PortableInvitation {
                schema: INVITATION_SCHEMA.into(),
                claim: invitation.claim(),
                secret,
                rendezvous: None,
            },
            &alias,
            true,
        );
        server.join().unwrap();
        fs::remove_file(alias).unwrap();
        if wrong_boot {
            assert!(matches!(result, Err(error) if error.contains("runtime marker")));
            assert!(!state.join("body/pending-join.json").exists());
        } else {
            assert_eq!(result.unwrap().host_advertisement, advertisement);
        }
        fs::remove_dir_all(state.parent().unwrap()).unwrap();
    }
}
