/// Export the external Zenoh-Pico PAL's C callbacks using a target-owned context.
///
/// The supplied function accepts `FnOnce(&mut Callbacks<...>)` and returns its
/// result. It must grant exclusive access on the one C worker, never be entered
/// recursively, and never share that context with the network owner. Only the
/// finite bridge/endpoint snapshot is shared across cores. It must not hold a
/// critical section, interrupt mask or cross-core spinlock across the callback;
/// waits require the network owner and sampler to keep running. It must not
/// expose an uninitialized context. C callers
/// obey the raw-pointer extents documented by `Callbacks` and the C header.
#[macro_export]
macro_rules! export_zenoh_pico_callbacks {
    ($with:path) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn conduit_z_alloc(size: usize) -> *mut u8 {
            $with(|c| c.allocate(size))
        }
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn conduit_z_realloc(p: *mut u8, size: usize) -> *mut u8 {
            $with(|c| unsafe { c.reallocate(p, size) })
        }
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn conduit_z_free(p: *mut u8) {
            $with(|c| unsafe { c.free(p) })
        }
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn conduit_z_boot_us() -> u64 {
            $with(|c| c.boot_us())
        }
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn conduit_z_sleep_us(duration: u64) -> i32 {
            $with(|c| c.sleep_us(duration))
        }
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn conduit_z_random_fill(dst: *mut u8, len: usize) {
            $with(|c| unsafe { c.random_fill(dst, len) })
        }
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn conduit_z_wall_time(secs: *mut u32, nanos: *mut u32) -> i32 {
            $with(|c| unsafe { c.wall_time(secs, nanos) })
        }
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn conduit_z_tcp_connect(
            ip: *const u8,
            port: u16,
            timeout: u32,
            slot: *mut u32,
            generation: *mut u32,
        ) -> i32 {
            $with(|c| unsafe { c.connect(ip, port, timeout, slot, generation) })
        }
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn conduit_z_tcp_read(
            slot: u32,
            generation: u32,
            dst: *mut u8,
            len: usize,
            timeout: u32,
        ) -> isize {
            $with(|c| unsafe { c.read(slot, generation, dst, len, timeout) })
        }
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn conduit_z_tcp_write(
            slot: u32,
            generation: u32,
            src: *const u8,
            len: usize,
            timeout: u32,
        ) -> isize {
            $with(|c| unsafe { c.write(slot, generation, src, len, timeout) })
        }
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn conduit_z_tcp_close(slot: u32, generation: u32) -> i32 {
            $with(|c| c.close(slot, generation))
        }
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn conduit_z_tcp_ready(
            slot: u32,
            generation: u32,
            timeout: u32,
        ) -> i32 {
            $with(|c| c.ready(slot, generation, timeout))
        }
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn conduit_z_tcp_set_blocking(
            slot: u32,
            generation: u32,
            blocking: bool,
        ) -> i32 {
            $with(|c| c.set_blocking(slot, generation, blocking))
        }
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn conduit_z_tcp_endpoints(
            slot: u32,
            generation: u32,
            local: *mut u8,
            local_len: usize,
            remote: *mut u8,
            remote_len: usize,
        ) -> i32 {
            $with(|c| unsafe {
                c.endpoints(slot, generation, local, local_len, remote, remote_len)
            })
        }
    };
}
