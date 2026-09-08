use super::io_error;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::path::Path;

pub(super) fn apply_copy_landlock(source: &Path, destination: &Path) -> Result<(), String> {
    const CREATE_RULESET_VERSION: u32 = 1;
    const RULE_PATH_BENEATH: u32 = 1;
    const READ_FILE: u64 = 1 << 2;
    const READ_DIR: u64 = 1 << 3;
    const WRITE_FILE: u64 = 1 << 1;
    const REMOVE_FILE: u64 = 1 << 5;
    const MAKE_REG: u64 = 1 << 8;
    const REFER: u64 = 1 << 13;
    const TRUNCATE: u64 = 1 << 14;
    const HANDLED_V3: u64 = (1 << 15) - 1;
    #[repr(C)]
    struct RulesetAttr {
        handled_access_fs: u64,
    }
    let abi = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            std::ptr::null::<RulesetAttr>(),
            0,
            CREATE_RULESET_VERSION,
        )
    };
    if abi < 3 {
        return Err("isolated copy Base requires Landlock ABI 3".into());
    }
    let attr = RulesetAttr {
        handled_access_fs: HANDLED_V3,
    };
    let ruleset = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            &attr,
            std::mem::size_of::<RulesetAttr>(),
            0,
        ) as i32
    };
    if ruleset < 0 {
        return Err(io_error(std::io::Error::last_os_error()));
    }
    add_path_rule(ruleset, source, READ_FILE | READ_DIR, RULE_PATH_BENEATH)?;
    add_path_rule(
        ruleset,
        destination,
        READ_DIR | WRITE_FILE | REMOVE_FILE | MAKE_REG | REFER | TRUNCATE,
        RULE_PATH_BENEATH,
    )?;
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0
        || unsafe { libc::syscall(libc::SYS_landlock_restrict_self, ruleset, 0) } != 0
    {
        unsafe { libc::close(ruleset) };
        return Err(io_error(std::io::Error::last_os_error()));
    }
    unsafe { libc::close(ruleset) };
    Ok(())
}

fn add_path_rule(ruleset: i32, path: &Path, access: u64, rule_type: u32) -> Result<(), String> {
    #[repr(C)]
    struct PathAttr {
        allowed_access: u64,
        parent_fd: i32,
        reserved: u32,
    }
    let directory = File::open(path).map_err(io_error)?;
    let attr = PathAttr {
        allowed_access: access,
        parent_fd: directory.as_raw_fd(),
        reserved: 0,
    };
    if unsafe { libc::syscall(libc::SYS_landlock_add_rule, ruleset, rule_type, &attr, 0) } != 0 {
        return Err(io_error(std::io::Error::last_os_error()));
    }
    Ok(())
}
