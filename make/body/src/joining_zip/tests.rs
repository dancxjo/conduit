use super::*;

fn fixture(names: &[&str]) -> Vec<u8> {
    stored_zip(
        &names
            .iter()
            .map(|name| JoiningZipEntry {
                name: (*name).into(),
                bytes: b"bounded payload".to_vec(),
            })
            .collect::<Vec<_>>(),
    )
}

#[test]
fn exact_stored_archive_preserves_named_payloads() {
    let entries = read_stored_joining_zip(&fixture(&["host.uf2", "conduit-spore.json"])).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].name, "host.uf2");
    assert_eq!(entries[1].name, "conduit-spore.json");
    assert!(entries
        .iter()
        .all(|entry| entry.bytes == b"bounded payload"));
}

#[test]
fn corruption_and_unaccounted_bytes_refuse() {
    let original = fixture(&["host.uf2", "conduit-spore.json"]);
    let mut corrupted = original.clone();
    corrupted[30 + "host.uf2".len()] ^= 1;
    assert!(read_stored_joining_zip(&corrupted).is_err());
    for bytes in [
        &original[..original.len() - 1],
        &[original.as_slice(), b"trailing"].concat(),
        b"",
    ] {
        assert!(read_stored_joining_zip(bytes).is_err());
    }
}

#[test]
fn unsafe_duplicate_and_unbounded_file_sets_refuse() {
    for names in [
        vec!["../host", "provision"],
        vec!["host", "host"],
        vec!["host"],
        vec!["..", "provision"],
        vec!["x\\y", "provision"],
        vec!["host"; 18],
    ] {
        assert!(read_stored_joining_zip(&fixture(&names)).is_err());
    }
}

#[test]
fn directory_count_and_storage_method_are_exact() {
    let original = fixture(&["host.uf2", "conduit-spore.json"]);
    let end = original.len() - 22;
    let central = usize_at_u32(&original, end + 16).unwrap();
    for (offset, value) in [
        (end + 4, 1),
        (end + 8, 1),
        (end + 10, 18),
        (central + 10, 8),
    ] {
        let mut malformed = original.clone();
        put_u16(&mut malformed, offset, value);
        assert!(read_stored_joining_zip(&malformed).is_err());
    }
}

fn stored_zip(entries: &[JoiningZipEntry]) -> Vec<u8> {
    let mut locals = Vec::new();
    let mut centrals = Vec::new();
    let mut local_offset = 0_u32;
    for entry in entries {
        let name = entry.name.as_bytes();
        let crc = crc32(&entry.bytes);
        let mut local = vec![0_u8; 30 + name.len() + entry.bytes.len()];
        put_u32(&mut local, 0, LOCAL_FILE);
        put_u16(&mut local, 4, 20);
        put_u16(&mut local, 6, 0x0800);
        put_u32(&mut local, 14, crc);
        put_u32(&mut local, 18, entry.bytes.len() as u32);
        put_u32(&mut local, 22, entry.bytes.len() as u32);
        put_u16(&mut local, 26, name.len() as u16);
        local[30..30 + name.len()].copy_from_slice(name);
        local[30 + name.len()..].copy_from_slice(&entry.bytes);

        let mut central = vec![0_u8; 46 + name.len()];
        put_u32(&mut central, 0, CENTRAL_FILE);
        put_u16(&mut central, 4, 0x0314);
        put_u16(&mut central, 6, 20);
        put_u16(&mut central, 8, 0x0800);
        put_u32(&mut central, 16, crc);
        put_u32(&mut central, 20, entry.bytes.len() as u32);
        put_u32(&mut central, 24, entry.bytes.len() as u32);
        put_u16(&mut central, 28, name.len() as u16);
        put_u32(&mut central, 42, local_offset);
        central[46..].copy_from_slice(name);
        local_offset += local.len() as u32;
        locals.extend(local);
        centrals.extend(central);
    }
    let central_bytes = centrals.len() as u32;
    let mut end = vec![0_u8; 22];
    put_u32(&mut end, 0, END);
    put_u16(&mut end, 8, entries.len() as u16);
    put_u16(&mut end, 10, entries.len() as u16);
    put_u32(&mut end, 12, central_bytes);
    put_u32(&mut end, 16, local_offset);
    locals.extend(centrals);
    locals.extend(end);
    locals
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
