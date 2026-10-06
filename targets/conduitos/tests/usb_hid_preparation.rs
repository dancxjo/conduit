//! Measure retained Source preparation using the production fixed arena allocator.
use conduitos::allocation::BootArena;
use conduitos::usb_base::{
    endpoint_read_proof_plan::EndpointReadProofSubject,
    hid_endpoint_proof_kernel::PreparedHidEndpointProofKernel, hid_endpoint_proof_plan,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
const BYTES: usize = conduitos::make::USB_HID_ENDPOINT_ARENA_BYTES as usize;
#[repr(align(4096))]
struct Storage([u8; BYTES]);
static mut STORAGE: Storage = Storage([0; BYTES]);
static ARENA: BootArena = BootArena::new();
std::thread_local! {
    // Harness/background allocations are not Source preparation storage. A
    // const initializer also keeps allocator routing free of TLS allocations.
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
}
struct Allocator;
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
fn arena_pointer(pointer: *mut u8) -> bool {
    // SAFETY: forming a raw address does not access or borrow the static storage.
    let start = unsafe { core::ptr::addr_of_mut!(STORAGE.0) as usize };
    (start..start + BYTES).contains(&(pointer as usize))
}
// SAFETY: System receives its own allocations; arena ranges are exclusive and
// classified by address even after measurement ends. Both preserve Layout.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if ACTIVE.try_with(Cell::get).unwrap_or(false) {
            unsafe { ARENA.alloc(layout) }
        } else {
            unsafe { System.alloc(layout) }
        }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if arena_pointer(pointer) {
            unsafe { ARENA.dealloc(pointer, layout) }
        } else {
            unsafe { System.dealloc(pointer, layout) }
        }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if arena_pointer(pointer) {
            unsafe { ARENA.realloc(pointer, layout, new_size) }
        } else {
            unsafe { System.realloc(pointer, layout, new_size) }
        }
    }
}
#[test]
fn retained_hid_source_preparation_has_a_measured_finite_arena_envelope() {
    let profile: serde_json::Value = serde_json::from_str(include_str!(
        "../proof/profiles/conduitos-usb-hid-endpoint-proof.profile.json"
    ))
    .unwrap();
    assert_eq!(
        profile["bounds"]["heap_arena_bytes"].as_u64(),
        Some(BYTES as u64)
    );
    // SAFETY: the sole test owns this aligned static range for the entire process.
    unsafe { ARENA.initialize(core::ptr::addr_of_mut!(STORAGE.0) as usize, BYTES) }.unwrap();
    let (start, started) = std::sync::mpsc::sync_channel(1);
    let (done, finished) = std::sync::mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        // Warm the synchronization paths before the measured interval.
        started.recv().unwrap();
        done.send(()).unwrap();
        started.recv().unwrap();
        let layout = Layout::from_size_align(160, 8).unwrap();
        // SAFETY: retain this exact allocation/Layout until the interval ends.
        let pointer = unsafe { ALLOCATOR.alloc(layout) };
        assert!(!pointer.is_null());
        assert!(
            !arena_pointer(pointer),
            "unrelated thread entered the Source arena"
        );
        done.send(()).unwrap();
        started.recv().unwrap();
        unsafe { ALLOCATOR.dealloc(pointer, layout) };
    });
    start.send(()).unwrap();
    finished.recv().unwrap();
    ACTIVE.set(true);
    start.send(()).unwrap();
    finished.recv().unwrap();
    {
        let subject = EndpointReadProofSubject {
            host_id: "proof/host",
            boot_id: "proof/boot",
            controller_base_id: "proof/controller",
            device_instance_id: "proof/device",
            root_port: 1,
            slot: 1,
            attachment_epoch: 1,
            endpoint_dci: 3,
            endpoint_epoch: 1,
        };
        let artifact =
            hid_endpoint_proof_plan::plan(&subject, "usb-hid-keyboard-endpoint").unwrap();
        let _play = PreparedHidEndpointProofKernel::prepare(
            artifact,
            conduit_composite::KernelCompositeSignStorage {
                additional_local_items: 4096,
                additional_remote_items: 4096,
            },
        )
        .unwrap();
    }
    ACTIVE.set(false);
    // The other thread still retains its allocation at this point. It must not
    // count as a leak or alter the measured preparation envelope.
    let live = ARENA.live_bytes();
    start.send(()).unwrap();
    worker.join().unwrap();
    eprintln!(
        "HID preparation arena peak={} live={} capacity={}",
        ARENA.used(),
        live,
        ARENA.capacity()
    );
    assert!(ARENA.used() <= BYTES);
    assert_eq!(live, 0);
}
