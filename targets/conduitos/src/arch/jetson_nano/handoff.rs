//! Strict Nano memory extraction from an already validated DTB. Unsupported
//! address-cell layouts or dynamic reservations refuse the boot, rather than
//! silently turning firmware-owned memory into writable RAM.
use super::{
    dtb::{Dtb, Token},
    memory::{Region, Regions},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Dtb,
    Board,
    Cells,
    Property,
    Reservation,
    Memory,
}

pub struct Memory {
    pub ram: Regions,
    pub reserved: Regions,
}

#[derive(Default)]
struct Node<'a> {
    reg: Option<&'a [u8]>,
    memory_type: bool,
    seen: u16,
}

fn once(seen: &mut u16, bit: u16) -> Result<(), Error> {
    if *seen & bit != 0 {
        return Err(Error::Property);
    }
    *seen |= bit;
    Ok(())
}

fn regions(value: &[u8], output: &mut Regions, disjoint: bool) -> Result<(), Error> {
    if value.is_empty() || value.len() % 16 != 0 {
        return Err(Error::Memory);
    }
    for pair in value.chunks_exact(16) {
        let start = u64::from_be_bytes(pair[..8].try_into().map_err(|_| Error::Memory)?);
        let size = u64::from_be_bytes(pair[8..].try_into().map_err(|_| Error::Memory)?);
        output
            .push(
                Region::new(start, size).map_err(|_| Error::Memory)?,
                disjoint,
            )
            .map_err(|_| Error::Memory)?;
    }
    Ok(())
}

pub fn memory(dtb: &Dtb<'_>) -> Result<Memory, Error> {
    let mut output = Memory {
        ram: Regions::new(),
        reserved: Regions::new(),
    };
    for reservation in dtb.reservations.iter() {
        output
            .reserved
            .push(reservation, false)
            .map_err(|_| Error::Reservation)?;
    }
    let mut tokens = dtb.tokens();
    let mut depth = 0;
    let mut root_seen = 0;
    let mut reserve_seen = 0;
    let mut reserve_present = false;
    let mut in_reserved = false;
    let mut in_memory = false;
    let mut node = Node::default();
    while let Some(token) = tokens.next().map_err(|_| Error::Dtb)? {
        match token {
            Token::Begin(name) => {
                depth += 1;
                if depth == 2 {
                    in_memory = name == b"memory" || name.starts_with(b"memory@");
                    in_reserved = name == b"reserved-memory";
                    if in_reserved {
                        if reserve_present {
                            return Err(Error::Reservation);
                        }
                        reserve_present = true;
                    }
                    node = Node::default();
                } else if in_reserved && depth == 3 {
                    // Parent properties precede all children in a validated DTB.
                    if reserve_seen != 7 {
                        return Err(Error::Cells);
                    }
                    node = Node::default();
                } else if (in_reserved && depth > 3) || (in_memory && depth > 2) {
                    return Err(Error::Reservation);
                }
            }
            Token::Property { name, value } if depth == 1 => match name {
                b"#address-cells" | b"#size-cells" => {
                    once(
                        &mut root_seen,
                        if name == b"#address-cells" { 1 } else { 2 },
                    )?;
                    if value != 2u32.to_be_bytes() {
                        return Err(Error::Cells);
                    }
                }
                b"compatible" => {
                    once(&mut root_seen, 4)?;
                    if value.last() != Some(&0) {
                        return Err(Error::Board);
                    }
                    let mut tegra = false;
                    let mut nano = false;
                    for entry in value[..value.len() - 1].split(|byte| *byte == 0) {
                        tegra |= entry == b"nvidia,tegra210";
                        nano |= entry == b"nvidia,p3450-0000" || entry == b"nvidia,jetson-nano";
                    }
                    if !tegra || !nano {
                        return Err(Error::Board);
                    }
                }
                _ => {}
            },
            Token::Property { name, value } if in_reserved && depth == 2 => match name {
                b"#address-cells" | b"#size-cells" => {
                    once(
                        &mut reserve_seen,
                        if name == b"#address-cells" { 1 } else { 2 },
                    )?;
                    if value != 2u32.to_be_bytes() {
                        return Err(Error::Cells);
                    }
                }
                b"ranges" => {
                    once(&mut reserve_seen, 4)?;
                    if !value.is_empty() {
                        return Err(Error::Reservation);
                    }
                }
                _ => {}
            },
            Token::Property { name, value }
                if (in_memory && depth == 2) || (in_reserved && depth == 3) =>
            {
                match name {
                    b"reg" => {
                        once(&mut node.seen, 1)?;
                        node.reg = Some(value);
                    }
                    b"device_type" => {
                        once(&mut node.seen, 2)?;
                        if value != b"memory\0" || !in_memory {
                            return Err(Error::Property);
                        }
                        node.memory_type = true;
                    }
                    b"status" => {
                        once(&mut node.seen, 4)?;
                        if value != b"okay\0" && value != b"ok\0" {
                            return Err(Error::Property);
                        }
                    }
                    // Dynamic carveouts need firmware allocation before our entry.
                    b"size" | b"alignment" | b"alloc-ranges" if in_reserved => {
                        return Err(Error::Reservation);
                    }
                    _ => {}
                }
            }
            Token::End => {
                if in_memory && depth == 2 {
                    if !node.memory_type {
                        return Err(Error::Memory);
                    }
                    regions(node.reg.ok_or(Error::Memory)?, &mut output.ram, true)?;
                } else if in_reserved && depth == 3 {
                    regions(
                        node.reg.ok_or(Error::Reservation)?,
                        &mut output.reserved,
                        false,
                    )?;
                }
                if depth == 2 {
                    in_memory = false;
                    in_reserved = false;
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    if root_seen != 7 || (reserve_present && reserve_seen != 7) {
        return Err(Error::Cells);
    }
    if output.ram.iter().next().is_none() {
        return Err(Error::Memory);
    }
    Ok(output)
}
