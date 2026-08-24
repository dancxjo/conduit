use std::os::fd::FromRawFd;
use std::path::PathBuf;

pub(crate) struct Pty {
    pub(crate) master: std::fs::File,
    pub(crate) slave_guard: std::fs::File,
    pub(crate) slave_path: PathBuf,
}

impl Pty {
    pub(crate) fn open() -> Self {
        let mut master = -1;
        let mut slave = -1;
        assert_eq!(
            unsafe {
                libc::openpty(
                    &mut master,
                    &mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    std::ptr::null(),
                )
            },
            0
        );
        let mut path = [0 as libc::c_char; 256];
        assert_eq!(
            unsafe { libc::ttyname_r(slave, path.as_mut_ptr(), path.len()) },
            0
        );
        let path = unsafe { std::ffi::CStr::from_ptr(path.as_ptr()) };
        Self {
            master: unsafe { std::fs::File::from_raw_fd(master) },
            slave_guard: unsafe { std::fs::File::from_raw_fd(slave) },
            slave_path: PathBuf::from(path.to_str().unwrap()),
        }
    }
}
