//! Codec proof only: journal authentication and flash commit remain external.
#[path = "../../../../targets/esp32/proof/tm1637/src/native_restart_compress.rs"]
mod codec;
fn encode(bytes: &[u8], width: usize) -> (Vec<u8>, codec::Lengths) {
    let mut out = Vec::new();
    let mut encoder = codec::Encoder::new(
        |chunk: &[u8]| {
            out.extend_from_slice(chunk);
            Ok(())
        },
        131072,
        200000,
    );
    for chunk in bytes.chunks(width) {
        encoder.write(chunk).unwrap();
    }
    let lengths = encoder.finish().unwrap();
    assert!(encoder.finish().is_err());
    assert!(encoder.write(b"late").is_err());
    drop(encoder);
    assert_eq!(lengths.plain, bytes.len());
    assert_eq!(lengths.encoded, out.len());
    (out, lengths)
}
fn decode(bytes: &[u8], width: usize, plain_limit: usize) -> Result<Vec<u8>, codec::Error> {
    let mut at = 0;
    let mut decoder = codec::Decoder::new(
        |out: &mut [u8; 128]| {
            let n = width.min(128).min(bytes.len() - at);
            out[..n].copy_from_slice(&bytes[at..at + n]);
            at += n;
            Ok(n)
        },
        plain_limit,
        200000,
    );
    let mut plain = Vec::new();
    let mut out = [0; 128];
    loop {
        let n = decoder.read(&mut out)?;
        if n == 0 {
            break;
        }
        plain.extend_from_slice(&out[..n]);
    }
    assert_eq!(decoder.read(&mut out)?, 0);
    drop(decoder);
    assert_eq!(at, bytes.len());
    Ok(plain)
}
#[test]
fn exhaustive_small_inputs_and_serializer_chunk_boundaries_are_deterministic() {
    for length in 0..=8 {
        for bits in 0..1usize << length {
            let data = (0..length)
                .map(|i| ((bits >> i) & 1) as u8)
                .collect::<Vec<_>>();
            let (expected, _) = encode(&data, 128);
            for width in [1, 2, 3, 7, 8, 17, 65, 127, 128] {
                assert_eq!(encode(&data, width).0, expected);
                assert_eq!(decode(&expected, width, 131072).unwrap(), data);
            }
        }
    }
}
#[test]
fn overlap_window_wrap_all_octets_and_maximum_plain_payload_roundtrip() {
    let mut random = 7u64;
    let mut noise = vec![0; 131072];
    for byte in &mut noise {
        random ^= random << 13;
        random ^= random >> 7;
        random ^= random << 17;
        *byte = random as u8;
    }
    for data in [
        vec![0; 131072],
        (0u8..=255).cycle().take(131072).collect(),
        noise,
    ] {
        let (expected, _) = encode(&data, 128);
        for width in [1, 17, 64, 65, 127, 128] {
            assert_eq!(encode(&data, width).0, expected);
            assert_eq!(decode(&expected, width, 131072).unwrap(), data);
        }
        assert!(decode(&expected, 128, data.len() - 1).is_err());
    }
    for length in [1023, 1024, 1025, 2047, 2048, 2049] {
        let data = (0u8..=255).cycle().take(length).collect::<Vec<_>>();
        let (encoded, _) = encode(&data, 65);
        assert_eq!(decode(&encoded, 1, 131072).unwrap(), data);
    }
}
#[test]
fn malformed_tokens_truncation_footer_and_trailing_input_are_refused() {
    let (encoded, _) = encode(b"canonical private JSON payload payload payload", 1);
    for end in 0..encoded.len() {
        assert!(
            decode(&encoded[..end], 3, 131072).is_err(),
            "truncation{end}"
        );
    }
    for at in (0..8).chain(encoded.len() - 4..encoded.len()) {
        let mut bad = encoded.clone();
        bad[at] ^= 1;
        assert!(decode(&bad, 1, 131072).is_err());
    }
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert!(decode(&trailing, 128, 131072).is_err());
    // A match before any history, reserved66-byte match, bad unused flag bits.
    for token in [0u16, 0xfc00] {
        let mut bad = codec::MAGIC.to_vec();
        bad.extend_from_slice(&[1]);
        bad.extend_from_slice(&token.to_le_bytes());
        assert!(decode(&bad, 128, 131072).is_err());
    }
    let (mut empty, _) = encode(b"", 128);
    empty[8] |= 2;
    assert!(decode(&empty, 1, 131072).is_err());
}
#[test]
fn limits_and_io_failures_never_allow_continuation_or_successful_eof() {
    let mut encoder = codec::Encoder::new(|_: &[u8]| Ok(()), 2, 100);
    assert!(encoder.write(b"abc").is_err());
    assert!(encoder.write(b"x").is_err());
    assert!(encoder.finish().is_err());
    let mut encoder = codec::Encoder::new(|_: &[u8]| Err(()), 100, 100);
    assert!(encoder.write(b"abc").is_err());
    assert!(encoder.finish().is_err());
    let mut encoder = codec::Encoder::new(|_: &[u8]| Ok(()), 100, 8);
    encoder.write(b"a").unwrap();
    assert!(encoder.finish().is_err());
    let (encoded, _) = encode(b"[true,false,null,123]", 128);
    for fault in 0..=encoded.len() {
        let mut at = 0;
        let mut reader = codec::Decoder::new(
            |out: &mut [u8; 128]| {
                if at == fault {
                    return Err(());
                }
                out[0] = encoded[at];
                at += 1;
                Ok(1)
            },
            131072,
            200000,
        );
        let mut out = [77; 128];
        assert!(reader.read(&mut out).is_err());
        assert_eq!(out, [0; 128]);
        assert!(reader.read(&mut out).is_err());
    }
    let mut reader = codec::Decoder::new(|_: &mut [u8; 128]| Ok(129), 131072, 200000);
    assert!(reader.read(&mut [0; 128]).is_err());
    let mut at = 0;
    let mut reader = codec::Decoder::new(
        |out: &mut [u8; 128]| {
            let n = encoded.len() - at;
            out[..n].copy_from_slice(&encoded[at..]);
            at += n;
            Ok(n)
        },
        131072,
        encoded.len() - 1,
    );
    assert!(reader.read(&mut [0; 128]).is_err());
}
#[test]
fn bounded_objects_and_realistic_json_reduce_storage_without_semantic_changes() {
    let sample = r#"{"schema":"conduit.environment/root-restart@1","body_id":"60d7c2a3be138d4301052535aeb3289291d77dda8a8fade49a16f7f61afa9bb4","records":[{"sequence":12,"sign_id":"3b966979c739428797bc2c7fa67affb446fbac876f7c67f145cb7d28ba78af7d","kind":{"HostJoined":{"change_id":"0bc0c250499048a0acda32ea1d48a5df4275876243b5273ddf959d66f97d87b2","part_id":"0011223344556677889900112233445566778899001122334455667788990011","host_id":"esp32/b0a732286178","boot_id":"boot/00112233445566778899001122334455"}}}],"admissions":{"receipts":[],"pending":[],"invitations":[],"continuity_keys":[],"pending_returns":[]}}"#;
    let data = sample.repeat(48).into_bytes();
    let (encoded, lengths) = encode(&data, 3);
    assert_eq!(decode(&encoded, 1, 131072).unwrap(), data);
    println!(
        "codec plain={} encoded={} encoder_bytes={} decoder_bytes={}",
        lengths.plain,
        lengths.encoded,
        core::mem::size_of::<codec::Encoder<fn(&[u8]) -> Result<(), ()>>>(),
        core::mem::size_of::<codec::Decoder<fn(&mut [u8; 128]) -> Result<usize, ()>>>()
    );
    assert!(encoded.len() < data.len() / 2);
}

