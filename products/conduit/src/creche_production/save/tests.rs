//! Exact file saving of actual fixture-bound products, no native/boot claim.
use super::*;
use crate::creche_production::tests::{job, Files};
use std::{
    fs::{self, OpenOptions},
    sync::Arc,
};
fn complete(files: &Files) -> CrecheProducedImages {
    let mut job = job("release", Arc::new(AtomicBool::new(false)));
    for host in ["first", "second"] {
        let input = files.input(&job, host, false);
        job.produce_host(host, input, files.output(&format!("{host}.iso")))
            .unwrap();
    }
    match job.finish() {
        Ok(products) => products,
        Err((error, _)) => panic!("fixture completion:{error:?}"),
    }
}
#[test]
fn exact_saved_receipt_requires_synced_readback_and_preserves_sources() {
    let files = Files::new();
    let mut products = complete(&files);
    let original = fs::read(files.root.join("first.iso")).unwrap();
    let mut output = files.output("saved.iso");
    let cancelled = AtomicBool::new(false);
    let receipt = products
        .save_host("first", &mut output, original.len() as u64, &cancelled)
        .unwrap();
    assert_eq!(fs::read(files.root.join("saved.iso")).unwrap(), original);
    assert_eq!(fs::read(files.root.join("first.iso")).unwrap(), original);
    assert_eq!(receipt.request(), products.request().0);
    assert_eq!(receipt.body_id(), products.biography().body_id.as_str());
    assert_eq!(receipt.host_entry(), "first");
    assert_eq!(receipt.target(), products.artifacts()[0].receipt().target);
    assert_eq!(
        receipt.artifact_sha256(),
        format!("sha256:{:x}", Sha256::digest(&original))
    );
    assert_eq!(receipt.artifact_bytes(), original.len() as u64);
    let value = serde_json::to_value(&receipt).unwrap();
    assert_eq!(value["artifact_saved"], true);
    for flag in ["carrier_realized", "boot_observed", "join_observed"] {
        assert_eq!(value[flag], false);
    }
    assert!(matches!(
        products.save_host("first", &mut output, u64::MAX, &cancelled),
        Err(CrecheSaveRefusal::InvalidOutput)
    ));
}
#[test]
fn rejected_bounds_access_and_cancellation_leave_caller_files_owned() {
    let files = Files::new();
    let mut products = complete(&files);
    let cancelled = AtomicBool::new(false);
    let mut output = files.output("output.iso");
    assert!(matches!(
        products.save_host("unknown", &mut output, u64::MAX, &cancelled),
        Err(CrecheSaveRefusal::UnknownHost)
    ));
    assert!(matches!(
        products.save_host("first", &mut output, 1, &cancelled),
        Err(CrecheSaveRefusal::BoundExhausted)
    ));
    let mut readonly = File::open(files.root.join("output.iso")).unwrap();
    assert!(matches!(
        products.save_host("first", &mut readonly, u64::MAX, &cancelled),
        Err(CrecheSaveRefusal::InvalidOutput)
    ));
    let mut writeonly = OpenOptions::new()
        .write(true)
        .open(files.root.join("output.iso"))
        .unwrap();
    assert!(matches!(
        products.save_host("first", &mut writeonly, u64::MAX, &cancelled),
        Err(CrecheSaveRefusal::InvalidOutput)
    ));
    let probe = output.try_clone().unwrap();
    assert!(matches!(
        products.save_host_checked("first", &mut output, u64::MAX, &mut || if probe
            .metadata()
            .unwrap()
            .len()
            > 0
        {
            Err(CrecheSaveRefusal::Cancelled)
        } else {
            Ok(())
        }),
        Err(CrecheSaveRefusal::Cancelled)
    ));
    assert!(output.metadata().unwrap().len() > 0);
    assert!(files.root.join("output.iso").exists());
    assert!(files.root.join("first.iso").exists());
}
#[test]
fn source_mutated_after_copy_started_cannot_receive_saved_receipt() {
    let files = Files::new();
    let mut products = complete(&files);
    let mut output = files.output("output.iso");
    let probe = output.try_clone().unwrap();
    let mut source = OpenOptions::new()
        .write(true)
        .open(files.root.join("first.iso"))
        .unwrap();
    let mut changed = false;
    let result = products.save_host_checked("first", &mut output, u64::MAX, &mut || {
        if !changed && probe.metadata().unwrap().len() > 0 {
            source.seek(SeekFrom::Start(100)).unwrap();
            source.write_all(&[0]).unwrap();
            source.sync_all().unwrap();
            changed = true;
        }
        Ok(())
    });
    assert!(changed);
    assert!(matches!(result, Err(CrecheSaveRefusal::SourceChanged)));
    assert!(output.metadata().unwrap().len() > 0);
    assert!(files.root.join("first.iso").exists());
}
