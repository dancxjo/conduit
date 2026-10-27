use super::*;
use crate::spi_base::{SpiDisposition, SpiTransaction, tests::request};
use conduit_core::validate_canonical_structured_value;
mod fixture;

#[derive(Default)]
struct Provider {
    effects: usize,
    revoked: bool,
    refusal: Option<SpiDisposition>,
}
impl SpiProvider for Provider {
    fn exchange(
        &mut self,
        request: &SpiTransaction<'_>,
        input: &mut [u8],
    ) -> Result<(), SpiDisposition> {
        assert!(!self.revoked);
        assert_eq!(input.len(), request.output().len());
        self.effects += 1;
        input.fill(self.effects as u8);
        self.refusal.map_or(Ok(()), Err)
    }
    fn revoke(&mut self) {
        self.revoked = true;
    }
}
fn tag(bytes: &[u8]) -> alloc::string::String {
    validate_canonical_structured_value(bytes)
        .unwrap()
        .variant_tag()
        .unwrap()
        .into()
}
#[test]
fn wrong_binding_replay_and_decode_refuse_before_bus_effects() {
    let mut owner = fixture::owner();
    let node = owner.node;
    let input = request(&SpiContract::prepare().unwrap(), &[0x55]);
    assert_eq!(
        owner
            .invoke(node, HostCallId(1), RequestId(0), &input)
            .err(),
        Some(SpiCallRefusal::WrongBinding)
    );
    assert_eq!(
        owner
            .invoke(node, HostCallId(0), RequestId(1), &input)
            .err(),
        Some(SpiCallRefusal::StaleRequest)
    );
    assert!(matches!(
        owner.invoke(node, HostCallId(0), RequestId(0), &input[..input.len() - 1]),
        Err(SpiCallRefusal::Decode(_))
    ));
    assert_eq!(owner.provider.effects, 0);
    assert_eq!(
        tag(owner
            .invoke(node, HostCallId(0), RequestId(0), &input)
            .unwrap()),
        "completed"
    );
    assert_eq!(
        owner
            .invoke(node, HostCallId(0), RequestId(0), &input)
            .err(),
        Some(SpiCallRefusal::StaleRequest)
    );
    assert_eq!(owner.provider.effects, 1);
}
#[test]
fn revoked_possession_prevents_further_effects() {
    let mut owner = fixture::owner();
    let node = owner.node;
    let input = request(&SpiContract::prepare().unwrap(), &[0x55]);
    owner.revoke(node, HostCallId(0)).unwrap();
    assert!(owner.provider.revoked);
    assert!(matches!(
        owner.invoke(node, HostCallId(0), RequestId(0), &input),
        Err(SpiCallRefusal::Capability(_))
    ));
    assert_eq!(owner.provider.effects, 0);
}
#[test]
fn physical_refusal_consumes_once_and_releases_inflight_lease() {
    let mut owner = fixture::owner();
    let node = owner.node;
    let input = request(&SpiContract::prepare().unwrap(), &[0x55]);
    owner.provider.refusal = Some(SpiDisposition::Refused);
    assert_eq!(
        tag(owner
            .invoke(node, HostCallId(0), RequestId(0), &input)
            .unwrap()),
        "refused"
    );
    owner.provider.refusal = None;
    assert_eq!(
        tag(owner
            .invoke(node, HostCallId(0), RequestId(1), &input)
            .unwrap()),
        "completed"
    );
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .completed_operations,
        2
    );
    assert_eq!(owner.provider.effects, 2);
}
#[test]
fn stale_plan_lowering_or_active_play_cannot_replace_possession() {
    let (fragment, mut lowered, active, placement) = fixture::selected();
    lowered.host_calls[0].binding.maximum_output_bytes += 1;
    assert_eq!(
        fixture::bind(&fragment, &lowered, &active, &placement).err(),
        Some(SpiCallRefusal::WrongBinding)
    );
    let (fragment, lowered, mut active, placement) = fixture::selected();
    active.active_play_id = conduit_core::ActivePlayId::from("play/stale");
    assert_eq!(
        fixture::bind(&fragment, &lowered, &active, &placement).err(),
        Some(SpiCallRefusal::WrongBinding)
    );
}
#[test]
fn admitted_dispatch_does_not_allocate() {
    let mut owner = fixture::owner();
    let node = owner.node;
    let input = request(&SpiContract::prepare().unwrap(), &[0x55; 512]);
    let allocations = crate::test_allocations::allocations(|| {
        assert_eq!(
            validate_canonical_structured_value(
                owner
                    .invoke(node, HostCallId(0), RequestId(0), &input)
                    .unwrap()
            )
            .unwrap()
            .variant_tag()
            .unwrap(),
            "completed"
        );
    });
    assert_eq!(allocations, 0);
}

#[test]
fn operation_exhaustion_refuses_before_any_extra_bus_effect() {
    let mut owner = fixture::owner();
    let node = owner.node;
    let input = request(&SpiContract::prepare().unwrap(), &[0x55]);
    for sequence in 0..2 {
        owner
            .invoke(node, HostCallId(0), RequestId(sequence), &input)
            .unwrap();
    }
    assert!(matches!(
        owner.invoke(node, HostCallId(0), RequestId(2), &input),
        Err(SpiCallRefusal::Capability(_))
    ));
    assert_eq!(owner.provider.effects, 2);
}
