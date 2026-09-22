use super::*;

const HEADER: &str =
    "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n";

#[test]
fn graymap_framing_is_exact_and_never_grows() {
    let mut output = Vec::with_capacity(32);
    encode_graymap(&[0, 1, 2, 3], 2, 2, &mut output).unwrap();
    assert_eq!(output, b"P5\n2 2\n255\n\0\x01\x02\x03");
    let original_capacity = output.capacity();
    assert_eq!(
        encode_graymap(&[0; 24], 6, 4, &mut output),
        Err(OcrProviderRefusal::OutputCapacity)
    );
    assert_eq!(output.capacity(), original_capacity);
}

#[test]
fn tsv_retains_word_geometry_and_bounded_confidence() {
    let tsv = format!("{HEADER}5\t1\t1\t1\t1\t1\t10\t20\t30\t12\t91.75\tTRINITY\n");
    let mut observed = Vec::new();
    assert_eq!(
        visit_tesseract_tsv(&tsv, 100, 80, |candidate| {
            observed.push(candidate);
            Ok(())
        }),
        Ok(1)
    );
    assert_eq!(
        observed,
        [OcrCandidate {
            text: "TRINITY",
            region: ImageRegion {
                x: 10,
                y: 20,
                width: 30,
                height: 12,
            },
            confidence_permille: 917,
        }]
    );
}

#[test]
fn malformed_overflow_and_out_of_frame_results_refuse() {
    let out_of_frame = format!("{HEADER}5\t1\t1\t1\t1\t1\t90\t20\t30\t12\t91\tword\n");
    assert_eq!(
        visit_tesseract_tsv(&out_of_frame, 100, 80, |_| Ok(())),
        Err(OcrProviderRefusal::InvalidProviderOutput)
    );
    let overflow = format!(
        "{HEADER}{}",
        (0..=MAXIMUM_OCR_ITEMS)
            .map(|index| format!("5\t1\t1\t1\t1\t{index}\t0\t0\t1\t1\t90\tw{index}\n"))
            .collect::<String>()
    );
    assert_eq!(
        visit_tesseract_tsv(&overflow, 10, 10, |_| Ok(())),
        Err(OcrProviderRefusal::OutputCapacity)
    );
}

#[cfg(unix)]
#[test]
fn executable_provider_binds_identity_and_visits_exact_candidates() {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "conduit-tesseract-provider-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir(&root).unwrap();
    let executable = root.join("tesseract");
    fs::write(
        &executable,
        format!(
            "#!/bin/sh\ncat >/dev/null\nprintf '{HEADER}5\\t1\\t1\\t1\\t1\\t1\\t2\\t3\\t4\\t5\\t88.2\\tCANTUS\\n'\n"
        ),
    )
    .unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    let mut provider =
        TesseractOcrProvider::prepare(&executable, "eng", 100, Duration::from_secs(2)).unwrap();
    assert_eq!(provider.language(), "eng");
    assert_eq!(provider.executable_sha256().len(), 64);
    let mut observed = Vec::new();
    assert_eq!(
        provider.recognize(&[0; 100], 10, 10, |candidate| {
            observed.push((
                candidate.text.to_owned(),
                candidate.region,
                candidate.confidence_permille,
            ));
            Ok(())
        }),
        Ok(1)
    );
    assert_eq!(
        observed,
        [(
            "CANTUS".to_owned(),
            ImageRegion {
                x: 2,
                y: 3,
                width: 4,
                height: 5,
            },
            882,
        )]
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn provider_that_never_reads_input_is_killed_at_the_exact_deadline() {
    use std::os::unix::fs::PermissionsExt;

    let root =
        std::env::temp_dir().join(format!("conduit-tesseract-timeout-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir(&root).unwrap();
    let executable = root.join("tesseract");
    fs::write(&executable, "#!/bin/sh\nwhile :; do :; done\n").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    let mut provider =
        TesseractOcrProvider::prepare(&executable, "eng", 1_000_000, Duration::from_millis(20))
            .unwrap();
    assert_eq!(
        provider.recognize(&vec![0; 1_000_000], 1_000, 1_000, |_| Ok(())),
        Err(OcrProviderRefusal::TimedOut)
    );
    fs::remove_dir_all(root).unwrap();
}
