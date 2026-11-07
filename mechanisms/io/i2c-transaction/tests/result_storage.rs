//! Canonical output equivalence and bounded storage for the native call path.
use conduit_core::{validate_canonical_structured_value, StructuredInfoValue};
use conduit_i2c_transaction::{
    contract::I2cContract, decode::PreparedI2cDecoder, result::PreparedI2cResultEncoder,
    I2cDisposition,
};
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod probe;
#[global_allocator]
static ALLOC: probe::Allocator = probe::Allocator;

#[test]
fn prepared_results_retain_exact_buffers_and_allocate_nothing_during_use() {
    let (contract, cost) = probe::observe(|| I2cContract::prepare_bounded(4096));
    let contract = contract.unwrap();
    let bytes = contract.retained_backing_bytes();
    assert_eq!(cost.requested_bytes, bytes);
    assert_eq!(cost.peak_bytes, bytes);
    assert_eq!(cost.reallocations, 0);
    assert!(I2cContract::prepare_bounded(bytes).is_ok());
    let (short, refused) = probe::observe(|| I2cContract::prepare_bounded(bytes - 1));
    assert!(matches!(
        short,
        Err(conduit_i2c_transaction::contract::I2cContractPreparationRefusal::Capacity)
    ));
    assert_eq!((refused.allocations, refused.reallocations), (0, 0));
    for nth in 1..=cost.allocations {
        let (result, failed) = probe::observe_refusing(nth, || I2cContract::prepare_bounded(bytes));
        assert!(matches!(
            result,
            Err(conduit_i2c_transaction::contract::I2cContractPreparationRefusal::Allocation)
        ));
        assert_eq!(failed.refused_allocations, 1);
        assert_eq!(failed.allocations, failed.deallocations);
    }
    println!("flat checked I2C interface: {bytes} retained/peak CPU bytes in {} fallible allocations; one-byte shortage refuses before allocation",cost.allocations);
    let (decoder, preparation) = probe::observe(|| PreparedI2cDecoder::new(&contract));
    assert_eq!(decoder.unwrap().retained_backing_bytes(), Some(0));
    assert_eq!((preparation.allocations, preparation.reallocations), (0, 0));
    let (encoder, preparation) = probe::observe(|| PreparedI2cResultEncoder::new(&contract));
    let mut encoder = encoder.unwrap();
    assert_eq!(preparation.requested_bytes, encoder.prepared_heap_bytes());
    assert_eq!(preparation.reallocations, 0);
    for nth in 1..=preparation.allocations {
        let (result, refused) =
            probe::observe_refusing(nth, || PreparedI2cResultEncoder::new(&contract));
        assert!(result.is_err());
        assert_eq!(refused.refused_allocations, 1);
        assert_eq!(refused.allocations, refused.deallocations);
    }
    println!(
        "I2C result preparation: {} fallible allocations, {} requested B, {} B CPU peak",
        preparation.allocations, preparation.requested_bytes, preparation.peak_bytes
    );
    let retained = encoder.prepared_heap_bytes();
    assert!(retained < 4096, "retained {retained}");
    println!("I2C result encoder retained heap payload: {retained} bytes");
    let octets = [255; 32];
    let (_, observation) = probe::observe(|| {
        for length in 0..=32 {
            let value = encoder.completed(length as u8, &octets[..length]).unwrap();
            validate_canonical_structured_value(value).unwrap();
            if length > 0 {
                let value = encoder
                    .completed(length as u8, &octets[..length - 1])
                    .unwrap();
                validate_canonical_structured_value(value).unwrap();
            }
        }
        for disposition in [
            I2cDisposition::DeviceError,
            I2cDisposition::NotAcknowledged,
            I2cDisposition::ArbitrationLost,
            I2cDisposition::TimedOut,
            I2cDisposition::ProviderLost,
            I2cDisposition::Unsupported,
            I2cDisposition::StaleAttachment,
            I2cDisposition::Refused,
        ] {
            validate_canonical_structured_value(encoder.disposition(disposition).unwrap()).unwrap();
        }
    });
    assert_eq!(observation.allocations, 0);
    assert_eq!(observation.reallocations, 0);
    // Independent owned decoder checks the largest successful value's exact Type.
    let value =
        StructuredInfoValue::from_canonical_bytes(encoder.completed(32, &octets).unwrap()).unwrap();
    assert_eq!(value.value_type(), &contract.decode_result_type().unwrap());
}
