//! Canonical output equivalence and bounded storage for the native call path.
use conduit_core::{validate_canonical_structured_value, StructuredInfoValue};
use conduit_i2c_transaction::{
    contract::I2cContract, result::PreparedI2cResultEncoder, I2cDisposition,
};
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod probe;
#[global_allocator]
static ALLOC: probe::Allocator = probe::Allocator;

#[test]
fn prepared_results_retain_exact_buffers_and_allocate_nothing_during_use() {
    let contract = I2cContract::prepare().unwrap();
    let mut encoder = PreparedI2cResultEncoder::new(&contract).unwrap();
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
    assert_eq!(value.value_type(), contract.result_type());
}
