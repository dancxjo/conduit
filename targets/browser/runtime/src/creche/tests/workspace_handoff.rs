use super::*;
use conduit_body::SpawnInvitationSecret;
use conduit_core::{BootId, HostId};

#[test]
fn workspace_moves_the_actual_issuer_and_preserves_issued_invitations() {
    session::clear_for_test();
    birth(701);
    session::attach_here("browser/creche", "browser-boot/creche", 702).unwrap();
    let original = session::with_session(|current| {
        current
            .admission
            .issue_spawn_invitation(
                SpawnInvitationSecret::from_csprng_bytes([17; 32]).unwrap(),
                [18; 32],
                10,
                1000,
            )
            .unwrap();
        Ok(serde_json::to_vec(&current.admission).unwrap())
    })
    .unwrap();
    let evidence = session::biography().unwrap();
    let (manager, custody) = super::super::handoff_workspace(&evidence).unwrap();
    assert_eq!(serde_json::to_vec(&manager).unwrap(), original);
    let custody = custody.unwrap();
    assert!(custody.matches(
        &evidence,
        &HostId::from("browser/creche"),
        &BootId::from("browser-boot/creche")
    ));
    assert!(!custody.matches(
        &evidence,
        &HostId::from("browser/other"),
        &BootId::from("browser-boot/creche")
    ));
    assert!(!custody.matches(
        &evidence,
        &HostId::from("browser/creche"),
        &BootId::from("browser-boot/restarted")
    ));
    assert!(session::current().is_none());
    assert!(super::super::handoff_workspace(&evidence).is_err());
}

#[test]
fn stale_handoff_retains_owner_and_public_restore_cannot_restore_custody() {
    session::clear_for_test();
    birth(711);
    session::attach_here("browser/creche", "browser-boot/creche", 712).unwrap();
    let evidence = session::biography().unwrap();
    let mut stale = evidence.clone();
    stale.friendly_name = "substituted".into();
    assert!(super::super::handoff_workspace(&stale).is_err());
    assert_eq!(session::biography().unwrap(), evidence);
    let snapshot = session::durable_snapshot().unwrap();
    session::forget_local();
    session::restore_durable(snapshot).unwrap();
    let (_, custody) = super::super::handoff_workspace(&evidence).unwrap();
    assert!(custody.is_none());
}

fn workspace(request: serde_json::Value) -> (i32, serde_json::Value) {
    use crate::workspace_runtime::*;
    let bytes = serde_json::to_vec(&request).unwrap();
    assert!(bytes.len() <= conduit_workspace_input_capacity());
    // Exercise the production byte ABI, including request decoding and wiping.
    unsafe {
        core::ptr::copy_nonoverlapping(
            bytes.as_ptr(),
            conduit_workspace_input_ptr() as *mut u8,
            bytes.len(),
        );
    }
    let code = conduit_workspace_request(bytes.len());
    let output = unsafe {
        core::slice::from_raw_parts(
            conduit_workspace_output_ptr() as *const u8,
            conduit_workspace_output_len(),
        )
    };
    (code, serde_json::from_slice(output).unwrap())
}
fn reservation() -> serde_json::Value {
    serde_json::json!({"action":"ReserveWomb", "reservation":{
        "host_id":"browser/creche", "boot_id":"browser-boot/creche", "target_id":"browser/wasm32/page",
        "secret":vec![71;32], "nonce":vec![72;32], "now_millis":10, "expires_at_millis":1000,
        "selection":{"catalog_generation":1,"implementations":["browser/dom@1","browser/keyboard-events@1","browser/pointer-events@1"]},
        "expected_release_digest":format!("sha256:{}", "1".repeat(64))
    }})
}

unsafe extern "C" {
    fn conduit_womb_seed_output_ptr() -> usize;
    fn conduit_womb_seed_output_len() -> usize;
    fn conduit_browser_body_input_ptr() -> usize;
    fn conduit_browser_body_input_capacity() -> usize;
    fn conduit_browser_body_start(length: usize) -> i32;
    fn conduit_browser_plot_complete_effect(
        play_length: usize,
        placement_length: usize,
        sequence: u32,
        output_length: usize,
    ) -> i32;
}

fn browser_output() -> serde_json::Value {
    use crate::plot_runner::abi::*;
    let bytes = unsafe {
        core::slice::from_raw_parts(
            conduit_tour_output_ptr() as *const u8,
            conduit_tour_output_len(),
        )
    };
    serde_json::from_slice(bytes).unwrap()
}

