use std::{
    fs::{self, File},
    io::{Seek, SeekFrom, Write},
    path::Path,
};

use super::ConduitosError;

const SECTOR_BYTES: u64 = 512;
const PARTITION_START_SECTOR: u32 = 32_768;
const SOURCE_DATE_EPOCH: &str = "1786233600";

pub(super) fn legacy_boot_script(script: &[u8]) -> Result<Vec<u8>, ConduitosError> {
    let script_len = u32::try_from(script.len())
        .map_err(|_| refusal("boot-script-creation-failed", "script exceeds u32"))?;
    let mut payload = Vec::with_capacity(8 + script.len() + 3);
    payload.extend_from_slice(&script_len.to_be_bytes());
    payload.extend_from_slice(&0_u32.to_be_bytes());
    payload.extend_from_slice(script);
    while payload.len() % 4 != 0 {
        payload.push(0);
    }
    let payload_len = u32::try_from(payload.len())
        .map_err(|_| refusal("boot-script-creation-failed", "payload exceeds u32"))?;
    let mut header = [0_u8; 64];
    header[0..4].copy_from_slice(&0x2705_1956_u32.to_be_bytes());
    header[8..12].copy_from_slice(
        &SOURCE_DATE_EPOCH
            .parse::<u32>()
            .map_err(|error| refusal("boot-script-creation-failed", error))?
            .to_be_bytes(),
    );
    header[12..16].copy_from_slice(&payload_len.to_be_bytes());
    header[24..28].copy_from_slice(&crc32(&payload).to_be_bytes());
    header[28] = 5;
    header[29] = 22;
    header[30] = 6;
    header[31] = 0;
    let name = b"ConduitOS Orange Pi 5";
    header[32..32 + name.len()].copy_from_slice(name);
    let header_crc = crc32(&header);
    header[4..8].copy_from_slice(&header_crc.to_be_bytes());
    let mut image = Vec::with_capacity(header.len() + payload.len());
    image.extend_from_slice(&header);
    image.extend_from_slice(&payload);
    Ok(image)
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

#[derive(Clone, Copy)]
struct FatGeometry {
    partition_offset: u64,
    bytes_per_sector: u64,
    sectors_per_cluster: u64,
    reserved_sectors: u64,
    fat_count: u64,
    sectors_per_fat: u64,
    root_cluster: u32,
}

impl FatGeometry {
    fn read(file: &mut File) -> Result<Self, ConduitosError> {
        let partition_offset = u64::from(PARTITION_START_SECTOR) * SECTOR_BYTES;
        let mut boot = [0_u8; 512];
        file.seek(SeekFrom::Start(partition_offset))
            .and_then(|_| std::io::Read::read_exact(file, &mut boot))
            .map_err(|error| refusal("fat-filesystem-invalid", error))?;
        let geometry = Self {
            partition_offset,
            bytes_per_sector: u64::from(u16::from_le_bytes([boot[11], boot[12]])),
            sectors_per_cluster: u64::from(boot[13]),
            reserved_sectors: u64::from(u16::from_le_bytes([boot[14], boot[15]])),
            fat_count: u64::from(boot[16]),
            sectors_per_fat: u64::from(u32::from_le_bytes(
                boot[36..40].try_into().expect("four bytes"),
            )),
            root_cluster: u32::from_le_bytes(boot[44..48].try_into().expect("four bytes")),
        };
        if geometry.bytes_per_sector != SECTOR_BYTES
            || geometry.sectors_per_cluster == 0
            || geometry.reserved_sectors == 0
            || geometry.fat_count == 0
            || geometry.sectors_per_fat == 0
            || geometry.root_cluster < 2
            || boot[82..90] != *b"FAT32   "
        {
            return Err(refusal(
                "fat-filesystem-invalid",
                "mkfs output is not the exact bounded FAT32 geometry",
            ));
        }
        Ok(geometry)
    }

    fn fat_offset(self, copy: u64) -> u64 {
        self.partition_offset
            + (self.reserved_sectors + copy * self.sectors_per_fat) * self.bytes_per_sector
    }

    fn cluster_bytes(self) -> u64 {
        self.bytes_per_sector * self.sectors_per_cluster
    }

    fn cluster_offset(self, cluster: u32) -> u64 {
        self.partition_offset
            + (self.reserved_sectors + self.fat_count * self.sectors_per_fat)
                * self.bytes_per_sector
            + u64::from(cluster - 2) * self.cluster_bytes()
    }
}

pub(super) fn write_fat_files(
    image: &Path,
    files: &[([u8; 11], Vec<u8>)],
) -> Result<(), ConduitosError> {
    let mut file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(image)
        .map_err(|error| refusal("fat-file-copy-failed", error))?;
    let geometry = FatGeometry::read(&mut file)?;
    let cluster_bytes = usize::try_from(geometry.cluster_bytes())
        .map_err(|_| refusal("fat-filesystem-invalid", "cluster size exceeds usize"))?;
    if files.len() * 32 > cluster_bytes {
        return Err(refusal(
            "fat-file-copy-failed",
            "root directory capacity is insufficient",
        ));
    }
    let mut next_cluster = 3_u32;
    let mut directory = vec![0_u8; cluster_bytes];
    for (index, (name, bytes)) in files.iter().enumerate() {
        let clusters = bytes.len().max(1).div_ceil(cluster_bytes);
        let first_cluster = next_cluster;
        for cluster_index in 0..clusters {
            let cluster = next_cluster;
            next_cluster = next_cluster
                .checked_add(1)
                .ok_or_else(|| refusal("fat-file-copy-failed", "cluster identity overflow"))?;
            let following = if cluster_index + 1 == clusters {
                0x0fff_ffff
            } else {
                next_cluster
            };
            write_fat_entry(&mut file, geometry, cluster, following)?;
            let start = cluster_index * cluster_bytes;
            let end = bytes.len().min(start + cluster_bytes);
            file.seek(SeekFrom::Start(geometry.cluster_offset(cluster)))
                .and_then(|_| file.write_all(&bytes[start..end]))
                .map_err(|error| refusal("fat-file-copy-failed", error))?;
            if end - start < cluster_bytes {
                let padding = vec![0_u8; cluster_bytes - (end - start)];
                file.write_all(&padding)
                    .map_err(|error| refusal("fat-file-copy-failed", error))?;
            }
        }
        let entry = &mut directory[index * 32..(index + 1) * 32];
        entry[..11].copy_from_slice(name);
        entry[11] = 0x20;
        entry[20..22].copy_from_slice(&((first_cluster >> 16) as u16).to_le_bytes());
        entry[26..28].copy_from_slice(&(first_cluster as u16).to_le_bytes());
        entry[28..32].copy_from_slice(
            &u32::try_from(bytes.len())
                .map_err(|_| refusal("fat-file-copy-failed", "file exceeds FAT32 u32 size"))?
                .to_le_bytes(),
        );
    }
    file.seek(SeekFrom::Start(
        geometry.cluster_offset(geometry.root_cluster),
    ))
    .and_then(|_| file.write_all(&directory))
    .map_err(|error| refusal("fat-file-copy-failed", error))
}

fn write_fat_entry(
    file: &mut File,
    geometry: FatGeometry,
    cluster: u32,
    value: u32,
) -> Result<(), ConduitosError> {
    for copy in 0..geometry.fat_count {
        file.seek(SeekFrom::Start(
            geometry.fat_offset(copy) + u64::from(cluster) * 4,
        ))
        .and_then(|_| file.write_all(&(value & 0x0fff_ffff).to_le_bytes()))
        .map_err(|error| refusal("fat-file-copy-failed", error))?;
    }
    Ok(())
}

pub(super) fn read_fat_file(image: &Path, name: [u8; 11]) -> Result<Vec<u8>, ConduitosError> {
    let mut file =
        File::open(image).map_err(|error| refusal("image-verification-failed", error))?;
    let geometry = FatGeometry::read(&mut file)?;
    let cluster_bytes = usize::try_from(geometry.cluster_bytes())
        .map_err(|_| refusal("fat-filesystem-invalid", "cluster size exceeds usize"))?;
    let mut directory = vec![0_u8; cluster_bytes];
    file.seek(SeekFrom::Start(
        geometry.cluster_offset(geometry.root_cluster),
    ))
    .and_then(|_| std::io::Read::read_exact(&mut file, &mut directory))
    .map_err(|error| refusal("image-verification-failed", error))?;
    let entry = directory
        .as_chunks::<32>()
        .0
        .iter()
        .take_while(|entry| entry[0] != 0)
        .find(|entry| entry[..11] == name)
        .ok_or_else(|| refusal("image-verification-failed", "FAT root file is absent"))?;
    let mut cluster = (u32::from(u16::from_le_bytes([entry[20], entry[21]])) << 16)
        | u32::from(u16::from_le_bytes([entry[26], entry[27]]));
    let size = usize::try_from(u32::from_le_bytes(
        entry[28..32].try_into().expect("four bytes"),
    ))
    .map_err(|_| refusal("image-verification-failed", "file size exceeds usize"))?;
    let maximum_clusters = size.max(1).div_ceil(cluster_bytes);
    let mut output = Vec::with_capacity(size);
    for _ in 0..maximum_clusters {
        let mut bytes = vec![0_u8; cluster_bytes];
        file.seek(SeekFrom::Start(geometry.cluster_offset(cluster)))
            .and_then(|_| std::io::Read::read_exact(&mut file, &mut bytes))
            .map_err(|error| refusal("image-verification-failed", error))?;
        output.extend_from_slice(&bytes);
        file.seek(SeekFrom::Start(
            geometry.fat_offset(0) + u64::from(cluster) * 4,
        ))
        .map_err(|error| refusal("image-verification-failed", error))?;
        let mut next = [0_u8; 4];
        std::io::Read::read_exact(&mut file, &mut next)
            .map_err(|error| refusal("image-verification-failed", error))?;
        let value = u32::from_le_bytes(next) & 0x0fff_ffff;
        if value >= 0x0fff_fff8 {
            break;
        }
        if value < 2 {
            return Err(refusal(
                "image-verification-failed",
                "FAT cluster chain terminated with an invalid identity",
            ));
        }
        cluster = value;
    }
    output.truncate(size);
    Ok(output)
}

fn refusal(reason: &'static str, detail: impl std::fmt::Display) -> ConduitosError {
    ConduitosError::refusal(reason, detail.to_string())
}
