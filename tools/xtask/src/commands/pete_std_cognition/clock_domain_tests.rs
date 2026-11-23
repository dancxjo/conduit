//! Pure source/identity guards; these fixtures do not alter Linux namespaces.
use super::*;
use std::{cell::Cell, io::Cursor};

const BOOT: &str = "12345678-1234-4234-8234-123456789abc";
const OTHER_BOOT: &str = "12345678-1234-4234-8234-123456789abd";
const OFFSETS_TEXT: &str = "monotonic 0 0\nboottime 0 0\n";
const NS: NamespaceId = NamespaceId {
    device: 1,
    inode: 2,
};
const OTHER_NS: NamespaceId = NamespaceId {
    device: 1,
    inode: 3,
};
fn pair(namespace: NamespaceId) -> Namespaces {
    Namespaces {
        active: namespace,
        children: namespace,
        leader_active: namespace,
        offset_source: namespace,
    }
}
fn domain(boot: &str, offsets: &str, namespace: NamespaceId) -> String {
    prepare_domain(
        boot,
        namespace,
        || Ok(pair(namespace)),
        || Ok(offsets.into()),
    )
    .unwrap()
}

#[test]
fn equal_scalar_domains_share_identity_without_process_or_namespace_inode() {
    let original = domain(BOOT, OFFSETS_TEXT, NS);
    assert_eq!(original, domain(BOOT, OFFSETS_TEXT, OTHER_NS));
    assert_eq!(
        original,
        domain(
            &format!("{BOOT}\n"),
            "boottime 99 1\nmonotonic -0 000\n",
            NS
        )
    );
    assert!(original.len() <= 256);
    assert!(original.starts_with("clock/linux-monotonic@1/"));
    for different in [
        domain(OTHER_BOOT, OFFSETS_TEXT, NS),
        domain(BOOT, "monotonic 0 1\nboottime 0 0\n", NS),
    ] {
        assert_ne!(original, different);
        let instant = |basis| {
            NativeTemporalInstant::new(basis, 1, NativeTemporalScale::Milliseconds, 100, 0).unwrap()
        };
        assert_eq!(
            instant(original.clone()).relation_to(&instant(different)),
            Err(conduit_core::TemporalRelationError::Incomparable)
        );
    }
    assert_eq!(
        monotonic_offset("monotonic -1 500000000\nboottime 0 0\n").unwrap(),
        (-1, 500_000_000)
    );
}

#[test]
fn malformed_boot_uuid_refuses_without_a_fallback() {
    for input in [
        "",
        "clock/pete-cognition",
        "00000000-0000-0000-0000-000000000000",
        "12345678-1234-4234-8234-123456789abC",
        "1234567811234-4234-8234-123456789abc",
        "12345678-1234-4234-8234-123456789abc\n\n",
        " 12345678-1234-4234-8234-123456789abc",
    ] {
        assert!(boot_uuid(input).is_err(), "{input:?}");
    }
}

#[test]
fn missing_duplicate_unknown_and_invalid_offsets_refuse() {
    for input in [
        "",
        "monotonic 0 0\n",
        "boottime 0 0\n",
        "monotonic 0 0\nmonotonic 0 1\nboottime 0 0\n",
        "monotonic 0 0\nboottime 0 0\nboottime 0 0\n",
        "monotonic 0 0\nrealtime 0 0\n",
        "monotonic 0 0\nboottime 0 0 extra\n",
        "monotonic 0 -1\nboottime 0 0\n",
        "monotonic 0 1000000000\nboottime 0 0\n",
        "monotonic 9223372036854775808 0\nboottime 0 0\n",
        "monotonic 0 0\nboottime x 0\n",
        "\nmonotonic 0 0\nboottime 0 0\n",
    ] {
        assert!(monotonic_offset(input).is_err(), "{input:?}");
    }
}