fn start_browser_body(request: serde_json::Value) -> (i32, serde_json::Value) {
    let bytes = serde_json::to_vec(&request).unwrap();
    let code = unsafe {
        assert!(bytes.len() <= conduit_browser_body_input_capacity());
        core::ptr::copy_nonoverlapping(
            bytes.as_ptr(),
            conduit_browser_body_input_ptr() as *mut u8,
            bytes.len(),
        );
        conduit_browser_body_start(bytes.len())
    };
    (code, browser_output())
}

fn complete_browser_effect(
    play: &str,
    placement: &str,
    sequence: u32,
    output: &[u8],
) -> (i32, serde_json::Value) {
    use crate::plot_runner::abi::*;
    let bytes = [play.as_bytes(), placement.as_bytes(), output].concat();
    assert!(bytes.len() <= conduit_tour_input_capacity());
    let code = unsafe {
        core::ptr::copy_nonoverlapping(
            bytes.as_ptr(),
            conduit_tour_input_ptr() as *mut u8,
            bytes.len(),
        );
        conduit_browser_plot_complete_effect(play.len(), placement.len(), sequence, output.len())
    };
    (code, browser_output())
}

#[test]
fn workspace_reserves_and_revokes_only_its_private_undelivered_provision() {
    // Each ABI instance owns its thread-local Root; no test-only reset of live
    // production custody or mutation of another test's admission owner.
    std::thread::spawn(|| {
        session::clear_for_test();
        birth(721);
        session::attach_here("browser/creche", "browser-boot/creche", 722).unwrap();
        let host = crate::installed_browser::advertisement("browser/creche".into(), "browser-boot/creche".into());
        assert_eq!(workspace(serde_json::json!({"action":"Arrive","advertisement":host})).0, 0);
        let mut unsupported = reservation();
        unsupported["reservation"]["target_id"] = "unsupported/target".into();
        assert_eq!(workspace(unsupported).1["code"], "Womb.UnsupportedTarget");
        let mut stale_boot = reservation();
        stale_boot["reservation"]["boot_id"] = serde_json::json!("browser-boot/restarted");
        assert_eq!(workspace(stale_boot).1["code"], "Womb.Denied");
        let (code, reserved) = workspace(reservation());
        assert_eq!(code, 0, "{reserved}");
        assert_eq!(reserved["disposition"], "reserved");
        assert_eq!(reserved["artifact_prepared"], false);
        assert_eq!(reserved["invitation_delivered"], false);
        assert_eq!(reserved["membership_claimed"], false);
        let (code, unrelated) = workspace(serde_json::json!({"action":"CreateInvitation",
            "host_id":"browser/creche", "boot_id":"browser-boot/creche",
            "secret":vec![73;32], "nonce":vec![74;32], "now_millis":11, "expires_at_millis":1000}));
        assert_eq!(code, 0, "{unrelated}");
        let before = workspace(serde_json::json!({"action":"Durable"})).1;
        assert_eq!(before["admission"]["invitations"].as_array().unwrap().len(), 2);
        assert_eq!(workspace(reservation()).1["code"], "Womb.Busy");
        let mut wrong = reserved["reservation_id"].clone();
        wrong[0] = serde_json::json!(wrong[0].as_u64().unwrap() ^ 1);
        let cancel = |id| serde_json::json!({"action":"CancelWombReservation", "host_id":"browser/creche", "boot_id":"browser-boot/creche", "reservation_id":id});
        assert_eq!(workspace(cancel(wrong)).1["code"], "Womb.StaleReservation");
        assert_eq!(workspace(serde_json::json!({"action":"Durable"})).1, before);
        let (code, cancelled) = workspace(cancel(reserved["reservation_id"].clone()));
        assert_eq!(code, 0, "{cancelled}");
        assert_eq!(cancelled["invitation_revoked"], true);
        assert_eq!(cancelled["provider_started"], false);
        let after = workspace(serde_json::json!({"action":"Durable"})).1;
        let remaining = after["admission"]["invitations"].as_array().unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0]["invitation_id"], unrelated["invitation_id"]);
        assert_eq!(after["admission"]["next_invitation_sequence"], before["admission"]["next_invitation_sequence"]);
        assert_eq!(after["evidence"], before["evidence"]);
        let (code, next) = workspace(reservation());
        assert_eq!(code, 0, "{next}");
        assert_ne!(next["reservation_id"], reserved["reservation_id"]);
        assert_eq!(workspace(cancel(next["reservation_id"].clone())).0, 0);
    }).join().unwrap();
}

