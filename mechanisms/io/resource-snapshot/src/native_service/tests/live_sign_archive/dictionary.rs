//! Explicit dictionary/tape candidate; identical native-service workload and
//! every-64-read sampling schedule to the independently retained refusal test.
use super::*;
use conduit_kernel::dictionary_live_sign_archive::DictionaryLiveSignLog;

type Evidence = DictionaryLiveSignLog<128, 4, 48, 2, 2>;
type Kernel = FixedScheduler<Back, HostedValueStore, Evidence, 3, 4, 2, 4, 6, 3, 6, 2>;

// Reuse the exact driver. Its legacy profile-specific report is deliberately
// unused: this candidate has separately explicit dictionary/tape stocks.
#[allow(dead_code, clippy::duplicate_mod)]
#[path = "workload.rs"]
mod workload;
use workload::Workload;

fn evidence() -> Evidence {
    let active = HostedSignLog::new_with_remote_storage(
        61,
        61 * core::mem::size_of::<KernelEvent>() as u32,
        6,
        conduit_kernel::remote_sign_storage_bytes(6).unwrap(),
    )
    .unwrap();
    assert!(active.reserved_storage_bytes() <= 24576);
    assert!(HostedSignLog::canonical_logical_storage_bytes(61, 6).unwrap() <= 24576);
    assert!(Evidence::required_archive_backing_bytes() <= 8192);
    assert!(Evidence::required_transfer_overlap_bytes(&active).unwrap() <= 12288);
    println!("ACTUAL_SERVICE_DICTIONARY_STORAGE_ROW {{\"archive_backing_bytes\":{},\"transfer_overlap_bytes\":{},\"scope\":\"test target owned backing; excludes allocator headers, caller, stack and Root\"}}",
        Evidence::required_archive_backing_bytes(), Evidence::required_transfer_overlap_bytes(&active).unwrap());
    Evidence::try_prepare_dictionary(
        active,
        ArchiveLimits {
            retained_events: 32768,
            retained_bytes: 2 * 1024 * 1024,
            archive_backing_bytes: 8192,
            transfer_overlap_bytes: 12288,
        },
    )
    .unwrap()
}

#[test]
fn dictionary_tape_1264_reads_with_identical_periodic_actual_appends() {
    let mut workload = Workload::new();
    for index in 0..HISTORY {
        workload.read(index, index % 64 == 0);
        workload.archive().unwrap();
    }
    assert_eq!(
        workload
            .kernel
            .signs()
            .events_with_remote()
            .collect::<Vec<_>>(),
        workload.expected
    );
    assert_eq!(workload.samples, 20);
    assert_eq!(
        workload.service.inspection(0).unwrap().completed_operations,
        20
    );
    assert_eq!(
        workload.service.inspection(1).unwrap().completed_operations,
        HISTORY
    );
    let signs = workload.kernel.signs();
    let remote = signs
        .events_with_remote()
        .filter(|entry| entry.remote.is_some())
        .count();
    assert_eq!(remote, 5056);
    assert_eq!(signs.archived_event_count(), 15472);
    assert_eq!(signs.dictionary_stats(), (70, 3, 40, 1264));
    println!("ACTUAL_SERVICE_DICTIONARY_ROW {{\"completed_reads\":1264,\"sampling_completions\":20,\"remote_events\":{remote},\"archived_events\":{},\"canonical_bytes\":{},\"dictionary_stats\":{:?},\"archive_backing_bytes\":{},\"owned_backing_bytes\":{},\"template_event_stock\":128,\"template_stock\":4,\"tape_stock\":48,\"request_lanes\":2,\"remote_lanes\":2,\"active_items\":61,\"remote_items\":6,\"archive_backing_limit\":8192,\"overlap_limit\":12288,\"scope\":\"real native record-service and NOR; synthetic sampling; not Source, Root admission, transport or HIL\"}}",
        signs.archived_event_count(), signs.archived_canonical_bytes(), signs.dictionary_stats(),
        Evidence::required_archive_backing_bytes(), signs.owned_backing_bytes().unwrap());
}
