// CPU adapter for the actual Root mailbox/lease code. Lifecycle admission is a
// stand-in here; production NativeAdmission/live-checkpoint checks are separate.
mod esp_println {
    pub use std::println;
}
mod native_play {
    pub(crate) use crate::journal;
}
mod native_restart {
    pub fn publication_ready() -> bool {
        true
    }
}
mod embassy_time {
    pub static NOW: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1000);
    pub struct Instant(u64);
    impl Instant {
        pub fn now() -> Self {
            Self(NOW.load(std::sync::atomic::Ordering::Relaxed))
        }
        pub fn as_millis(&self) -> u64 {
            self.0
        }
    }
}
mod local_renewal {
    pub fn reserve_box<T>() -> Result<Box<core::mem::MaybeUninit<T>>, &'static str> {
        let layout = core::alloc::Layout::new::<T>();
        let raw = unsafe { alloc::alloc::alloc(layout) }.cast::<core::mem::MaybeUninit<T>>();
        if raw.is_null() {
            return Err("test-capacity");
        }
        Ok(unsafe { Box::from_raw(raw) })
    }
}
struct NativeRoot {
    body: Option<()>,
    pending: bool,
    body_id: conduit_body::BodyId,
    boot: conduit_core::BootId,
    key: ed25519_dalek::SigningKey,
}
impl NativeRoot {
    fn local_renewal_pending(&self) -> bool {
        self.pending
    }
    fn begin_network_snapshot(&self) -> Result<journal::snapshot::Verifier, &'static str> {
        Ok(journal::snapshot::Verifier::start(journal::inspect()?))
    }
    fn endorse_network_snapshot(
        &self,
        verified: journal::snapshot::Verified,
    ) -> Result<(journal::snapshot::Draft, [u8; 240]), &'static str> {
        use ed25519_dalek::Signer;
        let draft = verified.offer(
            journal::snapshot::Scope {
                body: &self.body_id,
                boot: &self.boot,
                epoch: 0,
                predecessor: [0; 32],
            },
            self.key.verifying_key().to_bytes(),
        )?;
        let signed = draft.signed(self.key.sign(&draft.transcript()).to_bytes());
        Ok((draft, signed))
    }
}
#[path = "../../../../targets/esp32/proof/tm1637/src/native_root_network_export.rs"]
mod actual_root_export;
fn exercise_root_export() {
    use conduit_environment_http::network_custody::{self as http, Reply, Request};
    let mut root = NativeRoot {
        body: Some(()),
        pending: false,
        body_id: serde_json::from_str("\"body/network-export-test\"").unwrap(),
        boot: conduit_core::BootId::new("boot/export"),
        key: ed25519_dalek::SigningKey::from_bytes(&[0x51; 32]),
    };
    root.advance_network_export();
    FREE.store(0, Ordering::Relaxed);
    assert!(matches!(http::dispatch(Request::Offer), Reply::Pending));
    root.advance_network_export();
    assert!(journal::inspect().is_ok());
    assert!(matches!(http::dispatch(Request::Offer), Reply::Unavailable));
    FREE.store(1_000_000, Ordering::Relaxed);
    embassy_time::NOW.store(31001, Ordering::Relaxed);
    assert!(matches!(http::dispatch(Request::Offer), Reply::Pending));
    DRAIN_AFTER_PREFLIGHT.store(true, Ordering::Relaxed);
    root.advance_network_export();
    assert!(journal::inspect().is_ok());
    FREE.store(1_000_000, Ordering::Relaxed);
    embassy_time::NOW.store(61002, Ordering::Relaxed);
    assert!(matches!(http::dispatch(Request::Offer), Reply::Pending));
    root.pending = true;
    root.advance_network_export();
    assert!(journal::inspect().is_ok());
    root.pending = false;
    for _ in 0..64 {
        root.advance_network_export();
    }
    assert!(journal::inspect().is_err());
    {
        let _refusal =
            allocation_probe::refuse_next_allocation_size(journal::snapshot::export::REPLY_BYTES);
        assert!(matches!(http::dispatch(Request::Offer), Reply::Unavailable));
    }
    assert!(journal::inspect().is_err());
    DRAIN_AFTER_PREFLIGHT.store(true, Ordering::Relaxed);
    assert!(matches!(http::dispatch(Request::Offer), Reply::Unavailable));
    assert!(journal::inspect().is_err());
    FREE.store(1_000_000, Ordering::Relaxed);
    let Reply::Offer(bytes) = http::dispatch(Request::Offer) else {
        panic!("completed Root offer")
    };
    let offer: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let hex = offer["snapshot"].as_str().unwrap();
    let mut id = [0; 32];
    for (i, out) in id.iter_mut().enumerate() {
        *out = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap();
    }
    assert!(matches!(
        http::dispatch(Request::Chunk {
            snapshot: id,
            offset: 32256
        }),
        Reply::Offer(_)
    ));
    // Expiration runs even while Body replacement preparation is deferred.
    root.pending = true;
    embassy_time::NOW.store(181002, Ordering::Relaxed);
    root.advance_network_export();
    assert!(journal::inspect().is_ok());
    assert!(matches!(
        http::dispatch(Request::Chunk {
            snapshot: id,
            offset: 0
        }),
        Reply::Refused
    ));
    assert!(matches!(http::dispatch(Request::Offer), Reply::Unavailable));
    embassy_time::NOW.store(211003, Ordering::Relaxed);
    assert!(matches!(http::dispatch(Request::Offer), Reply::Pending));
    root.advance_network_export();
    assert!(journal::inspect().is_ok());
    root.pending = false;
    root.advance_network_export();
    assert!(journal::inspect().is_err());
    // An abandoned verification also drops its affine owner at the same bound.
    embassy_time::NOW.store(331003, Ordering::Relaxed);
    root.advance_network_export();
    assert!(journal::inspect().is_ok());
}
