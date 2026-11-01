//! CPU-only actual installed History admission, not NAT/Body/recorder acceptance.
#![cfg(feature = "history-source")]
use conduit_environment_host::installed_history_source as adapter;
#[allow(dead_code)]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_observed_wire.rs"]
mod signed_wire;
use conduit_core::*;
use conduit_environment_runtime::history_exchange::{self as h, installation::*, *};
use conduit_planner::*;
use conduit_plot::*;
use conduit_std_host::*;
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
struct Archive {
    reported: usize,
    raw: Vec<u8>,
    calls: Arc<AtomicUsize>,
}
impl HistoryProvider for Archive {
    fn retained_layout_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.raw.capacity()
            + 2 * std::mem::size_of::<usize>()
            + std::mem::size_of::<AtomicUsize>()
    }

    fn read_signed_batch(
        &mut self,
        cursor: u32,
        out: &mut [u8; 2048],
    ) -> Result<usize, HistoryProviderRefusal> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        if cursor != 7 {
            return Err(HistoryProviderRefusal::InvalidCursor);
        }
        out[..self.raw.len()].copy_from_slice(&self.raw);
        Ok(self.reported)
    }
    fn revoke(&mut self) {}
}
fn host() -> StdHost {
    StdHost::new_with_config(StdHostConfig {
        host_id: "cpu/history-owner".into(),
        boot_id: "cpu/history-owner/current-boot".into(),
        offer_generation: OfferGeneration(1),
    })
}
fn ready_owner(
    host: &StdHost,
    calls: Arc<AtomicUsize>,
) -> (ReadyHistoryBase<Archive>, AuthorityGrant, Vec<u8>) {
    ready_owner_with_reported(host, calls, None)
}
fn ready_owner_with_reported(
    host: &StdHost,
    calls: Arc<AtomicUsize>,
    reported: Option<usize>,
) -> (ReadyHistoryBase<Archive>, AuthorityGrant, Vec<u8>) {
    let raw = original_signed_batch();
    let identity = HistoryNativeIdentity {
        host_id: host.advertisement().host_id.clone(),
        boot_id: host.advertisement().boot_id.clone(),
        base_id: "cpu/history-base".into(),
        provider_instance_id: "cpu/actual-retained-archive".into(),
        provider_generation: 1,
        resource_pool_id: "cpu/retained-signed-evidence".into(),
        resource_generation_id: ResourceGenerationId("cpu/archive-generation1".into()),
        envelope_id: "cpu/actual-history-envelope".into(),
        artifact_id: "cpu/retained-history-code@1".into(),
    };
    let grant = AuthorityGrant {
        grant_id: "cpu/independent-history-grant".into(),
        contract_id: HISTORY_AUTHORITY.into(),
        host_call_contract_id: h::contract::HISTORY_CALL.into(),
        subject_kind: kind_id(h::contract::HISTORY_KIND),
        host_id: identity.host_id.clone(),
        boot_id: identity.boot_id.clone(),
        capability_id: HISTORY_IMPLEMENTATION.into(),
    };
    // SAFETY: Actual exclusively owned, already retained CPU archive; no IO or
    // per-read allocation. This test Host is initialized above, not a peer DTO.
    let archive = Archive {
        reported: reported.unwrap_or(raw.len()),
        raw: raw.clone(),
        calls,
    };
    let resource_bytes = h::minimum_attachment_bytes(&archive).unwrap();
    let base = unsafe {
        ReadyHistoryBase::new(
            identity,
            HistoryAttachment {
                generation: 1,
                resource_bytes,
            },
            archive,
        )
    }
    .unwrap_or_else(|e| panic!("{e:?}"));
    (base, grant, raw)
}
fn prepare(
    host: &StdHost,
    calls: Arc<AtomicUsize>,
) -> (
    NativeSourceDefinition,
    Box<dyn ImmediateNativeSource>,
    Vec<u8>,
) {
    let (base, grant, raw) = ready_owner(host, calls);
    let (definition, provider) = unsafe { adapter::PreparedHistorySource::prepare(base, grant) }
        .unwrap_or_else(|(_, e)| panic!("{e}"));
    (definition, provider, raw)
}
fn plan(host: &StdHost, grant: AuthorityGrant) -> Plan {
    let contract = HistoryReadContract::prepare().unwrap();
    let (_, profile) = contract.catalogs();
    let checked = contract.checked_document().unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, "history-read", &profile).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements = default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "request"),
        (PortDirection::Output, "result"),
    ]
    .into_iter()
    .map(|(direction, name)| {
        (
            ForeBoundaryKey {
                direction,
                front_port_id: name.into(),
                track: ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 2304,
            },
        )
    })
    .collect();
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        PlanningOptions {
            connection_bases: &empty,
            line_candidates: &lines,
            connection_item_capacity: 1,
            connection_byte_capacity: 2304,
            authority_grants: &[grant],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap()
}
struct Capture {
    raw: Vec<u8>,
    values: usize,
}
impl ExternalForeOutputAdapter for Capture {
    fn deliver(&mut self, _: ExternalForeDelivery) -> Result<(), String> {
        panic!("requires committed kernel witness")
    }
    fn deliver_committed(
        &mut self,
        w: CommittedForeValue<'_>,
        _: ExternalForeDelivery,
    ) -> Result<(), String> {
        assert_eq!(
            h::codec::decode_result(&HistoryReadContract::prepare().unwrap(), w.bytes()).unwrap(),
            self.raw
        );
        assert_eq!(w.scope().play.host_id.as_str(), "cpu/history-owner");
        assert_eq!(w.sequence(), 0);
        self.values += 1;
        Ok(())
    }
}
fn run(
    host: &mut StdHost,
    plan: &Plan,
    input: Vec<u8>,
    capture: &mut Capture,
) -> Result<(), String> {
    host.run_external_plot_to(
        plan.fragments[0].clone(),
        &[ExternalForeInput {
            front_port_id: "request".into(),
            track: ConnectionTrack::Payload,
            bytes: input,
        }],
        capture,
        &mut Vec::new(),
        &mut ThreadTimer,
    )
    .map(|_| ())
}
#[test]
fn actual_ready_history_owner_uses_std_sole_issuer_checked_source_and_committed_cord() {
    let mut host = host();
    let calls = Arc::new(AtomicUsize::new(0));
    let (base, grant, raw) = ready_owner(&host, calls.clone());
    let (_, attachment, entry) = base.native_registration(&HistoryReadContract::prepare().unwrap());
    assert_ne!(
        entry.implementation_id.as_str(),
        entry.capabilities[0]
            .implementation
            .implementation_id
            .as_str()
    );
    let call = &entry.capabilities[0].host_calls[0];
    assert_eq!(
        (
            call.maximum_input_bytes,
            call.maximum_output_bytes,
            attachment.resource_bytes
        ),
        (
            2304,
            2304,
            2048 + std::mem::size_of::<Archive>() as u64
                + raw.len() as u64
                + (2 * std::mem::size_of::<usize>() + std::mem::size_of::<AtomicUsize>()) as u64
        )
    );
    unsafe { adapter::install_cpu_history_source(&mut host, base, grant.clone()) }
        .unwrap_or_else(|_| panic!("actual History admission refused"));
    let plan = plan(&host, grant);
    let mut capture = Capture { raw, values: 0 };
    run(
        &mut host,
        &plan,
        h::codec::encode_request(&HistoryReadContract::prepare().unwrap(), 7).unwrap(),
        &mut capture,
    )
    .unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_eq!(capture.values, 1);
}
#[test]
fn history_cannot_replace_independent_grant_boot_artifact_resource_or_base_binding() {
    let mut selected_host = host();
    let (d, p, _) = prepare(&selected_host, Arc::new(AtomicUsize::new(0)));
    let grant = d.grant.clone();
    unsafe { selected_host.install_immediate_native_source(d, p) }
        .unwrap_or_else(|(_, _, e)| panic!("{e}"));
    let selected = plan(&selected_host, grant);
    for change in 0..5 {
        let mut current = host();
        let calls = Arc::new(AtomicUsize::new(0));
        let (mut d, p, raw) = prepare(&current, calls.clone());
        match change {
            0 => d.grant.grant_id = "cpu/not-selected-grant".into(),
            1 => d.base.capabilities[0].implementation.artifact_id = "cpu/other-artifact".into(),
            2 => {
                d.grant.boot_id = "cpu/stale-boot".into();
            }
            3 => d.base.resources[0].pool_id = "cpu/other-pool".into(),
            _ => d.base.implementation_id = "cpu/other-base-implementation".into(),
        }
        let installed = unsafe { current.install_immediate_native_source(d, p) };
        if change == 2 {
            assert!(installed.is_err());
            continue;
        }
        installed.unwrap_or_else(|(_, _, e)| panic!("{e}"));
        let mut capture = Capture { raw, values: 0 };
        assert!(
            run(
                &mut current,
                &selected,
                h::codec::encode_request(&HistoryReadContract::prepare().unwrap(), 7).unwrap(),
                &mut capture
            )
            .is_err(),
            "case{change}"
        );
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        assert_eq!(capture.values, 0);
    }
}
#[test]
fn malformed_canonical_history_request_never_invokes_native_owner_or_commits() {
    let mut host = host();
    let calls = Arc::new(AtomicUsize::new(0));
    let (d, p, raw) = prepare(&host, calls.clone());
    let grant = d.grant.clone();
    unsafe { host.install_immediate_native_source(d, p) }.unwrap_or_else(|(_, _, e)| panic!("{e}"));
    let selected = plan(&host, grant);
    let mut capture = Capture { raw, values: 0 };
    assert!(run(&mut host, &selected, vec![1, 2, 3], &mut capture).is_err());
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    assert_eq!(capture.values, 0);
}