#[test]
fn public_admitted_snapshot_cannot_reserve_issuer_provisioning() {
    std::thread::spawn(|| {
        session::clear_for_test();
        birth(731);
        session::attach_here("browser/creche", "browser-boot/creche", 732).unwrap();
        let evidence = session::biography().unwrap();
        let manager = conduit_body::AdmissionManager::new(evidence.body_id.clone()).unwrap();
        let host = crate::installed_browser::advertisement("browser/creche".into(), "browser-boot/creche".into());
        assert_eq!(workspace(serde_json::json!({"action":"OpenAdmitted", "evidence":evidence, "admission":manager,"host_id":"browser/creche","boot_id":"browser-boot/creche","advertisement":host})).0, 0);
        let before = workspace(serde_json::json!({"action":"Durable"})).1;
        assert_eq!(workspace(reservation()).1["code"], "Womb.Denied");
        assert_eq!(workspace(serde_json::json!({"action":"Durable"})).1, before);
    }).join().unwrap();
}

#[path = "workspace_handoff/womb_play.rs"]
mod womb_play;

#[test]
fn womb_release_staging_keeps_exact_bytes_private_and_refuses_reordered_chunks() {
    std::thread::spawn(|| {
        use sha2::{Digest, Sha256};
        session::clear_for_test();
        birth(761);
        session::attach_here("browser/creche", "browser-boot/creche", 762).unwrap();
        let host = crate::installed_browser::advertisement("browser/creche".into(), "browser-boot/creche".into());
        assert_eq!(workspace(serde_json::json!({"action":"Arrive","advertisement":host})).0, 0);
        let payload = b"\0asm\x01\0\0\0";
        let hash = format!("sha256:{:x}", Sha256::digest(payload));
        let mut aggregate = Sha256::new();
        aggregate.update(b"conduit.release/host-bundle-content@1\0runtime.wasm\0");
        aggregate.update(hash.as_bytes()); aggregate.update(b"\n");
        let digest = format!("sha256:{:x}",aggregate.finalize());
        let manifest = serde_json::to_vec(&serde_json::json!({
            "schema":"conduit.release/host-bundle@1","target_id":"browser/wasm32/page",
            "make_package_id":"browser-wasm@1","output":"browser-bundle",
            "builder_adapter":"conduit-host-browser/build-wasm@1","deployment_adapter":"conduit-host-browser/load@1",
            "source_identity":"source/fixture","bundle_sha256":digest,
            "files":[{"path":"runtime.wasm","bytes":payload.len(),"sha256":hash,"media_type":"application/wasm"}]
        })).unwrap();
        let mut request = reservation();
        request["reservation"]["expected_release_digest"] = digest.into();
        request["reservation"]["release_pin"] = serde_json::json!({"manifest_digest":format!("sha256:{:x}",Sha256::digest(&manifest)),"source_identity":"source/fixture"});
        let (code,reserved) = workspace(request); assert_eq!(code,0,"{reserved}");
        let stage = |operation| serde_json::json!({"action":"StageWombRelease","stage":{
            "host_id":"browser/creche","boot_id":"browser-boot/creche","reservation_id":reserved["reservation_id"],"operation":operation}});
        assert_eq!(workspace(stage(serde_json::json!({"kind":"Begin","manifest":manifest}))).0,0);
        assert!(workspace(stage(serde_json::json!({"kind":"Finish"}))).0 < 0);
        assert!(workspace(stage(serde_json::json!({"kind":"Chunk","file":0,"offset":1,"bytes":payload}))).0 < 0);
        assert_eq!(workspace(stage(serde_json::json!({"kind":"Chunk","file":0,"offset":0,"bytes":payload}))).0,0);
        let (code,verified) = workspace(stage(serde_json::json!({"kind":"Finish"})));
        assert_eq!(code,0,"{verified}"); assert_eq!(verified["bytes_verified"],true);
        assert_eq!(verified["artifact_prepared"],false); assert_eq!(verified["membership_claimed"],false);
        assert!(workspace(stage(serde_json::json!({"kind":"Chunk","file":0,"offset":8,"bytes":[0]}))).0 < 0);
        let mut foreign = stage(serde_json::json!({"kind":"Finish"}));
        foreign["stage"]["boot_id"] = "browser-boot/foreign".into();
        assert_eq!(workspace(foreign).1["code"],"Womb.Denied");
        assert_eq!(workspace(serde_json::json!({"action":"CancelWombReservation","host_id":"browser/creche","boot_id":"browser-boot/creche","reservation_id":reserved["reservation_id"]})).0,0);
        assert!(workspace(stage(serde_json::json!({"kind":"Finish"}))).0 < 0);
    }).join().unwrap();
}
