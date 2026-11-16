//! Synopsys DWC2 host-controller register facts, independent of USB classes.
pub(super) const AHB: usize = 0x008;
pub(super) const USB: usize = 0x00c;
pub(super) const RESET: usize = 0x010;
pub(super) const INTERRUPT_STATUS: usize = 0x014;
pub(super) const INTERRUPT_MASK: usize = 0x018;
pub(super) const RX_FIFO_SIZE: usize = 0x024;
pub(super) const NONPERIODIC_FIFO_SIZE: usize = 0x028;
pub(super) const CORE_ID: usize = 0x040;
pub(super) const HARDWARE_2: usize = 0x048;
pub(super) const HARDWARE_3: usize = 0x04c;
pub(super) const PERIODIC_FIFO_SIZE: usize = 0x100;
pub(super) const HOST_CONFIG: usize = 0x400;
pub(super) const HOST_INTERRUPT_MASK: usize = 0x418;
pub(super) const PORT: usize = 0x440;
pub(super) const CLOCK_GATE: usize = 0xe00;
pub(super) const AHB_IDLE: u32 = 1 << 31;
pub(super) const SOFT_RESET: u32 = 1;
pub(super) const CONNECTED: u32 = 1;
pub(super) const ENABLED: u32 = 1 << 2;
pub(super) const OVERCURRENT: u32 = 1 << 4;
pub(super) const PORT_RESET: u32 = 1 << 8;
pub(super) const PORT_POWER: u32 = 1 << 12;
// ENA must be written zero to preserve enable; change bits acknowledge on one.
pub(super) const PORT_WRITE_CLEAR: u32 = (1 << 1) | ENABLED | (1 << 3) | (1 << 5);
pub(super) const fn port_write(observed: u32) -> u32 {
    observed & !PORT_WRITE_CLEAR
}
