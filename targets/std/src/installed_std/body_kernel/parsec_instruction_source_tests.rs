//! Actual checked Mask preparation and descriptor refusals, not Source effects.
//! Parent/Mask identities here are prospective fixtures. Only the runtime
//! completion method can mint custody after a successful scheduler completion.
use super::parsec_live_fore_tests::input_fixture;
use crate::flow_activation::parsec_instruction_source_offer as source;
use crate::installed_std::parsec_instruction_source_admission::ParsecInstructionSourceAdmission;
use conduit_body::{BodyPlan, BodyPlayIdentity};
use conduit_core::{ActivePlayIdentity, HostAdvertisement};
use conduit_face::PlannedMaskPlot;
use conduit_kernel::{
    scheduler::HostCallRequest, BoundedValueRef, HostCallId, NodeId, RequestId, ValueRef,
};
use conduit_plan_lowering::lowering::{lower_plan_fragment, LoweredPlanFragment};

struct Fixture {
    host: HostAdvertisement,
    parent: BodyPlan,
    parent_play: BodyPlayIdentity,
    mask: PlannedMaskPlot,
    mask_play: ActivePlayIdentity,
    lowered: LoweredPlanFragment,
}
impl Fixture {
    fn new() -> Self {
        let (host, _, _, _, parent) = input_fixture::prepare();
        let parent_play = BodyPlayIdentity::bind(&parent, 0);
        let mask =
            crate::terminal_mask_execution::planning::plan_attached_parsec(host.advertisement())
                .unwrap();
        let mask_play = conduit_core::bind_active_play(
            &mask.plan.plan_id,
            &host.advertisement().host_id,
            &host.advertisement().boot_id,
            0,
        );
        let lowered = lower_plan_fragment(&mask.plan.fragments[0]).unwrap();
        Self {
            host: host.advertisement().clone(),
            parent,
            parent_play,
            mask,
            mask_play,
            lowered,
        }
    }
    fn prepared(&self) -> ParsecInstructionSourceAdmission {
        ParsecInstructionSourceAdmission::prepare(
            &self.host,
            &self.parent,
            &self.parent_play,
            &self.mask,
            &self.mask_play,
            &self.lowered.identity,
            &self.lowered.host_calls,
        )
        .unwrap()
    }
    fn request(&self) -> HostCallRequest {
        let source = self
            .lowered
            .host_calls
            .iter()
            .find(|op| op.contract_id.as_str() == source::CONTRACT)
            .unwrap();
        HostCallRequest {
            node: source.node,
            call: source.call,
            request: RequestId(0),
            input: BoundedValueRef::new(
                ValueRef {
                    slot: 0,
                    generation: 1,
                    byte_len: 200,
                },
                source::MAXIMUM_SHOW_BYTES,
            )
            .unwrap(),
        }
    }
    fn confirmed(&self) -> ParsecInstructionSourceAdmission {
        let mut prepared = self.prepared();
        // Model the internal issue hook only. This cannot produce a qualified owner
        // witness or completion token; actual issue/completion is a separate proof.
        prepared
            .confirm_issued(&self.parent_play, &self.mask_play, &self.host)
            .unwrap();
        prepared
    }
}

#[test]
fn instruction_source_preparation_cannot_accept_or_claim_completion() {
    let fixture = Fixture::new();
    let mut prepared = fixture.prepared();
    assert!(prepared.accept(fixture.request(), &fixture.host).is_err());
    assert!(prepared
        .confirm_issued(&fixture.parent_play, &fixture.mask_play, &fixture.host)
        .is_err());
    let mut confirmed = fixture.confirmed();
    confirmed.accept(fixture.request(), &fixture.host).unwrap();
    let value = ValueRef {
        slot: 1,
        generation: 1,
        byte_len: 50,
    };
    assert!(confirmed
        .claim(fixture.request().node, fixture.request().call, value)
        .is_err());
    assert!(confirmed
        .acknowledge(fixture.request().node, fixture.request().call, value)
        .is_err());
}

