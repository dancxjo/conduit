//! Bounded immutable make truth linked into the freestanding artifact.

pub const MAKE_SCHEMA: &str = "conduit.conduitos/make-record@1";
pub const MAX_ID_BYTES: usize = 160;
/// Emulator RAM for the canonical normal PC product preparation budget.
pub const PC_PRODUCT_QEMU_MEMORY: &str = "512M";
pub const PC_PRODUCT_QEMU_PROFILE: &str = "q35-single-cpu-512m-normal-product";
pub const IMPL_TIME_TICK: u16 = 1 << 0;
pub const IMPL_TICK_FACE: u16 = 1 << 1;
pub const IMPL_TEXT_LITERAL: u16 = 1 << 2;
pub const IMPL_TEXT_UPPER: u16 = 1 << 3;
pub const IMPL_TEXT_FACE: u16 = 1 << 4;
pub const IMPL_KEYBOARD: u16 = 1 << 5;
pub const IMPL_PC_SPEAKER: u16 = 1 << 6;
pub const IMPL_OPL2: u16 = 1 << 7;
pub const IMPL_NATIVE_PRESENTER: u16 = 1 << 8;
pub const IMPL_LINEAR_PRESENTER: u16 = 1 << 9;
pub const IMPL_HTTP_CLIENT: u16 = 1 << 10;
pub const IMPL_POINTER: u16 = 1 << 11;
pub const IMPL_I2C_TRANSACTION: u16 = 1 << 12;
pub const IMPL_MONOTONIC_DEADLINE: u16 = 1 << 13;
/// Compiled class-neutral USB control and endpoint-read transport machinery.
/// This inventory does not establish class, isochronous or physical acceptance.
pub const IMPL_USB_TRANSPORT: u16 = 1 << 14;
pub const IMPL_STARTUP_CHIME: u16 = 1 << 15;
pub const ALL_KNOWN_IMPLEMENTATIONS: u16 = u16::MAX;
pub const BASE_PCM_AUDIO: u16 = 1 << 3;
pub const DRIVER_PWM_AUDIO: u16 = 1 << 8;
pub const BASE_USB_CONTROLLER: u16 = 1 << 4;
pub const DRIVER_DWC2_USB: u16 = 1 << 9;
pub const FACILITY_NATIVE_COMPOSITOR: u16 = 1;
pub const FACILITY_HTTP_CLIENT: u16 = 1 << 1;
pub const RESOURCE_FACE_SURFACE: u16 = 1;
pub const RESOURCE_HTTP_CLIENT: u16 = 1 << 1;
pub const BASE_DISPLAY_SCANOUT: u16 = 1;
pub const BASE_HTTP_NETWORK: u16 = 1 << 2;
pub const DRIVER_LINEAR_FRAMEBUFFER: u16 = 1;
pub const DRIVER_HTTP_NETWORK: u16 = 1 << 2;
pub const DRIVER_DW_APB_UART2: u16 = 1 << 3;
pub const DRIVER_TEGRA210_UART: u16 = 1 << 7;
pub const DRIVER_IA32_DEBUGCON_SERIAL: u16 = 1 << 4;
pub const DRIVER_RISCV64_SBI_CONSOLE: u16 = 1 << 5;
pub const DRIVER_LOONGARCH64_UART: u16 = 1 << 6;
pub const PRESENTER_NATIVE_GRAPHICAL: u16 = 1;
pub const BASE_SERIAL_TEXT: u16 = 1 << 1;
pub const DRIVER_PL011_SERIAL: u16 = 1 << 1;
pub const PRESENTER_LINEAR_SERIAL: u16 = 1 << 1;
pub const PROOF_HOTPLUG: u16 = 1 << 0;
pub const PROOF_SCRIPTED_KEYBOARD: u16 = 1 << 1;
pub const PROOF_USB_CONFIGURATION: u16 = 1 << 2;
pub const PROOF_USB_ENDPOINT_READ: u16 = 1 << 3;
pub const USB_ENDPOINT_QEMU_PROFILE: &str =
    "q35-single-cpu-64m-headless-xhci-usb-kbd-endpoint-read";
/// Ordinary product/proof preparation retains the complete authored keymap registry.
pub const NATIVE_PREPARATION_ARENA_BYTES: u64 = 32 * 1024 * 1024;
pub const USB_HID_ENDPOINT_ARENA_BYTES: u64 = 128 * 1024 * 1024;
pub const USB_HID_MOUSE_ARENA_BYTES: u64 = 32 * 1024 * 1024;
pub const USB_HID_MOUSE_QEMU_PROFILE: &str =
    "q35-single-cpu-64m-headless-xhci-usb-mouse-hid-endpoint";
pub const USB_HID_ENDPOINT_QEMU_PROFILE: &str =
    "q35-single-cpu-256m-headless-xhci-usb-kbd-hid-capture-window";
pub const ALL_KNOWN_PROOF_INSTRUMENTATION: u16 =
    PROOF_HOTPLUG | PROOF_SCRIPTED_KEYBOARD | PROOF_USB_CONFIGURATION | PROOF_USB_ENDPOINT_READ;
