//! Real native provider and actual retained Face; opening-only artifact proof.
use super::*;
use crate::{ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, RunControl};
use conduit_core::{port_id, ConnectionTrack, OfferGeneration, SignId};
use conduit_face::{DirectArtifactAcknowledgedSpokenShow, FaceRole};
use std::{path::Path, time::Duration};

struct Capture(Vec<ExternalForeDelivery>);
impl ExternalForeOutputAdapter for Capture {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        self.0.push(value);
        Ok(())
    }
}

#[test]
#[ignore = "requires real eSpeak, native coverage, artifact directory and actual retained Face"]
fn native_provider_acknowledges_both_template_openings_on_the_same_actual_face() {
    prove_templates(
        false,
        &[
            DirectSpokenTemplateLook::Brief,
            DirectSpokenTemplateLook::Expanded,
        ],
    );
}

#[test]
#[ignore = "requires real eSpeak, native coverage, artifact directory and actual retained Face"]
fn native_provider_completes_both_templates_on_the_same_actual_face() {
    prove_templates(
        true,
        &[
            DirectSpokenTemplateLook::Brief,
            DirectSpokenTemplateLook::Expanded,
        ],
    );
}

#[test]
#[ignore = "requires real selected eSpeak, native coverage, artifact directory and actual retained control Face"]
fn native_provider_completes_control_template_on_actual_retained_face() {
    prove_templates(true, &[DirectSpokenTemplateLook::Control]);
}

