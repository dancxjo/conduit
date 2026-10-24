//! Actual production request owner and FrameGrant on the hosted CPU.
extern crate alloc;
#[path = "../src/protected_compositor/frame_grant.rs"]
mod frame_grant;
#[path = "../src/protected_compositor/request_owner.rs"]
mod request_owner;

#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

use conduitos_compositor_core::wire::{self, Envelope, Request, Response};
#[test]
fn production_begin_complete_and_quarantine_allocate_nothing() {
    let mut owner = request_owner::RequestOwner::new(9, 16, 16);
    let mut bytes = [0; wire::EXCHANGE_BYTES];
    let length = wire::encode_response(
        Envelope {
            instance: 9,
            request: 1,
        },
        Response::Unit,
        &mut bytes,
    )
    .unwrap();
    let ((), storage) = allocation_probe::observe(|| {
        owner
            .begin(Request::Initialize {
                width: 16,
                height: 16,
            })
            .unwrap();
        assert!(owner.command().unwrap().len() <= request_owner::COMMAND_BYTES);
        assert_eq!(
            owner.complete_response(&bytes[..length]),
            Ok(Response::Unit)
        );
        owner.quarantine();
    });
    assert_eq!(
        (
            storage.allocations,
            storage.reallocations,
            storage.deallocations
        ),
        (0, 0, 0)
    );
}
