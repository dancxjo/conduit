//! Linux host check of guest-private lifetime/arena retention, not CPL3 proof.
#[path = "memory.rs"]
mod memory;
use conduitos_compositor_core::{MAX_CHUNK, Rect};
use core::ffi::c_void;
unsafe extern "C" {
    fn mmap(
        address: *mut c_void,
        bytes: usize,
        protection: i32,
        flags: i32,
        fd: i32,
        offset: isize,
    ) -> *mut c_void;
    fn munmap(address: *mut c_void, bytes: usize) -> i32;
}
struct Grant(usize, usize);
impl Grant {
    fn new(address: usize, bytes: usize) -> Self {
        // Linux MAP_FIXED_NOREPLACE refuses collisions rather than overwriting.
        let mapped = unsafe { mmap(address as *mut c_void, bytes, 3, 0x100022, -1, 0) };
        assert_eq!(
            mapped as usize, address,
            "unable to reserve guest test grant"
        );
        Self(address, bytes)
    }
}
impl Drop for Grant {
    fn drop(&mut self) {
        assert_eq!(unsafe { munmap(self.0 as *mut c_void, self.1) }, 0);
    }
}
fn main() {
    let _metadata = Grant::new(0x625000, 256 * 1024);
    let _pixels = Grant::new(0x666000, 8 * 1024 * 1024);
    let _scratch = Grant::new(0xe67000, 8 * 1024 * 1024);
    unsafe { memory::initialize(7, 1, 4, 4) }.unwrap();
    assert!(unsafe { memory::initialize(8, 2, 4, 4) }.is_err());
    let mut invoked = false;
    assert!(unsafe { memory::with_core(8, 2, |_| invoked = true) }.is_err());
    assert!(!invoked);
    let rect = Rect {
        x: 0,
        y: 0,
        width: 4,
        height: 4,
    };
    let surface = unsafe { memory::with_core(7, 2, |c| c.admit(rect, 0)) }
        .unwrap()
        .unwrap();
    assert!(unsafe { memory::with_core(7, 2, |_| invoked = true) }.is_err());
    assert!(!invoked);
    let mut request = 2;
    for (revision, color) in [(1, 0x123456u32), (2, 0xabcdef)] {
        request += 1;
        unsafe { memory::with_core(7, request, |c| c.begin_upload(surface, revision, rect)) }
            .unwrap()
            .unwrap();
        request += 1;
        assert!(
            unsafe { memory::with_core(7, request, |c| c.prepare_upload(MAX_CHUNK)) }
                .unwrap()
                .unwrap()
        );
        request += 1;
        unsafe { memory::with_core(7, request, |c| c.upload_chunk(0, &[color; 16])) }
            .unwrap()
            .unwrap();
        request += 1;
        unsafe { memory::with_core(7, request, |c| c.commit_upload()) }
            .unwrap()
            .unwrap();
        request += 1;
        unsafe { memory::with_core(7, request, |c| c.visible(surface, true)) }
            .unwrap()
            .unwrap();
        request += 1;
        let sequence = unsafe { memory::with_core(7, request, |c| c.begin_frame()) }
            .unwrap()
            .unwrap();
        loop {
            let mut output = [0; 4];
            request += 1;
            let span =
                unsafe { memory::with_core(7, request, |c| c.compose_next(sequence, &mut output)) }
                    .unwrap()
                    .unwrap();
            assert_eq!(output, [color; 4]);
            if span.complete {
                break;
            }
        }
    }
    println!(
        "retained guest core preserved across entries and two arena swaps; stale instance/request blocked"
    );
}