pub const USB_CONFIGURATION_ARENA_BYTES: u64 = 256 * 1024 * 1024;
pub const USB_CONFIGURATION_QEMU_PROFILE: &str = "q35-single-cpu-512m-headless-xhci-usb-kbd-adlib";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MakeRecord {
    pub schema: &'static str,
    pub profile_id: &'static str,
    pub build_id: &'static str,
    /// Binding of the embedded resolved description. The whole-ISO ImageId is
    /// external because embedding its own digest would be circular.
    pub image_binding: &'static str,
    pub target: &'static str,
    pub implementations: u16,
    pub facilities: u16,
    pub resources: u16,
    pub bases: u16,
    pub drivers: u16,
    pub presenters: u16,
    pub proof_instrumentation: u16,
    pub face_surface_slots: u32,
    pub face_surface_bytes: u64,
    pub runtime_arena_ceiling: u64,
    pub operation_slot_ceiling: u32,
    pub timer_slot_ceiling: u32,
    pub evidence_item_ceiling: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MakeError {
    UnsupportedSchema,
    MalformedIdentity,
    WrongTarget,
    UnknownInventory,
    InvalidCeiling,
    RuntimeArenaExceeded,
}

impl MakeError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedSchema => "make-schema-unsupported",
            Self::MalformedIdentity => "make-identity-malformed",
            Self::WrongTarget => "make-target-mismatch",
            Self::UnknownInventory => "make-inventory-malformed",
            Self::InvalidCeiling => "make-ceiling-invalid",
            Self::RuntimeArenaExceeded => "make-runtime-arena-exceeded",
        }
    }
}

impl MakeRecord {
    pub const fn legacy() -> Self {
        Self {
            schema: "legacy-unbound",
            profile_id: "legacy-unbound",
            build_id: "legacy-unbound",
            image_binding: "legacy-unbound",
            target: "legacy-unbound",
            implementations: 0,
            facilities: 0,
            resources: 0,
            bases: 0,
            drivers: 0,
            presenters: 0,
            proof_instrumentation: 0,
            face_surface_slots: 0,
            face_surface_bytes: 0,
            runtime_arena_ceiling: 0,
            operation_slot_ceiling: 0,
            timer_slot_ceiling: 0,
            evidence_item_ceiling: 0,
        }
    }

