use super::*;
use crate::i2c_base::tests::request;
use conduit_core::{StructuredInfoValue, StructuredInfoValueShape};
mod fixture;

#[derive(Default)]
struct Provider {
    effects: usize,
    revoked: bool,
    actual: Option<usize>,
    refusal: Option<I2cDisposition>,
}
impl I2cProvider for Provider {
    fn transact(
        &mut self,
        _: &super::super::transaction::I2cTransaction<'_>,
        input: &mut [u8],
    ) -> Result<usize, I2cDisposition> {
        assert!(!self.revoked);
        self.effects += 1;
        input.fill(self.effects as u8);
        self.refusal
            .map_or(Ok(self.actual.unwrap_or(input.len())), Err)
    }
    fn revoke(&mut self) {
        self.revoked = true;
    }
}
fn tag(bytes: &[u8]) -> alloc::string::String {
    let value = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
    let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
        panic!("variant")
    };
    tag.into()
}

#[test]
fn exact_binding_replay_and_decode_fail_before_any_effect() {
    let mut owner = fixture::owner();
    let node = owner.node;
    let input = request(&I2cContract::prepare().unwrap(), 0x76, &[0xd0], 1);
    assert_eq!(
        owner
            .invoke(node, HostCallId(1), RequestId(0), &input)
            .err(),
        Some(I2cCallRefusal::WrongBinding)
    );
    assert_eq!(
        owner
            .invoke(node, HostCallId(0), RequestId(1), &input)
            .err(),
        Some(I2cCallRefusal::StaleRequest)
    );
    assert!(matches!(
        owner.invoke(node, HostCallId(0), RequestId(0), &input[..input.len() - 1]),
        Err(I2cCallRefusal::Decode(_))
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
        Some(I2cCallRefusal::StaleRequest)
    );
    assert_eq!(owner.provider.effects, 1);
}

#[test]
fn trusted_address_interval_and_revocation_bound_bus_authority() {
    let mut owner = fixture::owner();
    let node = owner.node;
    let input = request(&I2cContract::prepare().unwrap(), 0x50, &[0xd0], 1);
    assert_eq!(
        tag(owner
            .invoke(node, HostCallId(0), RequestId(0), &input)
            .unwrap()),
        "refused"
    );
    assert_eq!(owner.provider.effects, 0);
    owner.revoke(node, HostCallId(0)).unwrap();
    assert!(owner.provider.revoked);
    let input = request(&I2cContract::prepare().unwrap(), 0x76, &[0xd0], 1);
    assert!(matches!(
        owner.invoke(node, HostCallId(0), RequestId(1), &input),
        Err(I2cCallRefusal::Capability(_))
    ));
    assert_eq!(owner.provider.effects, 0);
}

#[test]
fn short_refused_and_malformed_completion_consume_once_and_release_lease() {
    let mut owner = fixture::owner();
    let node = owner.node;
    let input = request(&I2cContract::prepare().unwrap(), 0x76, &[0xd0], 2);
    owner.provider.actual = Some(1);
    assert_eq!(
        tag(owner
            .invoke(node, HostCallId(0), RequestId(0), &input)
            .unwrap()),
        "short"
    );
    owner.provider.actual = Some(3);
    assert_eq!(
        owner
            .invoke(node, HostCallId(0), RequestId(1), &input)
            .err(),
        Some(I2cCallRefusal::Result(I2cResultRefusal::ActualLength))
    );
    owner.provider.refusal = Some(I2cDisposition::TimedOut);
    assert_eq!(
        tag(owner
            .invoke(node, HostCallId(0), RequestId(2), &input)
            .unwrap()),
        "timed-out"
    );
    owner.provider.refusal = None;
    owner.provider.actual = None;
    assert_eq!(
        tag(owner
            .invoke(node, HostCallId(0), RequestId(3), &input)
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
        4
    );
    assert_eq!(owner.provider.effects, 4);
}

#[test]
fn matching_identifiers_cannot_replace_lowered_call_or_active_play() {
    let (fragment, mut lowered, active, placement) = fixture::selected();
    lowered.host_calls[0].binding.maximum_output_bytes += 1;
    assert_eq!(
        fixture::bind(&fragment, &lowered, &active, &placement).err(),
        Some(I2cCallRefusal::WrongBinding)
    );
    let (fragment, lowered, mut active, placement) = fixture::selected();
    active.active_play_id = conduit_core::ActivePlayId::from("play/stale");
    assert_eq!(
        fixture::bind(&fragment, &lowered, &active, &placement).err(),
        Some(I2cCallRefusal::WrongBinding)
    );
}
