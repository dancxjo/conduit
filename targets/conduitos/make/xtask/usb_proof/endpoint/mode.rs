//! Explicit appliance selection and finite emulator input, never device authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProofMode {
    Raw,
    Keyboard,
    Mouse,
}

impl ProofMode {
    pub(super) fn entry(self) -> Option<&'static str> {
        match self {
            Self::Raw => None,
            Self::Keyboard => Some("usb-hid-keyboard-capture-window"),
            Self::Mouse => Some("usb-hid-mouse-endpoint"),
        }
    }
    pub(super) fn qemu_profile(self) -> &'static str {
        match self {
            Self::Raw => conduitos::make::USB_ENDPOINT_QEMU_PROFILE,
            Self::Keyboard => conduitos::make::USB_HID_ENDPOINT_QEMU_PROFILE,
            Self::Mouse => conduitos::make::USB_HID_MOUSE_QEMU_PROFILE,
        }
    }
    pub(super) fn arena_bytes(self) -> u64 {
        match self {
            Self::Raw => 16 * 1024 * 1024,
            Self::Keyboard => conduitos::make::USB_HID_ENDPOINT_ARENA_BYTES,
            Self::Mouse => conduitos::make::USB_HID_MOUSE_ARENA_BYTES,
        }
    }
    pub(super) fn qemu_memory(self) -> &'static str {
        match self {
            Self::Keyboard => "256M",
            Self::Raw | Self::Mouse => "64M",
        }
    }
    pub(super) fn serial_name(self) -> &'static str {
        match self {
            Self::Raw => "usb-endpoint-serial.log",
            Self::Keyboard => "usb-hid-endpoint-serial.log",
            Self::Mouse => "usb-hid-mouse-serial.log",
        }
    }
    pub(super) fn receipt_name(self) -> &'static str {
        match self {
            Self::Raw => "usb-endpoint-read-proof.json",
            Self::Keyboard => "usb-hid-endpoint-proof.json",
            Self::Mouse => "usb-hid-mouse-proof.json",
        }
    }
    pub(super) fn device(self) -> &'static str {
        match self {
            Self::Raw | Self::Keyboard => "usb-kbd,bus=conduitos-xhci.0,port=1",
            Self::Mouse => "usb-mouse,bus=conduitos-xhci.0,port=1",
        }
    }
    pub(super) fn input_command(self, sequence: u64) -> String {
        let down = sequence.is_multiple_of(2);
        let events = match self {
            Self::Raw | Self::Keyboard => serde_json::json!([
                {"type":"key", "data":{"down":down,"key":{"type":"qcode","data":"a"}}}
            ]),
            Self::Mouse => serde_json::json!([
                {"type":"btn", "data":{"button":"left","down":down}},
                {"type":"rel", "data":{"axis":"x","value":if down {1} else {-1}}},
                {"type":"rel", "data":{"axis":"y","value":if down {2} else {-2}}}
            ]),
        };
        serde_json::json!({"execute":"input-send-event", "arguments":{"events":events}}).to_string()
    }
}