    pub fn validate(&self, runtime_arena_bytes: u64) -> Result<(), MakeError> {
        if self.schema != MAKE_SCHEMA {
            return Err(MakeError::UnsupportedSchema);
        }
        if !valid_id(self.profile_id) || !valid_id(self.build_id) || !valid_id(self.image_binding) {
            return Err(MakeError::MalformedIdentity);
        }
        if !matches!(
            self.target,
            "conduitos/x86_64/pc"
                | "conduitos/ia32/pc"
                | "conduitos/aarch64/virt"
                | "conduitos/riscv64/virt"
                | "conduitos/loongarch64/virt"
                | "conduitos/aarch64/orange-pi-5-rk3588s"
                | "conduitos/aarch64/jetson-nano-tegra210"
                | "conduitos/armv6/raspberry-pi-model-b-plus-v1.2"
                | "conduitos/aarch64/raspberry-pi-3-model-b-plus-rev-1.3"
        ) {
            return Err(MakeError::WrongTarget);
        }
        if self.implementations == 0
            || (!matches!(
                self.target,
                "conduitos/x86_64/pc"
                    | "conduitos/aarch64/virt"
                    | "conduitos/armv6/raspberry-pi-model-b-plus-v1.2"
                    | "conduitos/aarch64/raspberry-pi-3-model-b-plus-rev-1.3"
            ) && self.includes(IMPL_USB_TRANSPORT))
            || self.facilities & !(FACILITY_NATIVE_COMPOSITOR | FACILITY_HTTP_CLIENT) != 0
            || self.resources & !(RESOURCE_FACE_SURFACE | RESOURCE_HTTP_CLIENT) != 0
            || self.bases
                & !(BASE_DISPLAY_SCANOUT
                    | BASE_SERIAL_TEXT
                    | BASE_HTTP_NETWORK
                    | BASE_PCM_AUDIO
                    | BASE_USB_CONTROLLER)
                != 0
            || self.drivers
                & !(DRIVER_LINEAR_FRAMEBUFFER
                    | DRIVER_PL011_SERIAL
                    | DRIVER_HTTP_NETWORK
                    | DRIVER_DW_APB_UART2
                    | DRIVER_TEGRA210_UART
                    | DRIVER_IA32_DEBUGCON_SERIAL
                    | DRIVER_RISCV64_SBI_CONSOLE
                    | DRIVER_LOONGARCH64_UART
                    | DRIVER_PWM_AUDIO
                    | DRIVER_DWC2_USB)
                != 0
            || self.presenters & !(PRESENTER_NATIVE_GRAPHICAL | PRESENTER_LINEAR_SERIAL) != 0
            || self.proof_instrumentation & !ALL_KNOWN_PROOF_INSTRUMENTATION != 0
        {
            return Err(MakeError::UnknownInventory);
        }
        let native = self.includes(IMPL_NATIVE_PRESENTER);
        let linear = self.includes(IMPL_LINEAR_PRESENTER);
        if self.includes_facility(FACILITY_NATIVE_COMPOSITOR) != native
            || (self.resources & RESOURCE_FACE_SURFACE != 0) != native
            || (self.bases & BASE_DISPLAY_SCANOUT != 0) != native
            || (self.drivers & DRIVER_LINEAR_FRAMEBUFFER != 0) != native
            || (self.presenters & PRESENTER_NATIVE_GRAPHICAL != 0) != native
            || (self.face_surface_slots != 0) != native
            || (self.face_surface_bytes != 0) != native
        {
            return Err(MakeError::UnknownInventory);
        }
        let expected_serial_driver = match self.target {
            "conduitos/ia32/pc" => DRIVER_IA32_DEBUGCON_SERIAL,
            "conduitos/aarch64/orange-pi-5-rk3588s" => DRIVER_DW_APB_UART2,
            "conduitos/aarch64/jetson-nano-tegra210" => DRIVER_TEGRA210_UART,
            "conduitos/riscv64/virt" => DRIVER_RISCV64_SBI_CONSOLE,
            "conduitos/loongarch64/virt" => DRIVER_LOONGARCH64_UART,
            _ => DRIVER_PL011_SERIAL,
        };
        let expected_serial_drivers = if linear { expected_serial_driver } else { 0 };
        if (self.bases & BASE_SERIAL_TEXT != 0) != linear
            || (self.drivers & expected_serial_driver != 0) != linear
            || self.drivers
                & (DRIVER_PL011_SERIAL
                    | DRIVER_DW_APB_UART2
                    | DRIVER_TEGRA210_UART
                    | DRIVER_IA32_DEBUGCON_SERIAL
                    | DRIVER_RISCV64_SBI_CONSOLE
                    | DRIVER_LOONGARCH64_UART)
                != expected_serial_drivers
            || (self.presenters & PRESENTER_LINEAR_SERIAL != 0) != linear
            || (matches!(self.target, "conduitos/aarch64/orange-pi-5-rk3588s")
                && (native || !linear))
            || (matches!(
                self.target,
                "conduitos/aarch64/virt"
                    | "conduitos/ia32/pc"
                    | "conduitos/riscv64/virt"
                    | "conduitos/loongarch64/virt"
            ) && !linear)
            || (self.target == "conduitos/x86_64/pc" && linear)
        {
            return Err(MakeError::UnknownInventory);
        }
        let raspberry_pi = matches!(
            self.target,
            "conduitos/armv6/raspberry-pi-model-b-plus-v1.2"
                | "conduitos/aarch64/raspberry-pi-3-model-b-plus-rev-1.3"
        );
        let usb = self.includes(IMPL_USB_TRANSPORT);
        if (raspberry_pi
            && ((self.bases & BASE_USB_CONTROLLER != 0) != usb
                || (self.drivers & DRIVER_DWC2_USB != 0) != usb))
            || (!raspberry_pi
                && (self.bases & BASE_USB_CONTROLLER != 0 || self.drivers & DRIVER_DWC2_USB != 0))
        {
            return Err(MakeError::UnknownInventory);
        }
        let audio = self.includes(IMPL_STARTUP_CHIME);
        if (self.bases & BASE_PCM_AUDIO != 0) != audio
            || (self.drivers & DRIVER_PWM_AUDIO != 0) != audio
        {
            return Err(MakeError::UnknownInventory);
        }
        let http = self.includes(IMPL_HTTP_CLIENT);
        if self.includes_facility(FACILITY_HTTP_CLIENT) != http
            || (self.resources & RESOURCE_HTTP_CLIENT != 0) != http
            || (self.bases & BASE_HTTP_NETWORK != 0) != http
            || (self.drivers & DRIVER_HTTP_NETWORK != 0) != http
        {
            return Err(MakeError::UnknownInventory);
        }
        if self.runtime_arena_ceiling == 0
            || self.operation_slot_ceiling == 0
            || self.timer_slot_ceiling == 0
            || self.evidence_item_ceiling == 0
        {
            return Err(MakeError::InvalidCeiling);
        }
        if runtime_arena_bytes > self.runtime_arena_ceiling {
            return Err(MakeError::RuntimeArenaExceeded);
        }
        Ok(())
    }

    pub const fn includes(&self, implementation: u16) -> bool {
        self.implementations & implementation == implementation
    }

    pub const fn includes_facility(&self, facility: u16) -> bool {
        self.facilities & facility == facility
    }
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ID_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'-' | b'_' | b'/' | b'.')
        })
}

include!(concat!(env!("OUT_DIR"), "/make_record.rs"));

#[cfg(test)]
#[path = "make/tests.rs"]
mod tests;