fn original_signed_batch() -> Vec<u8> {
    use conduit_environment_runtime::{
        observed_record::ObservedRecord, recording::RecordingDisposition,
    };
    use ed25519_dalek::{Signer, SigningKey, Verifier};
    use sha2::{Digest, Sha256};
    let key = SigningKey::from_bytes(&[37; 32]);
    let mut scope = [0; 96];
    for (out, original) in scope.as_chunks_mut::<32>().0.iter_mut().zip([
        "original-fixture-Body",
        "original-fixture-Boot",
        "original-fixture-sampler-Play",
    ]) {
        out.copy_from_slice(&Sha256::digest(original.as_bytes()));
    }
    let contract =
        conduit_environment_runtime::observation::ObservationContract::prepare().unwrap();
    let forms: [u8; 32] = Sha256::digest(
        conduit_environment_runtime::observed_record::descriptor(&contract).unwrap(),
    )
    .into();
    let records = ObservedRecord {
        sequence: 8,
        observed_at_boot_ms: 1234,
        disposition: RecordingDisposition::ProviderFailed,
        quantities: None,
    }
    .encode()
    .unwrap();
    let transcript = signed_wire::transcript(&scope, &forms, 7, 8, &records);
    let signature = key.sign(&transcript);
    key.verifying_key().verify(&transcript, &signature).unwrap();
    let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    serde_json::to_vec(&serde_json::json!({"profile":"observed-history@1","scope":hex(&scope),"forms":hex(&forms),"after":7,"committed":8,"records":hex(&records),"signature":hex(&signature.to_bytes())})).unwrap()
}