fn prove_templates(complete: bool, looks: &[DirectSpokenTemplateLook]) {
    let face_path =
        std::path::PathBuf::from(std::env::var_os("CONDUIT_SPOKEN_TEMPLATE_FACE").unwrap());
    let coverage_path = std::path::PathBuf::from(
        std::env::var_os("CONDUIT_SPOKEN_TEMPLATE_LANGUAGE_COVERAGE").unwrap(),
    );
    let artifacts =
        std::path::PathBuf::from(std::env::var_os("CONDUIT_SPOKEN_TEMPLATE_ARTIFACT_DIR").unwrap());
    assert!(face_path.is_absolute() && coverage_path.is_absolute() && artifacts.is_absolute());
    std::fs::create_dir(&artifacts).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&artifacts, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let record: serde_json::Value =
        serde_json::from_slice(&std::fs::read(face_path).unwrap()).unwrap();
    let face: Face = serde_json::from_value(record.get("face").unwrap_or(&record).clone()).unwrap();
    assert!(face.basis.body_id.is_some());
    let config = crate::StdHostConfig {
        host_id: "proof/template-openings".into(),
        boot_id: crate::boot_identity::fresh_boot_id().into(),
        offer_generation: OfferGeneration(1),
    };
    let engine = std::fs::canonicalize("/usr/lib/x86_64-linux-gnu/libespeak-ng.so.1").unwrap();
    let discovery = crate::hosted_speech_synthesis::EspeakDiscovery::inspect(
        &std::env::var_os("CONDUIT_SPOKEN_TEMPLATE_PROVIDER")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| "/usr/bin/espeak-ng".into()),
        Path::new("/usr/lib/x86_64-linux-gnu/espeak-ng-data"),
        "en-us",
        &[engine],
    )
    .unwrap();
    let coverage = crate::hosted_speech_synthesis::read_language_coverage(&coverage_path).unwrap();
    let adapter = discovery
        .declare_language_coverage(coverage)
        .unwrap()
        .initialize(
            config.host_id.clone(),
            config.boot_id.clone(),
            config.offer_generation,
            "grant/template-openings".into(),
            Duration::from_secs(30),
        )
        .unwrap();
    let artifact = crate::hosted_wav_artifact::WavArtifactSelection::per_play_root_with_limits(
        artifacts.clone(),
        config.boot_id.clone(),
        config.offer_generation,
        if complete {
            crate::hosted_wav_artifact::WavArtifactRetentionLimits::new(
                128,
                128 * (44 + 30 * 48_000 * 4),
            )
            .unwrap()
        } else {
            crate::hosted_wav_artifact::WavArtifactRetentionLimits::DEFAULT
        },
    )
    .unwrap();
    let mut host = StdHost::new_with_composition(config, crate::StdHostComposition::reference());
    host.attach_espeak_speech_and_wav_artifact(adapter, artifact)
        .unwrap();

    let mut evidence = Vec::new();
    let mut mask_plans = std::collections::BTreeSet::new();
    let mut mask_plays = std::collections::BTreeSet::new();
    for (index, look) in looks.iter().copied().enumerate() {
        let selected = host
            .prepare_direct_spoken_template_mask(&face, look)
            .unwrap();
        let reproduced = plan_template_for_offer(
            &face,
            host.advertisement(),
            look,
            selected.mask().authority_grants.clone(),
        )
        .unwrap();
        assert_eq!(reproduced.planned_mask, selected.mask().planned_mask);
        let proposal = selected.remote_selection().unwrap();
        assert_eq!(
            proposal
                .validate_for_offer(&face, host.advertisement())
                .unwrap(),
            reproduced.planned_mask
        );
        let mut wrong_plan = proposal.clone();
        wrong_plan.plan.plan_id = "fixture/forged-spoken-plan".into();
        assert!(wrong_plan
            .validate_for_offer(&face, host.advertisement())
            .is_err());
        let mut stale_proposal = proposal;
        stale_proposal.face_revision += 1;
        assert!(stale_proposal
            .validate_for_offer(&face, host.advertisement())
            .is_err());
        assert!(plan_template_for_offer(&face, host.advertisement(), look, vec![]).is_err());
        assert!(mask_plans.insert(selected.mask().plan.plan_id.clone()));
        eprintln!(
            "actual checked {:?}: Face {} bytes, Plan {} bytes, {} wording bytes",
            look,
            serde_json::to_vec(&face).unwrap().len(),
            serde_json::to_vec(&selected.mask().planned_mask)
                .unwrap()
                .len(),
            selected.wording().wording.len()
        );
        let expected = selected.wording().clone();
        let opening = selected.opening_wording().to_owned();
        let front = face
            .subjects
            .iter()
            .find(|subject| subject.role == FaceRole::Body)
            .unwrap()
            .identity
            .clone();
        // A public ordinary preparation cannot claim a reserved checked Look.
        let forged = crate::direct_spoken_mask_runtime::DirectSpokenMaskPreparation::new(
            face.clone(),
            selected.mask().planned_mask.clone(),
            front.clone(),
            "artifact/forged-template".into(),
            "sign/forged/prepared".into(),
            "sign/forged/available".into(),
        )
        .unwrap();
        let forged_input = ExternalForeInput {
            front_port_id: port_id("face"),
            track: ConnectionTrack::Payload,
            bytes: forged.encoded_face().to_vec(),
        };
        let mut refused_capture = Capture(Vec::new());
        assert!(host
            .run_direct_spoken_mask_controlled_to(
                selected.mask().plan.fragments[0].clone(),
                forged,
                &[forged_input],
                &mut refused_capture,
                &mut std::io::sink(),
                &mut crate::ThreadTimer,
                &RunControl::default(),
            )
            .is_err());
        assert!(refused_capture.0.is_empty());
        let (fragment, mut preparation, reading) = selected
            .into_preparation(
                &host,
                &face,
                front,
                "artifact/checked-template-opening".into(),
                SignId::from(format!("sign/template-opening/{index}/prepared")),
                SignId::from(format!("sign/template-opening/{index}/available")),
            )
            .unwrap();
        preparation
            .validate_template_host(host.advertisement())
            .unwrap();
        let original_mask = preparation.planned_mask.clone();
        preparation.planned_mask.plan.plan_id = "forged/template-mask-plan".into();
        assert!(preparation
            .validate_template_host(host.advertisement())
            .is_err());
        preparation.planned_mask = original_mask;
        let mut stale = host.advertisement().clone();
        stale.offer_generation.0 += 1;
        assert!(preparation.validate_template_host(&stale).is_err());
        let mut missing_source = host.advertisement().clone();
        let source_ids: Vec<_> = crate::flow_activation::required_spoken_template_offers()
            .unwrap()
            .into_iter()
            .map(|offer| offer.capability_id)
            .collect();
        missing_source
            .capabilities
            .retain(|offer| !source_ids.contains(&offer.capability_id));
        assert!(preparation.validate_template_host(&missing_source).is_err());
        if complete {
            let mut session = host
                .execute_prepared_spoken_template_opening(
                    &face,
                    fragment,
                    preparation,
                    reading,
                    &RunControl::default(),
                )
                .unwrap();
            assert_eq!(session.template(), &expected);
            assert!(session.finish().is_err());
            let show = session.show().clone();
            if look == DirectSpokenTemplateLook::Control {
                assert!(serde_json::to_vec(&show).unwrap().len() <= 64 * 1024);
                assert!(show
                    .planned_mask
                    .plan
                    .fragments
                    .iter()
                    .flat_map(|fragment| &fragment.fore_ports)
                    .all(|fore| fore.byte_capacity <= 64 * 1024));
            }
            while !session.complete() {
                let before = session.completed_count();
                let execution = session
                    .read_next(&mut host, &face, &show, &RunControl::default())
                    .unwrap();
                assert!(execution.wav_path.starts_with(&artifacts));
                assert_eq!(session.completed_count(), before + 1);
            }
            let summary = session.finish().unwrap();
            assert_eq!(summary.template, expected);
            assert_eq!(summary.completed_wording_bytes, expected.wording.len());
            assert!(session
                .read_next(&mut host, &face, &show, &RunControl::default())
                .is_err());
            evidence.push(serde_json::json!({"look":look,"execution":summary}));
            continue;
        }
        let input = ExternalForeInput {
            front_port_id: port_id("face"),
            track: ConnectionTrack::Payload,
            bytes: preparation.encoded_face().to_vec(),
        };
        let mut capture = Capture(Vec::new());
        let report = host
            .run_direct_spoken_mask_controlled_to(
                fragment,
                preparation,
                &[input],
                &mut capture,
                &mut std::io::sink(),
                &mut crate::ThreadTimer,
                &RunControl::default(),
            )
            .unwrap();
        let kernel = report.kernel.unwrap();
        let delivery = capture
            .0
            .iter()
            .find(|value| value.front_port_id.as_str() == "show")
            .unwrap_or_else(|| {
                panic!(
                    "template opening produced no Show; kernel events: {:?}",
                    kernel.kernel_sign
                )
            });
        let shown: DirectArtifactAcknowledgedSpokenShow =
            serde_json::from_slice(&delivery.bytes).unwrap();
        shown.validate(&face).unwrap();
        assert_eq!(shown.show.show.active_play_id, kernel.active_play_id);
        assert!(mask_plays.insert(shown.show.show.active_play_id.clone()));
        assert_eq!(shown.show.show.play_sequence, index as u64);
        assert_eq!(shown.show.show.host_id, host.advertisement().host_id);
        let template = shown.checked_template.as_ref().unwrap();
        assert_eq!(template.wording, expected);
        assert_eq!(template.opening_wording, opening);
        assert_eq!(usize::from(template.frame_count), reading.frames_count());
        reading.validate_current(&host, &face).unwrap();
        let wav_path = std::path::PathBuf::from(shown.artifact.artifact_locator.as_ref().unwrap());
        assert!(wav_path.starts_with(&artifacts));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&wav_path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        let wav = std::fs::read(&wav_path).unwrap();
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(wav.len() - 44, shown.artifact.pcm_bytes as usize);
        use sha2::Digest;
        assert_eq!(
            format!("{:x}", sha2::Sha256::digest(&wav[44..])),
            shown.artifact.content_sha256
        );
        evidence.push(
            serde_json::json!({"look":look,"opening_wording":opening,"shown":shown,
            "wav_sha256":format!("{:x}",sha2::Sha256::digest(&wav)),
            "wav_bytes":wav.len(),"remaining_frames":reading.frames_count()-1}),
        );
    }
    assert_eq!(mask_plans.len(), looks.len());
    let path = artifacts.join(if complete {
        "complete-evidence.json"
    } else {
        "opening-evidence.json"
    });
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path).unwrap();
    serde_json::to_writer_pretty(file, &serde_json::json!({
        "proof_class":if complete {"real native speech provider; actual retained Face and Host; complete selected template artifact scripts"} else {"real native speech provider; actual retained Face; selected template opening artifacts only"},
        "complete_template_audio":complete,"speaker_delivery":false,"openings":evidence,
    })).unwrap();
}

