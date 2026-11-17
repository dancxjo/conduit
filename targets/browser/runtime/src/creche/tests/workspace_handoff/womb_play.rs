//! Actual kernel/provider completion and original issuer retirement proofs.
use super::*;

#[test]
fn ordinary_womb_body_play_requires_started_and_retained_provider_evidence() {
    run_womb_body_play(WombFixture::NoBuilder);
}
#[test]
fn ordinary_womb_body_play_prepares_private_seed_from_original_invitation() {
    run_womb_body_play(WombFixture::Prepared);
}
#[test]
fn womb_delivery_requires_kernel_prepared_and_prevents_revoking_exposed_seed() {
    run_womb_body_play(WombFixture::Deliver);
}
#[test]
fn womb_rechecks_invitation_expiry_after_selected_browser_producer_await() {
    run_womb_body_play(WombFixture::Expire);
}
#[test]
fn womb_cancelled_producer_rejects_late_image_completion() {
    run_womb_body_play(WombFixture::Cancel);
}
#[test]
fn womb_failed_producer_retains_failed_outcome_without_seed_or_membership() {
    run_womb_body_play(WombFixture::Failed);
}
#[test]
fn original_manager_consumption_releases_the_delivered_seed_without_revoking_membership() {
    run_womb_body_play(WombFixture::Admit);
}
#[test]
fn pending_producer_cancellation_requires_drain_and_preserves_owner_until_acknowledged() {
    run_womb_body_play(WombFixture::PendingCancel);
}
#[test]
fn foreign_fail_job_calls_preserve_the_original_pending_producer() {
    run_womb_body_play(WombFixture::ForeignFail);
}
enum WombFixture {
    Admit,
    PendingCancel,
    ForeignFail,
    NoBuilder,
    Prepared,
    Deliver,
    Expire,
    Cancel,
    Failed,
}
fn run_womb_body_play(case: WombFixture) {
    let with_release = !matches!(case, WombFixture::NoBuilder);
    let with_delivery = matches!(case, WombFixture::Deliver | WombFixture::Admit);
    let admits = matches!(case, WombFixture::Admit);
    let pending_cancel = matches!(case, WombFixture::PendingCancel);
    let foreign_fail = matches!(case, WombFixture::ForeignFail);
    let expires_after_await = matches!(case, WombFixture::Expire);
    let cancels_after_await = matches!(case, WombFixture::Cancel);
    let fails_after_await = matches!(case, WombFixture::Failed);
    std::thread::spawn(move || {
        let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../plots/womb/main.conduit"));
        session::clear_for_test();
        let born = birth(741);
        let previous = born.raw_body.workset.plots()[0].clone();
        session::attach_here("browser/creche", "browser-boot/creche", 742).unwrap();
        let host = crate::installed_browser::advertisement("browser/creche".into(), "browser-boot/creche".into());
        assert!(!host.capabilities.iter().any(|offer| offer.kind_id.as_str() == "womb/prepare"));
        assert_eq!(workspace(serde_json::json!({"action":"Arrive","advertisement":host})).0, 0);
        let mut reserve = reservation();
        let release = with_release.then(||crate::workspace_runtime::womb_provision::test_release::fixture().0);
        let manifest_bytes = release.as_ref().map(|manifest|serde_json::to_vec(manifest).unwrap());
        if let Some(manifest) = &release {
            use sha2::{Digest,Sha256};
            reserve["reservation"]["expected_release_digest"] = manifest.bundle_sha256.clone().into();
            reserve["reservation"]["release_pin"] = serde_json::json!({"manifest_digest":format!("sha256:{:x}",Sha256::digest(manifest_bytes.as_ref().unwrap())),"source_identity":manifest.source_identity});
        }
        let (code, reserved) = workspace(reserve);

        assert_eq!(code, 0, "{reserved}");
        if let Some(bytes) = manifest_bytes {
            let stage = |operation|serde_json::json!({"action":"StageWombRelease","stage":{"host_id":"browser/creche","boot_id":"browser-boot/creche","reservation_id":reserved["reservation_id"],"operation":operation}});
            assert_eq!(workspace(stage(serde_json::json!({"kind":"Begin","manifest":bytes}))).0,0);
            for (file,payload) in [b"\0asm\x01\0\0\0".as_slice(),b"export{}".as_slice()].iter().enumerate() {
                assert_eq!(workspace(stage(serde_json::json!({"kind":"Chunk","file":file,"offset":0,"bytes":payload}))).0,0);
            }
            let (code,checked)=workspace(stage(serde_json::json!({"kind":"Finish"})));
            assert_eq!(code,0,"{checked}");
        }
        let host = crate::installed_browser::advertisement("browser/creche".into(), "browser-boot/creche".into());
        assert_eq!(host.capabilities.iter().filter(|offer| offer.kind_id.as_str() == "womb/prepare").count(), 1);
        let inventory = crate::creche::initial_plots::reviewed_inventory(source).unwrap();
        let selected = &inventory.plots[0];
        let plot = conduit_body::ResidentPlot::new(selected.source_document_id.clone().into(), selected.checked_plot_id.clone().into());
        let (code, installed) = workspace(serde_json::json!({"action":"ChangeWorkset", "host_id":"browser/creche", "boot_id":"browser-boot/creche", "expected_revision":born.workload_revision, "plot":plot, "source":source, "edit":{"Replace":{"previous":previous}}}));
        assert_eq!(code, 0, "{installed}");
        let (code, proposal) = workspace(serde_json::json!({"action":"Propose", "host_id":"browser/creche", "boot_id":"browser-boot/creche", "source":source, "joined_lines":[], "browser_audio_authority":false}));
        assert_eq!(code, 0, "{proposal}");
        let plan: conduit_body::BodyPlan = serde_json::from_value(proposal["plan"].clone()).unwrap();
        plan.verify_seal().unwrap();
        let gear = &plan.plots[0].plan.fragments[0].placements[0];
        assert_eq!(gear.kind_id.as_str(), "womb/prepare");
        let factory = crate::installed_browser::factory(&gear.implementation_id).unwrap();
        let mut values = conduit_kernel::HostedValueStore::new(2, 65, 97).unwrap();
        let mut foreign = gear.clone();
        foreign.resources[0].pool_id = "browser/womb/foreign".into();
        assert!((factory.prepare)(&foreign, &mut values).is_err());
        let request = || serde_json::json!({"action":"WombInput", "host_id":"browser/creche", "boot_id":"browser-boot/creche", "reservation_id":reserved["reservation_id"]});
        assert_eq!(workspace(request()).1["code"], "Womb.NotStarted");
        let play = conduit_body::BodyPlayIdentity::bind(&plan, 1);
        assert_ne!(workspace(serde_json::json!({"action":"Started", "host_id":"browser/creche", "boot_id":"browser-boot/creche", "play":play, "wake_at_start":proposal["wake"]})).0, 0);
        assert_eq!(workspace(request()).1["code"], "Womb.NotStarted");
        let observations = host.resources.iter().enumerate().map(|(index, pool)| conduit_core::ResourceObservation {
            host_id: host.host_id.clone(), boot_id: host.boot_id.clone(), offer_generation: host.offer_generation,
            pool_id: pool.pool_id.clone(), class_id: pool.class_id.clone(), health: conduit_core::ResourceHealth::Ready,
            unreserved_units: pool.capacity_units, utilized_units: 0, sign_id: format!("sign/womb-observed-{index}").into(),
        }).collect::<Vec<_>>();
        let (code, started) = start_browser_body(serde_json::json!({
            "wake":proposal["wake"], "plan":plan, "play_sequence":1,
            "local_host_id":"browser/creche", "local_boot_id":"browser-boot/creche",
            "observations":observations, "body_evidence":proposal["body_evidence"],
            "source":source, "foreground_checked_plot_id":plan.plots[0].plot.checked_plot_id,
        }));
        assert_eq!(code, 0, "{started}");
        assert_eq!(started["progress"]["effect_kind"], "womb-prepare");
        assert_eq!(workspace(request()).1["code"], "Womb.NotStarted");
        let effect = &started["progress"];
        let run = || serde_json::json!({"action":"RunWombProvider", "call":{
            "host_id":"browser/creche", "boot_id":"browser-boot/creche", "reservation_id":reserved["reservation_id"],
            "active_play_id":started["play"]["active_play_id"], "placement_id":effect["placement_id"],
            "request_sequence":effect["request_sequence"], "now_millis":if expires_after_await {1000} else {20},
        }});
        assert_eq!(workspace(run()).1["code"], "Womb.NotStarted");
        assert_eq!(workspace(serde_json::json!({"action":"Started", "host_id":"browser/creche", "boot_id":"browser-boot/creche", "play":started["play"], "wake_at_start":started["wake_at_start"]})).0, 0);
        let (code, admitted_request) = workspace(request());
        assert_eq!(code, 0, "{admitted_request}");
        let request_bytes: [u8;32] = serde_json::from_value(admitted_request["request"].clone()).unwrap();
        let forged = conduit_body_make::womb_runtime::WombOutcome {
            request: conduit_body_make::womb_runtime::WombRequest(request_bytes),
            disposition: conduit_body_make::womb_runtime::WombDisposition::Prepared, evidence_digest:[99;32],
        }.encode();
        let play = started["play"]["active_play_id"].as_str().unwrap();
        let placement = effect["placement_id"].as_str().unwrap();
        let sequence = effect["request_sequence"].as_u64().unwrap() as u32;
        assert_ne!(complete_browser_effect(play, placement, sequence, &forged).0, 0);
        if with_release {
            let mut call=run()["call"].clone();
            call["now_millis"]=20.into();
            let (manifest,image)=crate::workspace_runtime::womb_provision::test_release::output();
            assert_eq!(workspace(serde_json::json!({"action":"CompleteWombHostJob","call":call,"manifest":manifest,"image":image})).1["code"],"Womb.StaleCall");
            let (code,job)=workspace(serde_json::json!({"action":"BeginWombHostJob","call":call}));assert_eq!(code,0,"{job}");
            assert_eq!(workspace(run()).1["code"],"Womb.ProviderPending");
            if pending_cancel {
                let before = workspace(serde_json::json!({"action":"Durable"})).1;
                assert_eq!(crate::plot_runner::abi::conduit_tour_cancel(), 0);
                assert_eq!(workspace(serde_json::json!({"action":"Lull","host_id":"browser/creche","boot_id":"browser-boot/creche","terminated_play":started["play"]})).0,0);
                let cancel = ||serde_json::json!({"action":"CancelWombReservation","host_id":"browser/creche","boot_id":"browser-boot/creche","reservation_id":reserved["reservation_id"]});
                let after_lull=workspace(serde_json::json!({"action":"Durable"})).1;
                assert_eq!(workspace(cancel()).1["code"],"Womb.DrainRequired");
                assert_eq!(workspace(serde_json::json!({"action":"Durable"})).1,after_lull);
                assert_eq!(before["admission"],after_lull["admission"]);
                assert_eq!(workspace(serde_json::json!({"action":"FailWombHostJob","call":call,"cancelled":true})).0,0);
                let (code,cancelled)=workspace(cancel());assert_eq!(code,0,"{cancelled}");assert_eq!(cancelled["invitation_revoked"],true);
                let after=workspace(serde_json::json!({"action":"Durable"})).1;
                assert!(after["admission"]["invitations"].as_array().unwrap().is_empty());
                assert_eq!(after["evidence"]["membership"],before["evidence"]["membership"]);
                assert_eq!(unsafe{conduit_womb_seed_output_len()},0);
                assert!(workspace(serde_json::json!({"action":"CompleteWombHostJob","call":call,"manifest":manifest,"image":image})).0<0);
                return;
            }
            if foreign_fail {
                let before=workspace(serde_json::json!({"action":"Durable"})).1;
                for field in ["host_id","boot_id","reservation_id","active_play_id","placement_id","request_sequence"] {
                    let mut foreign=call.clone();
                    foreign[field]=match field {
                        "reservation_id"=>serde_json::json!(vec![99;32]),
                        "request_sequence"=>serde_json::json!(sequence+1),
                        _=>serde_json::json!(format!("fixture/foreign-{field}")),
                    };
                    assert!(workspace(serde_json::json!({"action":"FailWombHostJob","call":foreign,"cancelled":false})).0<0,"foreign {field}");
                    assert_eq!(workspace(serde_json::json!({"action":"Durable"})).1,before,"foreign {field}");
                    assert_eq!(workspace(run()).1["code"],"Womb.ProviderPending","foreign {field}");
                }
            }
            let malformed=serde_json::json!({"action":"CompleteWombHostJob","call":call,"manifest":[123],"image":image});
            assert_eq!(workspace(malformed).1["code"],"Womb.InvalidRelease");
            let mut foreign:serde_json::Value=serde_json::from_slice(&manifest).unwrap();foreign["source_identity"]="source/foreign".into();
            assert_eq!(workspace(serde_json::json!({"action":"CompleteWombHostJob","call":call,"manifest":serde_json::to_vec(&foreign).unwrap(),"image":image})).1["code"],"Womb.InvalidRelease");
            let mut changed=image.clone();changed[0]^=1;
            assert_eq!(workspace(serde_json::json!({"action":"CompleteWombHostJob","call":call,"manifest":manifest,"image":changed})).1["code"],"Womb.InvalidRelease");
            if cancels_after_await || fails_after_await {
                assert_eq!(workspace(serde_json::json!({"action":"FailWombHostJob","call":call,"cancelled":cancels_after_await})).0,0);
                assert_eq!(workspace(serde_json::json!({"action":"CompleteWombHostJob","call":call,"manifest":manifest,"image":image})).1["code"],"Womb.StaleCall");
            } else if expires_after_await {
                let stale=run()["call"].clone();
                assert_eq!(workspace(serde_json::json!({"action":"CompleteWombHostJob","call":stale,"manifest":manifest,"image":image})).1["code"],"Admission.Expired");
                assert_eq!(workspace(serde_json::json!({"action":"FailWombHostJob","call":stale,"cancelled":false})).0,0);
            } else {
            let (code,retained)=workspace(serde_json::json!({"action":"CompleteWombHostJob","call":call,"manifest":manifest,"image":image}));assert_eq!(code,0,"{retained}");
            }
        }
        let (code, completed) = workspace(run());
        assert_eq!(code, 0, "{completed}");
        assert_eq!(workspace(run()).1, completed, "the same pending call retains one paid result");
        let outcome_bytes: Vec<u8> = serde_json::from_value(completed["outcome"].clone()).unwrap();
        let outcome = conduit_body_make::womb_runtime::WombOutcome::decode(&outcome_bytes).unwrap();
        assert_eq!(outcome.disposition, if cancels_after_await {conduit_body_make::womb_runtime::WombDisposition::Cancelled} else if fails_after_await {conduit_body_make::womb_runtime::WombDisposition::Failed} else if expires_after_await {conduit_body_make::womb_runtime::WombDisposition::StaleInvitation} else if with_release {conduit_body_make::womb_runtime::WombDisposition::Prepared} else {conduit_body_make::womb_runtime::WombDisposition::UnavailableBuilder});
        assert_eq!(unsafe{conduit_womb_seed_output_ptr()},0);
        let delivery=|now|serde_json::json!({"action":"BeginWombDelivery","host_id":"browser/creche","boot_id":"browser-boot/creche","reservation_id":reserved["reservation_id"],"now_millis":now});
        assert!(workspace(delivery(20)).0 < 0,"provider completion alone cannot expose a Seed");
        let (code, receipt) = complete_browser_effect(play, placement, sequence, &outcome_bytes);
        assert_eq!(code, 0, "{receipt}");
        assert_eq!(receipt["disposition"], "completed");
        let (code, result) = workspace(serde_json::json!({"action":"WombResult", "host_id":"browser/creche", "boot_id":"browser-boot/creche", "reservation_id":reserved["reservation_id"]}));
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["kernel_outcome"], completed["outcome"]);
        assert_eq!(result["artifact_prepared"], with_release && !expires_after_await && !cancels_after_await && !fails_after_await);
        assert_eq!(result["membership_claimed"], false);
        if with_delivery {
            assert!(workspace(delivery(1000)).0 < 0);
            assert_eq!(unsafe{conduit_womb_seed_output_ptr()},0);
            let (code,exposure)=workspace(delivery(20)); assert_eq!(code,0,"{exposure}");
            assert_eq!(exposure["delivery_committed"],true); assert_eq!(exposure["invitation_delivered"],false);
            let length=unsafe{conduit_womb_seed_output_len()};
            assert!(length>22); assert_eq!(length as u64,exposure["artifact"]["artifact_bytes"].as_u64().unwrap());
            let bytes=unsafe{core::slice::from_raw_parts(conduit_womb_seed_output_ptr() as *const u8,length)};
            assert_eq!(&bytes[..4],b"PK\x03\x04");
        }
        let current = workspace(serde_json::json!({"action":"Current"})).1;
        assert_eq!(current["evidence"]["body_id"], born.body_id);
        assert_eq!(current["evidence"]["membership"]["parts"].as_array().unwrap().len(), 1);
        assert_eq!(workspace(serde_json::json!({"action":"Lull", "host_id":"browser/creche", "boot_id":"browser-boot/creche", "terminated_play":started["play"]})).0, 0);
        let (code, cancelled) = workspace(serde_json::json!({"action":"CancelWombReservation", "host_id":"browser/creche", "boot_id":"browser-boot/creche", "reservation_id":reserved["reservation_id"]}));
        if with_delivery {
            assert_eq!(cancelled["code"],"Womb.DeliveryCommitted");
            let (code,release)=workspace(serde_json::json!({"action":"ReleaseWombAfterAdmission","host_id":"browser/creche","boot_id":"browser-boot/creche","reservation_id":reserved["reservation_id"]}));
            assert!(code<0);assert_eq!(release["code"],"Womb.NotAdmitted");
            assert!(unsafe{conduit_womb_seed_output_len()}>0,"refused retirement preserves the exposed candidate");
            if admits {
                let original=workspace(serde_json::json!({"action":"Durable"})).1;
                let record=&original["admission"]["invitations"][0];
                assert_eq!(record["used"],false);
                let claim=conduit_body::SpawnInvitationClaim {
                    invitation_id:serde_json::from_value(record["invitation_id"].clone()).unwrap(),
                    body_id:serde_json::from_value(original["evidence"]["body_id"].clone()).unwrap(), nonce:[72;32], expires_at_millis:1000,
                };
                let target=crate::installed_browser::advertisement("browser/target".into(),"browser-boot/target".into());
                let signature=SpawnInvitationSecret::from_csprng_bytes([71;32]).unwrap().sign(&claim.signing_transcript(&target.host_id,&target.boot_id,target.offer_generation));
                let proof=serde_json::json!({"invitation_id":claim.invitation_id,"body_id":claim.body_id,"host_id":target.host_id,"boot_id":target.boot_id,"nonce":claim.nonce,"signature":signature.as_slice()});
                let (code,accepted)=workspace(serde_json::json!({"action":"AdmitInvitation","host_id":"browser/creche","boot_id":"browser-boot/creche","advertisement":target,"proof":proof,"now_millis":20}));
                assert_eq!(code,0,"{accepted}");
                assert_eq!(accepted["body"]["evidence"]["membership"]["parts"].as_array().unwrap().len(),2);
                let used=workspace(serde_json::json!({"action":"Durable"})).1;
                assert_eq!(used["admission"]["invitations"][0]["used"],true);
                let (code,released)=workspace(serde_json::json!({"action":"ReleaseWombAfterAdmission","host_id":"browser/creche","boot_id":"browser-boot/creche","reservation_id":reserved["reservation_id"]}));
                assert_eq!(code,0,"{released}");assert_eq!(released["invitation_consumed"],true);assert_eq!(released["candidate_released"],true);
                assert_eq!(unsafe{conduit_womb_seed_output_len()},0);
                assert_eq!(workspace(serde_json::json!({"action":"Durable"})).1,used,"retirement does not mutate membership or the consumed original manager");
            }
            return;
        }
        assert_eq!(code, 0, "{cancelled}");
        assert_eq!(cancelled["provider_started"], true);
        let host = crate::installed_browser::advertisement("browser/creche".into(), "browser-boot/creche".into());
        assert!(!host.capabilities.iter().any(|offer| offer.kind_id.as_str() == "womb/prepare"));
    }).join().unwrap();
}