#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[test]
fn actual_seventeen_boot_two_plot_biography_compresses_with_zero_codec_allocations() {
    use conduit_body::{AdmissionManager, SealedBodyLifecycleSession};
    use conduit_core as c;
    let host = c::HostAdvertisement {
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "esp32/b0a732286178".into(),
        boot_id: format!("boot/{:032x}", 0).into(),
        offer_generation: c::OfferGeneration(1),
        profile: "fixture/root".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let sign = c::bind_sign(
        &"construction/host".into(),
        &"construction/boot".into(),
        None,
        1,
    );
    let image =
        conduit_environment_make::EnvironmentBodyImage::prepare_mask_from_sign(&sign).unwrap();
    let initial = conduit_environment_runtime::body_image::open_installed_image_sealed(
        image.body.evidence().clone(),
        &host,
    )
    .unwrap();
    let mut evidence = initial.evidence().clone();
    evidence.friendly_name = "x".repeat(21);
    for boot in 1..17 {
        evidence = SealedBodyLifecycleSession::resume_here(
            evidence,
            &host.host_id,
            &format!("boot/{boot:032x}").into(),
        )
        .unwrap()
        .evidence()
        .clone();
    }
    let manager = AdmissionManager::new(evidence.body_id.clone()).unwrap();
    #[derive(serde::Serialize)]
    struct Envelope<'a> {
        schema: &'static str,
        evidence: &'a conduit_body::BodyBiographyEvidence,
        archives: Vec<()>,
        admissions: &'a AdmissionManager,
    }
    let plain = serde_json::to_vec(&Envelope {
        schema: "conduit.environment/root-restart@1",
        evidence: &evidence,
        archives: vec![],
        admissions: &manager,
    })
    .unwrap();
    assert_eq!(plain.len(), 30540);
    let (encoded, _) = encode(&plain, 1);
    assert_eq!(decode(&encoded, 1, 131072).unwrap(), plain);
    println!(
        "actual17Boot JSON={} compressed={} saving={}",
        plain.len(),
        encoded.len(),
        plain.len() - encoded.len()
    );
    assert!(
        encoded.len() < 16304,
        "retain at least half of bank for future exact evidence"
    );
    let mut emitted = 0;
    let (lengths, encode_memory) = allocation_probe::observe(|| {
        let mut writer = codec::Encoder::new(
            |chunk: &[u8]| {
                emitted += chunk.len();
                Ok(())
            },
            131072,
            32608,
        );
        for chunk in plain.chunks(7) {
            writer.write(chunk).unwrap();
        }
        writer.finish().unwrap()
    });
    assert_eq!(lengths.encoded, encoded.len());
    assert_eq!(emitted, encoded.len());
    assert_eq!(
        (encode_memory.allocations, encode_memory.reallocations),
        (0, 0)
    );
    let mut cursor = 0;
    let (count, decode_memory) = allocation_probe::observe(|| {
        let mut reader = codec::Decoder::new(
            |out: &mut [u8; 128]| {
                let n = 128.min(encoded.len() - cursor);
                out[..n].copy_from_slice(&encoded[cursor..cursor + n]);
                cursor += n;
                Ok(n)
            },
            131072,
            32608,
        );
        let mut out = [0; 128];
        let mut count = 0;
        loop {
            let n = reader.read(&mut out).unwrap();
            if n == 0 {
                break;
            }
            count += n;
        }
        count
    });
    assert_eq!(count, plain.len());
    assert_eq!(
        (decode_memory.allocations, decode_memory.reallocations),
        (0, 0)
    );
}

