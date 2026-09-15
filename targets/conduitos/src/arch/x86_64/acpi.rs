//! Bounded ACPI discovery for x86 interrupt-controller topology.

const RSDP_V2_BYTES: usize = 36;
const SDT_HEADER_BYTES: usize = 36;
const MADT_FIXED_BYTES: usize = 44;
const MAX_SDT_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InterruptTopology {
    pub local_apic_address: u64,
    pub local_apic_id: u8,
    pub io_apic_address: u64,
    pub io_apic_gsi_base: u32,
    pub timer_gsi: u32,
    pub timer_flags: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcpiRefusal {
    Missing,
    Address,
    Signature,
    Length,
    Checksum,
    UnsupportedRoot,
    MissingMadt,
    MissingEnabledProcessor,
    MissingIoApic,
    UnsupportedTimerBus,
}

impl AcpiRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "acpi-rsdp-missing",
            Self::Address => "acpi-address-invalid",
            Self::Signature => "acpi-signature-invalid",
            Self::Length => "acpi-length-invalid",
            Self::Checksum => "acpi-checksum-invalid",
            Self::UnsupportedRoot => "acpi-root-unsupported",
            Self::MissingMadt => "acpi-madt-missing",
            Self::MissingEnabledProcessor => "acpi-processor-missing",
            Self::MissingIoApic => "acpi-ioapic-missing",
            Self::UnsupportedTimerBus => "acpi-timer-bus-unsupported",
        }
    }
}

pub fn discover(
    hhdm: u64,
    rsdp_address: Option<u64>,
    image_virtual_to_physical: fn(u64) -> Option<u64>,
) -> Result<InterruptTopology, AcpiRefusal> {
    let supplied = rsdp_address.ok_or(AcpiRefusal::Missing)?;
    let rsdp_address = if supplied >> 63 == 0 {
        super::mmio::map(supplied, hhdm, image_virtual_to_physical)
            .map_err(|_| AcpiRefusal::Address)? as u64
    } else {
        supplied
    };
    let rsdp = virtual_bytes(rsdp_address, 20)?;
    if &rsdp[..8] != b"RSD PTR " {
        return Err(AcpiRefusal::Signature);
    }
    checksum(&rsdp[..20])?;
    let (root, entry_bytes) = if rsdp[15] >= 2 {
        let extended = virtual_bytes(rsdp_address, RSDP_V2_BYTES)?;
        let length = usize::try_from(read_u32(extended, 20)?).map_err(|_| AcpiRefusal::Length)?;
        if length != RSDP_V2_BYTES {
            return Err(AcpiRefusal::UnsupportedRoot);
        }
        checksum(extended)?;
        (
            physical_sdt(hhdm, read_u64(extended, 24)?, image_virtual_to_physical)?,
            8,
        )
    } else {
        (
            physical_sdt(
                hhdm,
                u64::from(read_u32(rsdp, 16)?),
                image_virtual_to_physical,
            )?,
            4,
        )
    };
    let expected = if entry_bytes == 8 { b"XSDT" } else { b"RSDT" };
    if &root[..4] != expected || !(root.len() - SDT_HEADER_BYTES).is_multiple_of(entry_bytes) {
        return Err(AcpiRefusal::UnsupportedRoot);
    }
    for entry in root[SDT_HEADER_BYTES..].chunks_exact(entry_bytes) {
        let physical = if entry_bytes == 8 {
            read_u64(entry, 0)?
        } else {
            u64::from(read_u32(entry, 0)?)
        };
        let table = physical_sdt(hhdm, physical, image_virtual_to_physical)?;
        if &table[..4] == b"APIC" {
            return parse_madt(table);
        }
    }
    Err(AcpiRefusal::MissingMadt)
}

