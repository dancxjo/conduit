//! CPU transport ownership checks; no flash, physical timing or Play proof.
#![allow(dead_code)]
extern crate alloc;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_observed_request_queue.rs"]
mod queue;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[test]
fn immutable_acceptance_cache_retries_do_not_allocate_or_retain_json_copies() {
    let mut queue = queue::Queue::new();
    queue.submit(queue::Request::offer(1));
    let request = queue.take().unwrap();
    let accepted = [7u8; 416];
    queue.complete(request.key, Ok(accepted));
    let (sum, memory) = allocation_probe::observe(|| {
        let mut sum = 0;
        for _ in 0..1000 {
            match queue.submit(queue::Request::offer(1)) {
                queue::Response::Completed(Ok(proof)) => {
                    assert_eq!(proof, accepted);
                    sum += u64::from(proof[0]);
                }
                _ => panic!("same exact request must retain the immutable proof"),
            }
        }
        sum
    });
    assert_eq!(sum, 7000);
    assert_eq!(memory.allocations, 0);
    assert_eq!(memory.reallocations, 0);
    assert_eq!(memory.retained_bytes(), 0);
}