#[allow(dead_code)]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_body_checkpoint.rs"]
mod checkpoint;
#[derive(Clone)]
struct FaultFlash {
    bytes: std::rc::Rc<std::cell::RefCell<Vec<u8>>>,
    cut: std::rc::Rc<std::cell::Cell<Option<usize>>>,
    stages: std::rc::Rc<std::cell::Cell<usize>>,
}
impl FaultFlash {
    fn new(bytes: Vec<u8>, cut: Option<usize>) -> Self {
        Self {
            bytes: std::rc::Rc::new(std::cell::RefCell::new(bytes)),
            cut: std::rc::Rc::new(std::cell::Cell::new(cut)),
            stages: std::rc::Rc::new(std::cell::Cell::new(0)),
        }
    }
    fn stage(&self) -> Result<(), checkpoint::Refusal> {
        let at = self.stages.get();
        if self.cut.get() == Some(at) {
            return Err(checkpoint::Refusal::Storage);
        }
        self.stages.set(at + 1);
        Ok(())
    }
}
impl checkpoint::Storage for FaultFlash {
    fn capacity(&self) -> u32 {
        self.bytes.borrow().len() as u32
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), checkpoint::Refusal> {
        out.copy_from_slice(
            self.bytes
                .borrow()
                .get(at as usize..at as usize + out.len())
                .ok_or(checkpoint::Refusal::Storage)?,
        );
        Ok(())
    }
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), checkpoint::Refusal> {
        assert_eq!(at % 4, 0);
        assert_eq!(bytes.len() % 4, 0);
        for (index, word) in bytes.as_chunks::<4>().0.iter().enumerate() {
            self.stage()?;
            let mut data = self.bytes.borrow_mut();
            for (old, new) in data[at as usize + index * 4..at as usize + index * 4 + 4]
                .iter_mut()
                .zip(word)
            {
                assert_eq!(*old & new, *new);
                *old &= *new;
            }
        }
        Ok(())
    }
    fn erase_bank(&mut self, bank: u8) -> Result<(), checkpoint::Refusal> {
        for sector in 0..8 {
            self.stage()?;
            let at = usize::from(bank) * 32768 + sector * 4096;
            self.bytes.borrow_mut()[at..at + 4096].fill(255);
        }
        Ok(())
    }
}
#[test]
fn every_program_word_and_erase_sector_cut_preserves_legacy_or_complete_compressed_bank() {
    let binding = [19; 32];
    let old_seed = [20; 32];
    let new_seed = [21; 32];
    let legacy = br#"{"schema":"conduit.environment/root-restart@1","legacy":true}"#;
    let media = FaultFlash::new(vec![255; 65536], None);
    checkpoint::Checkpoint::open(media.clone())
        .unwrap()
        .save(&binding, &old_seed, legacy)
        .unwrap();
    let original = media.bytes.borrow().clone();
    let plain = (0u8..=255).cycle().take(131072).collect::<Vec<_>>();
    let (encoded, _) = encode(&plain, 7);
    assert!(encoded.len() < 32608);
    let complete = FaultFlash::new(original.clone(), None);
    checkpoint::Checkpoint::open(complete.clone())
        .unwrap()
        .save(&binding, &new_seed, &encoded)
        .unwrap();
    let stages = complete.stages.get();
    for cut in 0..=stages {
        let media = FaultFlash::new(original.clone(), Some(cut));
        let mut journal = checkpoint::Checkpoint::open(media.clone()).unwrap();
        let result = journal.save(&binding, &new_seed, &encoded);
        assert_eq!(result.is_ok(), cut == stages, "cut{cut}/{stages}");
        media.cut.set(None);
        let mut reopened = checkpoint::Checkpoint::open(media).unwrap();
        let mut seed = [0; 32];
        let mut payload = vec![0; 32608];
        let count = reopened
            .load(&binding, &mut seed, &mut payload)
            .unwrap()
            .unwrap();
        payload.truncate(count);
        if payload == legacy {
            assert_eq!(seed, old_seed);
        } else {
            assert_eq!(payload, encoded, "cut{cut}");
            assert_eq!(seed, new_seed);
            assert_eq!(decode(&payload, 17, 131072).unwrap(), plain);
        }
    }
    println!(
        "legacy_to_compressed_powercut_stages={stages} plain={} encoded={}",
        plain.len(),
        encoded.len()
    );
}
