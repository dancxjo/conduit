//! CPU lifecycle/mailbox fixtures. These do not prove physical rendering, Boot,
//! installed service wiring, or membership.
use super::*;
use conduit_body::{AdmissionManager, Body, BodyMembership, SpawnInvitationSecret};
use conduit_host_make::{build_host_image, BuildInputs};
use std::{
    io::Write,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::{Duration, Instant},
};

struct Custody {
    admitted: bool,
    drained: Arc<AtomicBool>,
}
impl CrecheMaskShowCustody for Custody {
    fn validate_acknowledged(&mut self, _: &Face, _: &MaskShow) -> Result<(), String> {
        if self.admitted {
            Ok(())
        } else {
            Err("no actual renderer receipt".into())
        }
    }
    fn validate_returned_interaction(
        &mut self,
        _: &Face,
        _: &MaskShow,
        _: &FaceInteraction,
    ) -> Result<(), String> {
        if self.admitted {
            Ok(())
        } else {
            Err("no actual typed return receipt".into())
        }
    }
    fn retire_and_drain(&mut self) -> Result<(), String> {
        self.drained.store(true, Ordering::Release);
        Ok(())
    }
}
// The custody fixture simply models presence/absence of an independently held
// receipt; production must provide the actual selected renderer implementation.
fn source() -> &'static str {
    include_str!("../../../../targets/std/profiles/linux-computer.host.conduit")
}
fn checked() -> CheckedHostConfiguration {
    conduit_host_make::check_host_configuration(
        conduit_host_make::parse_host_configuration_conduit(source()).unwrap(),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .unwrap()
}
fn binding() -> CrecheBodyBinding {
    let body = Body::born(
        "source/test".into(),
        "checked/test".into(),
        1,
        "sign/test-birth".into(),
    )
    .unwrap();
    let biography = BodyBiographyEvidence::born(
        body.clone(),
        BodyMembership::new(body.body_id.clone()).unwrap(),
        "Test Body".into(),
    )
    .unwrap();
    let now = clock().unwrap();
    let invitation = AdmissionManager::new(body.body_id)
        .unwrap()
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([7; 32]).unwrap(),
            [8; 32],
            now,
            now + 60_000,
        )
        .unwrap()
        .claim();
    CrecheBodyBinding {
        biography,
        invitation,
    }
}
fn factory() -> CrecheEncounterFactory {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "conduit-creche-artifact-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut artifact = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    artifact.write_all(b"artifact").unwrap();
    std::fs::remove_file(path).unwrap();
    let configuration = checked();
    let (image, image_manifest) = build_host_image(
        configuration.profile().clone(),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
        &SporeOutputKind::NativeBundle,
        &BuildInputs {
            source_identity: "git:reviewed".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    CrecheEncounterFactory::new(
        conduit_workspace_make::catalog(),
        conduit_workspace_make::package_set(),
        CrecheEncounterInstallation {
            encounter_id: "encounter/test".into(),
            resource_pool: "pool/creche".into(),
            limits: CrecheLimits {
                maximum_evidence_bytes: 65536,
                maximum_polls: 1000,
                minimum_poll_interval_ms: 1,
            },
            production_bounds: JoiningProductionBounds {
                maximum_artifact_bytes: 1024,
                maximum_manifest_bytes: 65536,
                maximum_receipt_bytes: 65536,
            },
            initial_source: Some(source().into()),
        },
        vec![CrecheReviewedArtifact {
            configuration,
            output: SporeOutputKind::NativeBundle,
            image,
            image_manifest,
            artifact,
            artifact_digest: format!("sha256:{:x}", Sha256::digest(b"artifact")),
            artifact_bytes: 8,
            source_identity: "git:reviewed".into(),
        }],
    )
    .unwrap()
}
fn start_fixture(adapter: &mut CrecheEncounterAdapter<'_>) {
    let (_, wake) = adapter
        .channel
        .binding
        .biography
        .body
        .wake(2, "sign/test-wake".into())
        .unwrap();
    let b = HostConfigurationEncounterBasis {
        host_id: "host/test".into(),
        boot_id: "boot/test".into(),
        encounter_id: "encounter/test".into(),
        execution: FaceBasis {
            body_id: Some(adapter.channel.binding.biography.body_id.clone()),
            wake_id: Some(wake.wake_id),
            source_document_id: Some("source/test".into()),
            checked_plot_id: Some("checked/test".into()),
            expanded_plot_id: Some("expanded/test".into()),
            plan_id: Some("plan/test".into()),
            active_play_id: Some("play/test".into()),
            sign_ids: vec![],
        },
    };
    let face = adapter.encounter.face(&b).unwrap();
    let request = CrecheRequest([9; 32]);
    adapter.basis = Some(b);
    adapter.request = Some(request);
    {
        let mut state = adapter.channel.lock().unwrap();
        state.face = Some(Arc::new(face));
        state.request = Some(request);
    }
    adapter.begin(request).unwrap();
}
#[test]
fn structural_available_show_does_not_grant_renderer_acknowledgment() {
    let mut factory = factory();
    let drained = Arc::new(AtomicBool::new(false));
    let (mut adapter, controller) = factory
        .encounter(
            binding(),
            Box::new(Custody {
                admitted: false,
                drained: drained.clone(),
            }),
        )
        .unwrap();
    assert!(controller.face().unwrap().is_none());
    start_fixture(&mut adapter);
    let face = controller.face().unwrap().unwrap();
    let show = crate::mask_test_common::available_mask_show(&face);
    assert!(controller
        .acknowledge_show(show)
        .unwrap_err()
        .contains("no actual renderer"));
    adapter.cancel_and_drain().unwrap();
    assert!(drained.load(Ordering::Acquire));
    assert!(controller.face().unwrap().is_none());
}
#[test]
fn checked_configuration_is_not_prepared_and_production_is_once_only() {
    let mut factory = factory();
    let drained = Arc::new(AtomicBool::new(false));
    let (mut adapter, controller) = factory
        .encounter(
            binding(),
            Box::new(Custody {
                admitted: true,
                drained: drained.clone(),
            }),
        )
        .unwrap();
    start_fixture(&mut adapter);
    let face = controller.face().unwrap().unwrap();
    let show = crate::mask_test_common::available_mask_show(&face);
    let input =
        FaceInteraction::new(&face, &show, "host-config/check", "host-config", vec![], 1).unwrap();
    controller.acknowledge_show(show).unwrap();
    controller.submit(input).unwrap();
    assert!(matches!(adapter.poll().unwrap(), CrechePoll::Pending));
    assert!(controller.take_products().unwrap().is_none());
    assert!(controller.start_production_if_reviewed().unwrap());
    assert!(!controller.start_production_if_reviewed().unwrap());
    let deadline = Instant::now() + Duration::from_secs(5);
    let outcome = loop {
        match adapter.poll().unwrap() {
            CrechePoll::Outcome(outcome) => break outcome,
            CrechePoll::Pending => {
                assert!(Instant::now() < deadline);
                std::thread::yield_now();
            }
        }
    };
    assert_eq!(outcome.disposition, CrecheDisposition::Prepared);
    assert!(!adapter.retained_evidence(&outcome).unwrap().is_empty());
    let products = controller.take_products().unwrap().unwrap();
    assert_eq!(
        products.spore.manifest.body_id,
        adapter.channel.binding.biography.body_id.as_str()
    );
    assert!(controller.take_products().unwrap().is_none());
    adapter.cancel_and_drain().unwrap();
    assert!(drained.load(Ordering::Acquire));
}
#[test]
fn retirement_prevents_delayed_production_and_repeated_begin() {
    let mut factory = factory();
    let (mut adapter, controller) = factory
        .encounter(
            binding(),
            Box::new(Custody {
                admitted: false,
                drained: Arc::new(AtomicBool::new(false)),
            }),
        )
        .unwrap();
    start_fixture(&mut adapter);
    assert!(adapter.begin(CrecheRequest([9; 32])).is_err());
    adapter.cancel_and_drain().unwrap();
    adapter.cancel_and_drain().unwrap();
    assert!(!controller.start_production_if_reviewed().unwrap());
    assert!(adapter.poll().is_err());
    assert!(controller.take_products().unwrap().is_none());
}

#[test]
fn factory_refuses_unverified_artifact_and_empty_inventory_before_advertisement() {
    let inputs = Arc::try_unwrap(factory().inputs).ok().unwrap();
    let mut artifacts = inputs.artifacts;
    artifacts[0].artifact_digest = format!("sha256:{}", "0".repeat(64));
    assert!(CrecheEncounterFactory::new(
        inputs.catalog,
        inputs.packages,
        inputs.installation,
        artifacts
    )
    .is_err());
    let inputs = Arc::try_unwrap(factory().inputs).ok().unwrap();
    assert!(CrecheEncounterFactory::new(
        inputs.catalog,
        inputs.packages,
        inputs.installation,
        vec![]
    )
    .is_err());
}

#[test]
fn editing_invalidates_prior_acknowledged_show() {
    let mut factory = factory();
    let (mut adapter, controller) = factory
        .encounter(
            binding(),
            Box::new(Custody {
                admitted: true,
                drained: Arc::new(AtomicBool::new(false)),
            }),
        )
        .unwrap();
    start_fixture(&mut adapter);
    let face = controller.face().unwrap().unwrap();
    let show = crate::mask_test_common::available_mask_show(&face);
    let input =
        FaceInteraction::new(&face, &show, "host-config/source", "host-config", vec![], 1).unwrap();
    controller.acknowledge_show(show.clone()).unwrap();
    controller.submit(input.clone()).unwrap();
    adapter.poll().unwrap();
    assert!(controller.submit(input).is_err());
    assert!(controller.acknowledge_show(show).is_err());
    adapter.cancel_and_drain().unwrap();
}

#[test]
fn failed_presentation_drain_does_not_report_retirement() {
    struct Failure;
    impl CrecheMaskShowCustody for Failure {
        fn validate_acknowledged(&mut self, _: &Face, _: &MaskShow) -> Result<(), String> {
            Err("no receipt".into())
        }
        fn validate_returned_interaction(
            &mut self,
            _: &Face,
            _: &MaskShow,
            _: &FaceInteraction,
        ) -> Result<(), String> {
            Err("no receipt".into())
        }
        fn retire_and_drain(&mut self) -> Result<(), String> {
            Err("presentation drain uncertain".into())
        }
    }
    let mut factory = factory();
    let (mut adapter, _) = factory.encounter(binding(), Box::new(Failure)).unwrap();
    start_fixture(&mut adapter);
    assert!(adapter
        .cancel_and_drain()
        .unwrap_err()
        .contains("uncertain"));
    assert!(!adapter.retired);
}
