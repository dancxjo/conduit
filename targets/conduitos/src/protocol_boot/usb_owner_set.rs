//! Move-only ownership passed from Root into native service preparation.
use super::*;

pub(in crate::protocol_boot) enum NativeUsbOwners {
    Isochronous {
        ready: Box<arch::ReadyUsbIsochronousHost>,
        control_issuer: Box<NativeProtocolIssuer>,
        isochronous_issuer: Box<NativeProtocolIssuer>,
    },
    Duplex {
        storage: Option<Box<crate::protocol_source::NativeStoragePreparation>>,
        #[cfg(all(target_arch = "x86_64", feature = "native-owner-network"))]
        network: Option<Box<super::network::NativeEthernetOwner>>,
        ready: Box<arch::ReadyUsbDuplexHost>,
        control_issuer: Box<NativeProtocolIssuer>,
        read_issuer: Box<NativeProtocolIssuer>,
        write_issuer: Box<NativeProtocolIssuer>,
    },
    Control {
        ready: Box<arch::ReadyUsbControlBase>,
        issuer: Box<NativeProtocolIssuer>,
    },
    Protocol {
        ready: Box<arch::ReadyUsbProtocolHost>,
        control_issuer: Box<NativeProtocolIssuer>,
        endpoint_issuer: Box<NativeProtocolIssuer>,
    },
}
