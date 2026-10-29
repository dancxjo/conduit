//! Opt-in verification of private physical flash captures. No UART or writes.
//! Captured readings, scopes and key material must never appear in test output.
use conduit_environment_runtime::{
    display_evidence_journal::{Refusal, Storage},
    observation::ObservationContract,
    observed_record::{self, ObservedRecord},
};
use ed25519_dalek::SigningKey;
use sha2::{Digest, Sha256};
#[allow(dead_code)]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_body_checkpoint.rs"]
mod checkpoint;
#[allow(dead_code)]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_restart_compress.rs"]
mod compress;
#[allow(dead_code)]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_local_history.rs"]
mod history;
#[allow(dead_code)]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_observed_recovery.rs"]
mod recovery;
struct ReadOnly(Vec<u8>);
impl Storage for ReadOnly {
    fn capacity(&self) -> u32 {
        self.0.len() as u32
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), Refusal> {
        let bytes = self
            .0
            .get(at as usize..at as usize + out.len())
            .ok_or(Refusal::Media)?;
        out.copy_from_slice(bytes);
        Ok(())
    }
    fn program(&mut self, _: u32, _: &[u8]) -> Result<(), Refusal> {
        Err(Refusal::Media)
    }
}
impl checkpoint::Storage for ReadOnly {
    fn capacity(&self) -> u32 {
        self.0.len() as u32
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), checkpoint::Refusal> {
        let bytes = self
            .0
            .get(at as usize..at as usize + out.len())
            .ok_or(checkpoint::Refusal::Storage)?;
        out.copy_from_slice(bytes);
        Ok(())
    }
    fn program(&mut self, _: u32, _: &[u8]) -> Result<(), checkpoint::Refusal> {
        Err(checkpoint::Refusal::Storage)
    }
    fn erase_bank(&mut self, _: u8) -> Result<(), checkpoint::Refusal> {
        Err(checkpoint::Refusal::Storage)
    }
}
struct PrivateRoot {
    seed: [u8; 32],
    binding: [u8; 32],
    payload: Vec<u8>,
}
impl Drop for PrivateRoot {
    fn drop(&mut self) {
        self.seed.fill(0);
        self.payload.fill(0);
    }
}
fn root(flash: &[u8], label: &str) -> PrivateRoot {
    assert!(flash.len() == 0x400000, "exact full flash capture required");
    let bytes = &flash[0x3f0000..0x400000];
    let mut highest: Option<(u64, u8, PrivateRoot)> = None;
    let mut committed = 0;
    for bank in 0..2 {
        let start = bank * 32768;
        let header = &bytes[start..start + 160];
        if &header[152..156] != b"done" {
            continue;
        }
        // Run the actual production header and whole-payload SHA validation for
        // each bank independently, then verify highest-bank selection below.
        let mut isolated = vec![255; 65536];
        isolated[start..start + 32768].copy_from_slice(&bytes[start..start + 32768]);
        let mut journal = checkpoint::Checkpoint::open(ReadOnly(isolated))
            .expect("committed Body bank integrity");
        let generation = journal.generation().expect("committed Body generation");
        let binding: [u8; 32] = header[24..56].try_into().expect("Body binding extent");
        let length = journal
            .payload_length(&binding)
            .expect("Body binding")
            .expect("Body payload present");
        let mut seed = [0; 32];
        let mut payload = vec![0; length];
        assert!(
            journal
                .load(&binding, &mut seed, &mut payload)
                .expect("Body authenticated payload")
                == Some(length)
        );
        let private = PrivateRoot {
            seed,
            binding,
            payload,
        };
        if let Some((_, _, previous)) = highest.as_ref() {
            assert!(
                previous.seed == private.seed,
                "Body banks disagree on private Root continuity"
            );
            assert!(
                previous.binding == private.binding,
                "Body banks disagree on Body continuity"
            );
        }
        eprintln!(
            "{label}: body_bank={bank} generation={generation} header_payload_sha_valid=true"
        );
        committed += 1;
        if highest.as_ref().is_none_or(|(old, _, _)| generation > *old) {
            highest = Some((generation, bank as u8, private));
        }
    }
    assert!(
        committed == 2,
        "both complete Body banks required for this physical proof"
    );
    let (generation, bank, private) = highest.expect("Body journal present");
    let selected = checkpoint::Checkpoint::open(ReadOnly(bytes.to_vec()))
        .expect("combined Body journal integrity");
    assert!(
        selected.generation() == Some(generation),
        "highest Body bank selected"
    );
    // Decode only the already authenticated payload. Check its actual Body ID
    // against the journal binding without emitting any private envelope data.
    let mut at = 0;
    let mut decoder = compress::Decoder::new(
        |out: &mut [u8; 128]| {
            let n = out.len().min(private.payload.len() - at);
            out[..n].copy_from_slice(&private.payload[at..at + n]);
            at += n;
            Ok(n)
        },
        128 * 1024,
        32768 - 160,
    );
    let mut plain = Vec::new();
    let mut chunk = [0; 128];
    loop {
        let n = decoder
            .read(&mut chunk)
            .expect("authenticated restart compression");
        if n == 0 {
            break;
        }
        plain.extend_from_slice(&chunk[..n]);
    }
    drop(decoder);
    let value = match serde_json::from_slice::<serde_json::Value>(&plain) {
        Ok(value) => value,
        Err(_) => panic!("authenticated restart JSON"), // Never format private input errors.
    };
    let body = value
        .get("evidence")
        .and_then(|e| e.get("body_id"))
        .and_then(|b| b.as_str())
        .expect("authenticated restart Body ID");
    assert!(
        <[u8; 32]>::from(Sha256::digest(body.as_bytes())) == private.binding,
        "restart Body binding"
    );
    plain.fill(0);
    eprintln!("{label}: selected_body_bank={bank} committed_banks={committed}");
    private
}
#[test]
#[ignore = "requires explicit private pre-repair and physical signed-reading flash captures"]
fn actual_physical_signed_readings_preserve_body_seed_and_original_headers() {
    let before =
        std::fs::read(std::env::var("THERMO_PHYSICAL_BEFORE").expect("set THERMO_PHYSICAL_BEFORE"))
            .expect("private previous flash readable");
    let actual =
        std::fs::read(std::env::var("THERMO_PHYSICAL_ACTUAL").expect("set THERMO_PHYSICAL_ACTUAL"))
            .expect("private physical flash readable");
    let minimum: usize = std::env::var("THERMO_PHYSICAL_MINIMUM_RECORDS")
        .unwrap_or_else(|_| "3".into())
        .parse()
        .expect("minimum count");
    let minimum_epochs: usize = std::env::var("THERMO_PHYSICAL_MINIMUM_EPOCHS")
        .unwrap_or_else(|_| "1".into())
        .parse()
        .expect("minimum epoch count");
    let minimum_boots: usize = std::env::var("THERMO_PHYSICAL_MINIMUM_BOOTS")
        .unwrap_or_else(|_| "1".into())
        .parse()
        .expect("minimum Boot count");
    let minimum_available: usize = std::env::var("THERMO_PHYSICAL_MINIMUM_AVAILABLE")
        .unwrap_or_else(|_| "0".into())
        .parse()
        .expect("minimum available count");
    let previous_root = root(&before, "before");
    let current_root = root(&actual, "actual");
    assert!(
        previous_root.seed == current_root.seed,
        "private Root seed continuity failed"
    );
    assert!(
        previous_root.binding == current_root.binding,
        "Body continuity failed"
    );
    let key = SigningKey::from_bytes(&current_root.seed).verifying_key();
    let basis = observed_record::descriptor(
        &ObservationContract::prepare().expect("linked observation contract"),
    )
    .expect("linked Forms");
    let forms = Sha256::digest(basis).into();
    let mut work = [0; 536];
    let mut total = 0;
    let mut occupied = 0;
    let mut preserved_records = 0;
    let mut available = 0;
    let mut scopes: Vec<[u8; 96]> = Vec::new();
    let mut boots: Vec<[u8; 32]> = Vec::new();
    for epoch in 0..5 {
        let start = 0x348000 + epoch * 0x20000;
        let old = &before[start..start + 0x20000];
        let current = &actual[start..start + 0x20000];
        let (old_prefix, old_erased) =
            recovery::retained_header_prefix(&mut ReadOnly(old.to_vec()), &basis, &mut work)
                .expect("previous physical epoch header chain");
        assert!(
            old_prefix >= 536,
            "sealed empty header present before signed history"
        );
        let old_extent = if old_erased {
            old_prefix as usize
        } else {
            let (journal, old_scope, _) = recovery::reopen_profile(
                ReadOnly(old[old_prefix as usize..].to_vec()),
                &basis,
                &mut work,
                true,
            )
            .expect("previous signed epoch");
            assert!(
                old_scope[..32] == previous_root.binding,
                "previous epoch Body scope"
            );
            preserved_records += journal.committed_count();
            old_prefix as usize + journal.retained_extent() as usize
        };
        assert!(
            old[..old_extent] == current[..old_extent],
            "old committed header or reading bytes changed"
        );
        let (prefix, erased) =
            recovery::retained_header_prefix(&mut ReadOnly(current.to_vec()), &basis, &mut work)
                .expect("physical epoch header chain");
        if erased {
            eprintln!("epoch={epoch} sealed_empty_header_bytes={prefix} committed_records=0");
            continue;
        }
        let (mut journal, scope, sequence) = recovery::reopen_profile(
            ReadOnly(current[prefix as usize..].to_vec()),
            &basis,
            &mut work,
            true,
        )
        .expect("physical signed epoch recovery");
        assert!(
            scope[..32] == current_root.binding,
            "record Body scope differs from authenticated Body journal"
        );
        assert!(!scopes.contains(&scope), "duplicate physical epoch scope");
        scopes.push(scope);
        let boot: [u8; 32] = scope[32..64].try_into().expect("Boot scope extent");
        if !boots.contains(&boot) {
            boots.push(boot);
        }
        let committed = journal.committed_count();
        assert!(
            sequence == u64::from(committed),
            "committed contiguous sequence"
        );
        let mut cursor = journal.replay_cursor();
        let mut bytes = [0; history::SIGNED_BYTES];
        let mut count = 0;
        let mut clock = 0;
        while let Some(n) = journal
            .replay(&mut cursor, &mut bytes, &mut work)
            .expect("physical record replay")
        {
            assert!(n == history::SIGNED_BYTES, "signed record extent");
            history::verify(
                &key,
                &scope,
                &forms,
                &bytes[..observed_record::BYTES],
                bytes[observed_record::BYTES..]
                    .try_into()
                    .expect("signature extent"),
            )
            .expect("physical full-scope Root signature");
            let record = ObservedRecord::decode(&bytes[..observed_record::BYTES])
                .expect("canonical physical observation");
            if record.quantities.is_some() {
                available += 1;
            }
            count += 1;
            assert!(
                record.sequence == count && record.observed_at_boot_ms >= clock,
                "physical record sequence or Boot clock"
            );
            clock = record.observed_at_boot_ms;
        }
        assert!(
            count == u64::from(committed),
            "all committed records replayed"
        );
        total += committed as usize;
        occupied += 1;
        eprintln!("epoch={epoch} sealed_empty_header_bytes={prefix} committed_records={committed} full_scope_signatures_valid=true");
    }
    assert!(
        total >= minimum,
        "physical capture lacks minimum committed readings"
    );
    assert!(
        occupied >= minimum_epochs,
        "physical capture lacks minimum retained scopes"
    );
    assert!(
        boots.len() >= minimum_boots,
        "physical capture lacks distinct signed Boot scopes"
    );
    assert!(
        available >= minimum_available,
        "physical capture lacks minimum available measurements"
    );
    eprintln!("physical_verified: root_seed_continuity=true body_continuity=true old_headers_unchanged=5 prior_records_unchanged={preserved_records} occupied_epochs={occupied} signed_records={total} available_measurements={available} distinct_boots={}", boots.len());
}
