use conduit_core::{Quantity, Unit};
use conduit_environment_runtime::{
    observed_record::{self, ObservedRecord, Refusal},
    recording::RecordingDisposition as D,
};
#[test]
fn existing_forms_retain_quantities_and_explicit_observation_time() {
    let original = ObservedRecord {
        sequence: 37,
        observed_at_boot_ms: 123456,
        disposition: D::Available,
        quantities: Some((
            Quantity::from_decimal(198, -1, Unit::Celsius).unwrap(),
            Quantity::new(59, Unit::Percent),
        )),
    };
    let bytes = original.encode().unwrap();
    assert_eq!(bytes.len(), 21);
    let recovered = ObservedRecord::decode(&bytes).unwrap();
    assert_eq!(recovered.sequence, 37);
    assert_eq!(recovered.observed_at_boot_ms, 123456);
    assert_eq!(recovered.disposition, original.disposition);
    assert_eq!(recovered.quantities, original.quantities);
    let mut alternate = original;
    alternate.quantities = Some((
        Quantity::new(68, Unit::Fahrenheit),
        Quantity::new(59, Unit::Percent),
    ));
    let recovered = ObservedRecord::decode(&alternate.encode().unwrap()).unwrap();
    assert_eq!(
        recovered.quantities.unwrap().0,
        Quantity::new(20, Unit::Celsius)
    );
    let contract =
        conduit_environment_runtime::observation::ObservationContract::prepare().unwrap();
    let descriptor = observed_record::descriptor(&contract).unwrap();
    assert_eq!(&descriptor[..4], b"ORH1");
    assert_ne!(&descriptor[36..165], &descriptor[165..294]);
}
#[test]
fn failures_retain_disposition_without_inventing_a_prior_measurement() {
    for disposition in [
        D::CaptureTimeout,
        D::ProviderFailed,
        D::GeometryRefused,
        D::ShortFrame,
        D::MalformedLength,
        D::Checksum,
        D::InvalidPayload,
    ] {
        let record = ObservedRecord {
            sequence: 1,
            observed_at_boot_ms: 90,
            disposition,
            quantities: None,
        };
        let mut bytes = record.encode().unwrap();
        assert_eq!(&bytes[17..], &[0; 4]);
        let decoded = ObservedRecord::decode(&bytes).unwrap();
        assert_eq!(decoded.disposition, disposition);
        assert!(decoded.quantities.is_none());
        bytes[17] = 1;
        assert!(matches!(
            ObservedRecord::decode(&bytes),
            Err(Refusal::Noncanonical)
        ));
    }
    let record = ObservedRecord {
        sequence: 1,
        observed_at_boot_ms: 90,
        disposition: D::Available,
        quantities: None,
    };
    assert_eq!(record.encode(), Err(Refusal::Noncanonical));
}

#[test]
fn committed_publication_uses_linked_type_and_shared_status_projection() {
    use conduit_environment_runtime::{
        observation::{ObservationContract, PreparedSourceFrameDecoder},
        publication::{PublicationChannel, PublicationSnapshot},
    };
    let contract = ObservationContract::prepare().unwrap();
    let mut decoder = PreparedSourceFrameDecoder::prepare(&contract).unwrap();
    let bytes = decoder.decode(5, [53, 0, 20, 2, 75]).unwrap();
    let snapshot = PublicationSnapshot {
        channel: PublicationChannel::Reading,
        sequence: 7,
        canonical: bytes,
    };
    let record = ObservedRecord::from_publication(snapshot, 987).unwrap();
    assert_eq!(record.sequence, 7);
    assert_eq!(record.observed_at_boot_ms, 987);
    assert_eq!(record.disposition, D::Available);
    assert_eq!(
        record.quantities.unwrap().1,
        Quantity::new(53, Unit::Percent)
    );
    assert!(ObservedRecord::from_publication(
        PublicationSnapshot {
            channel: PublicationChannel::ProviderFailure,
            ..snapshot
        },
        987
    )
    .is_err());
    assert!(ObservedRecord::from_publication(
        PublicationSnapshot {
            sequence: 0,
            ..snapshot
        },
        987
    )
    .is_err());
    let bytes = decoder.decode(5, [53, 0, 20, 2, 0]).unwrap();
    let record = ObservedRecord::from_publication(
        PublicationSnapshot {
            canonical: bytes,
            sequence: 8,
            channel: PublicationChannel::Reading,
        },
        1000,
    )
    .unwrap();
    assert_eq!(record.disposition, D::Checksum);
    assert!(record.quantities.is_none());
}