#[test]
fn instruction_source_refuses_wrong_parent_mask_boot_generation_and_full_offers() {
    let fixture = Fixture::new();
    let wrong_parent = BodyPlayIdentity::bind(&fixture.parent, 1);
    assert!(fixture
        .prepared()
        .confirm_issued(&wrong_parent, &fixture.mask_play, &fixture.host)
        .is_err());
    let wrong_mask = conduit_core::bind_active_play(
        &fixture.mask.plan.plan_id,
        &fixture.host.host_id,
        &fixture.host.boot_id,
        1,
    );
    assert!(fixture
        .prepared()
        .confirm_issued(&fixture.parent_play, &wrong_mask, &fixture.host)
        .is_err());
    for changed in [
        {
            let mut host = fixture.host.clone();
            host.boot_id = "different/boot".into();
            host
        },
        {
            let mut host = fixture.host.clone();
            host.offer_generation.0 += 1;
            host
        },
        {
            let mut host = fixture.host.clone();
            host.capabilities.pop();
            host
        },
    ] {
        assert!(fixture
            .prepared()
            .confirm_issued(&fixture.parent_play, &fixture.mask_play, &changed)
            .is_err());
        assert!(fixture
            .confirmed()
            .accept(fixture.request(), &changed)
            .is_err());
    }
}

#[test]
fn instruction_source_request_refuses_wrong_mask_node_call_bounds_and_replay() {
    let fixture = Fixture::new();
    let original = fixture.request();
    for request in [
        HostCallRequest {
            node: NodeId(original.node.0 + 1),
            ..original
        },
        HostCallRequest {
            call: HostCallId(1),
            ..original
        },
        HostCallRequest {
            request: RequestId(1),
            ..original
        },
        HostCallRequest {
            input: BoundedValueRef::new(original.input.value, source::MAXIMUM_SHOW_BYTES - 1)
                .unwrap(),
            ..original
        },
    ] {
        let mut admission = fixture.confirmed();
        assert!(admission.accept(request, &fixture.host).is_err());
        assert!(
            admission.accept(original, &fixture.host).is_err(),
            "invalid transition revokes the descriptor"
        );
    }
    let mut admission = fixture.confirmed();
    admission.accept(original, &fixture.host).unwrap();
    assert!(admission.accept(original, &fixture.host).is_err());
    admission.revoke();
    assert!(admission.accept(original, &fixture.host).is_err());
}

#[test]
fn instruction_source_lowered_descriptor_cannot_substitute_other_mask_placement_or_operation() {
    let fixture = Fixture::new();
    let index = fixture
        .lowered
        .host_calls
        .iter()
        .position(|op| op.contract_id.as_str() == source::CONTRACT)
        .unwrap();
    let mut operations = fixture.lowered.host_calls.clone();
    operations[index].binding.maximum_output_bytes -= 1;
    assert!(ParsecInstructionSourceAdmission::prepare(
        &fixture.host,
        &fixture.parent,
        &fixture.parent_play,
        &fixture.mask,
        &fixture.mask_play,
        &fixture.lowered.identity,
        &operations
    )
    .is_err());
    let mut identity = fixture.lowered.identity.clone();
    let other = identity
        .placements
        .iter()
        .find(|(node, _)| *node != fixture.request().node)
        .unwrap()
        .1
        .clone();
    identity
        .placements
        .iter_mut()
        .find(|(node, _)| *node == fixture.request().node)
        .unwrap()
        .1 = other;
    assert!(ParsecInstructionSourceAdmission::prepare(
        &fixture.host,
        &fixture.parent,
        &fixture.parent_play,
        &fixture.mask,
        &fixture.mask_play,
        &identity,
        &fixture.lowered.host_calls
    )
    .is_err());
    let mut absent = fixture.host.clone();
    absent
        .capabilities
        .retain(|offer| offer.implementation.implementation_id.as_str() != source::IMPLEMENTATION);
    assert!(ParsecInstructionSourceAdmission::prepare(
        &absent,
        &fixture.parent,
        &fixture.parent_play,
        &fixture.mask,
        &fixture.mask_play,
        &fixture.lowered.identity,
        &fixture.lowered.host_calls
    )
    .is_err());
}
