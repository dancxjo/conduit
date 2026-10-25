//! Bounded DTB envelope and token validation. No physical pointer is dereferenced
//! here: the boot entry must first provide a readable, bounded byte slice.
//! Format: https://devicetree-specification.readthedocs.io/en/stable/flattened-format.html

use super::memory::{Region, Regions};

const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_DEPTH: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Bounds,
    Header,
    Version,
    Alignment,
    Reservation,
    Structure,
    Depth,
}

pub struct Dtb<'a> {
    pub structure: &'a [u8],
    pub strings: &'a [u8],
    pub reservations: Regions,
    pub total_bytes: usize,
}

fn word(bytes: &[u8], offset: usize) -> Result<u32, Error> {
    let end = offset.checked_add(4).ok_or(Error::Bounds)?;
    Ok(u32::from_be_bytes(
        bytes
            .get(offset..end)
            .ok_or(Error::Bounds)?
            .try_into()
            .map_err(|_| Error::Bounds)?,
    ))
}

fn block(bytes: &[u8], offset: usize, length: usize) -> Result<&[u8], Error> {
    bytes
        .get(offset..offset.checked_add(length).ok_or(Error::Bounds)?)
        .ok_or(Error::Bounds)
}

impl<'a> Dtb<'a> {
    pub fn parse(bytes: &'a [u8], physical_address: u64) -> Result<Self, Error> {
        if physical_address & 7 != 0 {
            return Err(Error::Alignment);
        }
        if word(bytes, 0)? != 0xd00d_feed {
            return Err(Error::Header);
        }
        let total_bytes = word(bytes, 4)? as usize;
        if !(40..=MAX_BYTES).contains(&total_bytes) {
            return Err(Error::Bounds);
        }
        let bytes = block(bytes, 0, total_bytes)?;
        let version = word(bytes, 20)?;
        let last = word(bytes, 24)?;
        if version < 17 || last > 17 || last > version {
            return Err(Error::Version);
        }
        let structure_offset = word(bytes, 8)? as usize;
        let strings_offset = word(bytes, 12)? as usize;
        let reservation_offset = word(bytes, 16)? as usize;
        if structure_offset & 3 != 0 || reservation_offset & 7 != 0 {
            return Err(Error::Alignment);
        }
        let structure_size = word(bytes, 36)? as usize;
        let strings_size = word(bytes, 32)? as usize;
        // Require the specified canonical order; no section can alias the header
        // or reservation terminator, even if every individual offset is in bounds.
        if reservation_offset < 40
            || structure_offset < reservation_offset
            || strings_offset
                < structure_offset
                    .checked_add(structure_size)
                    .ok_or(Error::Bounds)?
        {
            return Err(Error::Header);
        }
        let structure = block(bytes, structure_offset, structure_size)?;
        let strings = block(bytes, strings_offset, strings_size)?;
        let mut reservations = Regions::new();
        let mut cursor = reservation_offset;
        loop {
            let entry = block(bytes, cursor, 16)?;
            cursor = cursor.checked_add(16).ok_or(Error::Bounds)?;
            if cursor > structure_offset {
                return Err(Error::Reservation);
            }
            let address = u64::from_be_bytes(entry[..8].try_into().map_err(|_| Error::Bounds)?);
            let size = u64::from_be_bytes(entry[8..].try_into().map_err(|_| Error::Bounds)?);
            if address == 0 && size == 0 {
                break;
            }
            reservations
                .push(
                    Region::new(address, size).map_err(|_| Error::Reservation)?,
                    true,
                )
                .map_err(|_| Error::Reservation)?;
        }
        // The blob is protected even when firmware omitted it from the map.
        reservations
            .push(
                Region::new(physical_address, total_bytes as u64).map_err(|_| Error::Bounds)?,
                false,
            )
            .map_err(|_| Error::Reservation)?;
        let dtb = Self {
            structure,
            strings,
            reservations,
            total_bytes,
        };
        let mut tokens = dtb.tokens();
        while tokens.next()?.is_some() {}
        Ok(dtb)
    }

    pub fn tokens(&self) -> Tokens<'_> {
        Tokens {
            structure: self.structure,
            strings: self.strings,
            cursor: 0,
            depth: 0,
            children: [false; MAX_DEPTH],
            root_seen: false,
            done: false,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum Token<'a> {
    Begin(&'a [u8]),
    End,
    Property { name: &'a [u8], value: &'a [u8] },
}

pub struct Tokens<'a> {
    structure: &'a [u8],
    strings: &'a [u8],
    cursor: usize,
    depth: usize,
    children: [bool; MAX_DEPTH],
    root_seen: bool,
    done: bool,
}

fn cstring(bytes: &[u8], offset: usize) -> Result<(&[u8], usize), Error> {
    let tail = bytes.get(offset..).ok_or(Error::Bounds)?;
    let length = tail
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(Error::Structure)?;
    Ok((
        &tail[..length],
        offset.checked_add(length + 1).ok_or(Error::Bounds)?,
    ))
}

impl<'a> Tokens<'a> {
    fn padded(&mut self, end: usize) -> Result<(), Error> {
        let aligned = end.checked_add(3).ok_or(Error::Bounds)? & !3;
        if block(self.structure, end, aligned - end)?
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(Error::Structure);
        }
        self.cursor = aligned;
        Ok(())
    }

    pub fn next(&mut self) -> Result<Option<Token<'a>>, Error> {
        if self.done {
            return Ok(None);
        }
        loop {
            let token = word(self.structure, self.cursor)?;
            self.cursor += 4;
            match token {
                1 => {
                    if self.depth == MAX_DEPTH {
                        return Err(Error::Depth);
                    }
                    let (name, end) = cstring(self.structure, self.cursor)?;
                    if self.depth == 0 {
                        if self.root_seen || !name.is_empty() {
                            return Err(Error::Structure);
                        }
                        self.root_seen = true;
                    } else {
                        if name.is_empty() {
                            return Err(Error::Structure);
                        }
                        self.children[self.depth - 1] = true;
                    }
                    self.children[self.depth] = false;
                    self.depth += 1;
                    self.padded(end)?;
                    return Ok(Some(Token::Begin(name)));
                }
                2 => {
                    self.depth = self.depth.checked_sub(1).ok_or(Error::Structure)?;
                    return Ok(Some(Token::End));
                }
                3 => {
                    if self.depth == 0 || self.children[self.depth - 1] {
                        return Err(Error::Structure);
                    }
                    let length = word(self.structure, self.cursor)? as usize;
                    let name_offset = word(self.structure, self.cursor + 4)? as usize;
                    self.cursor += 8;
                    let value = block(self.structure, self.cursor, length)?;
                    let (name, _) = cstring(self.strings, name_offset)?;
                    if name.is_empty() {
                        return Err(Error::Structure);
                    }
                    self.padded(self.cursor.checked_add(length).ok_or(Error::Bounds)?)?;
                    return Ok(Some(Token::Property { name, value }));
                }
                4 => {}
                9 => {
                    if self.depth != 0 || !self.root_seen || self.cursor != self.structure.len() {
                        return Err(Error::Structure);
                    }
                    self.done = true;
                    return Ok(None);
                }
                _ => return Err(Error::Structure),
            }
        }
    }
}
