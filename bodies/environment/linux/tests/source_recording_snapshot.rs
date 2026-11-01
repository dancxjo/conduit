//! Actual synced immutable Linux effects and fresh-process read. Host/resource
//! grants are fixtures. No selected Body State, kernel commit or ACK is claimed.
#![cfg(feature = "source-recording")]
use conduit_core as c;
use conduit_environment_measurement::{source_observation::*, EnvironmentalReading};
use conduit_environment_runtime::source_recording::*;
use conduit_linux_snapshot_store::{
    PreparedFileSnapshot, SnapshotDirectory, PUBLISH_OPERATION, READ_OPERATION,
};
#[path = "../../../../targets/std/resource-snapshot/tests/support/directory.rs"]
mod directory;
#[path = "../../../../mechanisms/io/resource-snapshot/tests/support/fixture.rs"]
mod fixture;
fn context() -> SourceRecordingContext {
    let host: c::HostId = "host/source-fixture".into();
    let boot: c::BootId = "boot/source-fixture".into();
    let plan: c::PlanId = "plan/source-fixture".into();
    SourceRecordingContext {
        body_id: "body/source-fixture".into(),
        attachment_id: "green".into(),
        host_id: host.clone(),
        boot_id: boot.clone(),
        capability_id: "source/green".into(),
        provider_instance_id: "provider/green".into(),
        provider_generation: 1,
        plan_id: plan.clone(),
        fragment_id: "fragment/source".into(),
        placement_id: "placement/source".into(),
        configuration_id: "sha256:configuration".into(),
        firmware_sha256: [42; 32],
        native_play: c::bind_active_play(&plan, &host, &boot, 1),
    }
}
fn content() -> Vec<u8> {
    let contract = ObservationContract::prepare().unwrap();
    let mut codec = PreparedObservationCodec::prepare(&contract).unwrap();
    let value = codec
        .encode(SourceObservation {
            observed_at_host_boot_ms: 9000,
            outcome: SourceOutcome::Available(SourceSample {
                reading: EnvironmentalReading::centi_celsius_centi_pascal(
                    2422,
                    10_134_081,
                    Some(46535),
                )
                .unwrap(),
                source_counter: 28,
                reported_source_ms: Some(825388),
            }),
        })
        .unwrap();
    let mut snapshot = SourceRecordingSnapshot::prepare(context()).unwrap();
    snapshot.append(1, value).unwrap();
    snapshot.content().unwrap().to_vec()
}
#[test]
fn source_snapshot_survives_fresh_process_with_exact_context_and_bytes() {
    let expected = content();
    if let Some(path) = std::env::var_os("CONDUIT_SOURCE_RECORDING_CHILD") {
        let directory = SnapshotDirectory::prepare(std::fs::File::open(path).unwrap()).unwrap();
        let (placement, reference) =
            fixture::placement("boot/reopened-read", false, expected.len());
        let mut reader = PreparedFileSnapshot::prepare(&directory, &placement, &reference).unwrap();
        let actual = reader.read(&placement.authority[0]).unwrap();
        assert_eq!(actual, expected);
        let recovered = SourceRecordingSnapshot::read(actual, &context()).unwrap();
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered.context(), &context());
        return;
    }
    let root = directory::Root::prepare();
    {
        let directory = root.directory();
        let (placement, reference) = fixture::placement("boot/publish", true, expected.len());
        // The existing bounded catalog accepts this generic JSON generation's
        // reference without changing its codec. Construction is not commitment.
        let catalog = conduit_environment_runtime::recording::PreparedRecordingCatalog::prepare(
            core::slice::from_ref(&reference),
            2,
            4096,
        )
        .unwrap();
        let mut reader =
            conduit_environment_runtime::recording::PreparedRecordingCatalogReader::default();
        reader.read(catalog.content()).unwrap();
        assert_eq!(
            reader.encoded_reference(0).unwrap(),
            reference.encode().unwrap()
        );
        let mut writer = PreparedFileSnapshot::prepare(&directory, &placement, &reference).unwrap();
        let mut wrong = placement.authority[0].clone();
        wrong.boot_id = "boot/foreign".into();
        assert!(writer.publish(&wrong, &expected).is_err());
        // A failed preflight does not create files. Use a fresh preparation for
        // the selected real publication and preserve non-replacement semantics.
        assert_eq!(std::fs::read_dir(&root.path).unwrap().count(), 0);
        let mut writer = PreparedFileSnapshot::prepare(&directory, &placement, &reference).unwrap();
        writer.publish(&placement.authority[0], &expected).unwrap();
        let mut occupied =
            PreparedFileSnapshot::prepare(&directory, &placement, &reference).unwrap();
        assert!(occupied
            .publish(&placement.authority[0], &expected)
            .is_err());
    }
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "source_snapshot_survives_fresh_process_with_exact_context_and_bytes",
            "--nocapture",
        ])
        .env("CONDUIT_SOURCE_RECORDING_CHILD", &root.path)
        .status()
        .unwrap();
    assert!(result.success());
}
