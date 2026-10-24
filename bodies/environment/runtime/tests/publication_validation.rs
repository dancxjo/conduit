//! Borrowed linked-Type checking has the same acceptance boundary as the
//! prepared contract validator, with no schema construction during Play.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core::{
    self as c, PreparedStructuredComposer as Composer, StructuredInfoTypeShape as Shape,
};
use conduit_environment_runtime::{
    observation::{ObservationContract, PreparedSourceFrameDecoder},
    publication::{PublicationChannel as Channel, PublicationContract},
};
#[test]
fn linked_validation_preserves_all_channel_contracts_and_refuses_truncation_without_allocation() {
    let mut decoder =
        PreparedSourceFrameDecoder::prepare(&ObservationContract::prepare().unwrap()).unwrap();
    let reading = decoder.decode(5, [53, 0, 20, 2, 75]).unwrap().to_vec();
    let capture_contract = PublicationContract::prepare(Channel::CaptureTimeout).unwrap();
    let Shape::Record { fields, .. } = capture_contract.value_type().shape() else {
        panic!()
    };
    let byte_type = fields
        .iter()
        .find(|f| f.name() == "length")
        .unwrap()
        .value_type();
    let wire_type = fields
        .iter()
        .find(|f| f.name() == "wire")
        .unwrap()
        .value_type();
    let mut byte = Composer::new(byte_type, 64).unwrap();
    byte.leaf(&[0]).unwrap();
    let mut wire = Composer::new(wire_type, 1024).unwrap();
    wire.collection(&[c::validate_canonical_structured_value(byte.encoded()).unwrap(); 8])
        .unwrap();
    let mut capture = Composer::new(capture_contract.value_type(), 1024).unwrap();
    capture
        .record(&[
            c::validate_canonical_structured_value(byte.encoded()).unwrap(),
            c::validate_canonical_structured_value(wire.encoded()).unwrap(),
        ])
        .unwrap();
    let empty_contract = PublicationContract::prepare(Channel::ProviderFailure).unwrap();
    let mut empty = Composer::new(empty_contract.value_type(), 64).unwrap();
    empty.leaf(&[]).unwrap();
    let values = [
        &reading[..],
        capture.encoded(),
        empty.encoded(),
        byte.encoded(),
        &[0][..],
    ];
    let validators = Channel::ALL.map(|channel| {
        c::PreparedStructuredValueValidator::new(
            PublicationContract::prepare(channel).unwrap().value_type(),
            channel.maximum_bytes(),
        )
        .unwrap()
    });
    let too_large = vec![0; 4097];
    let ((), memory) = allocation_probe::observe(|| {
        for (channel, validator) in Channel::ALL.into_iter().zip(&validators) {
            for value in values {
                assert_eq!(
                    channel.validate_value(value).is_ok(),
                    validator.validate(value).is_ok()
                );
                for length in 0..value.len() {
                    assert!(channel.validate_value(&value[..length]).is_err());
                }
            }
            assert!(channel.validate_value(&too_large).is_err());
        }
        Channel::Reading.validate_value(&reading).unwrap();
        Channel::CaptureTimeout
            .validate_value(capture.encoded())
            .unwrap();
        Channel::ProviderFailure
            .validate_value(empty.encoded())
            .unwrap();
        Channel::GeometryRefusal
            .validate_value(empty.encoded())
            .unwrap();
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
