//! Read-only finite FAT32 reader for the package's single-partition SD format.
use super::*;
fn u16le(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}
fn u32le(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
struct Fat {
    start: u64,
    data: u64,
    sectors: u32,
    copies: u8,
    cluster_bytes: usize,
    clusters: u32,
    root: u32,
}
impl Fat {
    fn offset(&self, cluster: u32) -> Result<u64, Refusal> {
        if cluster < 2 || cluster - 2 >= self.clusters {
            return Err(Refusal::InvalidFat);
        }
        Ok(self.data + u64::from(cluster - 2) * self.cluster_bytes as u64)
    }
    fn next(&self, file: &mut File, cluster: u32) -> Result<Option<u32>, Refusal> {
        self.offset(cluster)?;
        let offset = u64::from(cluster) * 4;
        if offset + 4 > u64::from(self.sectors) * 512 {
            return Err(Refusal::InvalidFat);
        }
        let mut bytes = [0; 4];
        read_at(file, self.start + offset, &mut bytes)?;
        let next = u32::from_le_bytes(bytes) & 0x0fff_ffff;
        for copy in 1..self.copies {
            let mut mirror = [0; 4];
            read_at(
                file,
                self.start + u64::from(copy) * u64::from(self.sectors) * 512 + offset,
                &mut mirror,
            )?;
            if u32::from_le_bytes(mirror) & 0x0fff_ffff != next {
                return Err(Refusal::InvalidFat);
            }
        }
        if next >= 0x0fff_fff8 {
            Ok(None)
        } else {
            self.offset(next)?;
            Ok(Some(next))
        }
    }
}
pub(super) fn verify_kernel(file: &mut File, expected: &[u8]) -> Result<(), Refusal> {
    let metadata = file.metadata().map_err(|_| Refusal::Io)?;
    if !metadata.is_file() || metadata.len() != 64 * 1024 * 1024 {
        return Err(Refusal::Bounds);
    }
    let mut sector = [0; 512];
    read_at(file, 0, &mut sector)?;
    if sector[510..] != [0x55, 0xaa]
        || sector[450] != 0x0c
        || sector[462..510].iter().any(|byte| *byte != 0)
    {
        return Err(Refusal::InvalidFat);
    }
    let start = u64::from(u32le(&sector, 454)) * 512;
    let partition_sectors = u32le(&sector, 458);
    if start != 2048 * 512 || start + u64::from(partition_sectors) * 512 != metadata.len() {
        return Err(Refusal::InvalidFat);
    }
    read_at(file, start, &mut sector)?;
    let reserved = u16le(&sector, 14);
    let fats = sector[16];
    let spc = sector[13];
    let fat_sectors = u32le(&sector, 36);
    if sector[510..] != [0x55, 0xaa]
        || u16le(&sector, 11) != 512
        || spc == 0
        || !spc.is_power_of_two()
        || spc > 128
        || reserved == 0
        || !(1..=2).contains(&fats)
        || fat_sectors == 0
        || u16le(&sector, 17) != 0
        || u16le(&sector, 22) != 0
        || u32le(&sector, 32) != partition_sectors
        || u16le(&sector, 40) != 0
        || u16le(&sector, 42) != 0
    {
        return Err(Refusal::InvalidFat);
    }
    let overhead = u32::from(reserved)
        .checked_add(
            u32::from(fats)
                .checked_mul(fat_sectors)
                .ok_or(Refusal::Bounds)?,
        )
        .ok_or(Refusal::Bounds)?;
    let data_sectors = partition_sectors
        .checked_sub(overhead)
        .ok_or(Refusal::InvalidFat)?;
    let clusters = data_sectors / u32::from(spc);
    if clusters < 65525 || u64::from(clusters + 2) * 4 > u64::from(fat_sectors) * 512 {
        return Err(Refusal::InvalidFat);
    }
    let fat = Fat {
        start: start + u64::from(reserved) * 512,
        data: start + u64::from(overhead) * 512,
        sectors: fat_sectors,
        copies: fats,
        cluster_bytes: usize::from(spc) * 512,
        clusters,
        root: u32le(&sector, 44),
    };
    let mut cluster = fat.root;
    let mut visited = std::collections::BTreeSet::new();
    let mut buffer = vec![0; fat.cluster_bytes];
    let mut kernel = None;
    // Root directory is bounded independently of the file chain.
    'directory: for _ in 0..128 {
        if !visited.insert(cluster) {
            return Err(Refusal::InvalidFat);
        }
        read_at(file, fat.offset(cluster)?, &mut buffer)?;
        for entry in buffer.as_chunks::<32>().0 {
            if entry[0] == 0 {
                break 'directory;
            }
            if entry[0] == 0xe5 || entry[11] == 0x0f {
                continue;
            }
            if &entry[..11] == b"KERNEL  IMG" {
                if entry[11] & 0x18 != 0 || kernel.is_some() {
                    return Err(Refusal::InvalidFat);
                }
                kernel = Some((
                    u32::from(u16le(entry, 20)) << 16 | u32::from(u16le(entry, 26)),
                    u32le(entry, 28),
                ));
            }
        }
        match fat.next(file, cluster)? {
            None => break,
            Some(next) => {
                cluster = next;
                if visited.len() == 128 {
                    return Err(Refusal::Bounds);
                }
            }
        }
    }
    let (mut cluster, bytes) = kernel.ok_or(Refusal::KernelMismatch)?;
    if bytes as usize != expected.len() || expected.is_empty() {
        return Err(Refusal::KernelMismatch);
    }
    visited.clear();
    let mut cursor = 0;
    loop {
        if !visited.insert(cluster) {
            return Err(Refusal::InvalidFat);
        }
        read_at(file, fat.offset(cluster)?, &mut buffer)?;
        let take = buffer.len().min(expected.len() - cursor);
        if buffer[..take] != expected[cursor..cursor + take] {
            return Err(Refusal::KernelMismatch);
        }
        cursor += take;
        let next = fat.next(file, cluster)?;
        if cursor == expected.len() {
            return if next.is_none() {
                Ok(())
            } else {
                Err(Refusal::InvalidFat)
            };
        }
        cluster = next.ok_or(Refusal::KernelMismatch)?;
    }
}
