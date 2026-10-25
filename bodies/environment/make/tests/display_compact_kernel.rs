//! CPU proof of exact selection and unchanged output execution; no GPIO claim.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "display_support/fixture.rs"]
mod fixture;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core as c;
use conduit_embedded_build as b;
use conduit_environment_make::{DisplayOutputSource, DisplaySourceMaterial, PreparedDisplayOutput};
use conduit_environment_runtime::{assigned::ResidentLayout, display_output::*};
use conduit_kernel::{self as k, scheduler::SchedulerStatus};
#[test]
fn compact_owner_refuses_foreign_scope_then_executes_the_checked_output() {
    let source = DisplayOutputSource::compile().unwrap();
    let material = DisplaySourceMaterial::prepare(&source).unwrap();
    let fixture = fixture::fixture(&source);
    let mut host = fixture.host.clone();
    let mut software = material
        .prepare_compact(host.host_id.clone(), host.boot_id.clone())
        .unwrap();
    host.capabilities
        .retain(|o| o.kind_id.as_str() == conduit_tm1637_wire::WRITE_KIND);
    let offers = software
        .current_offers(&host.host_id, &host.boot_id, host.offer_generation)
        .unwrap();
    host.capabilities.extend(
        offers
            .iter()
            .map(|json| serde_json::from_slice::<c::CapabilityOffer>(json).unwrap()),
    );
    let plan = PreparedDisplayOutput::prepare(
        &source,
        &host,
        &["conduit.base/local@1".into(), "fixture/base".into()],
        &[fixture.grant],
    )
    .unwrap();
    let fragment = &plan.plan().fragments[0];
    let image = b::generate_fore_native_plan(
        plan.plan(),
        &fragment.fragment_id,
        b::EmbeddedImageBounds::HOST_TOOLING,
        b::EmbeddedComputationBounds {
            maximum_program_bytes: 32768,
            maximum_total_program_bytes: 65536,
        },
        2,
    )
    .unwrap();
    let maxima = c::AssignedPlanMaxima {
        encoded_bytes: 65535,
        runtime_state_bytes: 32768,
        counts: [32; c::ASSIGNED_PLAN_COUNT_KINDS],
    };
    let bytes = image.encode(maxima).unwrap();
    let records: Vec<_> = c::AssignedPlanRecords::new(&bytes)
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let requirements = image.layout_requirements(maxima).unwrap();
    let computations = b::assigned_computation_requirements(image.kernel()).unwrap();
    let calls: Vec<_> = plan
        .lowered()
        .host_calls
        .iter()
        .map(|call| c::AssignedIdentity::from_text(call.contract_id.as_str()))
        .collect();
    let resources: Vec<_> = plan
        .lowered()
        .resources
        .iter()
        .map(|resource| resource.binding.resource.0)
        .collect();
    let identity = c::PlotIdentity {
        source_document_id: plan.plan().source_document_id.clone(),
        checked_plot_id: plan.plan().checked_plot_id.clone(),
        expanded_plot_id: plan.plan().expanded_plot_id.clone(),
    };
    let mut counts = [0; c::ASSIGNED_PLAN_COUNT_KINDS];
    counts.copy_from_slice(&bytes[80..93]);
    let profile = AssignedDisplayProfile {
        source: &identity,
        layout: ResidentLayout {
            records: &requirements,
            counts,
        },
        computations: c::AssignedComputationProfile {
            maximum_text_bytes: u16::MAX,
            maximum_total_text_bytes: u32::MAX,
            requirements: &computations,
        },
        fore: image.fore(),
        host_calls: &calls,
        resources: &resources,
        write: k::NodeId(3),
        maximum_value_bytes: 651,
        sign_items: plan.lowered().sign_items,
        sign_bytes: plan.lowered().sign_bytes,
    };
    let input = AssignedDisplayInput {
        header: &bytes[..c::ASSIGNED_PLAN_HEADER_BYTES],
        records: &records,
        maxima,
    };
    let stores = || {
        let signs = DisplaySignBudget::derive(1, profile.sign_items, profile.sign_bytes).unwrap();
        (
            k::HostedValueStore::new(12, 651, 12 * 651).unwrap(),
            k::HostedSignLog::new_with_remote_storage(
                signs.ordinary_items(),
                signs.ordinary_bytes(),
                signs.remote_items(),
                signs.remote_bytes(),
            )
            .unwrap(),
        )
    };
    let (values, signs) = stores();
    assert!(PreparedAssignedDisplayKernel::prepare(
        input,
        &profile,
        AssignedDisplayScope {
            host: &host.host_id,
            boot: &"stale/boot".into(),
            generation: host.offer_generation
        },
        &mut software,
        1,
        values,
        signs
    )
    .is_err());
    assert_eq!(
        software
            .current_offers(&host.host_id, &host.boot_id, host.offer_generation)
            .unwrap()
            .iter()
            .count(),
        3
    );
    let (values, signs) = stores();
    let mut prepared = PreparedAssignedDisplayKernel::prepare(
        input,
        &profile,
        AssignedDisplayScope {
            host: &host.host_id,
            boot: &host.boot_id,
            generation: host.offer_generation,
        },
        &mut software,
        1,
        values,
        signs,
    )
    .unwrap();
    assert_eq!(
        software
            .current_offers(&host.host_id, &host.boot_id, host.offer_generation)
            .unwrap()
            .iter()
            .count(),
        0
    );
    let mut frame =
        conduit_environment_runtime::display_packets::PreparedDisplayFrame::prepare().unwrap();
    let canonical = frame.encode([6, 91, 79, 102, 0, 0], 3, true).unwrap();
    let endpoint = k::RemoteEndpointId(prepared.input.endpoint());
    let cord = k::CordId(prepared.input.cord());
    let mut packet_encoder =
        conduit_environment_runtime::display_packets::PreparedDisplayPackets::prepare().unwrap();
    let expected = packet_encoder
        .encode_canonical([6, 91, 79, 102, 0, 0], 3, true)
        .unwrap()
        .to_vec();
    let ((), memory) = allocation_probe::observe(|| {
        prepared
            .kernel
            .admit_remote_input(endpoint, cord, 0, canonical)
            .unwrap();
        prepared.kernel.close_remote_input(endpoint, cord).unwrap();
        let mut writes = 0;
        let mut drained = false;
        for _ in 0..128 {
            let status = prepared.kernel.step().unwrap();
            if let Some(request) = prepared.kernel.next_host_request() {
                let bytes = prepared.kernel.host_value(request.input.value).unwrap();
                let output = if request.node == prepared.write {
                    assert_eq!(bytes, expected.as_slice());
                    writes += 1;
                    None
                } else {
                    let selected = prepared.selectors[usize::from(request.node.0)]
                        .as_mut()
                        .unwrap()
                        .select(bytes)
                        .unwrap();
                    selected.map(|bytes| {
                        k::BoundedValueRef::new(
                            prepared.kernel.store_host_value(bytes).unwrap(),
                            1024,
                        )
                        .unwrap()
                    })
                };
                prepared
                    .kernel
                    .complete_host_call(
                        request.node,
                        request.request,
                        k::HostCallOutcome {
                            disposition: k::HostCallDisposition::Completed,
                            output,
                            failure: None,
                        },
                    )
                    .unwrap();
            }
            if status == SchedulerStatus::Drained {
                drained = true;
                break;
            }
        }
        assert!(drained);
        assert_eq!(writes, 1);
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
