use super::*;
fn interrupt(packet: u16, interval: u8) -> InboundEndpointParameters {
    InboundEndpointParameters {
        address: 0x81,
        transfer_type: 3,
        packet_field: packet,
        interval,
    }
}
#[test]
fn interrupt_contexts_preserve_exact_direction_packet_and_realized_interval() {
    for speed in [1, 2] {
        for interval in 1..=255 {
            let (dci, words) = context_words(interrupt(8, interval), speed, 0x1000).unwrap();
            assert_eq!(dci, 3);
            let exponent = (words[0] >> 16) & 255;
            let realized_microframes = 1_u32 << exponent;
            let requested = u32::from(interval) * 8;
            assert!(realized_microframes <= requested && realized_microframes * 2 > requested);
            assert_eq!(words[1], (8 << 16) | (7 << 3) | 6);
            assert_eq!((words[2], words[3]), (0x1001, 0));
            assert_eq!(words[4], (8 << 16) | 8);
        }
    }
    for interval in 1..=16 {
        let (_, words) = context_words(interrupt(1024, interval), 3, 0x2000).unwrap();
        assert_eq!(words[0], u32::from(interval - 1) << 16);
    }
}
#[test]
fn bounded_bulk_profile_retains_packet_and_refuses_unproved_realizations() {
    for packet in [8, 16, 32, 64] {
        let parameters = InboundEndpointParameters {
            address: 0x8f,
            transfer_type: 2,
            packet_field: packet,
            interval: 0,
        };
        let (dci, words) = context_words(parameters, 1, 0x3000).unwrap();
        assert_eq!(dci, 31);
        assert_eq!(words[1], (u32::from(packet) << 16) | (6 << 3) | 6);
        assert_eq!(words[4], u32::from(packet));
    }
    for speed in [0, 4, 5, 15] {
        assert_eq!(
            context_words(interrupt(8, 1), speed, 0x1000),
            Err(EndpointSetupRefusal::Unsupported)
        );
    }
    for parameters in [
        interrupt(0, 1),
        interrupt(8, 0),
        interrupt(1025, 1),
        interrupt(8, 17),
    ] {
        assert!(context_words(parameters, 3, 0x1000).is_err() || parameters.packet_field == 65);
    }
    assert_eq!(
        context_words(interrupt(8 | (1 << 11), 1), 3, 0x1000),
        Err(EndpointSetupRefusal::Unsupported)
    );
    for address in [0, 1, 0x80, 0x91, 0xff] {
        let mut parameters = interrupt(8, 1);
        parameters.address = address;
        assert_eq!(
            context_words(parameters, 1, 0x1000),
            Err(EndpointSetupRefusal::Parameters)
        );
    }
    assert!(context_words(interrupt(8, 1), 1, 0x1001).is_err());
}

#[test]
fn packet_profiles_refuse_cross_speed_or_unsupported_transfer_types() {
    assert_eq!(
        context_words(interrupt(65, 1), 1, 0x1000),
        Err(EndpointSetupRefusal::Parameters)
    );
    assert_eq!(
        context_words(interrupt(9, 1), 2, 0x1000),
        Err(EndpointSetupRefusal::Parameters)
    );
    for transfer_type in [0, 1, 4, 255] {
        let mut parameters = interrupt(8, 1);
        parameters.transfer_type = transfer_type;
        assert_eq!(
            context_words(parameters, 1, 0x1000),
            Err(EndpointSetupRefusal::Unsupported)
        );
    }
}

#[test]
fn native_setup_refuses_overflow_overlap_and_segment_crossing_before_writing_dma() {
    assert_eq!(validate_dma_regions(0x1000, 0x2000, 0x3000), Ok(()));
    for (input, ring, buffer) in [
        (0, 0x2000, 0x3000),
        (0x1001, 0x2000, 0x3000),
        (0x1000, 0x2001, 0x3000),
        (0x1000, 0x2000, 0),
        (0x1000, 0x1000, 0x3000),
        (0x1000, 0x2000, 0x2000),
        (0x1000, 0x2000, 0x17ff),
        (0x1000, 0xffc0, 0x3000),
        (0x1000, 0x2000, 0xffff),
        (u64::MAX - 63, 0x2000, 0x3000),
    ] {
        assert_eq!(
            validate_dma_regions(input, ring, buffer),
            Err(EndpointSetupRefusal::Mapping)
        );
    }
}
