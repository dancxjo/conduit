use super::*;
use core::cell::RefCell;
use std::{collections::BTreeMap, vec::Vec};

struct Config {
    registers: BTreeMap<usize, u32>,
    writes: Vec<(usize, u32)>,
    base: usize,
    mask: u64,
    reject_enable: bool,
}
impl Config {
    fn new(bar: u64) -> Self {
        let base = address(2, 0, 0).unwrap();
        Self {
            registers: BTreeMap::from([
                (base, 0x000d_1b36),
                (base + 4, 0xfeed_0000),
                (base + 8, XHCI_CLASS),
                (base + 0x10, bar as u32 | 4),
                (base + 0x14, (bar >> 32) as u32),
            ]),
            writes: Vec::new(),
            base,
            mask: 0xffff_ffff_ffff_c004,
            reject_enable: false,
        }
    }
    fn read(&self, address: usize) -> u32 {
        let value = self.registers.get(&address).copied().unwrap_or(u32::MAX);
        if value == u32::MAX && address == self.base + 0x10 {
            self.mask as u32
        } else if value == u32::MAX && address == self.base + 0x14 {
            (self.mask >> 32) as u32
        } else {
            value
        }
    }
    fn write(&mut self, address: usize, value: u32) {
        if !(self.reject_enable && address == self.base + 4 && value & 6 == 6) {
            self.registers.insert(address, value);
        }
        self.writes.push((address, value));
    }
}
fn run(config: &RefCell<Config>) -> Result<ControllerLocation, PciRefusal> {
    discover(
        |address| config.borrow().read(address),
        |address, value| config.borrow_mut().write(address, value),
    )
}

#[test]
fn ecam_coordinates_are_root_bus_bounded_and_never_alias_invalid_offsets() {
    assert_eq!(address(2, 3, 0x14), Some(0x3f01_3014));
    assert_eq!(address(31, 7, 4092), Some(0x3f0f_fffc));
    for (device, function, offset) in [(32, 0, 0), (0, 8, 0), (0, 0, 4096), (0, 0, 3)] {
        assert_eq!(address(device, function, offset), None);
    }
}

#[test]
fn firmware_assigned_bar_is_preserved_and_command_never_clears_observed_status() {
    let config = RefCell::new(Config::new(0x1200_0000));
    let location = run(&config).unwrap();
    assert_eq!(location.bar_physical, 0x1200_0000);
    assert_eq!(
        (location.bus, location.device, location.function),
        (0, 2, 0)
    );
    assert_eq!((location.vendor, location.device_id), (0x1b36, 0x000d));
    let config = config.borrow();
    assert!(
        config
            .writes
            .iter()
            .filter(|(address, _)| *address == config.base + 4)
            .all(|(_, value)| *value <= u16::MAX as u32)
    );
    assert_eq!(config.registers[&(config.base + 0x10)], 0x1200_0004);
    assert_eq!(config.registers[&(config.base + 4)] & 6, 6);
}

#[test]
fn unassigned_single_endpoint_receives_the_dedicated_low_window_lease() {
    let config = RefCell::new(Config::new(0));
    assert_eq!(run(&config).unwrap().bar_physical, WINDOW_START);
    assert_eq!(
        config.borrow().registers[&(config.borrow().base + 0x10)],
        0x1000_0004
    );
}

#[test]
fn unassigned_bar_with_another_endpoint_refuses_without_leaving_probe_writes() {
    let mut fixture = Config::new(0);
    let other = address(3, 0, 0).unwrap();
    fixture.registers.insert(other, 0x1234_8086);
    fixture.registers.insert(other + 8, 0x0200_0000);
    let base = fixture.base;
    let config = RefCell::new(fixture);
    assert!(matches!(run(&config), Err(PciRefusal::AllocationConflict)));
    let config = config.borrow();
    assert_eq!(config.registers[&(base + 0x10)], 4);
    assert_eq!(config.registers[&(base + 0x14)], 0);
    assert_eq!(config.registers[&(base + 4)], 0);
}

