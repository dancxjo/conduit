//! Machine-readable failures for bounded USB enumeration.

use super::super::xhci::XhciError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UsbError {
    NoDevice,
    MultipleDevices,
    PortResetTimeout,
    PortResetFailed,
    EnableSlotFailed,
    AddressDeviceFailed,
    ContextGeometry,
    DmaAddressInvalid,
    TransferRingFull,
    ControlStall,
    ControlError,
    ControlTimeout,
    WrongController,
    WrongSlot,
    WrongEndpoint,
    DeviceVanished,
    MalformedDescriptor,
    OversizedConfiguration,
    TooManyInterfaces,
    TooManyEndpoints,
    TooManyDescriptorRecords,
    UnsupportedTopology,
    StaleDeviceInstance,
}

impl UsbError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoDevice => "usb-device-absent",
            Self::MultipleDevices => "usb-multiple-root-devices",
            Self::PortResetTimeout => "usb-port-reset-timeout",
            Self::PortResetFailed => "usb-port-reset-failed",
            Self::EnableSlotFailed => "usb-enable-slot-failed",
            Self::AddressDeviceFailed => "usb-address-device-failed",
            Self::ContextGeometry => "usb-context-geometry-unsupported",
            Self::DmaAddressInvalid => "usb-dma-address-invalid",
            Self::TransferRingFull => "usb-control-ring-full",
            Self::ControlStall => "usb-control-stall",
            Self::ControlError => "usb-control-error",
            Self::ControlTimeout => "usb-control-timeout",
            Self::WrongController => "usb-completion-wrong-controller",
            Self::WrongSlot => "usb-completion-wrong-slot",
            Self::WrongEndpoint => "usb-completion-wrong-endpoint",
            Self::DeviceVanished => "usb-device-vanished",
            Self::MalformedDescriptor => "usb-malformed-descriptor",
            Self::OversizedConfiguration => "usb-configuration-oversized",
            Self::TooManyInterfaces => "usb-too-many-interfaces",
            Self::TooManyEndpoints => "usb-too-many-endpoints",
            Self::TooManyDescriptorRecords => "usb-too-many-descriptor-records",
            Self::UnsupportedTopology => "usb-unsupported-topology",
            Self::StaleDeviceInstance => "usb-stale-device-instance",
        }
    }
}

impl From<XhciError> for UsbError {
    fn from(error: XhciError) -> Self {
        match error {
            XhciError::CommandTimeout => Self::ControlTimeout,
            _ => Self::ControlError,
        }
    }
}
