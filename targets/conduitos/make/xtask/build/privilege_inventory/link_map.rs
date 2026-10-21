//! Check LLD output-section facts against the actual Root ELF section table.
//! This is structural association, not proof of linker invocation provenance.
use super::elf::number;
use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Eq)]
struct Section {
    address: u64,
    bytes: u64,
    alignment: u64,
}

fn extent(bytes: &[u8], offset: u64, length: u64) -> Result<&[u8], String> {
    let start = usize::try_from(offset).map_err(|_| "section offset overflow")?;
    let end = offset
        .checked_add(length)
        .and_then(|end| usize::try_from(end).ok())
        .ok_or("section extent overflow")?;
    bytes
        .get(start..end)
        .ok_or_else(|| "truncated section".into())
}

fn sections(kernel: &[u8]) -> Result<BTreeMap<String, Section>, String> {
    let table = usize::try_from(number(kernel, 40, 8)?).map_err(|_| "section table overflow")?;
    let count = number(kernel, 60, 2)? as usize;
    let names = number(kernel, 62, 2)? as usize;
    if number(kernel, 58, 2)? != 64 || !(2..=4096).contains(&count) || names >= count {
        return Err("unsupported Root section table".into());
    }
    let end = table
        .checked_add(count * 64)
        .ok_or("section table overflow")?;
    kernel.get(table..end).ok_or("truncated section table")?;
    let strings = table + names * 64;
    if number(kernel, strings + 4, 4)? != 3 {
        return Err("Root section names require a string table".into());
    }
    let strings = extent(
        kernel,
        number(kernel, strings + 24, 8)?,
        number(kernel, strings + 32, 8)?,
    )?;
    let mut result = BTreeMap::new();
    for index in 1..count {
        let at = table + index * 64;
        let name = usize::try_from(number(kernel, at, 4)?).map_err(|_| "section name overflow")?;
        let tail = strings
            .get(name..)
            .ok_or("section name outside string table")?;
        let length = tail
            .iter()
            .position(|byte| *byte == 0)
            .ok_or("unterminated section name")?;
        let name = std::str::from_utf8(&tail[..length]).map_err(|_| "non-UTF8 section name")?;
        if !name.starts_with('.') || name.contains(char::is_whitespace) {
            return Err("unsupported Root output-section name".into());
        }
        let section = Section {
            address: number(kernel, at + 16, 8)?,
            bytes: number(kernel, at + 32, 8)?,
            alignment: number(kernel, at + 48, 8)?,
        };
        if result.insert(name.to_owned(), section).is_some() {
            return Err("duplicate Root output section".into());
        }
    }
    Ok(result)
}

pub(super) fn validate(kernel: &[u8], map: &[u8]) -> Result<(), String> {
    let expected = sections(kernel)?;
    let map = std::str::from_utf8(map).map_err(|_| "non-UTF8 Root link map")?;
    let mut lines = map.lines();
    let header = lines.next().ok_or("empty Root link map")?;
    if header.split_whitespace().collect::<Vec<_>>()
        != ["VMA", "LMA", "Size", "Align", "Out", "In", "Symbol"]
    {
        return Err("unsupported LLD Root link-map header".into());
    }
    let column = header.find("Out").ok_or("missing output-section column")?;
    let mut observed = BTreeMap::new();
    for line in lines {
        let Some(name) = line.get(column..) else {
            continue;
        };
        let name = name.trim_end();
        // Input sections and symbols are indented beyond the Out column.
        // Linker expressions such as `. = address` are not output sections.
        if !name.starts_with('.') || name.contains(char::is_whitespace) {
            continue;
        }
        let fields: Vec<_> = line
            .get(..column)
            .ok_or("invalid map column")?
            .split_whitespace()
            .collect();
        if fields.len() != 4 || observed.len() >= 4096 {
            return Err("invalid or excessive Root map sections".into());
        }
        let hex = |text| u64::from_str_radix(text, 16).map_err(|_| "invalid Root map address/size");
        let section = Section {
            address: hex(fields[0])?,
            bytes: hex(fields[2])?,
            alignment: fields[3]
                .parse::<u64>()
                .map_err(|_| "invalid Root map alignment")?,
        };
        hex(fields[1])?;
        if observed.insert(name.to_owned(), section).is_some() {
            return Err("duplicate Root map output section".into());
        }
    }
    if expected != observed {
        return Err("Root link-map output sections differ from actual ELF".into());
    }
    Ok(())
}
