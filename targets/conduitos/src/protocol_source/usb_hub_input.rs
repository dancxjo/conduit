//! Shared bounded hub enumeration policy; Root separately owns every effect.
use super::{
    ProtocolSourcePackage, ProtocolSourceRefusal, ProtocolSpecializationRequest as R,
    ProtocolValueReference,
};
pub fn usb_hub_input_package() -> Result<ProtocolSourcePackage, ProtocolSourceRefusal> {
    let value = |name: &str, maximum_bytes| ProtocolValueReference {
        type_name: name.into(),
        maximum_bytes,
    };
    ProtocolSourcePackage::compile(
        [
            include_str!("../../plots/usb/control.conduit"),
            include_str!("../../plots/usb/enumeration-descriptors.conduit"),
            include_str!("../../plots/usb/configuration-descriptors.conduit"),
            include_str!("../../plots/usb/configuration-walk.conduit"),
            include_str!("../../plots/usb/hub-port-status.conduit"),
            include_str!("../../plots/usb/hid-input-profile.conduit"),
            include_str!("../../plots/usb/hid-absolute-layout-types.conduit"),
            include_str!("../../plots/usb/hid-absolute-layout.conduit"),
            include_str!("../../plots/usb/hub-input-types.conduit"),
            include_str!("../../plots/usb/hub-input-transition.conduit"),
            include_str!("../../plots/usb/hub-input-routing.conduit"),
            include_str!("../../plots/usb/hub-input-lifecycle.conduit"),
        ]
        .join("\n"),
        &[
            R::SeededUntil {
                value: value("UsbAbsoluteHidLayoutState", 4096),
            },
            R::SeededUntilInstance {
                alias: "UsbHubInputCell".into(),
                value: value("UsbHubInputState", 4096),
            },
            R::FeedbackZipInstance {
                alias: "UsbHubControlJoin".into(),
                paired_alias: "UsbHubControlPair".into(),
                left: value("UsbHubInputState", 4096),
                right: value("UsbControlResult", 4096),
            },
            R::FeedbackZipInstance {
                alias: "UsbHubDeviceJoin".into(),
                paired_alias: "UsbHubDevicePair".into(),
                left: value("UsbHubInputState", 4096),
                right: value("UsbDeviceInitialDescriptorResult", 4096),
            },
            R::FeedbackZipInstance {
                alias: "UsbHubHubJoin".into(),
                paired_alias: "UsbHubHubPair".into(),
                left: value("UsbHubInputState", 4096),
                right: value("UsbHubDescriptorResult", 4096),
            },
            R::FeedbackZipInstance {
                alias: "UsbHubPrefixJoin".into(),
                paired_alias: "UsbHubPrefixPair".into(),
                left: value("UsbHubInputState", 4096),
                right: value("UsbConfigurationPrefixResult", 4096),
            },
            R::FeedbackZipInstance {
                alias: "UsbHubTopologyJoin".into(),
                paired_alias: "UsbHubTopologyPair".into(),
                left: value("UsbHubInputState", 4096),
                right: value("UsbConfigurationTopologyResult", 4096),
            },
            R::FeedbackZipInstance {
                alias: "UsbHubStatusJoin".into(),
                paired_alias: "UsbHubStatusPair".into(),
                left: value("UsbHubInputState", 4096),
                right: value("UsbHubPortStatusResult", 4096),
            },
            R::FeedbackZipInstance {
                alias: "UsbHubProfileJoin".into(),
                paired_alias: "UsbHubProfilePair".into(),
                left: value("UsbHubInputState", 4096),
                right: value("UsbHidInputProfileResult", 4096),
            },
            R::FeedbackZipInstance {
                alias: "UsbHubLayoutJoin".into(),
                paired_alias: "UsbHubLayoutPair".into(),
                left: value("UsbHubInputState", 4096),
                right: value("UsbAbsoluteHidLayoutResult", 4096),
            },
            R::FeedbackZipInstance {
                alias: "UsbHubWaitJoin".into(),
                paired_alias: "UsbHubWaitPair".into(),
                left: value("UsbHubInputState", 4096),
                right: value("UsbHubWaitObservation", 4096),
            },
            R::MergeInstance {
                alias: "UsbHubStateMerge".into(),
                value: value("UsbHubInputState", 4096),
            },
        ],
    )
}
