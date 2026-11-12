extern crate std;
use super::*;

#[test]
fn linked_result_matches_original_bytes_and_reuses_one_cache() {
    let contract = PulseReadContract::prepare().unwrap();
    let mut new = PreparedPulseCodec::prepare(&contract).unwrap();
    let mut old = legacy::PreparedPulseCodec::prepare(&contract).unwrap();
    let backing = new.retained_backing_bytes().unwrap();
    let old_backing = old.retained_backing_bytes().unwrap();
    assert_eq!(
        backing,
        fixed_value_bound(&contract.result).unwrap() as usize
    );
    assert!(old_backing > backing);
    std::println!(
        "pulse codec backing: old={old_backing}, new={backing}, saved={}",
        old_backing - backing
    );
    let address = new.output.as_ptr();
    for disposition in [
        PulseReadDisposition::Complete,
        PulseReadDisposition::TimedOut,
    ] {
        for count in 0..=8 {
            if count == 0 && disposition == PulseReadDisposition::Complete {
                continue;
            }
            for octet in [0, 1, 0x55, 0xaa, 255] {
                let wire = [octet; 8];
                let capture =
                    PulseReadCapture::from_complete_octets(disposition, &wire[..count]).unwrap();
                let bytes = new.result(capture).unwrap();
                assert_eq!(bytes, old.result(capture).unwrap());
                conduit_core::validate_canonical_structured_value(bytes).unwrap();
                if count == 8 {
                    assert_eq!(bytes.len(), backing);
                }
                assert_eq!(new.output.as_ptr(), address);
                assert_eq!(new.retained_backing_bytes(), Some(backing));
            }
        }
    }
    assert_eq!(
        new.geometry_refused().unwrap(),
        old.geometry_refused().unwrap()
    );
    let failed =
        PulseReadCapture::from_complete_octets(PulseReadDisposition::ProviderFailed, &[]).unwrap();
    assert_eq!(new.result(failed).unwrap(), old.result(failed).unwrap());
}

#[test]
fn invalid_capture_and_foreign_contract_remain_refused() {
    let contract = PulseReadContract::prepare().unwrap();
    let mut new = PreparedPulseCodec::prepare(&contract).unwrap();
    let mut old = legacy::PreparedPulseCodec::prepare(&contract).unwrap();
    for capture in [
        PulseReadCapture {
            disposition: PulseReadDisposition::Complete,
            length: 0,
            wire: [0; 8],
        },
        PulseReadCapture {
            disposition: PulseReadDisposition::TimedOut,
            length: 9,
            wire: [0; 8],
        },
        PulseReadCapture {
            disposition: PulseReadDisposition::TimedOut,
            length: 0,
            wire: [1; 8],
        },
        PulseReadCapture {
            disposition: PulseReadDisposition::ProviderFailed,
            length: 1,
            wire: [0; 8],
        },
    ] {
        assert!(matches!(
            new.result(capture),
            Err(PulseCodecRefusal::InvalidCapture)
        ));
        assert!(matches!(
            old.result(capture),
            Err(legacy::PulseCodecRefusal::InvalidCapture)
        ));
    }
    for member in 0..3 {
        let mut contract = PulseReadContract::prepare().unwrap();
        match member {
            0 => contract.request = contract.capture.clone(),
            1 => contract.capture = contract.request.clone(),
            _ => contract.result = contract.request.clone(),
        }
        assert!(matches!(
            PreparedPulseCodec::prepare(&contract),
            Err(StructuredInfoRefusal::WrongType)
        ));
    }
}
