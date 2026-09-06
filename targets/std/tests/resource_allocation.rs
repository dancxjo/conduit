mod resource_common;
use conduit_core::{BoundedResourceRef, ResourceSharing};
use conduit_kernel::state_delay::StateDelay;
use resource_common::*;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
thread_local! {static ENABLED:Cell<bool>=const{Cell::new(false)};static ALLOCATIONS:Cell<usize>=const{Cell::new(0)};}
struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = ENABLED.try_with(|enabled| {
            if enabled.get() {
                let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Counting = Counting;
#[test]
fn publication_and_read_many_do_not_allocate_after_preparation() {
    let mut frame = prepared(ResourceSharing::SingleWriterPublished, 2);
    let bytes = vec![42; FRAME_BYTES];
    let writer = writer();
    let reader = reader();
    ALLOCATIONS.with(|count| count.set(0));
    ENABLED.with(|enabled| enabled.set(true));
    frame.write_candidate(&writer, &bytes).unwrap();
    frame.publish(&writer).unwrap();
    let a = frame.acquire(&reader).unwrap();
    let b = frame.acquire(&reader).unwrap();
    assert_eq!(frame.read(a).unwrap(), frame.read(b).unwrap());
    frame.release(a).unwrap();
    frame.release(b).unwrap();
    frame.retire(&writer).unwrap();
    ENABLED.with(|enabled| enabled.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
}
#[test]
fn state_retains_reference_info_without_becoming_the_resource() {
    let first = prepared(ResourceSharing::ImmutableReadMany, 2);
    let next = prepared(ResourceSharing::ImmutableReadMany, 3);
    let first_encoding = first.reference().encode().unwrap();
    let next_encoding = next.reference().encode().unwrap();
    let mut state = StateDelay::<512>::new(1, 512, 2, &first_encoding).unwrap();
    state.offer_next(&next_encoding).unwrap();
    assert_eq!(
        BoundedResourceRef::decode(state.current()).unwrap(),
        *first.reference()
    );
    state.commit().unwrap();
    assert_eq!(
        BoundedResourceRef::decode(state.current()).unwrap(),
        *next.reference()
    );
    assert_eq!(first.reference().lifetime.version.digest(), [2; 32]);
}
