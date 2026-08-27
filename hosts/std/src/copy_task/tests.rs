use super::base::ExecutionFaults;
use super::{
    CopyRequestId, CopyResult, CopyStopToken, ProtectedFileAvailability, ProtectedFileRegistry,
};
use crate::{StdHost, StdHostConfig};
use conduit_core::{
    BaseImplementationId, BootId, CapabilityId, GearId, HostId, OfferGeneration,
    ProtectedResourceAccess, ProtectedResourceCommitPolicy, Quantity, QuantityUnit,
    ResourceBindingRoleId, ResourceHandleId, StructuredInfoValueShape,
};
use conduit_planner::{default_placements, plan_with_options, PlanningOptions};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("conduit-copy-{}-{sequence}", std::process::id()));
        std::fs::create_dir(&path).expect("create isolated copy test directory");
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct PlannedCopy {
    host: StdHost,
    fragment: conduit_core::PlanFragment,
    registry: ProtectedFileRegistry,
    source_handle: ResourceHandleId,
    destination_handle: ResourceHandleId,
}

fn planned_copy(
    source: &Path,
    destination: &Path,
    maximum_bytes: u64,
    destination_access: ProtectedResourceAccess,
    destination_policy: ProtectedResourceCommitPolicy,
    source_availability: ProtectedFileAvailability,
) -> PlannedCopy {
    let host = StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from("copy-host"),
        boot_id: BootId::from("copy-boot"),
        offer_generation: OfferGeneration(1),
    });
    let mut catalog = conduit_form::ProfileCatalog::new();
    conduit_std_catalog::install_copy_file_catalog(&mut catalog).expect("install copy catalog");
    let form = conduit_form::parse(
        "form copy-task {\n    task: file/copy\n    show: presentation/structured-info\n    task > show\n}\n",
        &catalog,
    )
    .expect("copy Form checks without resource paths");
    let placements = default_placements(&form, std::slice::from_ref(host.advertisement()))
        .expect("copy placement resolves by equal checked face");
    let source_handle = ResourceHandleId::from("handle/source");
    let destination_handle = ResourceHandleId::from("handle/destination");
    let mut registry = ProtectedFileRegistry::default();
    let source_grant = registry
        .register(
            source_handle.clone(),
            source,
            GearId::from("copy-task/task"),
            ResourceBindingRoleId::from(conduit_std_catalog::COPY_SOURCE_ROLE),
            host.advertisement().host_id.clone(),
            host.advertisement().boot_id.clone(),
            CapabilityId::from(conduit_std_catalog::COPY_FILE_CAPABILITY),
            ProtectedResourceAccess::ReadExisting,
            maximum_bytes,
            ProtectedResourceCommitPolicy::NotApplicable,
            source_availability,
        )
        .expect("register source choice");
    let destination_grant = registry
        .register(
            destination_handle.clone(),
            destination,
            GearId::from("copy-task/task"),
            ResourceBindingRoleId::from(conduit_std_catalog::COPY_DESTINATION_ROLE),
            host.advertisement().host_id.clone(),
            host.advertisement().boot_id.clone(),
            CapabilityId::from(conduit_std_catalog::COPY_FILE_CAPABILITY),
            destination_access,
            maximum_bytes,
            destination_policy,
            ProtectedFileAvailability::Available,
        )
        .expect("register destination choice");
    let overrides = BTreeMap::new();
    let plan = plan_with_options(
        &form,
        std::slice::from_ref(host.advertisement()),
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &overrides,
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[source_grant, destination_grant],
            line_offers: &[],
        },
    )
    .expect("protected choices seal into copy Plan");
    assert!(!format!("{form:?}").contains(source.to_string_lossy().as_ref()));
    assert!(!format!("{plan:?}").contains(source.to_string_lossy().as_ref()));
    PlannedCopy {
        host,
        fragment: plan.fragments.into_iter().next().expect("local fragment"),
        registry,
        source_handle,
        destination_handle,
    }
}

