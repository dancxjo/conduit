//! CPU lifecycle/mailbox fixtures. These do not prove physical rendering, Boot,
//! installed service wiring, or membership.
use super::*;
#[path = "../../tests/common/creche_bundle.rs"]
mod bundle_common;
mod bundle_fixture;
use bundle_fixture::factory;
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
        .unwrap();
    CrecheBodyBinding {
        biography,
        invitation: conduit_body::PortableInvitation {
            schema: conduit_body::INVITATION_SCHEMA.into(),
            claim: invitation.claim(),
            secret: invitation.secret.copy_for_target_provisioning(),
            rendezvous: None,
        },
    }
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
    let (mut factory, _directory) = factory();
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
    let (mut factory, _directory) = factory();
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
    assert_eq!(
        outcome.disposition,
        CrecheDisposition::Prepared,
        "{}",
        String::from_utf8_lossy(adapter.retained_evidence(&outcome).unwrap())
    );
    assert!(!adapter.retained_evidence(&outcome).unwrap().is_empty());
    let mut products = controller.take_products().unwrap().unwrap();
    bundle_common::assert_seed_package(
        &mut products.artifact,
        &products.spore.manifest,
        &adapter.channel.binding.invitation,
        &products.receipt,
        b"artifact",
    );
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
    let (mut factory, _directory) = factory();
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
fn factory_refuses_missing_bundle_proof_and_empty_inventory_before_advertisement() {
    let (prepared, _directory) = factory();
    let inputs = Arc::try_unwrap(prepared.inputs).ok().unwrap();
    let mut artifacts = inputs.artifacts;
    artifacts[0].bundle = None;
    assert!(CrecheEncounterFactory::new(
        inputs.catalog,
        inputs.packages,
        inputs.installation,
        artifacts
    )
    .is_err());
    let (prepared, _directory) = factory();
    let inputs = Arc::try_unwrap(prepared.inputs).ok().unwrap();
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
    let (mut factory, _directory) = factory();
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
    let (mut factory, _directory) = factory();
    let (mut adapter, _) = factory.encounter(binding(), Box::new(Failure)).unwrap();
    start_fixture(&mut adapter);
    assert!(adapter
        .cancel_and_drain()
        .unwrap_err()
        .contains("uncertain"));
    assert!(!adapter.retired);
}

#[test]
fn encounter_refuses_invalid_private_provision_before_effects() {
    let (mut factory, _directory) = factory();
    let mut binding = binding();
    binding.invitation.secret.fill(0);
    assert!(factory
        .encounter(
            binding,
            Box::new(Custody {
                admitted: false,
                drained: Arc::new(AtomicBool::new(false))
            })
        )
        .is_err());
}

#[test]
fn reviewed_bundle_refuses_nonmatching_retained_payload_handles() {
    let (factory, directory) = factory();
    let image = &factory.inputs.artifacts[0].image;
    let release = bundle_common::native_release(image, b"artifact");
    let mut wrong = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(directory.0.join("wrong-payload"))
        .unwrap();
    wrong.write_all(b"changed!").unwrap();
    assert!(CrecheReviewedBundle::native(
        release,
        std::collections::BTreeMap::from([("fixture.data".into(), wrong)]),
        directory.0.clone()
    )
    .is_err());
}