fn parse_madt(table: &[u8]) -> Result<InterruptTopology, AcpiRefusal> {
    if table.len() < MADT_FIXED_BYTES {
        return Err(AcpiRefusal::Length);
    }
    let mut local_apic_address = u64::from(read_u32(table, 36)?);
    let mut local_apic_id = None;
    let mut io_apic = None;
    let mut timer_override = None;
    let mut offset = MADT_FIXED_BYTES;
    while offset < table.len() {
        let header = table.get(offset..offset + 2).ok_or(AcpiRefusal::Length)?;
        let length = usize::from(header[1]);
        if length < 2 {
            return Err(AcpiRefusal::Length);
        }
        let entry = table
            .get(offset..offset.checked_add(length).ok_or(AcpiRefusal::Length)?)
            .ok_or(AcpiRefusal::Length)?;
        match (entry[0], length) {
            (0, 8) if read_u32(entry, 4)? & 1 != 0 && local_apic_id.is_none() => {
                local_apic_id = Some(entry[3]);
            }
            (1, 12) if io_apic.is_none() => {
                io_apic = Some((u64::from(read_u32(entry, 4)?), read_u32(entry, 8)?));
            }
            (2, 10) if entry[2] == 0 && entry[3] == 0 => {
                timer_override = Some((read_u32(entry, 4)?, read_u16(entry, 8)?));
            }
            (5, 12) => local_apic_address = read_u64(entry, 4)?,
            _ => {}
        }
        offset += length;
    }
    let (io_apic_address, io_apic_gsi_base) = io_apic.ok_or(AcpiRefusal::MissingIoApic)?;
    let (timer_gsi, timer_flags) = timer_override.unwrap_or((0, 0));
    if timer_gsi < io_apic_gsi_base {
        return Err(AcpiRefusal::UnsupportedTimerBus);
    }
    Ok(InterruptTopology {
        local_apic_address,
        local_apic_id: local_apic_id.ok_or(AcpiRefusal::MissingEnabledProcessor)?,
        io_apic_address,
        io_apic_gsi_base,
        timer_gsi,
        timer_flags,
    })
}

fn physical_sdt(
    hhdm: u64,
    physical: u64,
    image_virtual_to_physical: fn(u64) -> Option<u64>,
) -> Result<&'static [u8], AcpiRefusal> {
    let address = super::mmio::map(physical, hhdm, image_virtual_to_physical)
        .map_err(|_| AcpiRefusal::Address)? as u64;
    let header = virtual_bytes(address, SDT_HEADER_BYTES)?;
    let length = usize::try_from(read_u32(header, 4)?).map_err(|_| AcpiRefusal::Length)?;
    if !(SDT_HEADER_BYTES..=MAX_SDT_BYTES).contains(&length) {
        return Err(AcpiRefusal::Length);
    }
    let table = virtual_bytes(address, length)?;
    checksum(table)?;
    Ok(table)
}

fn virtual_bytes(address: u64, length: usize) -> Result<&'static [u8], AcpiRefusal> {
    if address == 0 || address.checked_add(length as u64).is_none() {
        return Err(AcpiRefusal::Address);
    }
    Ok(unsafe { core::slice::from_raw_parts(address as *const u8, length) })
}

fn checksum(bytes: &[u8]) -> Result<(), AcpiRefusal> {
    (bytes.iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte)) == 0)
        .then_some(())
        .ok_or(AcpiRefusal::Checksum)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, AcpiRefusal> {
    let value: [u8; 2] = bytes
        .get(offset..offset + 2)
        .ok_or(AcpiRefusal::Length)?
        .try_into()
        .map_err(|_| AcpiRefusal::Length)?;
    Ok(u16::from_le_bytes(value))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, AcpiRefusal> {
    let value: [u8; 4] = bytes
        .get(offset..offset + 4)
        .ok_or(AcpiRefusal::Length)?
        .try_into()
        .map_err(|_| AcpiRefusal::Length)?;
    Ok(u32::from_le_bytes(value))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, AcpiRefusal> {
    let value: [u8; 8] = bytes
        .get(offset..offset + 8)
        .ok_or(AcpiRefusal::Length)?
        .try_into()
        .map_err(|_| AcpiRefusal::Length)?;
    Ok(u64::from_le_bytes(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn madt_discovers_enabled_processor_ioapic_and_timer_override() {
        let mut table = [0_u8; 86];
        let table_length = table.len() as u32;
        table[..4].copy_from_slice(b"APIC");
        table[4..8].copy_from_slice(&table_length.to_le_bytes());
        table[36..40].copy_from_slice(&0xfee0_0000_u32.to_le_bytes());
        table[44..52].copy_from_slice(&[0, 8, 0, 7, 1, 0, 0, 0]);
        table[52..64].copy_from_slice(&[1, 12, 2, 0, 0, 0, 0xc0, 0xfe, 0, 0, 0, 0]);
        table[64..74].copy_from_slice(&[2, 10, 0, 0, 2, 0, 0, 0, 0, 0]);
        table[74..86].copy_from_slice(&[5, 12, 0, 0, 0, 0, 0xe0, 0xfe, 0, 0, 0, 0]);
        assert_eq!(
            parse_madt(&table),
            Ok(InterruptTopology {
                local_apic_address: 0xfee0_0000,
                local_apic_id: 7,
                io_apic_address: 0xfec0_0000,
                io_apic_gsi_base: 0,
                timer_gsi: 2,
                timer_flags: 0,
            })
        );
    }

    #[test]
    fn malformed_madt_entry_is_refused() {
        let mut table = [0_u8; MADT_FIXED_BYTES + 2];
        table[44..].copy_from_slice(&[1, 1]);
        assert_eq!(parse_madt(&table), Err(AcpiRefusal::Length));
    }
}
