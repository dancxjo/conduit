//! Actual Owner/Std kernel and software artifact, with an explicit CPU trusted
//! Mask-return fixture. No native rendering, deployment, Boot or join proof.
use super::*;
use crate::durable_host as installed;
use conduit::creche_encounter::{CrecheEncounterInstallation, CrecheReviewedArtifact};
use conduit_body::{BodyState, PortableInvitation, ResidentPlot};
use conduit_body_make::{JoiningProductionBounds, SporeBinding};
use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_face::FaceInteraction;
use conduit_host_make::{
    build_host_image, check_host_configuration, parse_host_configuration_conduit, BuildInputs,
    SporeOutputKind,
};
use conduit_std_host::{StdHostComposition, StdHostConfig};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    time::{Duration, Instant},
};

const SOURCE: &str = include_str!("../../../../../plots/creche/main.conduit");
const CONFIGURATION: &str =
    include_str!("../../../../../targets/std/profiles/linux-computer.host.conduit");
const HOST: &str = "host/creche-owner-fixture";
const BOOT: &str = "boot/creche-original-fixture";

struct Fixture {
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(installed::fresh_identity(
            "owner-creche-service-test",
            "actual-kernel",
        ));
        fs::create_dir_all(&root).unwrap();
        let installation = installed::Installation {
            schema: installed::INSTALL_SCHEMA.into(),
            host_id: HOST.into(),
            release_source_identity: "source/test".into(),
            release_bundle_sha256: installed::digest(b"bundle/test"),
            product_executable: "fixture-unused".into(),
            body_state: None,
            joined_body_state: None,
            selected_speech: None,
            selected_model: None,
            selected_host_configuration: None,
            selected_todo_checkpoint: None,
        };
        installed::write_json_atomic(&root.join("installation.json"), &installation).unwrap();
        Self { root }
    }
    fn factory(&self) -> CrecheEncounterFactory {
        let catalog = conduit_workspace_make::catalog();
        let packages = conduit_workspace_make::package_set();
        let configuration = check_host_configuration(
            parse_host_configuration_conduit(CONFIGURATION).unwrap(),
            &catalog,
            &packages,
        )
        .unwrap();
        let (image, image_manifest) = build_host_image(
            configuration.profile().clone(),
            &catalog,
            &packages,
            &SporeOutputKind::NativeBundle,
            &BuildInputs {
                source_identity: "git:cpu-fixture-prior-make".into(),
                toolchain_available: true,
            },
        )
        .unwrap();
        let path = self.root.join("prior-make-fixture.bin");
        // Each factory receives a distinct opened file. This models authorized
        // prior Make custody, not a fabricated build or a deployable executable.
        let mut file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        file.write_all(b"cpu-fixture-artifact").unwrap();
        CrecheEncounterFactory::new(
            catalog,
            packages,
            CrecheEncounterInstallation {
                encounter_id: "creche/owner-fixture".into(),
                resource_pool: "pool/owner-creche-fixture".into(),
                limits: conduit_std_host::hosted_creche::CrecheLimits {
                    maximum_evidence_bytes: 65536,
                    maximum_polls: 10000,
                    minimum_poll_interval_ms: 1,
                },
                production_bounds: JoiningProductionBounds {
                    maximum_artifact_bytes: 1024,
                    maximum_manifest_bytes: 65536,
                    maximum_receipt_bytes: 65536,
                },
                initial_source: Some(CONFIGURATION.into()),
            },
            vec![CrecheReviewedArtifact {
                configuration,
                output: SporeOutputKind::NativeBundle,
                image,
                image_manifest,
                artifact: file,
                artifact_digest: format!("sha256:{:x}", Sha256::digest(b"cpu-fixture-artifact")),
                artifact_bytes: 20,
                source_identity: "git:cpu-fixture-prior-make".into(),
            }],
        )
        .unwrap()
    }
    fn owner(&self, slot: Option<conduit_core::ResourceOffer>) -> Owner {
        let config = StdHostConfig {
            host_id: HostId::from(HOST),
            boot_id: BootId::from(BOOT),
            offer_generation: OfferGeneration(1),
        };
        let host = match slot {
            Some(slot) => {
                StdHost::new_with_creche_encounter(config, StdHostComposition::reference(), slot)
                    .unwrap()
            }
            None => StdHost::new_with_composition(config, StdHostComposition::reference()),
        };
        let plot = crate::plot_source::parse(SOURCE)
            .unwrap()
            .expand_entry_for_authoring()
            .unwrap();
        let resident = ResidentPlot::new(
            plot.expanded.source_document_id.clone(),
            plot.expanded.checked_plot_id.clone(),
        );
        let mut owner = Owner::open(host, resident, None, "One fixture Body").unwrap();
        owner.set_resident_plot_name(&plot).unwrap();
        owner.persist(&self.root).unwrap();
        fs::write(self.root.join("body/source.conduit"), SOURCE).unwrap();
        owner
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn ordinary_creche_play_prepares_one_same_body_seed_and_restores_original_host() {
    let fixture = Fixture::new();
    let factory = fixture.factory();
    let mut owner = fixture.owner(Some(factory.resource_offer()));
    let original_host = owner.host.advertisement().clone();
    let body = owner.session.evidence().body_id.clone();
    let original_birth = owner
        .session
        .evidence()
        .body
        .events
        .first()
        .unwrap()
        .clone();
    owner.install_creche_factory(factory).unwrap();
    let mut worker = owner.start_service_run(&fixture.root, 10_000).unwrap();
    assert!(owner.host.is_playing());
    assert!(owner.creche_factory.is_none());
    assert!(owner.creche_products.is_none());
    assert!(owner.start_service_run(&fixture.root, 10_000).is_err());
    let deadline = Instant::now() + Duration::from_secs(8);
    let face = loop {
        assert!(
            Instant::now() < deadline,
            "real Crèche Play did not reach its live Face"
        );
        assert!(!worker.progress(&mut owner, &fixture.root).unwrap());
        if owner.current_play_id().is_some() {
            if let Some(face) = owner
                .creche_live
                .as_ref()
                .and_then(|live| live.controller.face().unwrap())
            {
                break face;
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    };
    let play = owner.current_play_id().unwrap().clone();
    assert_eq!(face.basis.body_id.as_ref(), Some(&body));
    assert_eq!(face.basis.active_play_id.as_ref(), Some(&play));
    assert_eq!(owner.host.advertisement(), &original_host);
    for (name, expected) in [("host-id", HOST), ("boot-id", BOOT)] {
        assert!(face.properties.iter().any(|property| property.name == name
            && matches!(&property.value, conduit_face::FacePropertyValue::Identity(value) if value == expected)));
    }
    assert!(matches!(
        super::super::state::load(&fixture.root)
            .unwrap()
            .unwrap()
            .body
            .state,
        BodyState::Awake { .. }
    ));
    assert_eq!(
        owner.truth()["realization"]["play"]["active_play_id"],
        play.as_str()
    );
    // The current ordinary owner issued this invitation through its existing
    // authority path before Play. Do not clone a Body or mint a second manager.
    let invitation: PortableInvitation = serde_json::from_slice(
        &fs::read(fixture.root.join("body/creche/invitation.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(invitation.claim.body_id, body);
    let show = crate::mask_test_common::available_mask_show(&face);
    let input =
        FaceInteraction::new(&face, &show, "host-config/check", "host-config", vec![], 1).unwrap();
    // Structural Available is insufficient until the trusted route model below
    // records this exact typed return. This is explicitly a CPU fixture boundary.
    assert!(owner
        .creche_live
        .as_ref()
        .unwrap()
        .controller
        .acknowledge_show(show.clone())
        .is_err());
    owner.submit_validated_creche_return(&show, &input).unwrap();
    assert!(owner.creche_products.is_none());
    assert!(owner
        .creche_live
        .as_ref()
        .unwrap()
        .controller
        .take_products()
        .unwrap()
        .is_none());
    while !worker.progress(&mut owner, &fixture.root).unwrap() {
        assert!(
            Instant::now() < deadline,
            "Crèche did not finish its typed outcome and drain"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(!owner.host.is_playing());
    assert_eq!(owner.host.advertisement(), &original_host);
    assert!(owner.creche_factory.is_some());
    assert!(owner.current_play_id().is_none());
    let products = owner
        .creche_products
        .take()
        .expect("typed Prepared outcome retained its verified Seed");
    assert_eq!(products.spore.manifest.body_id, body.as_str());
    assert_eq!(
        products.spore.manifest.binding,
        SporeBinding::SelfJoining {
            invitation_id: invitation.claim.invitation_id.as_str().into()
        }
    );
    assert_eq!(
        products.spore.manifest.source_identity,
        match &original_birth {
            conduit_body::BodyLifecycleEvent::Born { sign_id, .. } => sign_id.as_str(),
            _ => panic!("original Birth event absent"),
        }
    );
    let receipt: serde_json::Value = serde_json::from_slice(&products.receipt).unwrap();
    assert_eq!(receipt["disposition"], "prepared");
    assert_eq!(receipt["body_id"], body.as_str());
    assert_eq!(receipt["spore_id"], products.spore.manifest.spore_id);
    assert_eq!(receipt["artifact_bytes"], 20);
    assert_eq!(owner.session.evidence().body_id, body);
    assert_eq!(
        owner.session.evidence().body.events.first(),
        Some(&original_birth)
    );
    let execution = owner.last_execution.as_ref().unwrap();
    assert_eq!(execution["play"]["active_play_id"], play.as_str());
    assert!(execution["failure"].is_null());
    assert!(execution["cleanup_failure"].is_null());
    assert!(execution["pre_play_refusal"].is_null());
    assert!(owner
        .creche_live
        .as_ref()
        .unwrap()
        .controller
        .take_products()
        .unwrap()
        .is_none());
    assert!(!owner
        .creche_live
        .as_ref()
        .unwrap()
        .controller
        .start_production_if_reviewed()
        .unwrap());
    // A second ordinary admission on the returned factory/original Host is a
    // stronger public idle witness than merely seeing an unoccupied owner slot.
    let mut second = owner.start_service_run(&fixture.root, 5_000).unwrap();
    while owner.current_play_id().is_none() {
        assert!(Instant::now() < deadline);
        assert!(!second.progress(&mut owner, &fixture.root).unwrap());
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_ne!(owner.current_play_id(), Some(&play));
    second.request_lull().unwrap();
    while !second.progress(&mut owner, &fixture.root).unwrap() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(!owner.host.is_playing());
    assert_eq!(owner.host.advertisement(), &original_host);
    assert_eq!(owner.session.evidence().body_id, body);
}

#[test]
fn creche_without_ready_provider_refuses_before_playing_or_invitation() {
    for advertised_slot in [false, true] {
        let fixture = Fixture::new();
        let factory = fixture.factory();
        let mut owner = fixture.owner(advertised_slot.then(|| factory.resource_offer()));
        let original_host = owner.host.advertisement().clone();
        let original_body = owner.session.evidence().body_id.clone();
        let error = owner
            .start_service_run(&fixture.root, 5_000)
            .err()
            .expect("uninitialized provider must refuse");
        if advertised_slot {
            assert!(
                error.contains("no retained initialized provider"),
                "{error}"
            );
        }
        assert!(!owner.host.is_playing());
        assert!(owner.current_play_id().is_none());
        assert!(owner.session.realization().is_none());
        assert_eq!(owner.host.advertisement(), &original_host);
        assert_eq!(owner.session.evidence().body_id, original_body);
        assert!(!fixture.root.join("body/creche/invitation.json").exists());
        assert!(owner.creche_products.is_none());
        assert_eq!(
            super::super::state::load(&fixture.root)
                .unwrap()
                .unwrap()
                .body
                .state,
            BodyState::Lulled
        );
    }
}