#[test]
fn production_installation_refusal_returns_owned_archive_without_replacing_live_base() {
    let mut host = host();
    let first = Arc::new(AtomicUsize::new(0));
    let (base, grant, raw) = ready_owner(&host, first.clone());
    unsafe { adapter::install_cpu_history_source(&mut host, base, grant.clone()) }
        .unwrap_or_else(|_| panic!("first owner refused"));
    let current_offer = host.advertisement().clone();
    let second = Arc::new(AtomicUsize::new(0));
    let (base, other_grant, _) = ready_owner(&host, second.clone());
    let refused = unsafe { adapter::install_cpu_history_source(&mut host, base, other_grant) };
    match refused {
        Err(adapter::HistoryInstallationRefusal::Host(definition, owner, _)) => {
            assert_eq!(
                definition.base.provider_instance_id.as_str(),
                "cpu/actual-retained-archive"
            );
            assert_eq!(
                definition.envelope_id.as_str(),
                "cpu/actual-history-envelope"
            );
            // Keep the returned affine owner alive throughout the first Play.
            assert_eq!(host.advertisement(), &current_offer);
            let selected = plan(&host, grant);
            let mut capture = Capture { raw, values: 0 };
            run(
                &mut host,
                &selected,
                h::codec::encode_request(&HistoryReadContract::prepare().unwrap(), 7).unwrap(),
                &mut capture,
            )
            .unwrap();
            assert_eq!(first.load(Ordering::Relaxed), 1);
            assert_eq!(second.load(Ordering::Relaxed), 0);
            drop(owner);
        }
        _ => panic!("refusal must return original native archive"),
    }
}

