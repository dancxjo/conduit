//! Actual native adapter source against controlled CPU SDK/stack/service shims.
//! This checks ownership/sequencing, not target ABI or physical restoration.
#![allow(dead_code, async_fn_in_trait)]
extern crate self as conduit_net;
extern crate self as conduit_network_join_call;
extern crate self as embassy_net;
extern crate self as embassy_time;
extern crate self as esp_radio;
#[path = "scan_host/shims.rs"]
mod shims;
pub use shims::*;
#[path = "../src/scan.rs"]
mod adapter;
use adapter::{Esp32NetworkScanProvider as Provider, RestoredAccessPointServices};
use core::{
    future::Future,
    task::{Context, Poll, Waker},
};
use scan::NetworkScanProvider;
fn poll<F: Future>(future: core::pin::Pin<&mut F>) -> Poll<F::Output> {
    future.poll(&mut Context::from_waker(Waker::noop()))
}
fn request() -> NetworkScanRequest {
    NetworkScanRequest {
        generation: 7,
        scan_deadline_ms: 15_000,
        cleanup_deadline_ms: 20_000,
    }
}
fn prepared() -> Provider<'static, 'static, Services> {
    let mut slot = Some(switch());
    Provider::prepare_from_restored(&mut slot).unwrap()
}
#[test]
fn readiness_capacity_and_interface_refusals_keep_exact_switch_owner() {
    for refusal in 0..4 {
        reset();
        let mut radio = switch();
        let address = radio.station_config.secret_ptr();
        match refusal {
            0 => radio.cleanup_required = true,
            1 => SDK.with_borrow_mut(|s| s.ap_link = false),
            2 => SDK.with_borrow_mut(|s| s.ready_generation = None),
            _ => radio.services.maximum = 1,
        }
        let mut slot = Some(radio);
        assert!(Provider::prepare_from_restored(&mut slot).is_err());
        assert_eq!(slot.as_ref().unwrap().station_config.secret_ptr(), address);
        SDK.with_borrow(|s| {
            assert_eq!(s.configurations, 0);
            assert_eq!(s.commands, 0);
            assert_eq!(s.controller_drops, 0);
        });
    }
}
#[test]
fn canceled_scan_keeps_owner_and_cancelled_restore_reuses_its_actual_ack() {
    reset();
    let mut provider = prepared();
    let mut output = [ScanObservation::EMPTY; 16];
    {
        let mut scan = core::pin::pin!(provider.scan(request(), &mut output));
        assert!(poll(scan.as_mut()).is_pending());
    }
    SDK.with_borrow(|s| {
        assert_eq!(s.commands, 1);
        assert_eq!(s.scan_starts, 1);
        assert_eq!(s.controller_drops, 0);
    });
    SDK.with_borrow_mut(|s| {
        s.terminal = Some(wifi::bounded_scan::Completion {
            status: 1,
            number: 0,
            scan_id: 81,
        });
        s.restore_ack = false;
    });
    {
        let mut cleanup = core::pin::pin!(provider.cleanup(20_000));
        assert!(poll(cleanup.as_mut()).is_pending());
    }
    SDK.with_borrow(|s| {
        assert_eq!(s.commands, 2);
        assert_eq!(s.stops, 1);
        assert_eq!(s.list_clears, 1);
        assert_eq!(s.controller_drops, 0);
    });
    SDK.with_borrow_mut(|s| {
        s.ready_generation = Some(2);
        s.now = 1;
    });
    {
        let mut cleanup = core::pin::pin!(provider.cleanup(20_000));
        assert!(matches!(poll(cleanup.as_mut()), Poll::Ready(Ok(()))));
    }
    SDK.with_borrow(|s| {
        assert_eq!(s.commands, 2);
        assert_eq!(s.ap_configurations, 1);
    });
    let radio = match provider.try_into_restored() {
        Ok(radio) => radio,
        Err(_) => panic!("actual restored owner"),
    };
    assert_eq!(radio.services.generation, 2);
    assert_eq!(radio.services.remaining_service_commands(), 0);
    drop(radio);
    SDK.with_borrow(|s| assert_eq!(s.controller_drops, 1));
}
#[test]
fn successful_scan_maps_once_without_association_and_keeps_exact_raw_inspection() {
    reset();
    let mut slot = Some(switch());
    let secret = slot.as_ref().unwrap().station_config.secret_ptr();
    let mut provider = Provider::prepare_from_restored(&mut slot).unwrap();
    SDK.with_borrow_mut(|s| {
        s.terminal = Some(wifi::bounded_scan::Completion {
            status: 0,
            number: 30,
            scan_id: 255,
        })
    });
    let mut output = [ScanObservation::EMPTY; 16];
    {
        let mut scan = core::pin::pin!(provider.scan(request(), &mut output));
        match poll(scan.as_mut()) {
            Poll::Ready(Ok(done)) => {
                assert_eq!(done.generation, 7);
                assert_eq!(done.total, 30);
                assert_eq!(done.count, 16);
            }
            _ => panic!("scan completion"),
        }
    }
    assert_eq!(provider.inspection().terminal.unwrap().scan_id, 255);
    assert_eq!(output[0].security, ObservedNetworkSecurity::Unknown);
    assert_eq!(
        output[1].security,
        ObservedNetworkSecurity::Known(KnownSecurity::Wpa2Personal)
    );
    {
        let mut again = core::pin::pin!(provider.scan(request(), &mut output));
        assert!(matches!(
            poll(again.as_mut()),
            Poll::Ready(Err(ScanFailure::Rejected))
        ));
    }
    {
        let mut cleanup = core::pin::pin!(provider.cleanup(20_000));
        assert!(matches!(poll(cleanup.as_mut()), Poll::Ready(Ok(()))));
    }
    let radio = match provider.try_into_restored() {
        Ok(radio) => radio,
        Err(_) => panic!("restored"),
    };
    assert_eq!(radio.station_config.secret_ptr(), secret);
    SDK.with_borrow(|s| {
        assert_eq!(s.scan_starts, 1);
        assert_eq!(s.associations, 0);
        assert_eq!(s.commands, 2);
    });
    drop(radio);
}
#[test]
fn no_deadline_extension_and_missing_terminal_never_restore_or_return_owner() {
    reset();
    let mut provider = prepared();
    let mut output = [ScanObservation::EMPTY; 16];
    {
        let mut scan = core::pin::pin!(provider.scan(request(), &mut output));
        assert!(poll(scan.as_mut()).is_pending());
    }
    {
        let mut cleanup = core::pin::pin!(provider.cleanup(20_001));
        assert!(matches!(
            poll(cleanup.as_mut()),
            Poll::Ready(Err(ScanFailure::Deadline))
        ));
    }
    {
        let mut cleanup = core::pin::pin!(provider.cleanup(20_000));
        assert!(poll(cleanup.as_mut()).is_pending());
    }
    SDK.with_borrow_mut(|s| s.now = 20_000);
    {
        let mut cleanup = core::pin::pin!(provider.cleanup(20_000));
        assert!(matches!(
            poll(cleanup.as_mut()),
            Poll::Ready(Err(ScanFailure::Deadline))
        ));
    }
    let provider = match provider.try_into_restored() {
        Err(owner) => owner,
        Ok(_) => panic!("missing terminal cannot return owner"),
    };
    SDK.with_borrow(|s| {
        assert_eq!(s.commands, 1);
        assert_eq!(s.ap_configurations, 0);
        assert_eq!(s.controller_drops, 0);
    });
    drop(provider);
    SDK.with_borrow(|s| assert_eq!(s.controller_drops, 0));
}
