//! Canonical interaction/custody regression with explicit software Mask ACK.
//! The selected-show slot simulates the trusted renderer; no native output claim.
use super::*;
use crate::creche_draft_fixture as draft_fixture;
use crate::creche_encounter::*;
use crate::creche_mask_fixture as mask_fixture;
use conduit_body::{Body, BodyPlan, BodyPlayIdentity, BodyPlotPlan, ResidentPlot};
use conduit_core::{ConnectionTrack, PortDirection};
use conduit_face::FaceInteractionArgument;
use conduit_face::MaskShow;
use conduit_face::{Face, FaceInteraction};
use conduit_std_host::hosted_creche::CrecheHost;
use conduit_std_host::hosted_creche::CrechePoll;
use conduit_std_host::hosted_creche::{CrecheLimits, HostedCrecheAdapter};
use std::collections::BTreeMap;
use std::sync::Mutex;

#[derive(Default)]
struct FixtureReturnedInput {
    show: Option<MaskShow>,
    returned: Option<FaceInteraction>,
}
struct Custody(Arc<Mutex<FixtureReturnedInput>>);
impl CrecheMaskShowCustody for Custody {
    fn validate_acknowledged(&mut self, _: &Face, show: &MaskShow) -> Result<(), String> {
        if self.0.lock().unwrap().show.as_ref() == Some(show) {
            Ok(())
        } else {
            Err("fixture selected renderer has not acknowledged this Show".into())
        }
    }
    fn validate_returned_interaction(
        &mut self,
        _: &Face,
        _: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<(), String> {
        if self.0.lock().unwrap().returned.as_ref() == Some(interaction) {
            Ok(())
        } else {
            Err("fixture Mask did not return this exact input occurrence".into())
        }
    }
    fn retire_and_drain(&mut self) -> Result<(), String> {
        *self.0.lock().unwrap() = FixtureReturnedInput::default();
        Ok(())
    }
}
fn adapter() -> (
    CrecheEncounterAdapter,
    CrecheEncounterController,
    Arc<Mutex<FixtureReturnedInput>>,
) {
    let mut draft = draft_fixture::encounter("Garden");
    let mut description = draft.construction().description().clone();
    for host in &mut description.hosts {
        host.configuration = "fixture/iso.host.conduit".into();
        host.spore.output = conduit_host_make::SporeOutputKind::DiskImage;
    }
    let config = conduit_host_make::parse_host_configuration_conduit(include_str!(
        "../../../../targets/conduitos/profiles/conduitos-native.host.conduit"
    ))
    .unwrap();
    let checked = conduit_body_make::check_body_description(
        description,
        &BTreeMap::from([("fixture/iso.host.conduit".into(), config)]),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .unwrap();
    draft
        .replace_construction(draft.revision(), checked, "first")
        .unwrap();
    let profiles = draft
        .construction()
        .hosts()
        .iter()
        .map(|h| h.configuration.clone())
        .collect();
    let acknowledged = Arc::new(Mutex::new(FixtureReturnedInput::default()));
    let (adapter, controller) = CrecheEncounterAdapter::new(
        draft,
        profiles,
        "fixture/deployment".into(),
        "fixture/creche-slot".into(),
        CrecheLimits {
            maximum_evidence_bytes: 4096,
            maximum_polls: 128,
            minimum_poll_interval_ms: 1,
        },
        32,
        Box::new(Custody(acknowledged.clone())),
    )
    .unwrap();
    (adapter, controller, acknowledged)
}
fn plan(
    adapter: &CrecheEncounterAdapter,
) -> (conduit_core::HostAdvertisement, BodyPlan, BodyPlayIdentity) {
    let mut host = conduit_std_host::StdHost::new().advertisement().clone();
    host.capabilities = vec![conduit_body_make::deployment_runtime_catalog::creche_std_offer()];
    host.resources = vec![adapter.resource_offer()];
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    conduit_body_make::deployment_runtime_catalog::install_creche_catalogs(
        &mut startup,
        &mut profile,
    )
    .unwrap();
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(include_str!("../../../../plots/creche/main.conduit")),
        &startup,
    )
    .unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "creche/main", &profile)
            .unwrap();
    let hosts = [host.clone()];
    let placements =
        conduit_planner::default_expanded_placements(&expanded.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "request", 32),
        (PortDirection::Output, "outcome", 65),
    ]
    .into_iter()
    .map(|(direction, name, bytes)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: conduit_core::port_id(name),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: bytes,
            },
        )
    })
    .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &expanded,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 65,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    let body = Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        "fixture/creche-born".into(),
    )
    .unwrap();
    let wake = body.wake(1, "fixture/creche-wake".into()).unwrap().1;
    let plan = BodyPlan::seal(
        &wake,
        vec![BodyPlotPlan {
            plot: ResidentPlot::new(
                plan.source_document_id.clone(),
                plan.checked_plot_id.clone(),
            ),
            plan,
        }],
    )
    .unwrap();
    let play = BodyPlayIdentity::bind(&plan, 0);
    (host, plan, play)
}
fn interaction(
    face: &Face,
    show: &MaskShow,
    action: &str,
    value: Option<&[u8]>,
) -> FaceInteraction {
    let action = face
        .actions
        .iter()
        .find(|a| a.identity == action)
        .unwrap_or_else(|| {
            panic!(
                "action {action} is absent from current Face revision {}",
                face.revision
            )
        });
    let arguments = value
        .map(|value| {
            vec![FaceInteractionArgument {
                name: action.arguments[0].name.clone(),
                value_kind: action.arguments[0].contract.value_kind.as_str().into(),
                value: value.to_vec(),
            }]
        })
        .unwrap_or_default();
    FaceInteraction::new(face, show, &action.identity, &action.target, arguments, 1).unwrap()
}
fn act(
    owner: &mut CrecheHost<'_>,
    controller: &CrecheEncounterController,
    acknowledged: &Arc<Mutex<FixtureReturnedInput>>,
    action: &str,
    value: Option<&[u8]>,
) {
    let face = controller.face().unwrap().unwrap();
    let show = mask_fixture::acknowledged_fixture_show(&face);
    acknowledged.lock().unwrap().show = Some(show.clone());
    controller.acknowledge_show(show.clone()).unwrap();
    let returned = interaction(&face, &show, action, value);
    acknowledged.lock().unwrap().returned = Some(returned.clone());
    controller.submit(returned).unwrap();
    std::thread::sleep(owner.poll_wait());
    assert_eq!(owner.poll().unwrap(), CrechePoll::Pending);
}

