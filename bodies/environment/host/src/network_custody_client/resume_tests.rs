use super::*;
use crate::network_custody_client::tests::{fixture, Fixture};
fn offered(f: &Fixture) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({"profile":"network-retirement-offer@1",
        "snapshot":hex(&offer_id(&f.scope.root, &f.offer)), "offer":hex(&f.offer)}))
    .unwrap()
}
fn response(f: &Fixture, path: &str) -> Result<(u16, Vec<u8>)> {
    if path == "/network/custody/offer" {
        return Ok((200, offered(f)));
    }
    assert!(path.starts_with("/network/custody/chunk?snapshot="));
    assert!(!path.contains("ack"));
    let offset: usize = path.rsplit("offset=").next().unwrap().parse().unwrap();
    Ok((
        200,
        serde_json::to_vec(&serde_json::json!({"profile":"network-retirement-chunk@1",
        "snapshot":hex(&offer_id(&f.scope.root, &f.offer)), "offset":offset,
        "bytes":hex(&f.snapshot[offset..offset+CHUNK])}))
        .unwrap(),
    ))
}
fn retain(
    f: &Fixture,
    get: impl FnMut(&str) -> Result<(u16, Vec<u8>)>,
) -> Result<VerifiedDurableSnapshot> {
    retain_readonly(
        &f.directory,
        &f.scope.body,
        f.scope.root,
        &f.scope.producer_boot,
        get,
        || (),
    )
}
#[test]
fn two_expired_leases_resume_identical_snapshot_without_refetch_or_ack() {
    let f = fixture();
    let mut lease = 0;
    let mut chunks = 0;
    let token = retain(&f, |path| {
        if path == "/network/custody/offer" {
            lease += 1;
        } else {
            let offset: usize = path.rsplit("offset=").next().unwrap().parse().unwrap();
            if (lease == 1 && offset == 4096) || (lease == 2 && offset == 8192) {
                return Ok((409, vec![]));
            }
            chunks += 1;
        }
        response(&f, path)
    })
    .unwrap();
    assert_eq!(lease, 3);
    assert_eq!(chunks, CAPACITY / CHUNK);
    token.revalidate().unwrap();
    // Completed cache is independently cryptographically checked, without GET.
    retain(&f, |_| {
        panic!("completed cache must not request another lease")
    })
    .unwrap();
    assert!(!token.directory.join("ack.bin").exists());
}
#[test]
fn process_restart_retains_partial_and_burned_lease() {
    let f = fixture();
    let mut first = true;
    assert!(retain(&f, |path| {
        if path == "/network/custody/offer" {
            if first {
                first = false;
                return response(&f, path);
            }
            return Err("interrupted process".into());
        }
        if path.ends_with("offset=0") {
            return response(&f, path);
        }
        // A malformed response aborts this process while keeping the first lease.
        Ok((200, b"not-json".to_vec()))
    })
    .is_err());
    assert!(f.directory.join("lease-0.bin").exists());
    let mut refetched = false;
    let token = retain(&f, |path| {
        if path.ends_with("offset=0") {
            refetched = true;
        }
        response(&f, path)
    })
    .unwrap();
    assert!(!refetched);
    assert!(f.directory.join("lease-1.bin").exists());
    assert!(!f.directory.join("lease-2.bin").exists());
    token.revalidate().unwrap();
}
#[test]
fn finite_bound_persists_across_process_restart() {
    let f = fixture();
    let mut count = 0;
    assert!(retain(&f, |_| {
        count += 1;
        Ok((409, vec![]))
    })
    .is_err());
    assert_eq!(count, LEASES);
    assert!(retain(&f, |_| panic!("persisted finite leases must stay spent")).is_err());
}
#[test]
fn partial_tamper_and_changed_signed_snapshot_never_mint_token() {
    for changed in [false, true] {
        let mut f = fixture();
        assert!(retain(&f, |path| {
            if path.ends_with("offset=512") {
                return Ok((200, b"bad-json".to_vec()));
            }
            response(&f, path)
        })
        .is_err());
        if changed {
            use ed25519_dalek::Signer;
            f.snapshot.fill(18);
            f.offer[112..144].copy_from_slice(&digest(&f.snapshot));
            let id = offer_id(&f.scope.root, &f.offer);
            f.offer[176..].copy_from_slice(
                &ed25519_dalek::SigningKey::from_bytes(&[41; 32])
                    .sign(&id)
                    .to_bytes(),
            );
        } else {
            let chunks = f.directory.join(format!(
                "unverified-{}",
                hex(&offer_id(&f.scope.root, &f.offer))
            ));
            std::fs::write(chunks.join("00000.bin"), [99; CHUNK]).unwrap();
        }
        assert!(retain(&f, |path| response(&f, path)).is_err());
        assert!(!f
            .directory
            .join(format!(
                "network-{}",
                hex(&offer_id(&f.scope.root, &f.offer))
            ))
            .exists());
    }
}
#[test]
fn completed_cache_tamper_and_foreign_boot_refuse_without_transport() {
    let f = fixture();
    let token = retain(&f, |path| response(&f, path)).unwrap();
    std::fs::write(token.directory.join("snapshot.bin"), vec![42; CAPACITY]).unwrap();
    assert!(retain(&f, |_| panic!(
        "tampered completion cannot be redownloaded over"
    ))
    .is_err());
    assert!(retain_readonly(
        &f.directory,
        &f.scope.body,
        f.scope.root,
        &BootId::from("different"),
        |_| panic!("foreign scope"),
        || ()
    )
    .is_err());
}
#[test]
fn bad_signature_stops_before_retaining_any_partial() {
    let mut f = fixture();
    f.offer[239] ^= 1;
    let mut requests = 0;
    assert!(retain(&f, |path| {
        requests += 1;
        assert_eq!(path, "/network/custody/offer");
        Ok((200, offered(&f)))
    })
    .is_err());
    assert_eq!(requests, 1);
    assert!(!f.directory.join("offer.bin").exists());
}
#[test]
fn concurrent_export_refuses_before_transport_or_lease_spend() {
    let f = fixture();
    let lock = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(f.directory.join("export.lock"))
        .unwrap();
    // SAFETY: actual retained owned private fixture descriptor.
    assert_eq!(
        unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
    assert!(retain(&f, |_| panic!("concurrent request")).is_err());
    assert!(!f.directory.join("lease-0.bin").exists());
}

#[test]
fn incremental_export_one_get_per_step_and_same_durable_token() {
    let f = fixture();
    let mut export = Export::open(
        &f.directory,
        &f.scope.body,
        f.scope.root,
        &f.scope.producer_boot,
    )
    .unwrap();
    let mut total = 0;
    loop {
        let mut requests = 0;
        let result = export
            .advance(Instant::now() + Duration::from_secs(3), |path, deadline| {
                requests += 1;
                assert!(deadline <= Instant::now() + Duration::from_secs(3));
                response(&f, path)
            })
            .unwrap();
        assert!(requests <= 1);
        total += requests;
        if let Progress::Complete(token) = result {
            token.revalidate().unwrap();
            assert_eq!(total, 1 + CAPACITY / CHUNK);
            assert!(export
                .advance(Instant::now() + Duration::from_secs(3), |_, _| panic!(
                    "finished"
                ))
                .is_err());
            break;
        }
    }
    assert!(!f.directory.join("lease-1.bin").exists());
    assert!(!f.directory.join("ack.bin").exists());
}
#[test]
fn incremental_deadlines_derive_now_and_do_not_extend_active_lease() {
    let f = fixture();
    let mut export = Export::open(
        &f.directory,
        &f.scope.body,
        f.scope.root,
        &f.scope.producer_boot,
    )
    .unwrap();
    export
        .advance(Instant::now() - Duration::from_secs(1), |_, _| {
            panic!("expired budget")
        })
        .unwrap();
    assert!(!f.directory.join("lease-0.bin").exists());
    let before = Instant::now();
    export
        .advance(before + Duration::from_secs(1000), |path, deadline| {
            assert!(deadline <= before + Duration::from_secs(151));
            response(&f, path)
        })
        .unwrap();
    let lease = export.lease_deadline().unwrap();
    let short = Instant::now() + Duration::from_secs(1);
    export
        .advance(short, |path, deadline| {
            assert_eq!(deadline, short.min(lease));
            response(&f, path)
        })
        .unwrap();
    export
        .advance(
            Instant::now() + Duration::from_secs(1000),
            |path, deadline| {
                assert_eq!(deadline, lease);
                response(&f, path)
            },
        )
        .unwrap();
    assert_eq!(export.lease_deadline(), Some(lease));
}
#[test]
fn incremental_interruption_spends_lease_and_resumes_only_same_signed_scope() {
    let f = fixture();
    {
        let mut export = Export::open(
            &f.directory,
            &f.scope.body,
            f.scope.root,
            &f.scope.producer_boot,
        )
        .unwrap();
        for _ in 0..2 {
            export
                .advance(Instant::now() + Duration::from_secs(3), |path, _| {
                    response(&f, path)
                })
                .unwrap();
        }
    }
    let mut export = Export::open(
        &f.directory,
        &f.scope.body,
        f.scope.root,
        &f.scope.producer_boot,
    )
    .unwrap();
    export
        .advance(Instant::now() + Duration::from_secs(3), |path, _| {
            assert_eq!(path, "/network/custody/offer");
            response(&f, path)
        })
        .unwrap();
    export
        .advance(Instant::now() + Duration::from_secs(3), |path, _| {
            assert!(path.ends_with("offset=512"));
            response(&f, path)
        })
        .unwrap();
    assert!(f.directory.join("lease-1.bin").exists());
}
#[test]
fn incremental_pending_requests_exhaust_finite_poll_and_lease_budget() {
    let f = fixture();
    let mut export = Export::open(
        &f.directory,
        &f.scope.body,
        f.scope.root,
        &f.scope.producer_boot,
    )
    .unwrap();
    let mut total = 0;
    loop {
        let result = export.advance(Instant::now() + Duration::from_secs(3), |_, _| {
            total += 1;
            Ok((202, vec![]))
        });
        if result.is_err() {
            break;
        }
        assert!(total <= LEASES * 376);
    }
    assert_eq!(total, LEASES * 376);
    drop(export);
    assert!(Export::open(
        &f.directory,
        &f.scope.body,
        f.scope.root,
        &f.scope.producer_boot
    )
    .unwrap()
    .advance(Instant::now() + Duration::from_secs(3), |_, _| panic!(
        "spent after restart"
    ))
    .is_err());
}
#[test]
fn incremental_late_offer_is_not_published_or_used_as_chunk_authority() {
    let f = fixture();
    let mut export = Export::open(
        &f.directory,
        &f.scope.body,
        f.scope.root,
        &f.scope.producer_boot,
    )
    .unwrap();
    export
        .advance(Instant::now() + Duration::from_millis(1), |_, _| {
            std::thread::sleep(Duration::from_millis(5));
            Ok((200, offered(&f)))
        })
        .unwrap();
    assert!(!f.directory.join("offer.bin").exists());
    export
        .advance(Instant::now() + Duration::from_secs(3), |path, _| {
            assert_eq!(path, "/network/custody/offer");
            response(&f, path)
        })
        .unwrap();
    assert!(!f.directory.join("lease-1.bin").exists());
}
#[test]
fn incremental_authenticated_refusal_is_terminal_for_this_owner() {
    let mut f = fixture();
    f.offer[239] ^= 1;
    let mut export = Export::open(
        &f.directory,
        &f.scope.body,
        f.scope.root,
        &f.scope.producer_boot,
    )
    .unwrap();
    assert!(export
        .advance(Instant::now() + Duration::from_secs(3), |_, _| Ok((
            200,
            offered(&f)
        )))
        .is_err());
    assert!(export
        .advance(Instant::now() + Duration::from_secs(3), |_, _| panic!(
            "terminal refusal cannot retry"
        ))
        .is_err());
    assert!(!f.directory.join("offer.bin").exists());
}
