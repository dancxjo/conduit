//! Shared exact assigned output preparation; authority and pins are CPU fixtures.
use super::fixture;
use conduit_core as c;
use conduit_embedded_build as b;
use conduit_environment_make::{DisplayOutputSource, DisplaySourceMaterial, PreparedDisplayOutput};
use conduit_environment_runtime::{assigned::ResidentLayout, display_output::*};
use conduit_kernel as k;
#[allow(
    dead_code,
    reason = "shared fixture; session proof supplies its own finite log"
)]
pub fn prepare_output(
    frames: u32,
    reusable: bool,
) -> PreparedAssignedDisplayKernel<k::HostedValueStore, k::HostedSignLog> {
    prepare_with_signs(frames, reusable, |signs| {
        k::HostedSignLog::new_with_remote_storage(
            signs.ordinary_items(),
            signs.ordinary_bytes(),
            signs.remote_items(),
            signs.remote_bytes(),
        )
        .unwrap()
    })
}
pub fn prepare_with_signs<L: k::SignSink>(
    frames: u32,
    reusable: bool,
    logs: impl Fn(DisplaySignBudget) -> L,
) -> PreparedAssignedDisplayKernel<k::HostedValueStore, L> {
    macro_rules! prepare {
        ($($argument:expr),* $(,)?) => {
            if reusable { PreparedAssignedDisplayKernel::prepare_for_session($($argument),*) }
            else { PreparedAssignedDisplayKernel::prepare($($argument),*) }
        };
    }
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
        sign_items: if reusable {
            64
        } else {
            plan.lowered().sign_items
        },
        sign_bytes: if reusable {
            64 * core::mem::size_of::<k::KernelEvent>() as u32
        } else {
            plan.lowered().sign_bytes
        },
    };
    let input = AssignedDisplayInput {
        header: &bytes[..c::ASSIGNED_PLAN_HEADER_BYTES],
        records: &records,
        maxima,
    };
    let stores = || {
        let signs = DisplaySignBudget::derive(
            if reusable { 1 } else { frames },
            profile.sign_items,
            profile.sign_bytes,
        )
        .unwrap();
        (
            DisplayValueBudget::derive(if reusable { 1 } else { frames }, 651)
                .unwrap()
                .prepare()
                .unwrap(),
            logs(signs),
        )
    };
    let (values, signs) = stores();
    assert!(prepare!(
        input,
        &profile,
        AssignedDisplayScope {
            host: &host.host_id,
            boot: &"stale/boot".into(),
            generation: host.offer_generation
        },
        &mut software,
        frames,
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
    let prepared = prepare!(
        input,
        &profile,
        AssignedDisplayScope {
            host: &host.host_id,
            boot: &host.boot_id,
            generation: host.offer_generation,
        },
        &mut software,
        frames,
        values,
        signs,
    )
    .unwrap();
    assert!(!software.is_ready());

    assert_eq!(
        software
            .current_offers(&host.host_id, &host.boot_id, host.offer_generation)
            .unwrap()
            .iter()
            .count(),
        0
    );
    prepared
}