#[test]
fn create_and_replace_copy_through_bounded_kernel_steps_with_exact_receipt() {
    let directory = TestDirectory::new();
    let source = directory.path("source.bin");
    let destination = directory.path("destination.bin");
    let bytes = vec![0x5a; conduit_std_catalog::COPY_CHUNK_BYTES as usize + 17];
    std::fs::write(&source, &bytes).expect("write source fixture");
    let mut copy = planned_copy(
        &source,
        &destination,
        bytes.len() as u64,
        ProtectedResourceAccess::Create,
        ProtectedResourceCommitPolicy::CreateOnly,
        ProtectedFileAvailability::Available,
    );
    let plan_id = copy.fragment.plan_id.clone();
    let play = copy.host.issue_kernel_play(&copy.fragment).unwrap();
    let receipt = copy
        .host
        .run_copy_fragment(
            play,
            CopyRequestId::new("request/create").unwrap(),
            copy.fragment,
            &mut copy.registry,
            &CopyStopToken::default(),
        )
        .expect("copy run is structurally valid");
    assert_eq!(
        receipt.result,
        CopyResult::Success {
            bytes_copied: bytes.len() as u64
        }
    );
    assert_eq!(receipt.plan_id, plan_id);
    assert_eq!(receipt.source_binding_id, copy.source_handle);
    assert_eq!(receipt.destination_binding_id, copy.destination_handle);
    assert!(receipt.kernel_events > 0);
    assert_success_presentation(&receipt, bytes.len() as u64);
    assert_eq!(std::fs::read(&destination).unwrap(), bytes);

    let replacement = vec![0x33; 19];
    std::fs::write(&source, &replacement).unwrap();
    let mut replace = planned_copy(
        &source,
        &destination,
        replacement.len() as u64,
        ProtectedResourceAccess::Replace,
        ProtectedResourceCommitPolicy::ReplaceExisting,
        ProtectedFileAvailability::Available,
    );
    let play = replace.host.issue_kernel_play(&replace.fragment).unwrap();
    let receipt = replace
        .host
        .run_copy_fragment(
            play,
            CopyRequestId::new("request/replace").unwrap(),
            replace.fragment,
            &mut replace.registry,
            &CopyStopToken::default(),
        )
        .unwrap();
    assert!(matches!(receipt.result, CopyResult::Success { .. }));
    assert_success_presentation(&receipt, replacement.len() as u64);
    assert_eq!(std::fs::read(&destination).unwrap(), replacement);
}

fn assert_success_presentation(receipt: &super::CopyRunReceipt, expected_bytes: u64) {
    let presented = receipt
        .presented_result
        .as_ref()
        .expect("successful copy reaches the planned presentation Gear");
    assert_eq!(
        presented.value_type(),
        &conduit_std_catalog::copy_result_type()
    );
    let StructuredInfoValueShape::Record(fields) = presented.shape() else {
        panic!("copy result is a record");
    };
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name(), "outcome");
    let StructuredInfoValueShape::Variant { tag, payload } = fields[0].value().shape() else {
        panic!("copy outcome is a variant");
    };
    assert_eq!(tag, "success");
    let StructuredInfoValueShape::Leaf(encoded) = payload.shape() else {
        panic!("successful copy carries a quantity");
    };
    let quantity = Quantity::decode(encoded).expect("presented byte quantity is canonical");
    assert_eq!(quantity.value(), i64::try_from(expected_bytes).unwrap());
    assert_eq!(quantity.unit(), QuantityUnit::Byte);
}

#[test]
fn stale_denied_oversized_and_destination_exists_remain_distinct() {
    let directory = TestDirectory::new();
    let source = directory.path("source.bin");
    let destination = directory.path("destination.bin");
    std::fs::write(&source, vec![1_u8; 32]).unwrap();

    let mut stale = planned_copy(
        &source,
        &destination,
        32,
        ProtectedResourceAccess::Create,
        ProtectedResourceCommitPolicy::CreateOnly,
        ProtectedFileAvailability::Available,
    );
    stale.registry.revoke(&stale.source_handle);
    assert_eq!(
        run(&mut stale, ExecutionFaults::default()),
        CopyResult::StaleHandle
    );

    let mut denied = planned_copy(
        &source,
        &destination,
        32,
        ProtectedResourceAccess::Create,
        ProtectedResourceCommitPolicy::CreateOnly,
        ProtectedFileAvailability::Denied,
    );
    assert_eq!(
        run(&mut denied, ExecutionFaults::default()),
        CopyResult::Denied
    );

    let mut oversized = planned_copy(
        &source,
        &destination,
        16,
        ProtectedResourceAccess::Create,
        ProtectedResourceCommitPolicy::CreateOnly,
        ProtectedFileAvailability::Available,
    );
    assert!(matches!(
        run(&mut oversized, ExecutionFaults::default()),
        CopyResult::Oversized { .. }
    ));

    std::fs::write(&destination, b"keep").unwrap();
    let mut exists = planned_copy(
        &source,
        &destination,
        32,
        ProtectedResourceAccess::Create,
        ProtectedResourceCommitPolicy::CreateOnly,
        ProtectedFileAvailability::Available,
    );
    assert_eq!(
        run(&mut exists, ExecutionFaults::default()),
        CopyResult::DestinationExists
    );
    assert_eq!(std::fs::read(&destination).unwrap(), b"keep");
}