use conduit_body::{PreparedBirthMedia, PREPARED_BIRTH_MEDIA_BYTES, PREPARED_BIRTH_MEDIA_SCHEMA};
use conduit_host_make::{build_default_host_image, BuildInputs};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    sync::atomic::AtomicU64,
};
const SLOT: usize = 65530;
const LEGACY: &[u8] = b"CONDUIT_SPORE_MEDIA@1\0";
fn handoff(source: &str) -> CrecheReviewedHandoff {
    let (mut adapter, controller, custody) = adapter();
    let (host, plan, play) = plan(&adapter);
    let mut owner = CrecheHost::prepare(&mut adapter, &host, &plan, &play).unwrap();
    owner.confirm_started(&host, &play).unwrap();
    owner.begin(owner.request()).unwrap();
    for (action, value) in [
        ("creche.deploy.next", None),
        ("creche.deploy.scope", Some(b"all".as_slice())),
        ("creche.deploy.next", None),
        ("creche.deploy.source", Some(source.as_bytes())),
        ("creche.deploy.review", None),
    ] {
        act(&mut owner, &controller, &custody, action, value);
    }
    let handoff = controller.take_reviewed().unwrap().unwrap();
    assert!(controller.take_reviewed().unwrap().is_none());
    owner.retire().unwrap();
    handoff
}
fn job(source: &str, cancelled: Arc<AtomicBool>) -> CrecheImageProduction {
    CrecheImageProduction::prepare(
        handoff(source),
        1,
        "fixture/production-birth".into(),
        1024 * 1024,
        cancelled,
    )
    .unwrap()
}
fn iso() -> Vec<u8> {
    let mut bytes = vec![0x5a; 160 * 1024];
    bytes[16 * 2048..16 * 2048 + 7].copy_from_slice(&[1, b'C', b'D', b'0', b'0', b'1', 1]);
    blank_slot(&mut bytes, SLOT);
    // A parser string is not a pristine provision slot.
    bytes[1024..1024 + LEGACY.len()].copy_from_slice(LEGACY);
    bytes
}
fn blank_slot(bytes: &mut [u8], offset: usize) {
    let slot = &mut bytes[offset..offset + PREPARED_BIRTH_MEDIA_BYTES];
    slot.fill(0xff);
    slot[..LEGACY.len()].copy_from_slice(LEGACY);
    slot[24..32].fill(0);
}
fn manifest(image: &HostImage, bytes: &[u8]) -> Value {
    let digest = format!("{:x}", Sha256::digest(bytes));
    json!({"schema":"conduit.host/target-build-manifest@3", "artifact_role":"product-host",
        "prepared_birth_media_schema":PREPARED_BIRTH_MEDIA_SCHEMA,
        "profile_id":image.manifest.profile_id, "build_id":image.manifest.build_id,
        "source_identity":image.manifest.source_identity, "toolchain_identity":image.manifest.toolchain_identity,
        "target":image.manifest.target, "resolved_description_binding":image.manifest.image_id,
        "image_id":format!("image:sha256:{digest}"),
        "image":{"role":"final-bootable-image","file":"fixture.iso","sha256":digest},
        "resolved_build":image.manifest})
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Files {
    root: PathBuf,
}
impl Files {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "conduit-production-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self { root }
    }
    fn output(&self, name: &str) -> File {
        OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(self.root.join(name))
            .unwrap()
    }
    fn input(&self, job: &CrecheImageProduction, host: &str, corrupt: bool) -> CrecheImageInput {
        let selected = job
            .prepared
            .construction()
            .hosts()
            .iter()
            .find(|h| h.description.name == host)
            .unwrap();
        let (image, _) = build_default_host_image(
            selected.configuration.profile().clone(),
            &conduit_workspace_make::catalog(),
            &conduit_workspace_make::package_set(),
            &BuildInputs {
                source_identity: "git:fixture-source".into(),
                toolchain_available: true,
            },
        )
        .unwrap();
        let mut bytes = iso();
        let metadata = manifest(&image, &bytes);
        if corrupt {
            bytes[100] ^= 1;
        }
        let name = format!("base-{}.iso", NEXT.fetch_add(1, Ordering::Relaxed));
        fs::write(self.root.join(&name), bytes).unwrap();
        CrecheImageInput::ReviewedRelease(Box::new(CrecheReviewedArtifact {
            image,
            target_build_manifest: serde_json::to_vec(&metadata).unwrap(),
            file: File::open(self.root.join(name)).unwrap(),
        }))
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
fn failed_bytes_retry_and_multiple_hosts_keep_exactly_one_birth() {
    let mut job = job("release", Arc::new(AtomicBool::new(false)));
    let birth = job.biography().clone();
    let files = Files::new();
    let bad = files.input(&job, "first", true);
    assert!(matches!(
        job.produce_host("first", bad, files.output("failed.iso")),
        Err(CrecheProductionRefusal::Artifact(
            PreparedBirthArtifactRefusal::InvalidManifest
        ))
    ));
    assert_eq!(job.biography(), &birth);
    assert!(job.artifacts().is_empty());
    assert!(files.root.join("failed.iso").exists());
    let first = files.input(&job, "first", false);
    job.produce_host("first", first, files.output("first.iso"))
        .unwrap();
    let second = files.input(&job, "second", false);
    job.produce_host("second", second, files.output("second.iso"))
        .unwrap();
    assert!(job.is_complete());
    assert_eq!(job.biography(), &birth);
    for artifact in job.artifacts() {
        assert!(artifact.builder_receipt().is_none());
        assert!(!artifact.receipt().boot_observed && !artifact.receipt().join_observed);
        let mut file = artifact.file.try_clone().unwrap();
        file.seek(SeekFrom::Start(SLOT as u64)).unwrap();
        let mut frame = vec![0; PREPARED_BIRTH_MEDIA_BYTES];
        file.read_exact(&mut frame).unwrap();
        let media = PreparedBirthMedia::decode(&frame).unwrap();
        assert_eq!(media.evidence(), &birth);
        assert_eq!(media.binding().host_entry, artifact.receipt().host_entry);
    }
    let duplicate = files.input(&job, "first", false);
    assert!(matches!(
        job.produce_host("first", duplicate, files.output("duplicate.iso")),
        Err(CrecheProductionRefusal::UnknownOrCompletedHost)
    ));
}
#[test]
fn source_mismatch_cancel_and_output_reuse_cannot_create_products() {
    let files = Files::new();
    let cancelled = Arc::new(AtomicBool::new(false));
    let mut source = job("build", cancelled.clone());
    let input = files.input(&source, "first", false);
    assert!(matches!(
        source.produce_host("first", input, files.output("wrong-source.iso")),
        Err(CrecheProductionRefusal::WrongImageSource)
    ));
    assert!(source.artifacts().is_empty());
    let mut release = job("release", cancelled.clone());
    let used = files.output("used.iso");
    fs::write(files.root.join("used.iso"), b"preserve").unwrap();
    let input = files.input(&release, "first", false);
    assert!(matches!(
        release.produce_host("first", input, used),
        Err(CrecheProductionRefusal::InvalidBound)
    ));
    assert_eq!(fs::read(files.root.join("used.iso")).unwrap(), b"preserve");
    cancelled.store(true, Ordering::Release);
    let input = files.input(&release, "first", false);
    assert!(matches!(
        release.produce_host("first", input, files.output("cancelled.iso")),
        Err(CrecheProductionRefusal::Cancelled)
    ));
    assert!(release.artifacts().is_empty());
    assert_eq!(
        fs::metadata(files.root.join("cancelled.iso"))
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn finish_requires_complete_unchanged_batch_and_retains_birth_on_refusal() {
    use std::io::Write;
    let mut job = job("release", Arc::new(AtomicBool::new(false)));
    let birth = job.biography().clone();
    job = match job.finish() {
        Err((CrecheProductionRefusal::UnknownOrCompletedHost, retained)) => *retained,
        _ => panic!("incomplete batch must retain its job"),
    };
    assert_eq!(job.biography(), &birth);
    let files = Files::new();
    for host in ["first", "second"] {
        let input = files.input(&job, host, false);
        job.produce_host(host, input, files.output(&format!("{host}.iso")))
            .unwrap();
    }
    let mut first = OpenOptions::new()
        .write(true)
        .open(files.root.join("first.iso"))
        .unwrap();
    first.seek(SeekFrom::Start(100)).unwrap();
    first.write_all(&[0]).unwrap();
    first.sync_all().unwrap();
    let mut job = match job.finish() {
        Err((
            CrecheProductionRefusal::Artifact(PreparedBirthArtifactRefusal::InvalidManifest),
            retained,
        )) => *retained,
        _ => panic!("changed earlier product must refuse final batch"),
    };
    assert_eq!(job.biography(), &birth);
    assert_eq!(job.artifacts().len(), 2);
    // Restore exact bytes so the same retained batch can be verified; never rebirth.
    first.seek(SeekFrom::Start(100)).unwrap();
    first.write_all(&[0x5a]).unwrap();
    first.sync_all().unwrap();
    job.cancelled.store(true, Ordering::Release);
    job = match job.finish() {
        Err((CrecheProductionRefusal::Cancelled, retained)) => *retained,
        _ => panic!("final-read cancellation must retain the completed job"),
    };
    assert_eq!(job.biography(), &birth);
    job.cancelled.store(false, Ordering::Release);
    let complete = match job.finish() {
        Ok(complete) => complete,
        Err((error, _)) => panic!("restored exact batch: {error:?}"),
    };
    assert_eq!(complete.biography(), &birth);
    let evidence = complete.evidence(64 * 1024).unwrap();
    let value: Value = serde_json::from_slice(&evidence).unwrap();
    assert_eq!(value["schema"], "conduit.creche/prepared-image-evidence@1");
    assert_eq!(value["artifact_prepared"], true);
    for key in ["carrier_realized", "boot_observed", "join_observed"] {
        assert_eq!(value[key], false);
    }
    assert_eq!(value["artifacts"].as_array().unwrap().len(), 2);
    assert_eq!(complete.evidence(evidence.len() as u64).unwrap(), evidence);
    assert!(matches!(
        complete.evidence(evidence.len() as u64 - 1),
        Err(CrecheProductionRefusal::InvalidBound)
    ));
}

mod completion;

mod custody;
