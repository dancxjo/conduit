//! Linux address ownership and an allocation-growth-denying execution boundary.
use super::super::{BackingLedger, BackingRole, BackingSpan, NativeBackingRefusal, SharedPoolUse};
use sha2::{Digest, Sha256};
use std::io::Read;
const MAX_MAPS: usize = 512;
const MAX_TEXT: usize = 256 * 1024;
#[derive(Clone, Copy)]
struct Region {
    start: u64,
    end: u64,
    offset: u64,
    device: u64,
    inode: u64,
    private_rw: bool,
    file: bool,
}
pub struct Snapshot {
    regions: Vec<Region>,
}
impl Snapshot {
    pub fn capture(pid: u32) -> Result<Self, NativeBackingRefusal> {
        let mut regions = Vec::new();
        regions
            .try_reserve_exact(MAX_MAPS)
            .map_err(|_| NativeBackingRefusal::BackingPreparation)?;
        let mut text = vec![0u8; MAX_TEXT];
        let mut file = std::fs::File::open(format!("/proc/{pid}/maps"))
            .map_err(|_| NativeBackingRefusal::Io)?;
        let mut length = 0;
        while length < text.len() {
            let n = file
                .read(&mut text[length..])
                .map_err(|_| NativeBackingRefusal::Io)?;
            if n == 0 {
                break;
            }
            length += n;
        }
        if length == MAX_TEXT {
            return Err(NativeBackingRefusal::InvalidLedger);
        }
        let text = std::str::from_utf8(&text[..length])
            .map_err(|_| NativeBackingRefusal::InvalidLedger)?;
        for line in text.lines() {
            if regions.len() == MAX_MAPS {
                return Err(NativeBackingRefusal::InvalidLedger);
            }
            let mut fields = line.split_whitespace();
            let (a, b) = fields
                .next()
                .and_then(|v| v.split_once('-'))
                .ok_or(NativeBackingRefusal::InvalidLedger)?;
            let perms = fields
                .next()
                .ok_or(NativeBackingRefusal::InvalidLedger)?
                .as_bytes();
            let offset = hex(fields.next().ok_or(NativeBackingRefusal::InvalidLedger)?)?;
            let (major, minor) = fields
                .next()
                .and_then(|v| v.split_once(':'))
                .ok_or(NativeBackingRefusal::InvalidLedger)?;
            let device = (hex(major)? << 32) | hex(minor)?;
            let inode = fields
                .next()
                .ok_or(NativeBackingRefusal::InvalidLedger)?
                .parse()
                .map_err(|_| NativeBackingRefusal::InvalidLedger)?;
            if perms.len() != 4 || (perms[1] == b'w' && (perms[0] != b'r' || perms[3] != b'p')) {
                return Err(NativeBackingRefusal::InvalidLedger);
            }
            let region = Region {
                start: hex(a)?,
                end: hex(b)?,
                offset,
                device,
                inode,
                private_rw: perms[0] == b'r' && perms[1] == b'w' && perms[3] == b'p',
                file: inode != 0,
            };
            if region.start >= region.end
                || regions
                    .last()
                    .is_some_and(|p: &Region| p.end > region.start)
            {
                return Err(NativeBackingRefusal::InvalidLedger);
            }
            regions.push(region);
        }
        Ok(Self { regions })
    }
    pub fn private_capacity(&self) -> Result<u64, NativeBackingRefusal> {
        self.regions
            .iter()
            .filter(|r| r.private_rw)
            .try_fold(0u64, |n, r| {
                n.checked_add(r.end - r.start)
                    .ok_or(NativeBackingRefusal::InvalidLedger)
            })
    }
    pub fn stack_at(&self, address: u64) -> Result<(u64, u64), NativeBackingRefusal> {
        self.regions
            .iter()
            .find(|r| r.private_rw && r.start <= address && address < r.end)
            .map(|r| (r.start, r.end))
            .ok_or(NativeBackingRefusal::InvalidLedger)
    }
    pub fn ledger(
        &self,
        io: BackingSpan,
        stack: BackingSpan,
    ) -> Result<BackingLedger, NativeBackingRefusal> {
        if io.role != BackingRole::FixedIo
            || stack.role != BackingRole::ExecutionStack
            || io.start >= io.end
            || stack.start >= stack.end
            || (io.start < stack.end && stack.start < io.end)
        {
            return Err(NativeBackingRefusal::InvalidLedger);
        }
        for span in [io, stack] {
            if !self
                .regions
                .iter()
                .any(|r| r.private_rw && r.start <= span.start && span.end <= r.end)
            {
                return Err(NativeBackingRefusal::InvalidLedger);
            }
        }
        let mut private = 0u64;
        let mut immutable = 0u64;
        let mut image = 0u64;
        let mut other = 0u64;
        let mut private_hash = Sha256::new();
        let mut file_hash = Sha256::new();
        let mut fixed_hash = Sha256::new();
        let mut file_spans = Vec::with_capacity(self.regions.len());
        for r in &self.regions {
            let bytes = r.end - r.start;
            if !r.private_rw {
                for value in [r.start, r.end, r.offset, r.device, r.inode] {
                    fixed_hash.update(value.to_le_bytes());
                }
            }
            if r.private_rw {
                private = private
                    .checked_add(bytes)
                    .ok_or(NativeBackingRefusal::InvalidLedger)?;
                private_hash.update(r.start.to_le_bytes());
                private_hash.update(r.end.to_le_bytes());
            } else if r.file {
                image = image
                    .checked_add(bytes)
                    .ok_or(NativeBackingRefusal::InvalidLedger)?;
                file_spans.push((
                    r.device,
                    r.inode,
                    r.offset,
                    r.offset
                        .checked_add(bytes)
                        .ok_or(NativeBackingRefusal::InvalidLedger)?,
                ));
            } else {
                other = other
                    .checked_add(bytes)
                    .ok_or(NativeBackingRefusal::InvalidLedger)?;
            }
        }
        file_spans.sort_unstable();
        let mut previous = None;
        for (device, inode, start, end) in file_spans {
            let new_start = match previous {
                Some((d, i, e)) if d == device && i == inode => start.max(e),
                _ => start,
            };
            if end > new_start {
                immutable = immutable
                    .checked_add(end - new_start)
                    .ok_or(NativeBackingRefusal::InvalidLedger)?;
                file_hash.update(device.to_le_bytes());
                file_hash.update(inode.to_le_bytes());
                file_hash.update(new_start.to_le_bytes());
                file_hash.update(end.to_le_bytes());
            }
            previous = Some((
                device,
                inode,
                end.max(
                    previous
                        .filter(|(d, i, _)| *d == device && *i == inode)
                        .map_or(0, |(_, _, e)| e),
                ),
            ));
        }
        let mapping: [u8; 32] = private_hash.finalize().into();
        let (_, io_bytes, stack_bytes) = super::super::ledger::charge_private_spans(&[io, stack])?;
        let pool = private
            .checked_sub(io_bytes)
            .and_then(|v| v.checked_sub(stack_bytes))
            .ok_or(NativeBackingRefusal::InvalidLedger)?;
        let mut pool_hash = Sha256::new();
        pool_hash.update(mapping);
        for p in [io, stack] {
            pool_hash.update(p.start.to_le_bytes());
            pool_hash.update(p.end.to_le_bytes());
        }
        Ok(BackingLedger {
            profile: super::super::profile::PROFILE.into(),
            private_bytes: private,
            immutable_file_bytes: immutable,
            image_mapping_bytes: image,
            shared_pool: SharedPoolUse {
                pool_identity: pool_hash.finalize().into(),
                capacity_bytes: pool,
                persistent_session_and_metadata: true,
                execution_scratch_and_metadata: true,
            },
            fixed_io_bytes: io_bytes,
            execution_stack_bytes: stack_bytes,
            private_mapping_identity: mapping,
            immutable_mapping_identity: file_hash.finalize().into(),
            fixed_mapping_identity: fixed_hash.finalize().into(),
            other_mapping_bytes: other,
            fixed_io_span: io,
            execution_stack_span: stack,
        })
    }
    /// Preparation-only write faults establish actual private backing, without residency claims.
    pub unsafe fn prefault(&self) {
        for r in &self.regions {
            if r.private_rw {
                for address in (r.start..r.end).step_by(4096) {
                    let p = address as *mut u8;
                    let b = unsafe { p.read_volatile() };
                    unsafe { p.write_volatile(b) };
                }
            }
        }
    }
}
fn hex(s: &str) -> Result<u64, NativeBackingRefusal> {
    u64::from_str_radix(s, 16).map_err(|_| NativeBackingRefusal::InvalidLedger)
}
pub fn freeze() -> Result<(), NativeBackingRefusal> {
    fn statement(code: u16, k: u32) -> libc::sock_filter {
        libc::sock_filter {
            code,
            jt: 0,
            jf: 0,
            k,
        }
    }
    fn jump(k: u32, jt: u8, jf: u8) -> libc::sock_filter {
        libc::sock_filter {
            code: 0x15,
            jt,
            jf,
            k,
        }
    }
    let mut filter = Vec::with_capacity(64);
    filter.push(statement(0x20, 4));
    filter.push(jump(0xc000003e, 1, 0));
    filter.push(statement(0x06, 0x80000000));
    filter.push(statement(0x20, 0));
    for syscall in [
        libc::SYS_read,
        libc::SYS_write,
        libc::SYS_close,
        libc::SYS_futex,
        libc::SYS_clock_gettime,
        libc::SYS_gettimeofday,
        libc::SYS_rt_sigprocmask,
        libc::SYS_rt_sigreturn,
        libc::SYS_sched_yield,
        libc::SYS_getpid,
        libc::SYS_gettid,
        libc::SYS_exit,
        libc::SYS_exit_group,
    ] {
        filter.push(jump(syscall as u32, 0, 1));
        filter.push(statement(0x06, 0x7fff0000));
    }
    filter.push(statement(0x06, 0x80000000));
    let program = libc::sock_fprog {
        len: filter.len() as u16,
        filter: filter.as_mut_ptr(),
    };
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0
        || unsafe { libc::syscall(libc::SYS_seccomp, 1, 1, &program) } != 0
    {
        return Err(NativeBackingRefusal::BackingPreparation);
    }
    // Prevent allocator cleanup from introducing an unadmitted trim/unmap syscall.
    std::mem::forget(filter);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn immutable_aliases_share_backing_and_native_roles_partition_exact_spans() {
        let io = BackingSpan {
            start: 0x2000,
            end: 0x3000,
            role: BackingRole::FixedIo,
        };
        let stack = BackingSpan {
            start: 0x4000,
            end: 0x4000 + super::super::super::profile::STACK_BYTES as u64,
            role: BackingRole::ExecutionStack,
        };
        let private = |start, end| Region {
            start,
            end,
            offset: 0,
            device: 0,
            inode: 0,
            private_rw: true,
            file: false,
        };
        let file = |start| Region {
            start,
            end: start + 0x4000,
            offset: 0,
            device: 1,
            inode: 7,
            private_rw: false,
            file: true,
        };
        let snapshot = Snapshot {
            regions: vec![
                private(0x1000, 0x2000),
                private(io.start, io.end),
                private(stack.start, stack.end),
                file(0x100000),
                file(0x200000),
            ],
        };
        let ledger = snapshot.ledger(io, stack).unwrap();
        assert_eq!(ledger.shared_pool.capacity_bytes, 4096);
        assert_eq!(ledger.immutable_file_bytes, 0x4000);
        assert_eq!(ledger.image_mapping_bytes, 0x8000);
        let grant = super::super::super::NativeBackingGrant {
            maximum_private_bytes: ledger.private_bytes,
            maximum_fixed_mapping_bytes: ledger.image_mapping_bytes + ledger.other_mapping_bytes,
        };
        assert_eq!(ledger.validate(grant), Ok(()));
        assert!(matches!(
            ledger.validate(super::super::super::NativeBackingGrant {
                maximum_private_bytes: ledger.private_bytes - 1,
                ..grant
            }),
            Err(NativeBackingRefusal::InsufficientBacking { .. })
        ));
        // Source aliases share file extents but each image reservation is charged,
        // including possible relocated private copies.
        assert!(matches!(
            ledger.validate(super::super::super::NativeBackingGrant {
                maximum_fixed_mapping_bytes: ledger.immutable_file_bytes,
                ..grant
            }),
            Err(NativeBackingRefusal::InsufficientBacking { .. })
        ));
        let mut duplicate = ledger.clone();
        duplicate.shared_pool.capacity_bytes += 4096;
        assert_eq!(
            duplicate.validate(grant),
            Err(NativeBackingRefusal::InvalidLedger)
        );
    }
}
