//! CPU SDK shim exercising the actual OwnedScan source, not target ABI proof.
//! Run directly with rustc --edition2024 --test tests/bounded_scan_host.rs.
#![allow(non_camel_case_types, dead_code)]
use std::{cell::RefCell, collections::VecDeque};
#[derive(Default)]
struct Sdk {
    starts: usize,
    stops: usize,
    clears: usize,
    reads: usize,
    dropped: usize,
    start_error: i32,
    stop_error: i32,
    clear_error: i32,
    read_error_at: Option<usize>,
    total: u16,
    events: VecDeque<event::MessageResult>,
}
thread_local! { static SDK: RefCell<Sdk> = RefCell::new(Sdk::default()); }
struct WifiController<'d>(core::marker::PhantomData<&'d ()>);
impl Drop for WifiController<'_> {
    fn drop(&mut self) {
        SDK.with_borrow_mut(|s| s.dropped += 1);
    }
}
mod scan {
    #[derive(Default)]
    pub struct ScanConfig {
        pub(crate) max: Option<usize>,
    }
}
fn wifi_start_scan(_: bool, _: scan::ScanConfig) -> i32 {
    SDK.with_borrow_mut(|s| {
        s.starts += 1;
        s.start_error
    })
}
// Production config is Copy and start borrows it before passing by value.
impl Copy for scan::ScanConfig {}
impl Clone for scan::ScanConfig {
    fn clone(&self) -> Self {
        *self
    }
}
mod event {
    #[derive(Clone)]
    pub enum EventInfo {
        ScanDone {
            status: u32,
            number: u8,
            scan_id: u8,
        },
        Other,
    }
    #[derive(Clone)]
    pub enum MessageResult {
        Lagged(u64),
        Message(EventInfo),
    }
    pub struct Channel;
    pub static EVENT_CHANNEL: Channel = Channel;
    pub struct Subscriber;
    impl Channel {
        pub fn subscriber(&self) -> Result<Subscriber, ()> {
            Ok(Subscriber)
        }
    }
    pub struct EventSubscriber<'a>(core::marker::PhantomData<&'a ()>);
    impl EventSubscriber<'_> {
        pub fn new(_: Subscriber) -> Self {
            Self(core::marker::PhantomData)
        }
        pub async fn next_event(&mut self) -> MessageResult {
            core::future::poll_fn(|_| {
                super::SDK.with_borrow_mut(|s| match s.events.pop_front() {
                    Some(event) => core::task::Poll::Ready(event),
                    None => core::task::Poll::Pending,
                })
            })
            .await
        }
    }
}
mod sys {
    pub mod include {
        pub const ESP_OK: u32 = 0;
        pub struct wifi_ap_record_t {
            pub ssid: [u8; 33],
            pub rssi: i8,
            pub authmode: u32,
        }
        pub unsafe fn esp_wifi_scan_stop() -> i32 {
            super::super::SDK.with_borrow_mut(|s| {
                s.stops += 1;
                s.stop_error
            })
        }
        pub unsafe fn esp_wifi_clear_ap_list() -> i32 {
            super::super::SDK.with_borrow_mut(|s| {
                s.clears += 1;
                s.clear_error
            })
        }
        pub unsafe fn esp_wifi_scan_get_ap_num(out: *mut u16) -> i32 {
            unsafe {
                out.write(super::super::SDK.with_borrow(|s| s.total));
            }
            0
        }
        pub unsafe fn esp_wifi_scan_get_ap_record(out: *mut wifi_ap_record_t) -> i32 {
            super::super::SDK.with_borrow_mut(|s| {
                let at = s.reads;
                s.reads += 1;
                if s.read_error_at == Some(at) {
                    return 73;
                }
                let mut ssid = [0; 33];
                ssid[..32].fill(at as u8 + 1);
                unsafe {
                    out.write(wifi_ap_record_t {
                        ssid,
                        rssi: -42,
                        authmode: u32::MAX,
                    });
                }
                0
            })
        }
    }
}
#[path = "../src/wifi/bounded_scan.rs"]
mod production;
use core::{
    future::Future,
    task::{Context, Poll, Waker},
};
use production::{Error, OwnedScan, Record};
fn poll<F: Future>(future: core::pin::Pin<&mut F>) -> Poll<F::Output> {
    future.poll(&mut Context::from_waker(Waker::noop()))
}
fn owner() -> OwnedScan<'static> {
    SDK.with_borrow_mut(|s| *s = Sdk::default());
    // The shim begins in its explicitly empty, quiescent state.
    unsafe { OwnedScan::from_quiescent(WifiController(core::marker::PhantomData)) }
}
fn event(status: u32, number: u8, id: u8) {
    SDK.with_borrow_mut(|s| {
        s.events
            .push_back(event::MessageResult::Message(event::EventInfo::ScanDone {
                status,
                number,
                scan_id: id,
            }))
    });
}
fn returned(owner: OwnedScan<'_>) -> WifiController<'_> {
    match owner.try_into_controller() {
        Ok(controller) => controller,
        Err(_) => panic!("terminal/list proof should return controller"),
    }
}
#[test]
fn cancelled_wait_and_cleanup_keep_actual_owner_until_real_terminal_and_clear() {
    let mut owner = owner();
    owner.start(&scan::ScanConfig::default(), 16).unwrap();
    {
        let mut wait = core::pin::pin!(owner.wait_terminal());
        assert!(poll(wait.as_mut()).is_pending());
    }
    assert!(matches!(
        owner.start(&scan::ScanConfig::default(), 16),
        Err(Error::Busy)
    ));
    {
        let mut cleanup = core::pin::pin!(owner.cleanup());
        assert!(poll(cleanup.as_mut()).is_pending());
    }
    SDK.with_borrow(|s| {
        assert_eq!(s.stops, 1);
        assert_eq!(s.clears, 0);
        assert_eq!(s.dropped, 0);
    });
    event(1, 0, 251);
    {
        let mut cleanup = core::pin::pin!(owner.cleanup());
        assert!(matches!(poll(cleanup.as_mut()), Poll::Ready(Ok(()))));
    }
    SDK.with_borrow(|s| {
        assert_eq!(s.stops, 1);
        assert_eq!(s.clears, 1);
    });
    drop(returned(owner));
    SDK.with_borrow(|s| assert_eq!(s.dropped, 1));
}
#[test]
fn actual_driver_caps_copy_at16_and_preserves_unknown_auth_total_and_event_id() {
    let mut owner = owner();
    owner.start(&scan::ScanConfig::default(), 16).unwrap();
    SDK.with_borrow_mut(|s| s.total = 300);
    event(0, 44, 0);
    let mut output = [Record::default(); 16];
    let report = {
        let mut finish = core::pin::pin!(owner.finish(&mut output));
        match poll(finish.as_mut()) {
            Poll::Ready(Ok(r)) => r,
            _ => panic!("ready terminal/list"),
        }
    };
    assert_eq!(
        (report.total, report.copied, report.truncated),
        (300, 16, true)
    );
    assert_eq!(
        (
            report.completion.status,
            report.completion.number,
            report.completion.scan_id
        ),
        (0, 44, 0)
    );
    assert!(!report.stop_requested);
    for (at, record) in output.iter().enumerate() {
        assert_eq!(record.authentication, u32::MAX);
        assert_eq!(record.ssid, [at as u8 + 1; 32]);
    }
    SDK.with_borrow(|s| {
        assert_eq!(s.reads, 16);
        assert_eq!(s.clears, 1);
    });
    drop(returned(owner));
}
#[test]
fn stop_error_is_not_quiescence_and_later_terminal_can_complete_cleanup() {
    let mut owner = owner();
    owner.start(&scan::ScanConfig::default(), 1).unwrap();
    SDK.with_borrow_mut(|s| s.stop_error = 97);
    assert!(matches!(owner.request_stop(), Err(Error::Sdk(97))));
    let mut owner = match owner.try_into_controller() {
        Err(owner) => owner,
        Ok(_) => panic!("stop error cannot return owner"),
    };
    event(1, 0, 13);
    {
        let mut cleanup = core::pin::pin!(owner.cleanup());
        assert!(matches!(poll(cleanup.as_mut()), Poll::Ready(Ok(()))));
    }
    SDK.with_borrow(|s| assert_eq!(s.stops, 1));
    drop(returned(owner));
}
#[test]
fn clear_refusal_retains_controller_and_cleanup_retries_only_list_clear() {
    let mut owner = owner();
    owner.start(&scan::ScanConfig::default(), 1).unwrap();
    event(0, 0, 6);
    SDK.with_borrow_mut(|s| s.clear_error = 91);
    let mut output = [Record::default(); 1];
    {
        let mut finish = core::pin::pin!(owner.finish(&mut output));
        assert!(matches!(
            poll(finish.as_mut()),
            Poll::Ready(Err(Error::Sdk(91)))
        ));
    }
    let mut owner = match owner.try_into_controller() {
        Err(owner) => owner,
        Ok(_) => panic!("uncleared list cannot return owner"),
    };
    SDK.with_borrow_mut(|s| s.clear_error = 0);
    {
        let mut cleanup = core::pin::pin!(owner.cleanup());
        assert!(matches!(poll(cleanup.as_mut()), Poll::Ready(Ok(()))));
    }
    SDK.with_borrow(|s| {
        assert_eq!(s.stops, 0);
        assert_eq!(s.clears, 2);
    });
    drop(returned(owner));
}
#[test]
fn lag_poison_refuses_publication_but_retained_owner_can_reconcile_real_terminal() {
    let mut owner = owner();
    owner.start(&scan::ScanConfig::default(), 1).unwrap();
    SDK.with_borrow_mut(|s| s.events.push_back(event::MessageResult::Lagged(1)));
    {
        let mut wait = core::pin::pin!(owner.wait_terminal());
        assert!(poll(wait.as_mut()).is_pending());
    }
    let mut owner = match owner.try_into_controller() {
        Err(owner) => owner,
        Ok(_) => panic!("missing terminal cannot return owner"),
    };
    event(0, 0, 1);
    let mut out = [Record::default(); 1];
    {
        let mut finish = core::pin::pin!(owner.finish(&mut out));
        assert!(matches!(
            poll(finish.as_mut()),
            Poll::Ready(Err(Error::Quarantined))
        ));
    }
    SDK.with_borrow(|s| {
        assert_eq!(s.reads, 0);
        assert_eq!(s.clears, 0);
    });
    {
        let mut cleanup = core::pin::pin!(owner.cleanup());
        assert!(matches!(poll(cleanup.as_mut()), Poll::Ready(Ok(()))));
    }
    drop(returned(owner));
    SDK.with_borrow(|s| {
        assert_eq!(s.dropped, 1);
        assert_eq!(s.clears, 1);
    });
}
#[test]
fn cancelling_after_terminal_receipt_keeps_list_custody_until_cleanup() {
    let mut owner = owner();
    owner.start(&scan::ScanConfig::default(), 1).unwrap();
    event(0, 0, 7);
    {
        let mut wait = core::pin::pin!(owner.wait_terminal());
        assert!(matches!(poll(wait.as_mut()), Poll::Ready(Ok(_))));
    }
    let mut owner = match owner.try_into_controller() {
        Err(owner) => owner,
        Ok(_) => panic!("terminal alone cannot return uncleared list"),
    };
    SDK.with_borrow(|s| assert_eq!(s.clears, 0));
    {
        let mut cleanup = core::pin::pin!(owner.cleanup());
        assert!(matches!(poll(cleanup.as_mut()), Poll::Ready(Ok(()))));
    }
    SDK.with_borrow(|s| {
        assert_eq!(s.stops, 0);
        assert_eq!(s.clears, 1);
    });
    drop(returned(owner));
}
#[test]
fn sdk_start_error_and_late_success_event_reconcile_without_publishing_success() {
    let mut owner = owner();
    SDK.with_borrow_mut(|s| s.start_error = 0x101);
    assert!(matches!(
        owner.start(&scan::ScanConfig::default(), 1),
        Err(Error::Sdk(0x101))
    ));
    {
        let mut cleanup = core::pin::pin!(owner.cleanup());
        assert!(poll(cleanup.as_mut()).is_pending());
    }
    event(0, 0, 8);
    let mut output = [Record::default(); 1];
    {
        let mut finish = core::pin::pin!(owner.finish(&mut output));
        assert!(matches!(
            poll(finish.as_mut()),
            Poll::Ready(Err(Error::Quarantined))
        ));
    }
    {
        let mut cleanup = core::pin::pin!(owner.cleanup());
        assert!(matches!(poll(cleanup.as_mut()), Poll::Ready(Ok(()))));
    }
    SDK.with_borrow(|s| {
        assert_eq!(s.reads, 0);
        assert_eq!(s.stops, 1);
    });
    drop(returned(owner));
}
#[test]
fn capacity_refuses_before_start_and_sdk_start_error_is_preserved_quarantined() {
    let mut owner = owner();
    for max in [0, 17, usize::MAX] {
        assert!(matches!(
            owner.start(&scan::ScanConfig::default(), max),
            Err(Error::Capacity)
        ));
    }
    SDK.with_borrow(|s| assert_eq!(s.starts, 0));
    SDK.with_borrow_mut(|s| s.start_error = 0x101);
    assert!(matches!(
        owner.start(&scan::ScanConfig::default(), 16),
        Err(Error::Sdk(0x101))
    ));
    let owner = match owner.try_into_controller() {
        Err(owner) => owner,
        Ok(_) => panic!("unknown start outcome cannot return owner"),
    };
    drop(owner);
    SDK.with_borrow(|s| assert_eq!(s.dropped, 0));
}

#[test]
fn direct_mapping_refusal_reports_exact_prefix_and_retains_terminal_cleanup() {
    let mut owner = owner();
    owner.start(&scan::ScanConfig::default(), 4).unwrap();
    SDK.with_borrow_mut(|s| s.total = 8);
    event(0, 8, 19);
    let mut output = [u32::MAX; 4];
    {
        let mut finish = core::pin::pin!(owner.finish_mapped(&mut output, |record| {
            if record.ssid[0] == 3 {
                None
            } else {
                Some(u32::from(record.ssid[0]))
            }
        }));
        assert!(matches!(
            poll(finish.as_mut()),
            Poll::Ready(Err(Error::MappingRefused { written: 2 }))
        ));
    }
    assert_eq!(output, [1, 2, u32::MAX, u32::MAX]);
    {
        let mut retry = core::pin::pin!(
            owner.finish_mapped(&mut output, |record| Some(u32::from(record.ssid[0])))
        );
        assert!(matches!(
            poll(retry.as_mut()),
            Poll::Ready(Err(Error::Quarantined))
        ));
    }
    SDK.with_borrow(|s| {
        assert_eq!(s.reads, 3);
        assert_eq!(s.clears, 0);
    });
    let mut owner = match owner.try_into_controller() {
        Err(owner) => owner,
        Ok(_) => panic!("mapping refusal cannot return uncleared list"),
    };
    {
        let mut cleanup = core::pin::pin!(owner.cleanup());
        assert!(matches!(poll(cleanup.as_mut()), Poll::Ready(Ok(()))));
    }
    SDK.with_borrow(|s| {
        assert_eq!(s.stops, 0);
        assert_eq!(s.clears, 1);
    });
    drop(returned(owner));
}
