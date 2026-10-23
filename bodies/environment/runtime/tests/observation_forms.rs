//! Compact storage keeps semantic quantities; the selected authored Forms own
//! coordinates. This is codec proof, not durable recorder or Line evidence.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core::{
    validate_canonical_structured_value as validate, PreparedStructuredComposer as C, Quantity,
    QuantityFormRefusal as F, StructuredInfoTypeShape as S, Unit,
};
use conduit_environment_runtime::observation::{
    ObservationContract, ObservationRefusal, PreparedObservationQuantityForms as Forms,
};

fn canonical(contract: &ObservationContract, temperature: Quantity, humidity: Quantity) -> Vec<u8> {
    let S::Record { fields, .. } = contract.observation_type().shape() else {
        panic!("record")
    };
    let mut h = C::new(fields[0].value_type(), 1024).unwrap();
    let mut t = C::new(fields[1].value_type(), 1024).unwrap();
    h.leaf(&humidity.encode()).unwrap();
    t.leaf(&temperature.encode()).unwrap();
    let mut observation = C::new(contract.observation_type(), 4096).unwrap();
    observation
        .record(&[
            validate(h.encoded()).unwrap(),
            validate(t.encoded()).unwrap(),
        ])
        .unwrap();
    observation.encoded().to_vec()
}
#[test]
fn authored_quantity_forms_round_trip_exact_observations_without_play_allocations() {
    let contract = ObservationContract::prepare().unwrap();
    let mut forms = Forms::prepare(&contract).unwrap();
    assert_ne!(Forms::TEMPERATURE_FORM_ID, Forms::HUMIDITY_FORM_ID);
    for (temperature, humidity) in [(198, 590), (-108, 530), (0, 0), (600, 950)] {
        let t = Quantity::from_decimal(temperature, -1, Unit::Celsius).unwrap();
        let h = Quantity::from_decimal(humidity, -1, Unit::Percent).unwrap();
        let original = canonical(&contract, t, h);
        assert!(original.len() > Forms::PACKED_BYTES);
        let ((), measured) = allocation_probe::observe(|| {
            let packed = forms.pack(&original).unwrap();
            assert_eq!(&packed[..2], &(temperature as i16).to_le_bytes());
            assert_eq!(&packed[2..], &(humidity as u16).to_le_bytes());
            assert_eq!(forms.unpack(&packed).unwrap(), original);
        });
        assert_eq!((measured.allocations, measured.reallocations), (0, 0));
    }
    let fahrenheit = canonical(
        &contract,
        Quantity::new(68, Unit::Fahrenheit),
        Quantity::new(59, Unit::Percent),
    );
    let celsius = canonical(
        &contract,
        Quantity::new(20, Unit::Celsius),
        Quantity::new(59, Unit::Percent),
    );
    let ((), measured) = allocation_probe::observe(|| {
        let packed = forms.pack(&fahrenheit).unwrap();
        assert_eq!(forms.unpack(&packed).unwrap(), celsius);
    });
    assert_eq!((measured.allocations, measured.reallocations), (0, 0));
}
#[test]
fn compact_storage_refuses_inexact_overflowing_truncated_and_wrong_type_values() {
    let contract = ObservationContract::prepare().unwrap();
    let mut forms = Forms::prepare(&contract).unwrap();
    for (temperature, humidity, refusal) in [
        (
            Quantity::from_decimal(1981, -2, Unit::Celsius).unwrap(),
            Quantity::new(59, Unit::Percent),
            F::Inexact,
        ),
        (
            Quantity::from_decimal(32768, -1, Unit::Celsius).unwrap(),
            Quantity::new(59, Unit::Percent),
            F::Overflow,
        ),
        (
            Quantity::new(20, Unit::Celsius),
            Quantity::new(-1, Unit::Percent),
            F::Overflow,
        ),
    ] {
        let value = canonical(&contract, temperature, humidity);
        assert_eq!(forms.pack(&value), Err(ObservationRefusal::Form(refusal)));
    }
    assert!(matches!(
        forms.pack(&[0]),
        Err(ObservationRefusal::Canonical(_))
    ));
    for bytes in [&[][..], &[0; 3][..], &[0; 5][..]] {
        assert!(matches!(
            forms.unpack(bytes),
            Err(ObservationRefusal::Form(F::WrongLength))
        ));
    }
}
