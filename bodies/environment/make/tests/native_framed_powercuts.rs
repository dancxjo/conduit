//! Actual physical two-pass transaction with legacy→framed immutable archives.
use super::*;
fn framed(
    journal: &mut Checkpoint<RcCutMedia>,
    source: &SourceSnapshot,
    resident: &payload::archive::Resident,
    f: &Fixture,
) -> Result<(), Refusal> {
    let layouts = payload::layout::Layouts::default();
    let prepared = payload::rewrite::PreparedRewrite::prepare_framed(
        resident.proof.as_ref().unwrap(),
        source,
        &f.evidence,
        &[],
        &f.manager,
        &resident.summary,
        resident.span,
        resident.prefix.as_ref(),
        resident.local_count,
        &f.root,
        &layouts,
    )
    .map_err(|_| Refusal::Corrupt)?;
    let completion = journal.rewrite_streamed_attested(source, &f.binding, &f.seed, |io| {
        prepared
            .emit(|out| io.read_source(out).map_err(|_| ()), &mut |b| {
                io.write(b)
            })
            .map(|_| ())
            .map_err(|_| Refusal::Corrupt)
    })?;
    prepared
        .committed(completion)
        .map_err(|_| Refusal::Corrupt)?;
    Ok(())
}
fn load(
    journal: &mut Checkpoint<RcCutMedia>,
    f: &Fixture,
) -> (payload::archive::Resident, SourceSnapshot) {
    let (mut resident, source) = loaded(journal, f);
    payload::authenticate_receipts(&mut resident, &source).unwrap();
    (resident, source)
}
fn verify(bytes: Vec<u8>, f: &Fixture) {
    let mut j = Checkpoint::open(RcCutMedia::new(bytes)).unwrap();
    let generation = j.generation().unwrap();
    let (r, _) = load(&mut j, f);
    assert!(generation == 1 || generation == 2);
    assert!(r.prefix.is_none());
    assert_eq!(r.local_count, f.receipt.summary.count);
    assert_eq!(r.summary.count, f.receipt.summary.count);
}
#[test]
fn every_framed_migration_mutation_cut_preserves_original_canonical_history_and_retry() {
    let f = setup();
    let baseline = RcCutMedia::new(f.original.clone());
    let mut j = Checkpoint::open(baseline.clone()).unwrap();
    let (r, s) = load(&mut j, &f);
    baseline.stages.set(0);
    framed(&mut j, &s, &r, &f).unwrap();
    let count = baseline.stages.get();
    verify(baseline.bytes.borrow().clone(), &f);
    for cut in 0..count {
        let media = RcCutMedia::new(f.original.clone());
        let mut j = Checkpoint::open(media.clone()).unwrap();
        let (r, s) = load(&mut j, &f);
        media.stages.set(0);
        media.cut.set(Some(cut));
        assert!(framed(&mut j, &s, &r, &f).is_err());
        assert!(media.bytes.borrow()[..32768] == f.original[..32768]);
        verify(media.bytes.borrow().clone(), &f);
        media.disarm();
        framed(&mut j, &s, &r, &f).unwrap();
        verify(media.bytes.borrow().clone(), &f);
    }
    println!(
        "framed_mutation_cuts={} cpu_algorithm_only=true original_history_preserved=true",
        count
    );
}

#[test]
fn framed_fixture_reports_exact_physical_extent() {
    let f = setup();
    let media = RcCutMedia::new(f.original.clone());
    let mut j = Checkpoint::open(media.clone()).unwrap();
    let (r, s) = load(&mut j, &f);
    media.stages.set(0);
    framed(&mut j, &s, &r, &f).unwrap();
    println!(
        "framed_fixture encoded={} mutations={}",
        j.payload_length(&f.binding).unwrap().unwrap(),
        media.stages.get()
    );
}
