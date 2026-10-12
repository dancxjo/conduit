//! Prepare the exact native USB service before the ordinary execution loop.
use super::*;

pub(in crate::protocol_boot) fn prepare_usb(
    entry: PreparedProtocolEntry,
    request: &crate::protocol_boot::usb_request::UsbProtocolBootRequest,
    host: HostId,
    boot: BootId,
    owners: crate::protocol_boot::usb_owners::NativeUsbOwners,
) -> Result<(NativePlay, Vec<Output>), &'static str> {
    validate_input_values(&entry, &request.inputs)?;
    let session = crate::protocol_boot::body::admit(entry.resident(), &host, &boot)?;
    let advertisement = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: host,
        boot_id: boot,
        offer_generation: OfferGeneration(1),
        profile: "conduitos/native@1".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let play = match owners {
        crate::protocol_boot::usb_owners::NativeUsbOwners::Isochronous {
            ready,
            control_issuer,
            isochronous_issuer,
        } => crate::protocol_source::prepare_native_usb_isochronous_protocol(
            entry,
            session,
            advertisement,
            *ready,
            *control_issuer,
            *isochronous_issuer,
            0,
            1,
            request.additional_remote_sign_items,
        ),
        crate::protocol_boot::usb_owners::NativeUsbOwners::Duplex {
            ready,
            control_issuer,
            read_issuer,
            write_issuer,
            storage,
            #[cfg(all(target_arch = "x86_64", feature = "native-owner-network"))]
            network,
        } => {
            #[cfg(all(target_arch = "x86_64", feature = "native-owner-network"))]
            let result = match network {
                Some(network) => crate::protocol_source::prepare_native_usb_duplex(
                    entry,
                    session,
                    advertisement,
                    *ready,
                    *control_issuer,
                    *read_issuer,
                    *write_issuer,
                    0,
                    1,
                    request.additional_remote_sign_items,
                    Some(crate::protocol_source::NativeEthernetPreparation {
                        ready: *network.ready,
                        issuer: *network.issuer,
                        random_seed: network.random_seed,
                        timestamp: smoltcp::time::Instant::ZERO,
                    }),
                    storage.map(|storage| *storage),
                ),
                None => crate::protocol_source::prepare_native_usb_duplex(
                    entry,
                    session,
                    advertisement,
                    *ready,
                    *control_issuer,
                    *read_issuer,
                    *write_issuer,
                    0,
                    1,
                    request.additional_remote_sign_items,
                    None,
                    storage.map(|storage| *storage),
                ),
            };
            #[cfg(not(all(target_arch = "x86_64", feature = "native-owner-network")))]
            let result = crate::protocol_source::prepare_native_usb_duplex(
                entry,
                session,
                advertisement,
                *ready,
                *control_issuer,
                *read_issuer,
                *write_issuer,
                0,
                1,
                request.additional_remote_sign_items,
                storage.map(|storage| *storage),
            );
            result
        }
        crate::protocol_boot::usb_owners::NativeUsbOwners::Control { ready, issuer } => {
            crate::protocol_source::prepare_native_usb_protocol_with_sign_storage(
                entry,
                session,
                advertisement,
                *ready,
                *issuer,
                0,
                1,
                request.additional_remote_sign_items,
            )
        }
        crate::protocol_boot::usb_owners::NativeUsbOwners::Protocol {
            ready,
            control_issuer,
            endpoint_issuer,
        } => crate::protocol_source::prepare_native_usb_endpoint_protocol(
            entry,
            session,
            advertisement,
            *ready,
            *control_issuer,
            *endpoint_issuer,
            0,
            1,
            request.additional_remote_sign_items,
        ),
    }
    .map_err(|refusal| {
        report_preparation_refusal(&refusal.reason);
        "protocol-usb-plan-admission-refused"
    })?;
    prepare_outputs(play, &request.inputs)
}