#[test]
fn grant_change_after_prepare_refuses_at_use_without_mutating_the_plan() {
    let directory = TestDirectory::new();
    let source = directory.path("source.bin");
    let destination = directory.path("destination.bin");
    std::fs::write(&source, b"protected").unwrap();
    let mut copy = planned_copy(
        &source,
        &destination,
        9,
        ProtectedResourceAccess::Create,
        ProtectedResourceCommitPolicy::CreateOnly,
        ProtectedFileAvailability::Available,
    );
    let immutable_fragment = copy.fragment.clone();
    let destination_handle = copy.destination_handle.clone();
    let play = copy.host.issue_kernel_play(&copy.fragment).unwrap();
    let receipt = copy
        .host
        .run_copy_fragment_with_use_hook(
            super::executor::CopyRunContext {
                play,
                request_id: CopyRequestId::new("request/revoke-at-use").unwrap(),
                fragment: copy.fragment.clone(),
                registry: &mut copy.registry,
                stop: &CopyStopToken::default(),
                faults: ExecutionFaults::default(),
            },
            |registry| {
                registry
                    .set_availability(&destination_handle, ProtectedFileAvailability::Denied)
                    .unwrap();
            },
        )
        .unwrap();
    assert_eq!(receipt.result, CopyResult::Denied);
    assert_eq!(copy.fragment, immutable_fragment);
    assert!(!destination.exists());

    copy.registry
        .set_availability(
            &copy.destination_handle,
            ProtectedFileAvailability::Available,
        )
        .unwrap();
    assert_eq!(
        run(&mut copy, ExecutionFaults::default()),
        CopyResult::Success { bytes_copied: 9 }
    );
}

#[test]
fn revoked_handle_identity_cannot_be_reissued_to_revive_an_old_plan() {
    let directory = TestDirectory::new();
    let source = directory.path("source.bin");
    let destination = directory.path("destination.bin");
    std::fs::write(&source, b"protected").unwrap();
    let mut copy = planned_copy(
        &source,
        &destination,
        9,
        ProtectedResourceAccess::Create,
        ProtectedResourceCommitPolicy::CreateOnly,
        ProtectedFileAvailability::Available,
    );
    copy.registry.revoke(&copy.source_handle);
    let placement = &copy.fragment.placements[0];
    let reissue = copy.registry.register(
        copy.source_handle.clone(),
        &source,
        placement.gear_id.clone(),
        ResourceBindingRoleId::from(conduit_std_catalog::COPY_SOURCE_ROLE),
        placement.host_id.clone(),
        placement.boot_id.clone(),
        placement.capability_id.clone(),
        ProtectedResourceAccess::ReadExisting,
        9,
        ProtectedResourceCommitPolicy::NotApplicable,
        ProtectedFileAvailability::Available,
    );
    assert!(reissue.is_err());
    assert_eq!(
        run(&mut copy, ExecutionFaults::default()),
        CopyResult::StaleHandle
    );
}

