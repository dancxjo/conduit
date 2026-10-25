//! Real policy/Ready-owner CPU check; the pins panic if preparation performs I/O.
use super::{c, native_authority, native_retirement};
#[path = "fixture.rs"]
mod fixture;
use conduit_environment_make::{DisplayOutputSource, PreparedDisplayOutput};
use conduit_environment_runtime::{assigned::ResidentLayout, display_output::*};
use conduit_plan_lowering::selected_assigned_operation::*;
use conduit_tm1637_wire::{installation::ReadyWriteBase, WriteContract};
pub fn check(issuer: &mut native_authority::RootIssuer, sampler: &c::ActivePlayIdentity) {
    let source = DisplayOutputSource::compile().unwrap();
    let mut fixture = fixture::fixture(&source);
    // Rebuild Ready pins/software under the actual test sampler Host/Boot.
    fixture.host.host_id = sampler.host_id.clone();
    fixture.host.boot_id = sampler.boot_id.clone();
    let mut pins = Some(fixture::ready(&sampler.host_id, &sampler.boot_id));
    let contract = WriteContract::prepare().unwrap();
    let grant = issuer
        .grant_display(pins.as_ref().unwrap().native_identity())
        .unwrap();
    let output = PreparedDisplayOutput::prepare(
        &source,
        &fixture.host,
        &["conduit.base/local@1".into(), "fixture/base".into()],
        &[grant],
    )
    .unwrap();
    let fragment = &output.plan().fragments[0];
    let image = conduit_embedded_build::generate_fore_native_plan(
        output.plan(),
        &fragment.fragment_id,
        conduit_embedded_build::EmbeddedImageBounds::HOST_TOOLING,
        conduit_embedded_build::EmbeddedComputationBounds {
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
    let records = c::AssignedPlanRecords::new(&bytes)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let requirements = image.layout_requirements(maxima).unwrap();
    let computations =
        conduit_embedded_build::assigned_computation_requirements(image.kernel()).unwrap();
    let calls = output
        .lowered()
        .host_calls
        .iter()
        .map(|c| c::AssignedIdentity::from_text(c.contract_id.as_str()))
        .collect::<Vec<_>>();
    let resources = output
        .lowered()
        .resources
        .iter()
        .map(|r| r.binding.resource.0)
        .collect::<Vec<_>>();
    let identity = c::PlotIdentity {
        source_document_id: output.plan().source_document_id.clone(),
        checked_plot_id: output.plan().checked_plot_id.clone(),
        expanded_plot_id: output.plan().expanded_plot_id.clone(),
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
        write: conduit_kernel::NodeId(3),
        maximum_value_bytes: 651,
        sign_items: output.lowered().sign_items,
        sign_bytes: output.lowered().sign_bytes,
    };
    let admitted = ValidatedAssignedDisplay::admit(
        AssignedDisplayInput {
            header: &bytes[..c::ASSIGNED_PLAN_HEADER_BYTES],
            records: &records,
            maxima,
        },
        &profile,
        AssignedDisplayScope {
            host: &sampler.host_id,
            boot: &sampler.boot_id,
            generation: c::OfferGeneration(1),
        },
    )
    .unwrap();
    let next = issuer.next_play_sequence().unwrap();
    let active = c::bind_active_play(
        &output.plan().plan_id,
        &sampler.host_id,
        &sampler.boot_id,
        next,
    );
    assert!(issuer
        .reserve_display_play_matching(
            &admitted,
            &active.plan_id,
            &fragment.fragment_id,
            sampler.active_play_id.as_str()
        )
        .is_err());
    assert_eq!(issuer.next_play_sequence(), Some(next));
    let active = issuer
        .reserve_display_play_matching(
            &admitted,
            &active.plan_id,
            &fragment.fragment_id,
            active.active_play_id.as_str(),
        )
        .unwrap();
    assert_eq!(issuer.next_play_sequence(), Some(next + 1));
    let selected = || SelectedAssignedForeOperationPlan {
        operation: SelectedAssignedOperationPlan {
            input: AssignedOperationInput::Records {
                header: &bytes[..c::ASSIGNED_PLAN_HEADER_BYTES],
                records: &records,
            },
            maxima,
            requirements: c::AssignedPlanRequirements {
                host: admitted.identity().host,
                boot: admitted.identity().boot,
                host_calls: &calls,
                resources: &resources,
                remote_bindings: &[],
            },
            computations: profile.computations,
            fragment_id: &fragment.fragment_id,
            active: &active,
            node: profile.write,
        },
        fore: image.fore(),
    };
    let (table, handle, claim) = issuer
        .issue_assigned_display(pins.as_ref().unwrap(), &admitted, selected(), 2, &contract)
        .unwrap();
    let mut bound = Some(Box::new(
        ReadyWriteBase::bind_assigned_fore_retained(
            &mut pins,
            table,
            handle,
            claim,
            &contract,
            selected(),
        )
        .unwrap(),
    ));
    assert!(pins.is_none());
    assert_eq!(
        bound.as_ref().unwrap().inspection().scope.active_play_id,
        active.active_play_id
    );
    // Retirement is exact-scope and independent of the still-owned sampler.
    bound
        .as_mut()
        .unwrap()
        .revoke(profile.write, conduit_kernel::HostCallId(0))
        .unwrap();
    let retired = native_retirement::RetiredDisplay::retire_boxed(&mut bound).unwrap();
    assert!(issuer.finish_display_play(sampler, &retired).is_err());
    issuer.finish_display_play(&active, &retired).unwrap();
    assert!(issuer.finish_display_play(&active, &retired).is_err());
    assert_eq!(issuer.next_play_sequence(), Some(next + 1));
}
