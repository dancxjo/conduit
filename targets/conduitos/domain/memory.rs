//! Freestanding compiler support, limited to the domain's own mapped bytes.
use core::ffi::c_void;

#[unsafe(no_mangle)]
unsafe extern "C" fn bcmp(left: *const c_void, right: *const c_void, length: usize) -> i32 {
    for index in 0..length {
        // Volatile reads keep this compiler support routine from lowering its
        // own loop back into a call to bcmp. Page permissions still bound access.
        if unsafe { left.cast::<u8>().add(index).read_volatile() }
            != unsafe { right.cast::<u8>().add(index).read_volatile() }
        {
            return 1;
        }
    }
    0
}

#[unsafe(no_mangle)]
unsafe extern "C" fn memcpy(
    destination: *mut c_void,
    source: *const c_void,
    length: usize,
) -> *mut c_void {
    for index in 0..length {
        unsafe {
            destination
                .cast::<u8>()
                .add(index)
                .write_volatile(source.cast::<u8>().add(index).read_volatile());
        }
    }
    destination
}

#[unsafe(no_mangle)]
unsafe extern "C" fn memmove(
    destination: *mut c_void,
    source: *const c_void,
    length: usize,
) -> *mut c_void {
    if destination as usize <= source as usize {
        unsafe { memcpy(destination, source, length) }
    } else {
        for index in (0..length).rev() {
            unsafe {
                destination
                    .cast::<u8>()
                    .add(index)
                    .write_volatile(source.cast::<u8>().add(index).read_volatile());
            }
        }
        destination
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn memset(destination: *mut c_void, value: i32, length: usize) -> *mut c_void {
    for index in 0..length {
        unsafe {
            destination
                .cast::<u8>()
                .add(index)
                .write_volatile(value as u8);
        }
    }
    destination
}