#[test]
fn partial_cancellation_and_cleanup_failure_are_distinct_and_never_commit() {
    let directory = TestDirectory::new();
    let source = directory.path("source.bin");
    let bytes = vec![9_u8; conduit_std_catalog::COPY_CHUNK_BYTES as usize * 2];
    std::fs::write(&source, &bytes).unwrap();

    for (name, faults, expected) in [
        (
            "partial.bin",
            ExecutionFaults {
                fail_after_bytes: Some(u64::from(conduit_std_catalog::COPY_CHUNK_BYTES)),
                ..ExecutionFaults::default()
            },
            CopyResult::Partial {
                bytes_copied: u64::from(conduit_std_catalog::COPY_CHUNK_BYTES),
            },
        ),
        (
            "cancelled.bin",
            ExecutionFaults {
                stop_after_bytes: Some(u64::from(conduit_std_catalog::COPY_CHUNK_BYTES)),
                ..ExecutionFaults::default()
            },
            CopyResult::Cancelled {
                bytes_copied: u64::from(conduit_std_catalog::COPY_CHUNK_BYTES),
            },
        ),
        (
            "cleanup.bin",
            ExecutionFaults {
                stop_after_bytes: Some(u64::from(conduit_std_catalog::COPY_CHUNK_BYTES)),
                cleanup_failure: true,
                ..ExecutionFaults::default()
            },
            CopyResult::CleanupFailed {
                bytes_copied: u64::from(conduit_std_catalog::COPY_CHUNK_BYTES),
            },
        ),
    ] {
        let destination = directory.path(name);
        let mut copy = planned_copy(
            &source,
            &destination,
            bytes.len() as u64,
            ProtectedResourceAccess::Create,
            ProtectedResourceCommitPolicy::CreateOnly,
            ProtectedFileAvailability::Available,
        );
        assert_eq!(run(&mut copy, faults), expected);
        assert!(!destination.exists());
    }
}

#[test]
fn public_stop_token_cancels_the_kernel_before_any_chunk_is_copied() {
    let directory = TestDirectory::new();
    let source = directory.path("source.bin");
    let destination = directory.path("destination.bin");
    std::fs::write(&source, vec![7_u8; 32]).unwrap();
    let mut copy = planned_copy(
        &source,
        &destination,
        32,
        ProtectedResourceAccess::Create,
        ProtectedResourceCommitPolicy::CreateOnly,
        ProtectedFileAvailability::Available,
    );
    let stop = CopyStopToken::default();
    stop.request_stop();
    let play = copy.host.issue_kernel_play(&copy.fragment).unwrap();
    let receipt = copy
        .host
        .run_copy_fragment(
            play,
            CopyRequestId::new("request/stop").unwrap(),
            copy.fragment,
            &mut copy.registry,
            &stop,
        )
        .unwrap();
    assert_eq!(receipt.result, CopyResult::Cancelled { bytes_copied: 0 });
    assert!(receipt.kernel_events > 0);
    assert!(!destination.exists());
}

#[test]
fn issued_play_refuses_a_different_immutable_plan_before_effects() {
    let directory = TestDirectory::new();
    let source = directory.path("source.bin");
    let destination = directory.path("destination.bin");
    std::fs::write(&source, b"protected").unwrap();
    let mut copy = planned_copy(
        &source,
        &destination,
        9,
        ProtectedResourceAccess::Create,
        ProtectedResourceCommitPolicy::CreateOnly,
        ProtectedFileAvailability::Available,
    );
    let play = copy.host.issue_kernel_play(&copy.fragment).unwrap();
    copy.fragment.plan_id = conduit_core::PlanId::from("plan/replaced-after-issue");
    let error = copy
        .host
        .run_copy_fragment(
            play,
            CopyRequestId::new("request/stale-plan").unwrap(),
            copy.fragment,
            &mut copy.registry,
            &CopyStopToken::default(),
        )
        .unwrap_err();
    assert_eq!(
        error,
        "copy Play identity does not match its immutable Plan fragment"
    );
    assert!(!destination.exists());
}

fn run(copy: &mut PlannedCopy, faults: ExecutionFaults) -> CopyResult {
    let play = copy.host.issue_kernel_play(&copy.fragment).unwrap();
    copy.host
        .run_copy_fragment_with_faults(
            play,
            CopyRequestId::new("request/negative").unwrap(),
            copy.fragment.clone(),
            &mut copy.registry,
            &CopyStopToken::default(),
            faults,
        )
        .expect("negative result remains a receipt, not structural failure")
        .result
}
