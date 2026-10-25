//! Minimal generated FAT fixtures, not firmware or physical boot evidence.
use super::*;
use conduit_body::{Body, BodyBiographyEvidence, BodyMembership, PreparedBirthMediaBinding};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    sync::atomic::{AtomicU64, Ordering},
};
fn media() -> PreparedBirthMedia {
    let body = Body::born(
        "source/selected".into(),
        "checked/selected".into(),
        41,
        "sign/born".into(),
    )
    .unwrap();
    let membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let evidence = BodyBiographyEvidence::born(body, membership, "Pi Cedar".into()).unwrap();
    PreparedBirthMedia::from_evidence(
        &evidence,
        PreparedBirthMediaBinding {
            host_entry: "first".into(),
            first_host_entry: "first".into(),
            construction_id: "construction/exact".into(),
            configuration_id: "configuration/exact".into(),
            profile_id: "profile/exact".into(),
            build_id: "build/exact".into(),
            target: crate::B_PLUS_TARGET.into(),
        },
    )
    .unwrap()
}
fn kernel() -> Vec<u8> {
    let mut kernel = vec![0x5a; 64 + SLOT_HEADER_BYTES + PREPARED_BIRTH_MEDIA_BYTES];
    kernel[64..].fill(0);
    kernel[64..64 + SLOT_MAGIC.len()].copy_from_slice(SLOT_MAGIC);
    kernel
}
fn bind(kernel: &mut [u8], media: &PreparedBirthMedia) -> Result<KernelBirthReceipt, Refusal> {
    let sha = digest(kernel);
    bind_kernel(
        kernel,
        media,
        CompiledKernelBinding {
            profile_id: "profile/exact",
            build_id: "build/exact",
            base_kernel_sha256: &sha,
        },
    )
}
#[test]
fn slot_binding_retains_birth_and_refuses_wrong_build_or_occupied_ambiguous_slot() {
    let media = media();
    let mut raw = kernel();
    let pristine = raw.clone();
    assert_eq!(
        bind_kernel(
            &mut raw,
            &media,
            CompiledKernelBinding {
                profile_id: "wrong",
                build_id: "build/exact",
                base_kernel_sha256: &digest(&pristine)
            }
        ),
        Err(Refusal::Binding)
    );
    assert_eq!(raw, pristine);
    let receipt = bind(&mut raw, &media).unwrap();
    assert!(
        !receipt.boot_observed
            && !receipt.membership_observed
            && !receipt.ordinary_product_consumer_verified
    );
    assert_eq!(
        PreparedBirthMedia::decode(&raw[receipt.frame_offset..]).unwrap(),
        media
    );
    assert_eq!(bind(&mut raw, &media), Err(Refusal::SlotOccupied));
    let mut ambiguous = pristine.clone();
    ambiguous.extend_from_slice(&pristine[64..]);
    assert_eq!(bind(&mut ambiguous, &media), Err(Refusal::SlotAmbiguous));
}
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Disk {
    path: std::path::PathBuf,
    file: File,
    data: u64,
}
impl Disk {
    fn new(kernel: &[u8]) -> Self {
        let path = std::env::temp_dir().join(format!(
            "conduit-pi-birth-fat-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.set_len(64 * 1024 * 1024).unwrap();
        let mut sector = [0; 512];
        sector[450] = 0x0c;
        sector[454..458].copy_from_slice(&2048_u32.to_le_bytes());
        sector[458..462].copy_from_slice(&129024_u32.to_le_bytes());
        sector[510..].copy_from_slice(&[0x55, 0xaa]);
        put(&mut file, 0, &sector);
        sector.fill(0);
        sector[11..13].copy_from_slice(&512_u16.to_le_bytes());
        sector[13] = 1;
        sector[14..16].copy_from_slice(&32_u16.to_le_bytes());
        sector[16] = 1;
        sector[32..36].copy_from_slice(&129024_u32.to_le_bytes());
        sector[36..40].copy_from_slice(&1024_u32.to_le_bytes());
        sector[44..48].copy_from_slice(&2_u32.to_le_bytes());
        sector[510..].copy_from_slice(&[0x55, 0xaa]);
        put(&mut file, 2048 * 512, &sector);
        let fat = (2048 + 32) * 512;
        let data = (2048 + 32 + 1024) * 512;
        put(&mut file, fat + 8, &0x0fff_ffff_u32.to_le_bytes());
        let count = kernel.len().div_ceil(512);
        for index in 0..count {
            let cluster = 3 + index as u32;
            let next = if index + 1 == count {
                0x0fff_ffff
            } else {
                cluster + 1
            };
            put(&mut file, fat + u64::from(cluster) * 4, &next.to_le_bytes());
        }
        let mut entry = [0; 32];
        entry[..11].copy_from_slice(b"KERNEL  IMG");
        entry[11] = 0x20;
        entry[26..28].copy_from_slice(&3_u16.to_le_bytes());
        entry[28..32].copy_from_slice(&(kernel.len() as u32).to_le_bytes());
        put(&mut file, data, &entry);
        put(&mut file, data + 512, kernel);
        Self { path, file, data }
    }
}
impl Drop for Disk {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
fn put(file: &mut File, offset: u64, bytes: &[u8]) {
    file.seek(SeekFrom::Start(offset)).unwrap();
    file.write_all(bytes).unwrap();
}
#[test]
fn real_fat_file_chain_readback_matches_bound_kernel_and_rejects_changed_bytes() {
    let media = media();
    let mut kernel = kernel();
    let receipt = bind(&mut kernel, &media).unwrap();
    let mut disk = Disk::new(&kernel);
    verify_sd_kernel(&mut disk.file, &kernel, &media, &receipt).unwrap();
    put(&mut disk.file, disk.data + 512 + 100, &[0xff]);
    assert_eq!(
        verify_sd_kernel(&mut disk.file, &kernel, &media, &receipt),
        Err(Refusal::KernelMismatch)
    );
}
#[test]
fn invalid_mbr_and_cyclic_kernel_chain_refuse() {
    let media = media();
    let mut kernel = kernel();
    let receipt = bind(&mut kernel, &media).unwrap();
    let mut disk = Disk::new(&kernel);
    put(
        &mut disk.file,
        (2048 + 32) * 512 + 3 * 4,
        &3_u32.to_le_bytes(),
    );
    assert!(verify_sd_kernel(&mut disk.file, &kernel, &media, &receipt).is_err());
    put(&mut disk.file, 510, &[0, 0]);
    assert_eq!(
        verify_sd_kernel(&mut disk.file, &kernel, &media, &receipt),
        Err(Refusal::InvalidFat)
    );
}