#[test]
fn ambiguous_or_foreign_controller_is_refused_before_configuration_effects() {
    for foreign in [false, true] {
        let mut fixture = Config::new(0x1000_0000);
        let other = address(3, 0, 0).unwrap();
        fixture
            .registers
            .insert(other, if foreign { 0x1234_8086 } else { 0x000d_1b36 });
        fixture.registers.insert(other + 8, XHCI_CLASS);
        let config = RefCell::new(fixture);
        assert!(matches!(
            run(&config),
            Err(PciRefusal::MultipleControllers | PciRefusal::WrongController)
        ));
        assert!(config.borrow().writes.is_empty());
    }
}

#[test]
fn excessive_bar_extent_and_highmem_bar_are_refused_after_restoring_probe() {
    for (bar, mask, refusal) in [
        (0x1000_0000, 0xffff_ffff_ffc0_0004, PciRefusal::BarExtent),
        (0x1_1000_0000, 0xffff_ffff_ffff_c004, PciRefusal::InvalidBar),
    ] {
        let mut fixture = Config::new(bar);
        fixture.mask = mask;
        let base = fixture.base;
        let config = RefCell::new(fixture);
        assert!(matches!(run(&config), Err(error) if error == refusal));
        assert_eq!(config.borrow().registers[&(base + 0x10)], bar as u32 | 4);
        assert_eq!(
            config.borrow().registers[&(base + 0x14)],
            (bar >> 32) as u32
        );
    }
}

#[test]
fn low_window_geometry_does_not_admit_cross_block_or_misaligned_extent() {
    assert_eq!(
        validate_bar(0x1000_1000, 16384),
        Err(PciRefusal::InvalidBar)
    );
    assert_eq!(validate_bar(0x3eff_0000, 4096), Err(PciRefusal::InvalidBar));
    assert_eq!(
        validate_bar(u64::MAX - 4095, 8192),
        Err(PciRefusal::InvalidBar)
    );
    assert_eq!(validate_bar(0x1000_0000, 16384), Ok(()));
}

#[test]
fn rejected_enable_restores_the_original_unassigned_bar_and_command() {
    let mut fixture = Config::new(0);
    fixture.reject_enable = true;
    let base = fixture.base;
    let config = RefCell::new(fixture);
    assert!(matches!(
        run(&config),
        Err(PciRefusal::ConfigurationRejected)
    ));
    let config = config.borrow();
    assert_eq!(config.registers[&(base + 0x10)], 4);
    assert_eq!(config.registers[&(base + 0x14)], 0);
    assert_eq!(config.registers[&(base + 4)], 0);
}

#[test]
fn native_refusals_are_distinct_machine_readable_dispositions() {
    let refusals = [
        PciRefusal::Absent,
        PciRefusal::WrongController,
        PciRefusal::MultipleControllers,
        PciRefusal::RootTopology,
        PciRefusal::InvalidBar,
        PciRefusal::BarExtent,
        PciRefusal::AllocationConflict,
        PciRefusal::ConfigurationRejected,
    ];
    for (index, refusal) in refusals.iter().enumerate() {
        assert!(refusal.as_str().starts_with("aarch64-virt-xhci-"));
        assert!(
            refusals[..index]
                .iter()
                .all(|prior| prior.as_str() != refusal.as_str())
        );
    }
}

#[test]
fn downstream_bridge_cannot_hide_bar_ownership_outside_the_root_bus_scan() {
    let mut fixture = Config::new(0);
    let bridge = address(3, 0, 0).unwrap();
    fixture.registers.insert(bridge, 0x1234_8086);
    fixture.registers.insert(bridge + 8, 0x0604_0000);
    let config = RefCell::new(fixture);
    assert!(matches!(run(&config), Err(PciRefusal::RootTopology)));
    assert!(config.borrow().writes.is_empty());
}