#[test]
fn bounded_source_reads_accept_exact_bytes_and_stop_after_one_extra_byte() {
    let mut reader = Cursor::new(b"abcde".as_slice());
    assert_eq!(
        read_bounded(&mut reader, 3).unwrap_err(),
        "byte bound exceeded"
    );
    assert_eq!(reader.position(), 4);
    assert_eq!(read_bounded(Cursor::new(b"abc"), 3).unwrap(), "abc");
    assert!(read_bounded(Cursor::new([0xff]), 1)
        .unwrap_err()
        .contains("UTF-8"));
    struct Broken;
    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("fixture"))
        }
    }
    assert!(read_bounded(Broken, BOOT_BOUND)
        .unwrap_err()
        .contains("read failed"));
    assert!(read_bounded(Cursor::new(vec![b'x'; BOOT_BOUND + 1]), BOOT_BOUND).is_err());
    assert!(read_bounded(Cursor::new(vec![b'x'; OFFSETS_BOUND + 1]), OFFSETS_BOUND).is_err());
}

#[test]
fn ambiguous_or_changed_namespace_never_publishes_a_domain() {
    for before in [
        Namespaces {
            active: NS,
            children: OTHER_NS,
            ..pair(NS)
        },
        pair(OTHER_NS),
    ] {
        assert!(prepare_domain(
            BOOT,
            NS,
            || Ok(before),
            || panic!("must refuse before offset read")
        )
        .is_err());
    }
    for after in [
        pair(OTHER_NS),
        Namespaces {
            active: NS,
            children: OTHER_NS,
            ..pair(NS)
        },
    ] {
        let mut sequence = [pair(NS), after].into_iter();
        assert!(prepare_domain(
            BOOT,
            NS,
            || Ok(sequence.next().unwrap()),
            || Ok(OFFSETS_TEXT.into())
        )
        .unwrap_err()
        .contains("changed during preparation"));
    }
    assert!(prepare_domain(
        BOOT,
        NS,
        || Err("namespace unavailable".into()),
        || panic!("no offset read")
    )
    .is_err());
    assert!(prepare_domain(
        BOOT,
        NS,
        || Ok(pair(NS)),
        || Err("offsets unavailable".into())
    )
    .is_err());
}

#[test]
fn prepared_sampler_refuses_changed_namespace_without_relabeling_ticks() {
    let sampled = Cell::new(0);
    let sample = || {
        sampled.set(sampled.get() + 1);
        Ok(100)
    };
    assert_eq!(sample_in_domain(NS, || Ok(NS), sample).unwrap(), 100);
    assert_eq!(sampled.get(), 1);
    assert!(sample_in_domain(NS, || Ok(OTHER_NS), sample).is_err());
    assert_eq!(sampled.get(), 1, "no clock read on pre-sample mismatch");
    let mut sequence = [NS, OTHER_NS].into_iter();
    assert!(sample_in_domain(NS, || Ok(sequence.next().unwrap()), sample).is_err());
    assert_eq!(
        sampled.get(),
        2,
        "post-sample mismatch discards that one read"
    );
    assert!(sample_in_domain(NS, || Err("namespace unavailable".into()), sample).is_err());
    assert_eq!(sampled.get(), 2);
    assert!(sample_in_domain(NS, || Ok(NS), || Err("sample unavailable".into())).is_err());
}

#[test]
fn leader_offset_source_mismatch_refuses_before_reading_offsets() {
    for before in [
        Namespaces {
            offset_source: OTHER_NS,
            ..pair(NS)
        },
        Namespaces {
            leader_active: OTHER_NS,
            ..pair(NS)
        },
    ] {
        assert!(prepare_domain(
            BOOT,
            NS,
            || Ok(before),
            || panic!("leader/source mismatch must refuse before offset read"),
        )
        .unwrap_err()
        .contains("caller/offset-source namespace mismatch"));
    }
}

#[test]
fn leader_offset_source_transition_discards_the_single_bounded_read() {
    for after in [
        Namespaces {
            offset_source: OTHER_NS,
            ..pair(NS)
        },
        Namespaces {
            leader_active: OTHER_NS,
            ..pair(NS)
        },
    ] {
        let mut sequence = [pair(NS), after].into_iter();
        let reads = Cell::new(0);
        let result = prepare_domain(
            BOOT,
            NS,
            || Ok(sequence.next().unwrap()),
            || {
                reads.set(reads.get() + 1);
                Ok(OFFSETS_TEXT.into())
            },
        );
        assert!(result.unwrap_err().contains("changed during preparation"));
        assert_eq!(reads.get(), 1, "no offset retry or fallback source");
    }
}