#[test]
fn oversized_native_history_result_never_commits_and_actual_child_reservation_retires() {
    let mut host = host();
    let calls = Arc::new(AtomicUsize::new(0));
    let (base, grant, raw) = ready_owner_with_reported(&host, calls.clone(), Some(2049));
    unsafe { adapter::install_cpu_history_source(&mut host, base, grant.clone()) }
        .unwrap_or_else(|_| panic!("owner refused"));
    let selected = plan(&host, grant);
    let mut capture = Capture { raw, values: 0 };
    for count in 1..=2 {
        assert!(run(
            &mut host,
            &selected,
            h::codec::encode_request(&HistoryReadContract::prepare().unwrap(), 7).unwrap(),
            &mut capture
        )
        .is_err());
        assert_eq!(calls.load(Ordering::Relaxed), count);
        assert_eq!(capture.values, 0);
    }
}

// Instrument only the selected native callback, never the whole Std run or
// preparation/Fore delivery. Const thread-local state cannot allocate itself.
thread_local! {
    static COUNT_READ_ALLOCATIONS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static READ_ALLOCATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
struct ReadAllocationProbe;
#[global_allocator]
static ALLOCATOR: ReadAllocationProbe = ReadAllocationProbe;
fn count_read_allocation() {
    let _ = COUNT_READ_ALLOCATIONS.try_with(|enabled| {
        if enabled.get() {
            READ_ALLOCATIONS.with(|count| count.set(count.get() + 1));
        }
    });
}
// SAFETY: All allocation/deallocation requests forward unchanged to System.
// The probe only observes const thread-local scalar state.
unsafe impl std::alloc::GlobalAlloc for ReadAllocationProbe {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        count_read_allocation();
        unsafe { std::alloc::System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: std::alloc::Layout) -> *mut u8 {
        count_read_allocation();
        unsafe { std::alloc::System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: std::alloc::Layout, size: usize) -> *mut u8 {
        count_read_allocation();
        unsafe { std::alloc::System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        unsafe { std::alloc::System.dealloc(ptr, layout) }
    }
}
struct CheckedRead {
    owner: Box<dyn ImmediateNativeSource>,
    allocations: Arc<AtomicUsize>,
    reads: Arc<AtomicUsize>,
}
// SAFETY: The wrapped actual owner has already been prepared under its native
// CPU contract. Only the installed Std issuer invokes this counting wrapper;
// it neither issues authority nor substitutes another provider.
unsafe impl ImmediateNativeSource for CheckedRead {
    fn read(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize, String> {
        READ_ALLOCATIONS.with(|count| count.set(0));
        COUNT_READ_ALLOCATIONS.with(|enabled| enabled.set(true));
        let result = self.owner.read(input, output);
        COUNT_READ_ALLOCATIONS.with(|enabled| enabled.set(false));
        self.allocations.fetch_add(
            READ_ALLOCATIONS.with(|count| count.get()),
            Ordering::Relaxed,
        );
        self.reads.fetch_add(1, Ordering::Relaxed);
        result
    }
}
#[test]
fn actual_selected_history_callback_and_encoder_allocate_nothing_on_success_or_refusal() {
    for cursor in [7, 8] {
        let mut host = host();
        let calls = Arc::new(AtomicUsize::new(0));
        let (definition, owner, raw) = prepare(&host, calls.clone());
        let grant = definition.grant.clone();
        let allocations = Arc::new(AtomicUsize::new(0));
        let reads = Arc::new(AtomicUsize::new(0));
        let owner = Box::new(CheckedRead {
            owner,
            allocations: allocations.clone(),
            reads: reads.clone(),
        });
        unsafe { host.install_immediate_native_source(definition, owner) }
            .unwrap_or_else(|(_, _, e)| panic!("{e}"));
        let selected = plan(&host, grant);
        let mut capture = Capture { raw, values: 0 };
        let result = run(
            &mut host,
            &selected,
            h::codec::encode_request(&HistoryReadContract::prepare().unwrap(), cursor).unwrap(),
            &mut capture,
        );
        assert_eq!(result.is_ok(), cursor == 7);
        assert_eq!(reads.load(Ordering::Relaxed), 1);
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert_eq!(allocations.load(Ordering::Relaxed), 0);
        assert_eq!(capture.values, usize::from(cursor == 7));
    }
}
