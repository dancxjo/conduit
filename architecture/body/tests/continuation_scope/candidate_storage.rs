//! Caller-owned candidate output, never another command or retained heap image.
use conduit_body::*;
use conduit_core as c;
#[allow(clippy::too_many_arguments)]
pub(super) fn check(
    intent: &VerifiedContinuationIntent<'_>,
    bundle: &CheckedContinuationBundle,
    full: &ContinuationFullScope<'_>,
    actual: &c::HostAdvertisement,
    pins: &[ContinuationBundleEntry<'_>],
    entries: &[ContinuationLocalEntry<'_>],
    documents: &[&[u8]],
    scratch: &mut [u8],
    budget: usize,
) {
    let rows: Vec<Vec<_>> = documents
        .iter()
        .map(|document| {
            let (metadata, _) = decode_body_fragment_document(document).unwrap();
            FragmentResources::new(metadata.resource_bindings)
                .map(Result::unwrap)
                .collect()
        })
        .collect();
    let resources: Vec<_> = rows.iter().map(Vec::as_slice).collect();
    let total: usize = documents.iter().map(|d| d.len()).sum();
    let mut output = vec![0xa5; total + 16];
    let (result, cost) = super::allocation_probe::observe(|| {
        build_continuation_candidate_with_resources_into::<0>(
            intent,
            bundle,
            full,
            actual,
            pins,
            entries,
            &resources,
            &mut output[..total],
            scratch,
            budget,
        )
    });
    let ranges = result.unwrap();
    assert_eq!(ranges.len(), documents.len());
    assert_eq!(ranges.used_bytes(), total);
    assert_eq!(cost.reallocations, 0);
    assert_eq!(
        cost.allocations, cost.deallocations,
        "all temporary allocations retire before returning ranges"
    );
    for (i, original) in documents.iter().enumerate() {
        assert_eq!(ranges.document(&output, i), Some(*original));
    }
    assert_eq!(
        &output[total..],
        &[0xa5; 16],
        "outside admitted output untouched"
    );
    assert!(ranges.range(documents.len()).is_none());
    assert!(ranges.document(&output[..total - 1], 0).is_none());
    assert!(matches!(
        build_continuation_candidate_with_resources_into::<0>(
            intent,
            bundle,
            full,
            actual,
            pins,
            entries,
            &resources,
            &mut output[..total - 1],
            scratch,
            budget
        ),
        Err(ContinuationScopeRefusal::Bounds)
    ));
    let (short, cost) = super::allocation_probe::observe(|| {
        build_continuation_candidate_with_resources_into::<0>(
            intent,
            bundle,
            full,
            actual,
            pins,
            entries,
            &resources,
            &mut output[..total],
            &mut scratch[..1],
            budget,
        )
    });
    assert!(matches!(short, Err(ContinuationScopeRefusal::Bounds)));
    assert_eq!(
        cost.allocations, 0,
        "short scratch refuses before allocating"
    );
    if rows.iter().flatten().any(|r| r.content.is_some()) {
        let mut changed = rows.clone();
        for row in changed.iter_mut().flatten() {
            if let Some(content) = &mut row.content {
                content.contract.maximum_items += 1;
            }
        }
        let changed_rows: Vec<_> = changed.iter().map(Vec::as_slice).collect();
        let mut inventory = actual.clone();
        for pool in &mut inventory.resources {
            if let Some(content) = &mut pool.content {
                content.contract.maximum_items += 1;
            }
        }
        assert!(
            build_continuation_candidate_with_resources_into::<0>(
                intent,
                bundle,
                full,
                &inventory,
                pins,
                entries,
                &changed_rows,
                &mut output[..total],
                scratch,
                budget
            )
            .is_err(),
            "joint row/inventory drift cannot replace signed static recipe facts"
        );
    }
}