#[test]
fn control_wording_keeps_real_provider_admission_separate_from_fixture_equipment() {
    // Deterministic speech equipment fixture: planning only, no provider/audio claim.
    let root = tempfile::tempdir().unwrap();
    let mut host = StdHost::new_with_composition(
        crate::StdHostConfig {
            host_id: "fixture/control-plan".into(),
            boot_id: "fixture/control-plan/boot".into(),
            offer_generation: OfferGeneration(1),
        },
        crate::StdHostComposition::minimal(),
    );
    let artifact = crate::hosted_wav_artifact::WavArtifactSelection::new(
        root.path().join("unused.wav"),
        host.advertisement().boot_id.clone(),
        host.advertisement().offer_generation,
    )
    .unwrap();
    host.attach_deterministic_speech_and_wav_artifact(artifact)
        .unwrap();
    let face = Face::new(
        1,
        conduit_face::FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![
            conduit_face::FaceSubject {
                identity: "control".into(),
                role: FaceRole::Region,
                name: "Room".into(),
            },
            conduit_face::FaceSubject {
                identity: "status".into(),
                role: FaceRole::Status,
                name: "Settings".into(),
            },
        ],
        vec![conduit_face::FaceRelationship {
            source: "control".into(),
            target: "status".into(),
            kind: conduit_face::FaceRelationshipKind::Contains,
        }],
        vec![],
        vec![conduit_face::FaceText {
            subject: "status".into(),
            text: "Heat requested; equipment activity unconfirmed.".into(),
        }],
    )
    .unwrap();
    let wording = host
        .prepare_spoken_template_wording(&face, DirectSpokenTemplateLook::Control.entry())
        .unwrap();
    assert_eq!(wording.template_identity, "spoken/control@1");
    assert!(wording.wording.contains("equipment activity unconfirmed"));
    assert!(host
        .prepare_direct_spoken_template_mask(&face, DirectSpokenTemplateLook::Control)
        .is_err());
    assert!(plan_template_for_offer(
        &face,
        host.advertisement(),
        DirectSpokenTemplateLook::Control,
        vec![]
    )
    .is_err());
    let mut wrong_face = face.clone();
    wrong_face.subjects[0].role = FaceRole::Collection;
    assert!(host
        .prepare_spoken_template_wording(&wrong_face, DirectSpokenTemplateLook::Control.entry())
        .is_err());
    assert!(std::fs::read_dir(root.path()).unwrap().next().is_none());
}
