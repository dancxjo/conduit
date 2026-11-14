//! CPU fixture: exact Body scope, inert controller and actual generated resident.
use crate::{mixed_preparation::PreparedMixedKernel, native_preparation::NativePossession};
use conduit_core::*;
use conduit_i2c_transaction::{
    contract::*, installation::*, I2cDisposition, I2cProvider, I2cTransaction,
};
use conduit_kernel::*;
use conduit_plan_lowering::assigned_play_scope::AssignedPlayScope;
use conduit_plan_lowering::selected_assigned_operation::*;
#[path = "../../../../../mechanisms/io/i2c-transaction/src/owner/tests/assigned_body_scope.rs"]
mod body_scope;
struct Provider<'a>(&'a core::cell::Cell<usize>, bool);
impl I2cProvider for Provider<'_> {
    fn transact(
        &mut self,
        _: &I2cTransaction<'_>,
        input: &mut [u8],
    ) -> Result<usize, I2cDisposition> {
        if self.1 {
            return Err(I2cDisposition::ProviderLost);
        }
        self.0.set(self.0.get() + 1);
        input.fill(0x60);
        Ok(input.len())
    }
    fn revoke(&mut self) {
        self.1 = true;
    }
}
#[path = "admission.rs"]
mod admission;
pub fn exercise() {
    run(false);
    run(true);
}
fn run(reverse: bool) {
    let outcome = crate::resident::generated::ENTRY_COUNT == 5;
    let (document, program, image) = crate::support::context_for(outcome);
    let (contract, contract_cost) = crate::probe::observe(I2cContract::prepare);
    let contract = contract.unwrap();
    println!("native I2C contract preparation outside kernel backing: {} requested B, {} B CPU peak, {} allocations; must be separately funded before physical installation", contract_cost.requested_bytes, contract_cost.peak_bytes, contract_cost.allocations);
    let mut authority = crate::support::authority::scope();
    authority.capability_id = I2C_IMPLEMENTATION.into();
    authority.implementation_id = I2C_IMPLEMENTATION.into();
    authority.authority_contract_id = I2C_AUTHORITY.into();
    authority.authority_grant_id = format!("{}/i2c/1/authority", authority.boot_id.as_str()).into();
    let plan = crate::support::model(&document, &program, &image, &authority, &contract, reverse);
    let fragment = &plan.fragments[0];
    let context = body_scope::Context::new(&plan);
    let resident_placement = fragment
        .placements
        .iter()
        .find(|p| p.artifact_id == *image.artifact_id())
        .unwrap();
    let native_placement = fragment
        .placements
        .iter()
        .find(|p| p.kind_id.as_str() == I2C_KIND)
        .unwrap();
    let bound = image
        .bind(
            &plan,
            &fragment.fragment_id,
            &resident_placement.placement_id,
        )
        .unwrap();
    let generated = conduit_embedded_build::generate_fore_native_plan(
        &plan,
        &fragment.fragment_id,
        conduit_embedded_build::EmbeddedImageBounds::HOST_TOOLING,
        conduit_embedded_build::EmbeddedComputationBounds {
            maximum_program_bytes: 32768,
            maximum_total_program_bytes: 32768,
        },
        2,
    )
    .unwrap();
    let maxima = AssignedPlanMaxima {
        encoded_bytes: 8192,
        runtime_state_bytes: 16384,
        counts: [32; ASSIGNED_PLAN_COUNT_KINDS],
    };
    let bytes = generated
        .encode_with_resident_computations(maxima, core::slice::from_ref(&bound))
        .unwrap();
    let resources: Vec<_> = generated
        .kernel()
        .resources
        .iter()
        .map(|r| r.resource)
        .collect();
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &plan,
        &fragment.fragment_id,
    )
    .unwrap()
    .0;
    let native_node = lowered
        .nodes
        .iter()
        .find(|n| n.placement_id == native_placement.placement_id)
        .unwrap()
        .node;
    let calls = [AssignedIdentity::from_text(I2C_CALL)];
    let selected = |scope| SelectedAssignedForeOperationPlan {
        operation: SelectedAssignedOperationPlan {
            input: AssignedOperationInput::Contiguous(&bytes),
            maxima,
            requirements: AssignedPlanRequirements {
                host: AssignedIdentity::from_text(authority.host_id.as_str()),
                boot: AssignedIdentity::from_text(authority.boot_id.as_str()),
                host_calls: &calls,
                resources: &resources,
                remote_bindings: &[],
            },
            computations: AssignedComputationProfile {
                maximum_text_bytes: 32768,
                maximum_total_text_bytes: 32768,
                requirements: &[],
            },
            fragment_id: fragment.fragment_id.as_str(),
            scope,
            node: native_node,
        },
        fore: generated.fore(),
    };
    let effects = core::cell::Cell::new(0);
    let mut slot = ready_slot(&authority, &effects);
    let mut library = crate::resident::generated::Initialized::prepare().unwrap();
    let original = library.offers().map(|o| o.unwrap().as_ptr());
    if outcome {
        assert!(
            crate::resident::generated::VALUE_SLOT_BYTES
                > library.available_value_slot_bytes(4).unwrap()
        );
    }

    let issue = || {
        crate::support::authority::authorization_for_play(
            authority.clone(),
            context.plan_id(),
            context.play_id(),
        )
    };
    macro_rules! prepare {
        ($ceiling:expr) => {{
            let (table, handle, claim) = issue();
            PreparedMixedKernel::prepare(
                &mut library,
                NativePossession {
                    slot: &mut slot,
                    table,
                    handle,
                    claim,
                },
                &contract,
                selected(context.scope(fragment)),
                0,
                $ceiling,
            )
        }};
    }
    let plot_active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let (table, handle, claim) = issue();
    let (refused, cost) = crate::probe::observe(|| {
        PreparedMixedKernel::prepare(
            &mut library,
            NativePossession {
                slot: &mut slot,
                table,
                handle,
                claim,
            },
            &contract,
            selected(AssignedPlayScope::Plot(&plot_active)),
            0,
            16384,
        )
    });
    assert!(matches!(
        refused,
        Err(crate::native_preparation::Refusal::Scope)
    ));
    drop(refused);
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert!(slot.is_some());
    assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);
    let (table, handle, mut claim) = issue();
    claim.resource_generation_id = ResourceGenerationId("stale".into());
    assert!(matches!(
        PreparedMixedKernel::prepare(
            &mut library,
            NativePossession {
                slot: &mut slot,
                table,
                handle,
                claim
            },
            &contract,
            selected(context.scope(fragment)),
            0,
            16384
        ),
        Err(crate::native_preparation::Refusal::Native(_))
    ));
    assert!(slot.is_some());
    assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);
    assert!(prepare!(0).is_err());
    assert!(slot.is_some());
    assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);
    let prepared = prepare!(16384).unwrap();
    let charge = prepared.storage_bytes();
    assert!(charge <= 16384);
    drop(prepared);
    assert!(slot.is_some());
    assert_eq!(effects.get(), 0);
    assert!(prepare!(charge - 1).is_err());
    assert!(slot.is_some());
    assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);
    let prepared = prepare!(charge).unwrap();
    let (mut bound, cost) = crate::probe::observe(|| prepared.bind());
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert!(slot.is_none());
    assert_eq!(effects.get(), 0);
    let request_type = contract.decode_request_type().unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = request_type.shape() else {
        panic!("request")
    };
    let ty = |name| {
        fields
            .iter()
            .find(|f| f.name() == name)
            .unwrap()
            .value_type()
            .clone()
    };
    let input = StructuredInfoValue::record(
        request_type.clone(),
        vec![
            StructuredFieldValue::new(
                "address",
                StructuredInfoValue::leaf(ty("address"), vec![if outcome { 0x70 } else { 0x76 }])
                    .unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "read",
                StructuredInfoValue::leaf(ty("read"), vec![u8::from(!outcome)]).unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "write",
                StructuredInfoValue::sequence(
                    ty("write"),
                    if outcome {
                        vec![StructuredInfoValue::leaf(
                            StructuredInfoType::leaf(kind_id("value/u8")).unwrap(),
                            vec![0],
                        )
                        .unwrap()]
                    } else {
                        vec![]
                    },
                )
                .unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let mut encoder =
        conduit_i2c_transaction::result::PreparedI2cResultEncoder::new(&contract).unwrap();
    let expected_native = if outcome {
        encoder.completed(0, &[])
    } else {
        encoder.completed(1, &[0x60])
    }
    .unwrap()
    .to_vec();
    let expected = if outcome {
        program.evaluate(&expected_native).unwrap()
    } else {
        expected_native
    };
    let incoming = generated
        .fore()
        .iter()
        .find(|f| f.direction() == AssignedForeDirection::Input)
        .unwrap();
    let outgoing = generated
        .fore()
        .iter()
        .find(|f| f.direction() == AssignedForeDirection::Output)
        .unwrap();
    let (retired, cost) = crate::probe::observe(|| {
        bound
            .kernel
            .admit_remote_input(
                RemoteEndpointId(incoming.endpoint()),
                CordId(incoming.cord()),
                0,
                &input,
            )
            .unwrap();
        for _ in 0..8 {
            bound.kernel.step().unwrap();
            let _ = bound.dispatch_one().unwrap();
            if bound
                .kernel
                .remote_egress_offer(
                    RemoteEndpointId(outgoing.endpoint()),
                    CordId(outgoing.cord()),
                )
                .unwrap()
                .is_some()
            {
                break;
            }
        }
        assert_eq!(effects.get(), 1);
        let offer = bound
            .kernel
            .remote_egress_offer(
                RemoteEndpointId(outgoing.endpoint()),
                CordId(outgoing.cord()),
            )
            .unwrap()
            .unwrap();
        assert_eq!(bound.kernel.host_value(offer.value).unwrap(), expected);
        bound
            .kernel
            .remote_egress_accept(
                RemoteEndpointId(outgoing.endpoint()),
                CordId(outgoing.cord()),
                offer.sequence,
            )
            .unwrap();
        bound
            .kernel
            .remote_egress_delivered(
                RemoteEndpointId(outgoing.endpoint()),
                CordId(outgoing.cord()),
                offer.sequence,
            )
            .unwrap();
        let retired = bound
            .cancel_and_retire()
            .unwrap_or_else(|_| panic!("mixed kernel not drained"));
        assert!(retired.residents_returned);
        let mut call = Some(retired.call);
        assert!(matches!(
            conduit_i2c_transaction::owner::I2cHostCall::retire_retained(&mut call),
            Err(conduit_i2c_transaction::owner::I2cRetirementRefusal::NotRevoked)
        ));
        call.as_mut()
            .unwrap()
            .revoke(retired.native_node, conduit_kernel::HostCallId(0))
            .unwrap();
        let (provider, attachment) =
            conduit_i2c_transaction::owner::I2cHostCall::retire_retained(&mut call).unwrap();
        assert!(call.is_none());
        assert_eq!(attachment.minimum_address, 0x70);
        assert!(core::ptr::eq(provider.0, &effects));
        assert!(provider.1); // Quarantined custody is not a Ready provider.
        retired.signs
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert!(retired
        .events()
        .any(|e| e.kind == KernelEventKind::RunCancelled));
    assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);
    admission::run(
        &context,
        fragment,
        &contract,
        &authority,
        &image,
        maxima,
        (&input, &expected),
    );
    println!("actual mixed Root kernel: {charge} retained bytes; mixed resident/native Host Call and typed egress; zero binding/execution allocation; standalone CPU execution component; signed-set admission tested separately, no physical Start claim");
}

fn ready_slot<'a>(
    authority: &BaseCapabilityScope,
    effects: &'a core::cell::Cell<usize>,
) -> Option<ReadyI2cBase<Provider<'a>>> {
    // SAFETY: exclusively retained inert CPU provider; no physical controller exists.
    Some(unsafe {
        ReadyI2cBase::new(
            I2cNativeIdentity {
                base_implementation_id: "rp2040/i2c@1".into(),
                host_id: authority.host_id.clone(),
                boot_id: authority.boot_id.clone(),
                base_id: "base/i2c".into(),
                provider_instance_id: authority.base_instance_id.clone(),
                provider_generation: authority.base_provider_generation,
                resource_pool_id: authority.resource_pool_id.clone(),
                resource_generation_id: authority.resource_generation_id.clone(),
                envelope_id: authority.envelope_id.clone(),
                artifact_id: "fixture/image".into(),
            },
            conduit_i2c_transaction::owner::I2cAttachment {
                generation: 7,
                minimum_address: 0x70,
                maximum_address: 0x77,
                resource_bytes: 4096,
            },
            Provider(effects, false),
        )
        .unwrap()
    })
}
