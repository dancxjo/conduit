//! Fake pins and Root key: exact native checks, not physical readiness evidence.
use conduit_core as c;
use conduit_environment_runtime::{display_mask::PreparedDisplayMask, mask_renderer::*};
use conduit_kernel::NodeId;
use conduit_seven_segment_mask::{DigitWiring, SevenSegmentLook, Tm1637Profile, WireBit};
use std::{cell::Cell, rc::Rc};
#[derive(Default)]
pub struct Pins {
    pub edges: Cell<usize>,
    pub nack: Cell<bool>,
    pub released: Cell<[bool; 2]>,
}
pub struct Provider(pub Rc<Pins>);
impl conduit_tm1637_wire::OpenDrain for Provider {
    type Error = ();
    fn clock(&mut self, released: bool) -> Result<(), ()> {
        self.0.edges.set(self.0.edges.get() + 1);
        let mut pins = self.0.released.get();
        pins[0] = released;
        self.0.released.set(pins);
        Ok(())
    }
    fn data(&mut self, released: bool) -> Result<(), ()> {
        self.0.edges.set(self.0.edges.get() + 1);
        let mut pins = self.0.released.get();
        pins[1] = released;
        self.0.released.set(pins);
        Ok(())
    }
    fn read_data(&mut self) -> Result<bool, ()> {
        Ok(self.0.nack.get())
    }
    fn half_period(&mut self) -> Result<(), ()> {
        Ok(())
    }
}
pub fn ready(pins: Rc<Pins>) -> ReadyRenderer<Provider> {
    ready_with_resource_bytes(pins, 4096)
}
pub fn ready_with_resource_bytes(pins: Rc<Pins>, resource_bytes: u64) -> ReadyRenderer<Provider> {
    let identity = RendererIdentity {
        host_id: "fixture/mask-host".into(),
        boot_id: "fixture/mask-boot".into(),
        base_id: "base/fixture/display".into(),
        provider_instance_id: "instance/fixture/display".into(),
        provider_generation: 1,
        resource_pool_id: "pool/fixture/display".into(),
        resource_generation_id: c::ResourceGenerationId("generation/fixture/display".into()),
        envelope_id: "envelope/fixture/display".into(),
        artifact_id: "fixture/renderer@1".into(),
        implementation_id: "fixture/renderer@1".into(),
        base_implementation_id: "fixture/display-base@1".into(),
    };
    let mask = PreparedDisplayMask::prepare(
        Tm1637Profile {
            digits: core::array::from_fn(|i| DigitWiring {
                address: i as u8,
                segment_bits: [0, 1, 2, 3, 4, 5, 6],
            }),
            decimal_points: [None; 4],
            colon: Some(WireBit { address: 1, bit: 7 }),
        },
        SevenSegmentLook::fahrenheit().with_colon_fraction(),
        3000,
        120000,
        3,
    )
    .unwrap();
    unsafe {
        ReadyRenderer::new(
            identity,
            Bounds {
                face_bytes: 8192,
                show_bytes: 8192,
                resource_bytes,
            },
            Provider(pins),
            mask,
        )
    }
    .unwrap()
}
pub fn grant(ready: &ReadyRenderer<Provider>) -> c::AuthorityGrant {
    let id = ready.identity();
    c::AuthorityGrant {
        grant_id: "grant/fixture/render".into(),
        contract_id: AUTHORITY.into(),
        host_call_contract_id: CALL.into(),
        subject_kind: conduit_face::RENDERER_KIND.into(),
        host_id: id.host_id.clone(),
        boot_id: id.boot_id.clone(),
        capability_id: CAPABILITY.into(),
    }
}
pub fn issue(
    ready: &ReadyRenderer<Provider>,
    active: &c::ActivePlayIdentity,
) -> (
    c::BaseCapabilityTable,
    c::BaseCapabilityHandle,
    c::BaseOperationClaim,
) {
    let id = ready.identity();
    let grant = grant(ready);
    let scope = c::BaseCapabilityScope {
        host_id: id.host_id.clone(),
        boot_id: id.boot_id.clone(),
        base_instance_id: id.provider_instance_id.clone(),
        base_provider_generation: id.provider_generation,
        plan_id: active.plan_id.clone(),
        active_play_id: active.active_play_id.clone(),
        authority_grant_id: grant.grant_id.clone(),
        authority_contract_id: grant.contract_id.clone(),
        capability_id: CAPABILITY.into(),
        implementation_id: id.implementation_id.clone(),
        operation_contract_id: CALL.into(),
        subject_kind: conduit_face::RENDERER_KIND.into(),
        resource_pool_id: id.resource_pool_id.clone(),
        resource_generation_id: id.resource_generation_id.clone(),
        envelope_id: id.envelope_id.clone(),
        maximum_parameter_bytes: 8192,
        maximum_result_bytes: 8192,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 2,
    };
    let authority = c::BaseCapabilityAuthority {
        grant,
        base_instance_id: id.provider_instance_id.clone(),
        base_provider_generation: id.provider_generation,
        resource_pool_id: id.resource_pool_id.clone(),
        resource_generation_id: id.resource_generation_id.clone(),
        operation_contract_id: CALL.into(),
        envelope_id: id.envelope_id.clone(),
        maximum_parameter_bytes: 8192,
        maximum_result_bytes: 8192,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 2,
    };
    let claim = c::BaseOperationClaim {
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: scope.base_provider_generation,
        plan_id: scope.plan_id.clone(),
        active_play_id: scope.active_play_id.clone(),
        implementation_id: scope.implementation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        subject_kind: scope.subject_kind.clone(),
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        parameter_bytes: 8192,
        work_units: 1,
    };
    let mut table = c::BaseCapabilityTable::new(
        id.host_id.clone(),
        id.boot_id.clone(),
        id.provider_instance_id.clone(),
        1,
        [7; 32],
        1,
    )
    .unwrap();
    let handle = table
        .issue(c::CapabilityIssueRequest { scope, authority })
        .unwrap();
    (table, handle, claim)
}
pub fn bind(
    planned: &conduit_face::PlannedMaskPlot,
    ready: &mut Option<ReadyRenderer<Provider>>,
    active: &c::ActivePlayIdentity,
    wrong: bool,
) -> Result<(RendererCall<Provider>, NodeId), &'static str> {
    use conduit_embedded_build as b;
    use conduit_plan_lowering::selected_assigned_operation::*;
    let fragment = &planned.plan.fragments[0];
    let image = b::generate_fore_native_plan(
        &planned.plan,
        &fragment.fragment_id,
        b::EmbeddedImageBounds::HOST_TOOLING,
        b::EmbeddedComputationBounds {
            maximum_program_bytes: 0,
            maximum_total_program_bytes: 0,
        },
        3,
    )
    .unwrap();
    let maxima = c::AssignedPlanMaxima {
        encoded_bytes: 16384,
        runtime_state_bytes: 65535,
        counts: [32; c::ASSIGNED_PLAN_COUNT_KINDS],
    };
    let wire = image.encode(maxima).unwrap();
    let calls = [c::AssignedIdentity::from_text(CALL)];
    let resources: Vec<_> = image
        .kernel()
        .resources
        .iter()
        .map(|r| r.resource)
        .collect();
    let (index, gear) = fragment
        .placements
        .iter()
        .enumerate()
        .find(|(_, g)| g.kind_id.as_str() == conduit_face::RENDERER_KIND)
        .unwrap();
    let node = NodeId(index as u16);
    let (table, handle, mut claim) = issue(ready.as_ref().unwrap(), active);
    if wrong {
        claim.resource_generation_id = c::ResourceGenerationId("wrong".into());
    }
    let call = ReadyRenderer::bind_assigned_retained(
        ready,
        table,
        handle,
        claim,
        &gear.configuration,
        SelectedAssignedForeOperationPlan {
            fore: image.fore(),
            operation: SelectedAssignedOperationPlan {
                input: AssignedOperationInput::Contiguous(&wire),
                maxima,
                requirements: c::AssignedPlanRequirements {
                    host: c::AssignedIdentity::from_text(fragment.host_id.as_str()),
                    boot: c::AssignedIdentity::from_text(fragment.boot_id.as_str()),
                    host_calls: &calls,
                    resources: &resources,
                    remote_bindings: &[],
                },
                computations: c::AssignedComputationProfile {
                    maximum_text_bytes: 0,
                    maximum_total_text_bytes: 0,
                    requirements: &[],
                },
                fragment_id: &fragment.fragment_id,
                active,
                node,
            },
        },
    )?;
    Ok((call, node))
}
